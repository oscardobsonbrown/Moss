//! Direct-to-IOSurface Metal MapLibre Native renderer for Moss on macOS arm64.

use std::path::PathBuf;

const STYLE_TEMPLATE: &str = include_str!("../assets/apple-style.json");

const LIGHT_PALETTE: &[(&str, &str)] = &[
    ("{{NAME}}", "Moss Apple Light"),
    ("{{LAND}}", "#f7f1df"),
    ("{{NATURAL}}", "#d0e3b4"),
    ("{{PARK}}", "#bde6ab"),
    ("{{HOSPITAL}}", "#fbd3da"),
    ("{{AIRPORT}}", "#cfb2db"),
    ("{{WATER}}", "#a2daf2"),
    ("{{BUILDING}}", "#e5dcc7"),
    ("{{ROAD_LOCAL_CASE}}", "#ddd6c7"),
    ("{{ROAD_LOCAL}}", "#fdfcf8"),
    ("{{ROAD_ART_CASE}}", "#d9d1c0"),
    ("{{ROAD_ART}}", "#ffffff"),
    ("{{ROAD_HWY_CASE}}", "#efd151"),
    ("{{ROAD_HWY}}", "#ffe15f"),
    ("{{LBL_WATER}}", "#4f86a6"),
    ("{{LBL_WATER_HALO}}", "#c9ebfa"),
    ("{{LBL_ROAD}}", "#625e56"),
    ("{{LBL_SUBURB}}", "#736d64"),
    ("{{LBL_CITY}}", "#3f3b35"),
    ("{{LBL_COUNTRY}}", "#625e56"),
];

const DARK_PALETTE: &[(&str, &str)] = &[
    ("{{NAME}}", "Moss Apple Dark"),
    ("{{LAND}}", "#1f2124"),
    ("{{NATURAL}}", "#222b23"),
    ("{{PARK}}", "#213426"),
    ("{{HOSPITAL}}", "#38242a"),
    ("{{AIRPORT}}", "#2d2638"),
    ("{{WATER}}", "#162b3d"),
    ("{{BUILDING}}", "#282b30"),
    ("{{ROAD_LOCAL_CASE}}", "#1c1e21"),
    ("{{ROAD_LOCAL}}", "#33373e"),
    ("{{ROAD_ART_CASE}}", "#1e2024"),
    ("{{ROAD_ART}}", "#444952"),
    ("{{ROAD_HWY_CASE}}", "#7d6422"),
    ("{{ROAD_HWY}}", "#b89434"),
    ("{{LBL_WATER}}", "#69a7cc"),
    ("{{LBL_WATER_HALO}}", "#162b3d"),
    ("{{LBL_ROAD}}", "#b0b5bd"),
    ("{{LBL_SUBURB}}", "#969ca6"),
    ("{{LBL_CITY}}", "#e4e7ec"),
    ("{{LBL_COUNTRY}}", "#adb3bc"),
];

pub fn style_json(dark: bool) -> String {
    let mut out = STYLE_TEMPLATE.to_owned();
    for (token, value) in if dark { DARK_PALETTE } else { LIGHT_PALETTE } {
        out = out.replace(token, value);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraState {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub latitude: f64,
    pub longitude: f64,
    /// MapLibre 512 px zoom level (Moss 256 px zoom minus 1.0).
    pub zoom: f64,
    pub dark: bool,
}

#[derive(Clone, Debug)]
pub struct RendererConfig {
    pub cache_path: PathBuf,
    pub max_cache_bytes: u64,
    pub initial: CameraState,
}

pub fn is_supported() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod platform {
    use super::*;
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;

    unsafe extern "C" {
        fn moss_mln_create(
            w: u32,
            h: u32,
            scale: f32,
            cache: *const c_char,
            max_bytes: u64,
            style: *const c_char,
            lat: f64,
            lng: f64,
            zoom: f64,
            on_frame: extern "C" fn(*mut c_void, *mut c_void),
            on_error: extern "C" fn(*mut c_void, *const c_char),
            ctx: *mut c_void,
        ) -> *mut c_void;
        fn moss_mln_set_camera(ptr: *mut c_void, lat: f64, lng: f64, zoom: f64);
        fn moss_mln_resize(ptr: *mut c_void, w: u32, h: u32);
        fn moss_mln_set_style_json(ptr: *mut c_void, style: *const c_char);
        fn moss_mln_pump(ptr: *mut c_void, timeout_sec: f64);
        fn moss_mln_wake(ptr: *mut c_void);
        fn moss_mln_destroy(ptr: *mut c_void);
    }

    struct Callbacks {
        on_frame: Box<dyn Fn(*mut c_void) + Send + Sync>,
        on_error: Box<dyn Fn(Option<String>) + Send + Sync>,
    }

    extern "C" fn frame_trampoline(ctx: *mut c_void, buf: *mut c_void) {
        if !ctx.is_null() && !buf.is_null() {
            (unsafe { &*(ctx as *const Callbacks) }.on_frame)(buf);
        }
    }

    extern "C" fn error_trampoline(ctx: *mut c_void, msg: *const c_char) {
        if !ctx.is_null() {
            let err = (!msg.is_null()).then(|| {
                unsafe { CStr::from_ptr(msg) }
                    .to_string_lossy()
                    .into_owned()
            });
            (unsafe { &*(ctx as *const Callbacks) }.on_error)(err);
        }
    }

    struct HostPtr(*mut c_void);
    // Safety: `moss_mln_wake` is thread-safe (`CFRunLoopSourceSignal` + `CFRunLoopWakeUp`),
    // and the worker clears this pointer under the mutex before `moss_mln_destroy`.
    unsafe impl Send for HostPtr {}

    struct Shared {
        desired: Mutex<(CameraState, u64)>,
        stop: AtomicBool,
        host: Mutex<HostPtr>,
    }

    pub struct MapLibreRenderer {
        shared: Arc<Shared>,
        worker: Option<JoinHandle<()>>,
    }

    impl MapLibreRenderer {
        pub fn new(
            config: RendererConfig,
            on_frame: Box<dyn Fn(*mut c_void) + Send + Sync>,
            on_error: Box<dyn Fn(Option<String>) + Send + Sync>,
        ) -> Option<Self> {
            let shared = Arc::new(Shared {
                desired: Mutex::new((config.initial, 0)),
                stop: AtomicBool::new(false),
                host: Mutex::new(HostPtr(std::ptr::null_mut())),
            });
            let s = shared.clone();
            let worker = std::thread::Builder::new()
                .name("moss-maplibre-metal".into())
                .spawn(move || {
                    if let Some(parent) = config.cache_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let cache = CString::new(config.cache_path.to_string_lossy().as_ref())
                        .unwrap_or_else(|_| CString::new("").unwrap());
                    let styles = [
                        CString::new(style_json(false)).unwrap(),
                        CString::new(style_json(true)).unwrap(),
                    ];
                    let cbs = Box::new(Callbacks { on_frame, on_error });
                    let cb_ptr = (&*cbs as *const Callbacks) as *mut c_void;

                    let create = |c: CameraState| unsafe {
                        moss_mln_create(
                            c.width.max(1),
                            c.height.max(1),
                            c.scale.clamp(1.0, 3.0),
                            cache.as_ptr(),
                            config.max_cache_bytes,
                            styles[usize::from(c.dark)].as_ptr(),
                            c.latitude,
                            c.longitude,
                            c.zoom.clamp(0.0, 18.0),
                            frame_trampoline,
                            error_trampoline,
                            cb_ptr,
                        )
                    };

                    let mut cur = config.initial;
                    let mut cur_retry = 0u64;
                    let mut host = create(cur);
                    s.host.lock().unwrap().0 = host;

                    while !s.stop.load(Ordering::Acquire) {
                        let (want, retry) = *s.desired.lock().unwrap();
                        if (want.scale - cur.scale).abs() > 0.01 {
                            s.host.lock().unwrap().0 = std::ptr::null_mut();
                            unsafe { moss_mln_destroy(host) };
                            (cur, cur_retry) = (want, retry);
                            host = create(cur);
                            s.host.lock().unwrap().0 = host;
                        } else {
                            if (want.width, want.height) != (cur.width, cur.height) {
                                (cur.width, cur.height) = (want.width, want.height);
                                unsafe {
                                    moss_mln_resize(host, cur.width.max(1), cur.height.max(1))
                                };
                            }
                            if want.dark != cur.dark || retry != cur_retry {
                                (cur.dark, cur_retry) = (want.dark, retry);
                                unsafe {
                                    moss_mln_set_style_json(
                                        host,
                                        styles[usize::from(cur.dark)].as_ptr(),
                                    )
                                };
                            }
                            if (want.latitude, want.longitude, want.zoom)
                                != (cur.latitude, cur.longitude, cur.zoom)
                            {
                                (cur.latitude, cur.longitude, cur.zoom) =
                                    (want.latitude, want.longitude, want.zoom);
                                unsafe {
                                    moss_mln_set_camera(
                                        host,
                                        cur.latitude,
                                        cur.longitude,
                                        cur.zoom.clamp(0.0, 18.0),
                                    )
                                };
                            }
                        }
                        unsafe { moss_mln_pump(host, 0.025) };
                    }

                    s.host.lock().unwrap().0 = std::ptr::null_mut();
                    unsafe { moss_mln_destroy(host) };
                    drop(cbs);
                })
                .ok()?;

            Some(Self {
                shared,
                worker: Some(worker),
            })
        }

        pub fn set_camera(&self, camera: CameraState) {
            let mut guard = self.shared.desired.lock().unwrap();
            if guard.0 != camera {
                guard.0 = camera;
                drop(guard);
                self.wake();
            }
        }

        pub fn retry(&self) {
            self.shared.desired.lock().unwrap().1 =
                self.shared.desired.lock().unwrap().1.wrapping_add(1);
            self.wake();
        }

        fn wake(&self) {
            let guard = self.shared.host.lock().unwrap();
            if !guard.0.is_null() {
                unsafe { moss_mln_wake(guard.0) };
            }
        }
    }

    impl Drop for MapLibreRenderer {
        fn drop(&mut self) {
            self.shared.stop.store(true, Ordering::Release);
            self.wake();
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
mod platform {
    use super::*;
    use std::ffi::c_void;

    pub struct MapLibreRenderer;
    impl MapLibreRenderer {
        pub fn new(
            _: RendererConfig,
            _: Box<dyn Fn(*mut c_void) + Send + Sync>,
            _: Box<dyn Fn(Option<String>) + Send + Sync>,
        ) -> Option<Self> {
            None
        }
        pub fn set_camera(&self, _: CameraState) {}
        pub fn retry(&self) {}
    }
}

pub use platform::MapLibreRenderer;
