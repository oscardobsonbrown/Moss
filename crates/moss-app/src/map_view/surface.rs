//! Zero-copy MapLibre Native Metal surface backed by `rune::NativeSurface`.

use super::{MapViewport, viewport::MAX_LAT};
use moss_maplibre::{CameraState, MapLibreRenderer, RendererConfig};
use rune::{NativeSurface, Waker};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const MAX_CACHE_BYTES: u64 = 48 * 1024 * 1024;

pub fn use_maplibre() -> bool {
    moss_maplibre::is_supported()
        && std::env::var("MOSS_MAP_TILE_URL").is_err()
        && std::env::var("MOSS_MAP_NETWORK").as_deref() != Ok("off")
        && (!cfg!(test) || std::env::var("MOSS_MAP_TEST_MAPLIBRE").as_deref() == Ok("1"))
}

pub fn camera_from_viewport(viewport: MapViewport, dark: bool, scale: f32) -> CameraState {
    CameraState {
        width: viewport.width.round().clamp(1.0, 4096.0) as u32,
        height: viewport.height.round().clamp(1.0, 2048.0) as u32,
        scale: if scale.is_finite() && scale > 0.0 {
            scale.clamp(1.0, 3.0)
        } else {
            1.0
        },
        latitude: viewport.latitude.clamp(-MAX_LAT, MAX_LAT),
        longitude: viewport.longitude,
        // Moss MapViewport uses 256 px tiles; MapLibre Native uses 512 px base tiles.
        zoom: (viewport.zoom - 1.0).clamp(0.0, 18.0),
        dark,
    }
}

pub struct MapSurface {
    surface: NativeSurface,
    renderer: MapLibreRenderer,
    error: Arc<Mutex<Option<String>>>,
    #[allow(dead_code)]
    frames: Arc<AtomicU64>,
    last: Mutex<(bool, f32)>,
}

impl MapSurface {
    pub fn start(
        viewport: MapViewport,
        workspace_dir: PathBuf,
        dark: bool,
        scale: f32,
        wake: Waker,
    ) -> Option<Self> {
        let surface = NativeSurface::new();
        let error = Arc::new(Mutex::new(None::<String>));
        let frames = Arc::new(AtomicU64::new(0));

        let (surface_cb, wake_frame, frames_cb) = (surface.clone(), wake.clone(), frames.clone());
        let on_frame = Box::new(move |raw: *mut std::ffi::c_void| {
            #[cfg(target_os = "macos")]
            if let Ok(pb) = unsafe { rune::native_surface::macos::PixelBuffer::from_raw(raw) } {
                let seq = frames_cb.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = surface_cb.push(pb, seq);
                wake_frame();
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (raw, &surface_cb, &wake_frame, &frames_cb);
        });

        let error_cb = error.clone();
        let on_error = Box::new(move |msg: Option<String>| {
            *error_cb.lock().unwrap() = msg.map(|m| {
                if m.is_empty() {
                    "Map tiles are unavailable. Check your connection.".to_owned()
                } else {
                    format!("Map tiles are unavailable: {m}")
                }
            });
            wake();
        });

        let renderer = MapLibreRenderer::new(
            RendererConfig {
                cache_path: workspace_dir.join("map-tile-cache/maplibre-cache.db"),
                max_cache_bytes: MAX_CACHE_BYTES,
                initial: camera_from_viewport(viewport, dark, scale),
            },
            on_frame,
            on_error,
        )?;
        Some(Self {
            surface,
            renderer,
            error,
            frames,
            last: Mutex::new((dark, scale)),
        })
    }

    pub fn native_surface(&self) -> &NativeSurface {
        &self.surface
    }

    pub fn sync_from_view(&self, viewport: MapViewport, dark: bool, scale: f32) {
        *self.last.lock().unwrap() = (dark, scale);
        self.renderer
            .set_camera(camera_from_viewport(viewport, dark, scale));
    }

    pub fn set_viewport(&self, viewport: MapViewport) {
        let (dark, scale) = *self.last.lock().unwrap();
        self.renderer
            .set_camera(camera_from_viewport(viewport, dark, scale));
    }

    pub fn retry(&self) {
        *self.error.lock().unwrap() = None;
        self.renderer.retry();
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }

    #[allow(dead_code)]
    pub fn frame_count(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moss_zoom_converts_to_maplibre_512px_zoom() {
        let vp = MapViewport {
            latitude: 35.6812,
            longitude: 139.7671,
            zoom: 15.0,
            width: 960.0,
            height: 600.0,
        };
        let cam = camera_from_viewport(vp, true, 2.0);
        assert_eq!((cam.width, cam.height, cam.scale, cam.zoom, cam.dark), (960, 600, 2.0, 14.0, true));
        let min = camera_from_viewport(MapViewport { zoom: 0.0, ..vp }, false, 0.0);
        assert_eq!((min.zoom, min.scale, min.dark), (0.0, 1.0, false));
    }

    #[test]
    fn embedded_light_and_dark_styles_are_valid_json() {
        for dark in [false, true] {
            let json = moss_maplibre::style_json(dark);
            assert!(!json.contains("{{"), "unresolved palette token in style");
            let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid style JSON");
            assert_eq!(parsed["version"], 8);
            assert_eq!(parsed["layers"].as_array().unwrap().len(), 22);
        }
    }

    #[test]
    #[ignore = "Requires macOS Metal GPU and exercises the live MapLibre Native IOSurface worker"]
    fn metal_surface_renders_iosurface_frame() {
        let dir = std::env::temp_dir().join(format!("moss-mln-smoke-{}", moss_core::new_id()));
        let vp = MapViewport {
            latitude: 35.6812,
            longitude: 139.7671,
            zoom: 14.0,
            width: 640.0,
            height: 400.0,
        };
        let surface = MapSurface::start(vp, dir.clone(), false, 2.0, Arc::new(|| {}))
            .expect("MapSurface starts on macOS arm64");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while surface.frame_count() == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(surface.frame_count() > 0, "must deliver at least one IOSurface frame");
        drop(surface);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
