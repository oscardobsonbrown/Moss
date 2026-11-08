use super::{
    MapTile, MapViewport, TilePosition,
    network::{load_tile, tile_template},
};
use rune::{Image, ImageCache, Waker};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TileKey {
    provider: String,
    z: u8,
    x: u32,
    y: u32,
}
impl TileKey {
    fn new(provider: &str, p: &TilePosition) -> Self {
        Self {
            provider: provider.into(),
            z: p.z,
            x: p.x,
            y: p.y,
        }
    }
}
struct Request {
    key: TileKey,
    position: TilePosition,
    directory: PathBuf,
}
struct Response {
    key: TileKey,
    data: Option<Result<Vec<u8>, String>>,
}
type Fetch = Arc<dyn Fn(&Path, &str, &TilePosition) -> Result<Vec<u8>, String> + Send + Sync>;
struct Entry {
    image: Image,
    last_used: u64,
}

/// A bounded viewport-aware worker and decoded-image cache.
pub struct MapTileLoader {
    sender: mpsc::SyncSender<Request>,
    receiver: mpsc::Receiver<Response>,
    stop: Arc<AtomicBool>,
    visible: Arc<Mutex<BTreeSet<TileKey>>>,
    cache: ImageCache,
    entries: BTreeMap<TileKey, Entry>,
    inflight: BTreeSet<TileKey>,
    failed: BTreeSet<TileKey>,
    viewport: Option<MapViewport>,
    directory: PathBuf,
    provider: String,
    clock: u64,
    pub tiles: Vec<MapTile>,
    pub error: Option<String>,
}
impl MapTileLoader {
    pub fn new(wake: Waker) -> Self {
        Self::with_fetch(wake, Arc::new(load_tile))
    }
    /// Build a decoded fixture without network access for Dashboard integration tests.
    #[cfg(test)]
    pub(crate) fn fake_cached(viewport: MapViewport, directory: PathBuf) -> Self {
        let fetch: Fetch = Arc::new(|_, _, _| Err("Test tiles are unavailable.".into()));
        let mut loader = Self::with_fetch(Arc::new(|| {}), fetch);
        loader.provider = tile_template();
        loader.directory = directory;
        loader.viewport = Some(viewport);
        let position = viewport
            .tiles()
            .into_iter()
            .next()
            .expect("visible fixture tile");
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
            130,
        ];
        let image = loader.cache.load(png);
        image.wait().expect("fixture image decodes");
        loader.entries.insert(
            TileKey::new(&loader.provider, &position),
            Entry {
                image,
                last_used: 0,
            },
        );
        loader.rebuild();
        loader
    }
    fn with_fetch(wake: Waker, fetch: Fetch) -> Self {
        let (sender, requests) = mpsc::sync_channel::<Request>(2);
        let (results, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let visible = Arc::new(Mutex::new(BTreeSet::new()));
        let worker_visible = visible.clone();
        let worker_wake = wake.clone();
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                let wanted = worker_visible.lock().unwrap().contains(&request.key);
                let data = if wanted {
                    Some(fetch(
                        &request.directory,
                        &request.key.provider,
                        &request.position,
                    ))
                } else {
                    None
                };
                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                if results
                    .send(Response {
                        key: request.key,
                        data,
                    })
                    .is_err()
                {
                    break;
                }
                worker_wake();
            }
        });
        Self {
            sender,
            receiver,
            stop,
            visible,
            cache: ImageCache::new(wake),
            entries: BTreeMap::new(),
            inflight: BTreeSet::new(),
            failed: BTreeSet::new(),
            viewport: None,
            directory: PathBuf::new(),
            provider: String::new(),
            clock: 0,
            tiles: Vec::new(),
            error: None,
        }
    }
    /// Update placement and request missing tiles for this visible viewport only.
    pub fn request(&mut self, viewport: MapViewport, directory: PathBuf) {
        let provider = tile_template();
        let provider_changed = self.provider != provider || self.directory != directory;
        if provider_changed {
            self.failed.clear();
            self.error = None;
        }
        self.provider = provider;
        self.directory = directory;
        self.viewport = Some(viewport);
        let visible: BTreeSet<_> = viewport
            .tiles()
            .iter()
            .map(|p| TileKey::new(&self.provider, p))
            .collect();
        self.failed.retain(|key| visible.contains(key));
        *self.visible.lock().unwrap() = visible;
        self.rebuild();
        self.pump();
    }
    /// Retry failed visible tiles without dropping decoded images.
    pub fn retry(&mut self) {
        self.failed.clear();
        self.error = None;
        self.pump();
    }
    pub fn pause(&mut self) {
        self.viewport = None;
        self.visible.lock().unwrap().clear();
    }
    fn pump(&mut self) {
        let Some(viewport) = self.viewport else {
            return;
        };
        let decoding = self
            .entries
            .values()
            .filter(|entry| !entry.image.is_ready())
            .count();
        for position in viewport.tiles() {
            // Bound image decode work too: a warm disk cache can outrun rasterization.
            if self.inflight.len() + decoding >= 4 {
                break;
            }
            let key = TileKey::new(&self.provider, &position);
            if self.entries.contains_key(&key)
                || self.inflight.contains(&key)
                || self.failed.contains(&key)
            {
                continue;
            }
            let request = Request {
                key: key.clone(),
                position,
                directory: self.directory.join("map-tile-cache"),
            };
            match self.sender.try_send(request) {
                Ok(()) => {
                    self.inflight.insert(key);
                }
                Err(mpsc::TrySendError::Full(_)) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.error = Some("Map tile worker stopped.".into());
                    break;
                }
            }
        }
    }
    fn rebuild(&mut self) {
        let Some(viewport) = self.viewport else {
            return;
        };
        self.clock = self.clock.wrapping_add(1);
        let mut output = Vec::new();
        // Draw cached parent tiles first so ready high-resolution tiles cover them.
        for z in 0..viewport.tile_zoom() {
            for position in viewport.tiles_at_zoom(z) {
                let key = TileKey::new(&self.provider, &position);
                if let Some(entry) = self.entries.get_mut(&key) {
                    if entry.image.is_ready() && entry.image.error().is_none() {
                        entry.last_used = self.clock;
                        output.push(MapTile {
                            position,
                            image: entry.image.clone(),
                        });
                    }
                }
            }
        }
        for position in viewport.tiles() {
            let key = TileKey::new(&self.provider, &position);
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.last_used = self.clock;
                if entry.image.is_ready() && entry.image.error().is_none() {
                    output.push(MapTile {
                        position,
                        image: entry.image.clone(),
                    });
                }
            }
        }
        self.tiles = output;
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(response) = self.receiver.try_recv() {
            self.inflight.remove(&response.key);
            match response.data {
                None => {}
                Some(Ok(bytes)) => {
                    self.clock = self.clock.wrapping_add(1);
                    self.entries.insert(
                        response.key,
                        Entry {
                            image: self.cache.load(bytes),
                            last_used: self.clock,
                        },
                    );
                    if self.entries.len() > 128 {
                        if let Some(key) = self
                            .entries
                            .iter()
                            .min_by_key(|(_, e)| e.last_used)
                            .map(|(k, _)| k.clone())
                        {
                            self.entries.remove(&key);
                        }
                    }
                    self.error = None;
                }
                Some(Err(error)) => {
                    self.failed.insert(response.key);
                    self.error = Some(error);
                }
            }
            changed = true;
        }
        let bad: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.image.error().is_some())
            .map(|(key, _)| key.clone())
            .collect();
        for key in bad {
            self.entries.remove(&key);
            self.failed.insert(key);
            self.error = Some("Map tile could not be decoded.".into());
            changed = true;
        }
        let before: Vec<_> = self
            .tiles
            .iter()
            .map(|tile| (tile.position.z, tile.position.world_x, tile.position.y))
            .collect();
        self.rebuild();
        let after: Vec<_> = self
            .tiles
            .iter()
            .map(|tile| (tile.position.z, tile.position.world_x, tile.position.y))
            .collect();
        changed |= before != after;
        self.pump();
        changed
    }
}
impl Drop for MapTileLoader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn latest_viewport_reports_fake_offline_error_without_network() {
        let wake: Waker = Arc::new(|| {});
        let mut loader = MapTileLoader::new(wake);
        let directory = std::env::temp_dir().join("moss-map-disabled-test");
        loader.request(MapViewport::default(), directory.clone());
        loader.request(
            MapViewport {
                longitude: 179.,
                ..Default::default()
            },
            directory,
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while loader.error.is_none() {
            assert!(Instant::now() < deadline);
            loader.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            loader
                .error
                .as_deref()
                .unwrap()
                .contains("disabled in tests")
        );
        assert!(loader.tiles.is_empty());
    }
    #[test]
    fn cached_tiles_move_reuse_identity_and_supply_zoom_fallback() {
        use std::sync::atomic::AtomicUsize;
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
            130,
        ];
        let fetch: Fetch = Arc::new(move |_, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(png.clone())
        });
        let mut loader = MapTileLoader::with_fetch(Arc::new(|| {}), fetch);
        let viewport = MapViewport {
            latitude: 10.,
            longitude: 10.,
            zoom: 3.,
            width: 100.,
            height: 100.,
        };
        let dir = PathBuf::from("/tmp/fake-map-cache");
        loader.request(viewport, dir.clone());
        let deadline = Instant::now() + Duration::from_secs(3);
        while loader.entries.is_empty() || !loader.inflight.is_empty() {
            assert!(Instant::now() < deadline);
            loader.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        for entry in loader.entries.values() {
            entry.image.wait().unwrap();
        }
        loader.poll();
        let count_before = count.load(Ordering::SeqCst);
        let first = loader.tiles[0].clone();
        let moved = viewport.pan(1., 0.);
        loader.request(moved, dir.clone());
        loader.poll();
        assert_eq!(count.load(Ordering::SeqCst), count_before);
        assert_eq!(loader.tiles[0].image.id(), first.image.id());
        assert!((loader.tiles[0].position.left - first.position.left - 1.).abs() < 0.01);
        let zoomed = moved.zoom_by_at(1., 50., 50.);
        loader.request(zoomed, dir.clone());
        assert!(
            loader
                .tiles
                .iter()
                .any(|tile| tile.position.z == viewport.tile_zoom() && tile.position.size == 512.)
        );
        while !loader.inflight.is_empty() {
            assert!(Instant::now() < deadline);
            loader.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        let after_zoom = count.load(Ordering::SeqCst);
        loader.request(moved, dir);
        loader.poll();
        assert_eq!(count.load(Ordering::SeqCst), after_zoom);
        assert!(
            loader
                .tiles
                .iter()
                .any(|tile| tile.image.id() == first.image.id())
        );
        assert!(loader.entries.len() <= 128 && loader.inflight.len() <= 3);
    }
    #[test]
    fn retry_recovers_failed_visible_tiles_without_automatic_loop() {
        use std::sync::atomic::AtomicUsize;
        let calls = Arc::new(AtomicUsize::new(0));
        let attempts = calls.clone();
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
            130,
        ];
        let fetch: Fetch = Arc::new(move |_, _, _| {
            if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                Err("offline".into())
            } else {
                Ok(png.clone())
            }
        });
        let mut loader = MapTileLoader::with_fetch(Arc::new(|| {}), fetch);
        let view = MapViewport {
            latitude: 10.,
            longitude: 10.,
            zoom: 3.,
            width: 20.,
            height: 20.,
        };
        loader.request(view, PathBuf::from("/tmp/fake-retry"));
        let deadline = Instant::now() + Duration::from_secs(3);
        while loader.error.is_none() {
            assert!(Instant::now() < deadline);
            loader.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        for _ in 0..5 {
            loader.poll();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        loader.retry();
        while loader.entries.is_empty() {
            assert!(Instant::now() < deadline);
            loader.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(loader.error.is_none());
    }
}
