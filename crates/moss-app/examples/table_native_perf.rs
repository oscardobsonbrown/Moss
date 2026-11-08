//! Disposable, self-driven native Table presentation benchmark.
#![allow(dead_code)]
#[path = "../src/advanced_properties.rs"]
mod advanced_properties;
#[path = "../src/block_commands.rs"]
mod block_commands;
#[path = "../src/board_interaction.rs"]
mod board_interaction;
#[path = "../src/board_view.rs"]
mod board_view;
#[path = "../src/calendar_view.rs"]
mod calendar_view;
#[path = "../src/caret.rs"]
mod caret;
#[path = "../src/chart_view.rs"]
mod chart_view;
#[path = "../src/child_pages.rs"]
mod child_pages;
#[path = "../src/computed_refresh.rs"]
mod computed_refresh;
#[path = "../src/dashboard_view.rs"]
mod dashboard_view;
#[path = "../src/database_properties.rs"]
mod database_properties;
#[path = "../src/database_views.rs"]
mod database_views;
#[path = "../src/editor_commands.rs"]
mod editor_commands;
#[path = "../src/emoji_catalog.rs"]
mod emoji_catalog;
#[path = "../src/fonts.rs"]
mod fonts;
#[path = "../src/map_clusters.rs"]
mod map_clusters;
#[path = "../src/map_interaction.rs"]
mod map_interaction;
#[path = "../src/map_view.rs"]
mod map_view;
#[path = "../src/page_appearance.rs"]
mod page_appearance;
#[path = "../src/page_blocks.rs"]
mod page_blocks;
#[path = "../src/page_covers.rs"]
mod page_covers;
#[path = "../src/page_creation.rs"]
mod page_creation;
#[path = "../src/page_open.rs"]
mod page_open;
#[path = "../src/page_personalization.rs"]
mod page_personalization;
#[path = "../src/place_property.rs"]
mod place_property;
#[path = "../src/property_controls.rs"]
mod property_controls;
#[path = "../src/record_open.rs"]
mod record_open;
#[path = "../src/records.rs"]
mod records;
#[path = "../src/relation_titles.rs"]
mod relation_titles;
#[path = "../src/rich_text.rs"]
mod rich_text;
#[path = "../src/sidebar_favorites.rs"]
mod sidebar_favorites;
#[path = "../src/sidebar_page_menu.rs"]
mod sidebar_page_menu;
#[path = "../src/sidebar_reveal.rs"]
mod sidebar_reveal;
#[path = "../src/sidebar_section_menu.rs"]
mod sidebar_section_menu;
#[path = "../src/sidebar_sections.rs"]
mod sidebar_sections;
#[path = "../src/sidebar_tree.rs"]
mod sidebar_tree;
#[path = "../src/simple_table.rs"]
mod simple_table;
#[path = "../src/state.rs"]
mod state;
#[path = "../src/table_summaries.rs"]
mod table_summaries;
#[path = "../src/table_window.rs"]
mod table_window;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/view_management.rs"]
mod view_management;
use rune::*;
use state::State;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

static PHASE: AtomicUsize = AtomicUsize::new(0);
static READY: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
fn window_state() -> (bool, bool, bool, bool) {
    use std::ffi::{c_char, c_void};
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    unsafe fn object(receiver: *mut c_void, selector: &'static [u8]) -> *mut c_void {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { send(receiver, sel_registerName(selector.as_ptr().cast())) }
    }
    unsafe fn boolean(receiver: *mut c_void, selector: &'static [u8]) -> bool {
        if receiver.is_null() {
            return false;
        }
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { send(receiver, sel_registerName(selector.as_ptr().cast())) }
    }
    unsafe {
        let app = object(
            objc_getClass(c"NSApplication".as_ptr()),
            b"sharedApplication\0",
        );
        let window = object(app, b"keyWindow\0");
        let screen = if window.is_null() {
            std::ptr::null_mut()
        } else {
            object(window, b"screen\0")
        };
        let main = object(objc_getClass(c"NSScreen".as_ptr()), b"mainScreen\0");
        (
            boolean(app, b"isActive\0"),
            boolean(window, b"isKeyWindow\0"),
            boolean(window, b"isVisible\0"),
            !screen.is_null() && screen == main,
        )
    }
}

#[cfg(not(target_os = "macos"))]
fn window_state() -> (bool, bool, bool, bool) {
    (false, false, false, false)
}

fn init() -> State {
    let path = std::path::PathBuf::from(
        std::env::var_os("MOSS_TABLE_BENCH_DIR").expect("scratch fixture required"),
    );
    assert!(path.starts_with("/tmp") || path.starts_with("/private/tmp"));
    let mut state = State::open(path, Some("bench-workspace")).expect("open scratch fixture");
    let mut db = state.store.get_database("bench-database").unwrap().unwrap();
    let wrapped = std::env::var_os("MOSS_TABLE_BENCH_WRAP").is_some();
    db.views[0].config = serde_json::json!({"load_limit":999,"wrap_cells":wrapped,"freeze_through":if wrapped { serde_json::json!("status") } else { serde_json::Value::Null },"show_page_icon":true});
    db.views[0].query.filter = None;
    db.views[0].query.sorts.clear();
    state.store.save_database(&db).unwrap();
    state.open_page("page-000000000", false);
    state.open_page("bench-database-page", false);
    assert_eq!(
        state.tab().unwrap().records.len(),
        999,
        "native fixture must load 999 records"
    );
    state
}
fn poll(state: &mut State, time: f64, tx: &mut Tx) -> bool {
    let changed = records::poll(state, time, tx);
    // Drive the app's own scroll API; this does not control another application.
    if time > 0.5 {
        static LOGICAL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let logical =
            *LOGICAL.get_or_init(|| std::env::var_os("MOSS_TABLE_BENCH_LOGICAL").is_some());
        let distance = if logical {
            let motion = if READY.load(Ordering::Relaxed) {
                ((time * 2.).sin() * 40. + 40.) as f32
            } else {
                0.
            };
            match PHASE.load(Ordering::Relaxed) {
                0 => motion,
                1 => 19_000_000. + motion,
                2 => state
                    .tab()
                    .and_then(|tab| tab.table_window.as_ref())
                    .map_or(100_000_000., |window| {
                        (window.scroll.get().max[1] - 80. + motion).max(0.)
                    }),
                _ if std::env::var_os("MOSS_PERF_KEEP_SCROLLING").is_some() => state
                    .tab()
                    .and_then(|tab| tab.table_window.as_ref())
                    .map_or(0., |window| {
                        (window.scroll.get().max[1] - 80. + ((time * 2.).sin() * 40. + 40.) as f32)
                            .max(0.)
                    }),
                _ => {
                    tx.quit();
                    return changed;
                }
            }
        } else {
            ((time - 0.5) * 1800.) as f32
        };
        tx.scroll_to_instant("db-body", (distance / 4.) % 400., distance);
    }
    tx.wake_after(Duration::from_secs_f64(1. / 120.));
    changed
}
fn main() {
    let spec = AppSpec {
        poll: Some(poll),
        fonts: fonts::all(),
        families: fonts::FAMILIES,
        ..AppSpec::new("Moss Table Performance", (1440., 900.), init, ui::view)
    };
    let mut drawn = 0usize;
    let mut updates = Vec::with_capacity(600);
    let mut encodes = Vec::with_capacity(600);
    let mut gaps = Vec::with_capacity(600);
    let mut previous = None;
    let mut skipped = 0;
    let mut phase_frames = 0usize;
    App::new(spec)
        .on_ready(|window| window.focus())
        .on_frame_encoded(move |frame| {
            if frame.outcome() == FrameOutcome::Skipped { skipped += 1; }
            if frame.outcome() == FrameOutcome::Drawn {
                let now = std::time::Instant::now();
                if let Some(last) = previous { gaps.push(now.duration_since(last).as_secs_f64() * 1000.); }
                previous = Some(now);
                if let Some(ms) = frame.update_ms() { updates.push(ms); }
                if let Some(ms) = frame.encode_ms() { encodes.push(ms); }
            }
            if frame.seq() == 599 {
                fn report(name: &str, values: &mut [f64]) {
                    values.sort_by(f64::total_cmp);
                    if values.is_empty() { return; }
                    let p = |n: f64| values[((values.len() - 1) as f64 * n).ceil() as usize];
                    eprintln!("{name} n={} p50={:.3} p95={:.3} p99={:.3} max={:.3} over8.33={}", values.len(), p(0.5), p(0.95), p(0.99), values[values.len()-1], values.iter().filter(|x| **x > 8.33).count());
                }
                eprintln!("frame hooks skipped={skipped} reported_hz={:?}; submit gaps are not presentation timing", frame.refresh_hz());
                report("update wall ms", &mut updates);
                report("encode CPU wall ms", &mut encodes);
                report("submit gap wall ms", &mut gaps);
            }
        })
        .on_frame_drawn(move |runtime| {
            drawn += 1;
            if drawn == 1 || drawn == 120 || drawn == 600 {
                let (active, key, visible, main_screen) = window_state();
                eprintln!("task window frame={drawn} app_active={active} key={key} visible={visible} main_screen={main_screen}");
            }
            if std::env::var_os("MOSS_TABLE_BENCH_LOGICAL").is_some() {
                let phase = PHASE.load(Ordering::Relaxed);
                let ordinal = [0usize, 500_000, 999_999].get(phase).copied();
                if let (Some(ordinal), Some(tab)) = (ordinal, runtime.state.tab()) {
                    if let Some(window) = &tab.table_window {
                        let local = ordinal.checked_sub(window.start as usize).filter(|i| *i < window.window_len);
                        if !READY.load(Ordering::Relaxed) && window.total == Some(1_000_000) && window.visible.contains(&ordinal) && !window.loading {
                            if let Some(local) = local.filter(|i| runtime.find(&format!("cell-{i}-title")).is_some()) {
                                let expected = format!("record-{ordinal:09}");
                                assert_eq!(tab.records[local].id, expected);
                                eprintln!("logical checkpoint phase={phase} ordinal={ordinal} id={expected} cached={}..{} visible={:?}", window.start, window.start + window.window_len as u64, window.visible);
                                READY.store(true, Ordering::Relaxed);
                            }
                        }
                        if READY.load(Ordering::Relaxed) {
                            phase_frames += 1;
                            if phase_frames >= 80 { phase_frames = 0; READY.store(false, Ordering::Relaxed); PHASE.store(phase + 1, Ordering::Relaxed); }
                        }
                    }
                }
            }
            if drawn == 1 || drawn == 180 || drawn == 400 || drawn == 600 {
                let visible: Vec<_> = [0, 100, 200, 300]
                    .into_iter()
                    .filter(|index| runtime.find(&format!("cell-{index}-title")).is_some())
                    .collect();
                eprintln!("sample frame {drawn}: mounted probe rows {visible:?}");
                if let Some(tab) = runtime.state.tab() {
                    if let Some(window) = &tab.table_window {
                        eprintln!("logical frame {drawn}: total={:?} start={} cached={} visible={:?} loading={} error={:?} first={:?} last={:?}", window.total, window.start, window.window_len, window.visible, window.loading, window.error, tab.records.first().map(|r| &r.id), tab.records.get(window.window_len.saturating_sub(1)).map(|r| &r.id));
                    }
                }
            }
        })
        .titlebar(Titlebar::new(44.))
        .font_smoothing(FontSmoothing::Off)
        .run();
}
