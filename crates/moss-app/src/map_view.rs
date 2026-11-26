use moss_core::{MapBounds, MapPoint, PlaceValue, Record, RecordQuery, Store, ViewKind};
use rune::{TextEdit, Tx, Waker};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[path = "map_view/fit.rs"]
mod fit;
#[path = "map_view/network.rs"]
mod network;
#[path = "map_view/surface.rs"]
pub(crate) mod surface;
#[path = "map_view/tile_loader.rs"]
mod tile_loader;
#[path = "map_view/viewport.rs"]
mod viewport;
pub(crate) use network::attribution;
use network::photon;
pub use surface::MapSurface;
pub use tile_loader::MapTileLoader;
use viewport::MAX_LAT;
pub use viewport::{MapTile, MapViewport, TilePosition};

const MAX_MARKERS: u32 = 500;

struct QueryRequest {
    epoch: u64,
    db: String,
    query: RecordQuery,
    title: String,
    place: String,
    bounds: MapBounds,
    record_search: String,
}
struct QueryResponse {
    epoch: u64,
    points: Result<moss_core::MapPoints, String>,
    candidates: Result<Vec<Record>, String>,
}
struct SearchResponse {
    epoch: u64,
    query: String,
    result: Result<Vec<PlaceValue>, String>,
}

pub struct MapState {
    pub viewport: MapViewport,
    pub fit_pending: bool,
    fit_epoch: u64,
    fit_start: Option<MapViewport>,
    fit_worker: Option<fit::FitWorker>,
    pub drag_start: Option<MapViewport>,
    pub property: Option<String>,
    pub source_open: bool,
    pub location_editor: bool,
    pub coordinate_entry: bool,
    pub points: Vec<MapPoint>,
    pub truncated: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub tiles: Vec<MapTile>,
    pub tile_error: Option<String>,
    pub surface: Option<MapSurface>,
    pub place_query: TextEdit,
    pub place_name: TextEdit,
    pub latitude: TextEdit,
    pub longitude: TextEdit,
    pub record_query: TextEdit,
    pub candidates: Vec<Record>,
    pub selected_place: Option<PlaceValue>,
    pub search_results: Vec<PlaceValue>,
    pub search_selected: usize,
    pub search_error: Option<String>,
    pub search_loading: bool,
    pub selected_record: Option<String>,
    pub expected_place: Value,
    view_key: String,
    request_key: String,
    epoch: u64,
    tile_loader: Option<MapTileLoader>,
    tile_viewport: Option<MapViewport>,
    wake: Option<Waker>,
    query_tx: Option<mpsc::SyncSender<QueryRequest>>,
    query_rx: Option<mpsc::Receiver<QueryResponse>>,
    query_worker: Option<std::thread::JoinHandle<()>>,
    query_stop: Arc<AtomicBool>,
    query_interrupt: Arc<Mutex<Option<moss_core::InterruptHandle>>>,
    query_pending: Option<QueryRequest>,
    search_rx: Option<mpsc::Receiver<SearchResponse>>,
    search_epoch: u64,
    search_cache: BTreeMap<String, Vec<PlaceValue>>,
}
impl Default for MapState {
    fn default() -> Self {
        Self {
            viewport: Default::default(),
            fit_pending: false,
            fit_epoch: 0,
            fit_start: None,
            fit_worker: None,
            drag_start: None,
            property: None,
            source_open: false,
            location_editor: false,
            coordinate_entry: false,
            points: vec![],
            truncated: false,
            loading: false,
            error: None,
            tiles: vec![],
            tile_error: None,
            surface: None,
            place_query: TextEdit::new(""),
            place_name: TextEdit::new(""),
            latitude: TextEdit::new(""),
            longitude: TextEdit::new(""),
            record_query: TextEdit::new(""),
            candidates: vec![],
            selected_place: None,
            search_results: vec![],
            search_selected: 0,
            search_error: None,
            search_loading: false,
            selected_record: None,
            expected_place: Value::Null,
            view_key: String::new(),
            request_key: String::new(),
            epoch: 0,
            tile_loader: None,
            tile_viewport: None,
            wake: None,
            query_tx: None,
            query_rx: None,
            query_worker: None,
            query_stop: Arc::new(AtomicBool::new(false)),
            query_interrupt: Arc::new(Mutex::new(None)),
            query_pending: None,
            search_rx: None,
            search_epoch: 0,
            search_cache: BTreeMap::new(),
        }
    }
}
impl MapState {
    pub fn cancel_fit(&mut self) {
        self.fit_epoch = self.fit_epoch.wrapping_add(1);
        self.fit_pending = false;
        self.fit_start = None;
        if let Some(worker) = &mut self.fit_worker {
            worker.cancel();
        }
    }
    pub fn set_waker(&mut self, wake: Waker) {
        if !surface::use_maplibre() {
            self.tile_loader = Some(MapTileLoader::new(wake.clone()));
        }
        self.wake = Some(wake);
    }
    pub fn invalidate(&mut self) {
        self.request_key.clear();
    }
    pub fn resize(&mut self, w: f32, h: f32) {
        let (w, h) = ((w as f64).clamp(1., 4096.), (h as f64).clamp(1., 2048.));
        if (self.viewport.width - w).abs() > 1. || (self.viewport.height - h).abs() > 1. {
            self.viewport.width = w;
            self.viewport.height = h;
            self.invalidate();
        }
    }
    pub fn set_coordinates(&mut self) -> bool {
        let (Ok(latitude), Ok(longitude)) = (
            self.latitude.text.trim().parse(),
            self.longitude.text.trim().parse(),
        ) else {
            self.search_error = Some("Enter valid latitude and longitude.".into());
            return false;
        };
        let place = PlaceValue {
            name: self.place_name.text.trim().to_owned(),
            latitude,
            longitude,
        };
        if !place.valid() {
            self.search_error = Some("Enter a name and coordinates within valid ranges.".into());
            return false;
        }
        self.viewport.latitude = place.latitude.clamp(-MAX_LAT, MAX_LAT);
        self.viewport.longitude = place.longitude;
        self.viewport.zoom = self.viewport.zoom.max(12.);
        self.invalidate();
        self.selected_place = Some(place);
        self.search_error = None;
        true
    }
    fn ensure_workers(&mut self, path: PathBuf) {
        if self.query_tx.is_none() {
            let (tx, rx) = mpsc::sync_channel::<QueryRequest>(1);
            let (send, receive) = mpsc::channel();
            let wake = self.wake.clone();
            let stop = self.query_stop.clone();
            let interrupt = self.query_interrupt.clone();
            let dbpath = path.join("workspace.sqlite3");
            self.query_worker = Some(std::thread::spawn(move || {
                let store = Store::open_read_only(dbpath);
                if let Ok(opened) = &store {
                    *interrupt.lock().unwrap() = Some(opened.interrupt_handle());
                }
                while let Ok(req) = rx.recv() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let data = store.as_ref().map_err(ToString::to_string);
                    let points = data.clone().and_then(|s| {
                        s.map_points(
                            &req.db,
                            &req.query,
                            &req.title,
                            &req.place,
                            req.bounds,
                            MAX_MARKERS,
                        )
                        .map_err(|e| e.to_string())
                    });
                    let candidates = data.and_then(|s| {
                        let mut q = req.query.clone();
                        q.offset = 0;
                        q.limit = 100;
                        let db = s
                            .get_database(&req.db)
                            .map_err(|e| e.to_string())?
                            .ok_or("Database missing")?;
                        let title = db
                            .properties
                            .iter()
                            .find(|p| p.kind == moss_core::PropertyKind::Title)
                            .ok_or("Title property missing")?;
                        for term in [&req.title, &req.record_search] {
                            if !term.is_empty() {
                                let f = moss_core::FilterExpr::Rule(moss_core::FilterRule {
                                    property_id: title.id.clone(),
                                    op: moss_core::FilterOp::Contains,
                                    value: json!(term),
                                });
                                q.filter = Some(match q.filter.take() {
                                    Some(old) => moss_core::FilterExpr::All(vec![old, f]),
                                    None => f,
                                });
                            }
                        }
                        s.query_records(&req.db, &q).map_err(|e| e.to_string())
                    });
                    if stop.load(Ordering::Acquire)
                        || send
                            .send(QueryResponse {
                                epoch: req.epoch,
                                points,
                                candidates,
                            })
                            .is_err()
                    {
                        break;
                    }
                    if let Some(w) = &wake {
                        w();
                    }
                }
            }));
            self.query_tx = Some(tx);
            self.query_rx = Some(receive);
        }
    }
    pub fn search(&mut self) {
        let query = self.place_query.text.trim().to_owned();
        if query.len() > 256 {
            self.search_error = Some("Place search is limited to 256 bytes.".into());
            return;
        }
        if query.is_empty() {
            self.search_error = Some("Enter a place to search.".into());
            return;
        }
        if let Some(cached) = self.search_cache.get(&query) {
            self.search_results = cached.clone();
            self.search_error = None;
            return;
        }
        self.search_epoch += 1;
        let epoch = self.search_epoch;
        let (send, recv) = mpsc::channel();
        self.search_rx = Some(recv);
        self.search_loading = true;
        self.search_error = None;
        let wake = self.wake.clone();
        std::thread::spawn(move || {
            let result = photon(&query);
            let _ = send.send(SearchResponse {
                epoch,
                query,
                result,
            });
            if let Some(w) = wake {
                w();
            }
        });
    }
    pub fn search_changed(&mut self) {
        self.search_epoch = self.search_epoch.wrapping_add(1);
        self.search_loading = false;
        self.search_results.clear();
        self.search_error = None;
        self.search_rx = None;
    }
    fn poll_search(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(receiver) = &self.search_rx {
            if let Ok(response) = receiver.try_recv() {
                if response.epoch == self.search_epoch {
                    self.search_loading = false;
                    match response.result {
                        Ok(places) => {
                            if self.search_cache.len() >= 64 {
                                if let Some(key) = self.search_cache.keys().next().cloned() {
                                    self.search_cache.remove(&key);
                                }
                            }
                            self.search_cache.insert(response.query, places.clone());
                            self.search_results = places;
                        }
                        Err(error) => self.search_error = Some(error),
                    }
                    changed = true;
                }
            }
        }
        if self.search_loading {
            tx.wake_after(Duration::from_millis(50));
        }
        changed
    }
}
impl Drop for MapState {
    fn drop(&mut self) {
        self.query_stop.store(true, Ordering::Release);
        self.query_tx.take();
        if let Ok(interrupt) = self.query_interrupt.lock() {
            if let Some(interrupt) = interrupt.as_ref() {
                interrupt.interrupt();
            }
        }
        if let Some(worker) = self.query_worker.take() {
            let _ = worker.join();
        }
        self.fit_worker.take();
    }
}

fn map_context(s: &crate::state::State) -> Option<(String, String, RecordQuery, String, Value)> {
    let tab = s.tab()?;
    let db = tab.database.as_ref()?;
    let view = db.views.get(tab.view)?;
    (view.kind == ViewKind::Map).then(|| {
        (
            db.id.clone(),
            view.id.clone(),
            view.query.clone(),
            tab.title_search.clone(),
            view.config.clone(),
        )
    })
}
fn fit_context(
    db: &str,
    view: &str,
    query: &RecordQuery,
    title: &str,
    place: &str,
    generation: u64,
) -> String {
    format!(
        "{db}|{view}|{}|{title}|{place}|{generation}",
        serde_json::to_string(query).unwrap_or_default()
    )
}
impl crate::state::State {
    pub fn retry_map_tiles(&mut self) {
        if let Some(surface) = &self.database_views.map.surface {
            surface.retry();
        }
        if let Some(loader) = &mut self.database_views.map.tile_loader {
            loader.retry();
        }
    }
    pub fn fit_map_pins(&mut self) {
        if !self.finish_map_interaction() {
            return;
        }
        let Some((db, view, query, title, config)) = map_context(self) else {
            return;
        };
        let place = config
            .get("map_place_property")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if place.is_empty() {
            let m = &mut self.database_views.map;
            m.cancel_fit();
            m.viewport = MapViewport {
                width: m.viewport.width,
                height: m.viewport.height,
                zoom: 1.,
                ..Default::default()
            };
            m.invalidate();
            self.map_position_changed();
            return;
        }
        let context = fit_context(
            &db,
            &view,
            &query,
            &title,
            &place,
            self.store.local_change_count(),
        );
        let m = &mut self.database_views.map;
        m.cancel_fit();
        m.fit_pending = true;
        m.fit_start = Some(m.viewport);
        if m.fit_worker.is_none() {
            m.fit_worker = Some(fit::FitWorker::new(self.path.clone(), m.wake.clone()));
        }
        m.fit_worker.as_mut().unwrap().submit(fit::Request {
            epoch: m.fit_epoch,
            context,
            db,
            query,
            title,
            place,
        });
    }
    pub fn select_map_source(&mut self, id: &str) {
        let Some((db_id, view_id, _, _, config)) = map_context(self) else {
            return;
        };
        if !self
            .tab()
            .and_then(|t| t.database.as_ref())
            .is_some_and(|db| {
                db.properties
                    .iter()
                    .any(|p| p.id == id && p.kind == moss_core::PropertyKind::Place)
            })
        {
            self.error = Some("Place property is unavailable.".into());
            return;
        }
        let old = config
            .get("map_place_property")
            .cloned()
            .unwrap_or(Value::Null);
        let new = id.to_owned();
        match self
            .store
            .edit_view_config(&db_id, &view_id, ViewKind::Map, move |c| {
                if c.get("map_place_property").cloned().unwrap_or(Value::Null) != old {
                    return Err("Map source changed in another window.".into());
                }
                c.as_object_mut()
                    .ok_or("Invalid view settings")?
                    .insert("map_place_property".into(), json!(new));
                Ok(())
            }) {
            Ok(db) => {
                for tab in &mut self.tabs {
                    if tab.database.as_ref().is_some_and(|d| d.id == db_id) {
                        tab.database = Some(db.clone());
                    }
                }
                let m = &mut self.database_views.map;
                m.property = Some(id.into());
                m.source_open = false;
                m.points.clear();
                m.candidates.clear();
                m.selected_record = None;
                m.invalidate();
            }
            Err(e) => self.error = Some(format!("Map source is not saved: {e}")),
        }
    }
    pub fn create_map_source(&mut self) {
        let Some((db_id, _, _, _, _)) = map_context(self) else {
            return;
        };
        match self
            .store
            .add_property(&db_id, "Place", moss_core::PropertyKind::Place)
        {
            Ok(db) => {
                let id = db.properties.last().map(|p| p.id.clone());
                for tab in &mut self.tabs {
                    if tab.database.as_ref().is_some_and(|d| d.id == db_id) {
                        tab.database = Some(db.clone());
                    }
                }
                if let Some(id) = id {
                    self.select_map_source(&id);
                }
            }
            Err(e) => self.error = Some(format!("Place property is not created: {e}")),
        }
    }
    pub fn save_map_viewport(&mut self) -> bool {
        let Some((db_id, view_id, _, _, config)) = map_context(self) else {
            return false;
        };
        let viewport = self.database_views.map.viewport;
        let old = ["map_center_lat", "map_center_lon", "map_zoom"]
            .map(|key| config.get(key).cloned().unwrap_or(Value::Null));
        match self
            .store
            .edit_view_config(&db_id, &view_id, ViewKind::Map, move |c| {
                for (i, key) in ["map_center_lat", "map_center_lon", "map_zoom"]
                    .into_iter()
                    .enumerate()
                {
                    if c.get(key).cloned().unwrap_or(Value::Null) != old[i] {
                        return Err("Map position changed in another window.".into());
                    }
                }
                let object = c.as_object_mut().ok_or("Invalid view settings")?;
                object.insert("map_center_lat".into(), json!(viewport.latitude));
                object.insert("map_center_lon".into(), json!(viewport.longitude));
                object.insert("map_zoom".into(), json!(viewport.zoom));
                Ok(())
            }) {
            Ok(db) => {
                for tab in &mut self.tabs {
                    if tab.database.as_ref().is_some_and(|d| d.id == db_id) {
                        tab.database = Some(db.clone());
                    }
                }
                true
            }
            Err(e) => {
                self.error = Some(format!("Map position is not saved: {e}"));
                false
            }
        }
    }
    pub fn choose_map_record(&mut self, id: &str) {
        let m = &mut self.database_views.map;
        let Some(record) = m.candidates.iter().find(|r| r.id == id) else {
            return;
        };
        m.expected_place = m
            .property
            .as_ref()
            .and_then(|p| record.values.get(p))
            .cloned()
            .unwrap_or(Value::Null);
        m.selected_record = Some(id.into());
    }
    pub fn save_map_place(&mut self) {
        let m = &self.database_views.map;
        let (Some(id), Some(property), Some(place)) = (
            m.selected_record.clone(),
            m.property.clone(),
            m.selected_place.clone(),
        ) else {
            self.database_views.map.search_error = Some("Choose a place and a record.".into());
            return;
        };
        let expected = m.expected_place.clone();
        match self
            .store
            .set_record_place_if_unchanged(&id, &property, &expected, Some(place))
        {
            Ok(record) => {
                self.sync_record(&record);
                let m = &mut self.database_views.map;
                m.expected_place = record.values.get(&property).cloned().unwrap_or(Value::Null);
                m.search_error = None;
                m.invalidate();
            }
            Err(e) => {
                self.database_views.map.search_error = Some(format!("Place is not saved: {e}"))
            }
        }
    }
}
pub fn poll(s: &mut crate::state::State, _time: f64, tx: &mut Tx) -> bool {
    let Some((db_id, view_id, query, title, config)) = map_context(s) else {
        let m = &mut s.database_views.map;
        if !m.view_key.is_empty() {
            m.view_key.clear();
            m.cancel_fit();
            m.epoch = m.epoch.wrapping_add(1);
            if let Some(loader) = &mut m.tile_loader {
                loader.pause();
            }
            m.surface = None;
            m.tile_viewport = None;
            m.query_pending = None;
            if !s
                .cell_picker
                .as_ref()
                .is_some_and(crate::property_controls::CellPicker::is_place)
            {
                m.search_changed();
            }
        }
        return m.poll_search(tx);
    };
    let path = s.path.clone();
    let generation = s.store.local_change_count();
    let fit_place = config
        .get("map_place_property")
        .and_then(Value::as_str)
        .unwrap_or("");
    let fit_identity = fit_context(&db_id, &view_id, &query, &title, fit_place, generation);
    let key = format!("{db_id}/{view_id}");
    let moving = s.database_views.map_interaction.is_moving();
    let dark_hint = s.prefs.dark.unwrap_or(false);
    let m = &mut s.database_views.map;
    let mut changed = false;
    if m.view_key != key {
        m.cancel_fit();
        m.view_key = key;
        m.property = config
            .get("map_place_property")
            .and_then(Value::as_str)
            .map(str::to_owned);
        m.viewport = MapViewport {
            width: m.viewport.width,
            height: m.viewport.height,
            ..MapViewport::from_config(&config)
        };
        m.points.clear();
        m.candidates.clear();
        m.selected_record = None;
        m.selected_place = None;
        m.place_query = TextEdit::new("");
        m.record_query = TextEdit::new("");
        m.place_name = TextEdit::new("");
        m.latitude = TextEdit::new("");
        m.longitude = TextEdit::new("");
        m.search_changed();
        m.location_editor = false;
        m.coordinate_entry = false;
        m.invalidate();
        changed = true;
    }
    let signature = format!(
        "{}|{}|{}|{:?}|{}|{}",
        db_id,
        view_id,
        serde_json::to_string(&query).unwrap_or_default(),
        m.viewport,
        title,
        m.record_query.text
    );
    let configured = config
        .get("map_place_property")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if m.property != configured {
        m.property = configured;
        m.points.clear();
        m.candidates.clear();
        m.selected_record = None;
        m.invalidate();
        changed = true;
    }
    let signature = format!("{signature}|{:?}|{generation}", m.property);
    if m.request_key != signature && m.drag_start.is_none() && !moving {
        m.ensure_workers(path.clone());
        m.request_key = signature;
        m.epoch = m.epoch.wrapping_add(1);
        let epoch = m.epoch;
        m.loading = m.property.is_some();
        m.error = None;
        if let Some(place) = m.property.clone() {
            m.query_pending = Some(QueryRequest {
                epoch,
                db: db_id,
                query,
                title,
                place,
                bounds: m.viewport.bounds(),
                record_search: m.record_query.text.clone(),
            });
        }
        changed = true;
    }
    if let Some(request) = m.query_pending.take() {
        if let Some(sender) = &m.query_tx {
            match sender.try_send(request) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(request)) => m.query_pending = Some(request),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    m.loading = false;
                    m.error = Some("Map data worker stopped.".into());
                }
            }
        }
    }
    if surface::use_maplibre() && m.tile_loader.is_none() {
        if m.surface.is_none() {
            if let Some(wake) = m.wake.clone() {
                m.surface = MapSurface::start(m.viewport, path.clone(), dark_hint, 2.0, wake);
                if m.surface.is_some() {
                    m.tile_viewport = Some(m.viewport);
                    changed = true;
                }
            }
        }
        if let Some(surface) = &m.surface {
            if m.tile_viewport != Some(m.viewport) {
                surface.set_viewport(m.viewport);
                m.tile_viewport = Some(m.viewport);
                changed = true;
            }
            let next_err = surface.error();
            if m.tile_error != next_err {
                m.tile_error = next_err;
                changed = true;
            }
        }
    } else if let Some(loader) = &mut m.tile_loader {
        if m.tile_viewport != Some(m.viewport) {
            loader.request(m.viewport, path.clone());
            m.tile_viewport = Some(m.viewport);
            changed = true;
        }
        if loader.poll() {
            changed = true;
        }
        m.tiles = loader.tiles.clone();
        m.tile_error = loader.error.clone();
    }
    if let Some(receiver) = &m.query_rx {
        while let Ok(result) = receiver.try_recv() {
            if result.epoch != m.epoch {
                continue;
            }
            m.loading = false;
            match result.points {
                Ok(points) => {
                    m.points = points.points;
                    m.truncated = points.truncated;
                }
                Err(e) => m.error = Some(format!("Map points are not loaded: {e}")),
            }
            match result.candidates {
                Ok(records) => m.candidates = records,
                Err(e) => m.error = Some(format!("Records are not loaded: {e}")),
            }
            changed = true;
        }
    }
    let mut fitted = false;
    if let Some(worker) = &mut m.fit_worker {
        while let Some(response) = worker.poll() {
            if !m.fit_pending || response.epoch != m.fit_epoch {
                continue;
            }
            m.fit_pending = false;
            let start = m.fit_start.take();
            if response.context != fit_identity
                || !start.is_some_and(|start| {
                    start.latitude == m.viewport.latitude
                        && start.longitude == m.viewport.longitude
                        && start.zoom == m.viewport.zoom
                })
                || m.drag_start.is_some()
            {
                continue;
            }
            match response.result {
                Ok(bounds) => {
                    m.viewport = match bounds {
                        Some(bounds) => m.viewport.fit_bounds(bounds, 40.),
                        None => MapViewport {
                            width: m.viewport.width,
                            height: m.viewport.height,
                            zoom: 1.,
                            ..Default::default()
                        },
                    };
                    m.request_key.clear();
                    fitted = true;
                }
                Err(error) => m.error = Some(format!("Map extent is not loaded: {error}")),
            }
            changed = true;
        }
    }
    changed |= m.poll_search(tx);
    if m.loading
        || m.search_loading
        || m.query_pending.is_some()
        || m.tiles.iter().any(|t| !t.image.is_ready())
    {
        tx.wake_after(Duration::from_millis(50));
    }
    if fitted {
        s.map_position_changed();
    }
    changed
}

#[cfg(test)]
#[path = "map_view_tests.rs"]
mod ui_tests;
