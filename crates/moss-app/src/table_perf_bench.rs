use super::*;
use serde_json::Value;
use std::time::Instant;

const DATABASE_PAGE: &str = "bench-database-page";
const DOCUMENT_PAGE: &str = "page-000000000";

fn report(name: &str, mut samples: Vec<f64>, budget: f64) {
    samples.sort_by(f64::total_cmp);
    let percentile = |p: f64| samples[((samples.len() - 1) as f64 * p).ceil() as usize];
    let over = samples.iter().filter(|v| **v > budget).count();
    println!(
        "{name}: samples={} p50={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms over_{budget:.2}ms={over}",
        samples.len(),
        percentile(0.50),
        percentile(0.95),
        percentile(0.99),
        samples.last().unwrap()
    );
}
fn milliseconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.
}
fn render(rt: &mut Runtime<State>, id: &str) {
    rt.update(|s, tx| {
        s.open_page(id, false);
        tx.instant();
    });
    rt.frame(0.);
}

fn save_snapshot(
    rt: &mut Runtime<State>,
    gpu: &rune::render::Gpu,
    renderer: &mut rune::render::Renderer,
    path: &std::path::Path,
) {
    use rune::render::{Offscreen, write_png};
    let target = Offscreen::new(&gpu.device, 2880, 1800);
    renderer.upload_images(&gpu.device, &gpu.queue, &mut rt.images);
    renderer.prepare(
        &gpu.device,
        &gpu.queue,
        &mut rt.text.atlas,
        &rt.instances,
        (2880, 1800),
    );
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer.draw(&mut encoder, &target.view, rt.clear_color());
    target.copy(&mut encoder);
    gpu.queue.submit(Some(encoder.finish()));
    write_png(path, 2880, 1800, &target.read(&gpu.device)).unwrap();
}

#[test]
#[ignore = "requires a disposable million-row fixture"]
fn million_row_table_headless_benchmark() {
    let path = std::path::PathBuf::from(
        std::env::var_os("MOSS_TABLE_BENCH_DIR")
            .expect("set MOSS_TABLE_BENCH_DIR to the disposable fixture"),
    );
    assert!(
        path.to_string_lossy().starts_with("/tmp/")
            || path.to_string_lossy().starts_with("/private/tmp/")
    );
    let samples: usize = std::env::var("MOSS_TABLE_BENCH_SAMPLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    assert!(samples >= 2);
    let start = Instant::now();
    let state = State::open(path, Some("bench-workspace")).unwrap();
    let text = rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES);
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 2.,
        },
        text,
    );
    rt.poll = Some(records::poll);
    rt.on_key = Some(key);
    rt.frame(0.);
    let startup = milliseconds(start);
    println!(
        "HEADLESS wall time only; OS cache uncontrolled; no native presentation, GPU fence, or vsync timing. State + Runtime first document frame: {startup:.3}ms"
    );
    for (label, limit, wrap, freeze, filtered) in [
        ("plain-100", 100, false, false, false),
        ("plain-999", 999, false, false, false),
        ("wrap-freeze-100", 100, true, true, false),
        ("wrap-freeze-999", 999, true, true, false),
        ("filter-sort-100", 100, false, false, true),
    ] {
        let mut db = rt
            .state
            .store
            .get_database("bench-database")
            .unwrap()
            .unwrap();
        db.views[0].config = json!({"load_limit":limit, "wrap_cells":wrap, "freeze_through": if freeze { json!("status") } else { Value::Null }, "show_page_icon":true});
        db.views[0].query.filter = filtered.then(|| {
            FilterExpr::Rule(FilterRule {
                property_id: "status".into(),
                op: FilterOp::Eq,
                value: json!("Open"),
            })
        });
        db.views[0].query.sorts = if filtered {
            vec![SortRule {
                property_id: "title".into(),
                descending: false,
            }]
        } else {
            vec![]
        };
        rt.state.store.save_database(&db).unwrap();
        let mut opens = Vec::with_capacity(samples);
        let mut scrolls = Vec::with_capacity(samples * 4);
        let mut deep = Vec::with_capacity(samples);
        for sample in 0..samples {
            rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 0.));
            rt.frame(0.);
            render(&mut rt, DOCUMENT_PAGE);
            let before = Instant::now();
            render(&mut rt, DATABASE_PAGE);
            assert!(rt.find("database-title").is_some());
            assert!(rt.find("cell-0-title").is_some());
            assert!(!rt.state.tab().unwrap().records.is_empty());
            if !filtered {
                assert_eq!(rt.state.tab().unwrap().records.len(), if wrap { limit as usize } else { (limit as usize).min(crate::table_window::INITIAL_WINDOW) });
            }
            opens.push(milliseconds(before));
            let rect = rt.presented("db-body").unwrap().0;
            let (x, y) = (
                (rect[0] + rect[0].max((rect[0] + rect[2]).min(1440.))) * 0.5,
                (rect[1] + rect[1].max((rect[1] + rect[3]).min(900.))) * 0.5,
            );
            for step in 0..4 {
                let before = Instant::now();
                rt.wheel_xy(x, y, if step % 2 == 0 { -44. } else { 34. }, -56.);
                rt.frame(1. / 120.);
                scrolls.push(milliseconds(before));
            }
            if sample == 0 && limit == 999 {
                let mut travel = Vec::new();
                for _ in 0..2000 {
                    if rt.scroll_at_bottom("db-body") == Some(true) {
                        break;
                    }
                    let before = Instant::now();
                    rt.wheel_xy(x, y, -24., -240.);
                    rt.frame(1. / 120.);
                    travel.push(milliseconds(before));
                }
                println!(
                    "{label} long wheel traversal reached bottom: {}",
                    rt.scroll_at_bottom("db-body") == Some(true)
                );
                assert!(
                    rt.find("cell-0-title").is_none(),
                    "wheel traversal did not leave the first row"
                );
                report(
                    &format!("{label} long wheel traversal headless frame"),
                    travel,
                    8.33,
                );
            }
            if label == "plain-100" {
                let before = Instant::now();
                rt.update(|s, tx| {
                    s.move_record_page(500_000);
                    tx.instant();
                });
                rt.frame(0.);
                assert!(!rt.state.tab().unwrap().records.is_empty());
                deep.push(milliseconds(before));
            }
        }
        println!(
            "{label} first open (OS cache uncontrolled): {:.3}ms",
            opens[0]
        );
        if !deep.is_empty() {
            report("deep offset 500000 query+headless frame", deep, 20.);
        }
        report(
            &format!("{label} warm open+first frame, OS cache uncontrolled"),
            opens[1..].to_vec(),
            20.,
        );
        report(
            &format!("{label} mixed wheel+headless frame, OS cache uncontrolled"),
            scrolls,
            8.33,
        );
    }
}

#[test]
#[ignore = "requires a disposable rank-indexed million-row fixture"]
fn million_row_logical_table_windows() {
    let path = std::path::PathBuf::from(
        std::env::var_os("MOSS_TABLE_BENCH_DIR").expect("scratch fixture required"),
    );
    assert!(path.starts_with("/tmp") || path.starts_with("/private/tmp"));
    let state = State::open(path, Some("bench-workspace")).unwrap();
    let text = rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES);
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 2.,
        },
        text,
    );
    rt.poll = Some(records::poll);
    rt.frame(0.);
    render(&mut rt, DOCUMENT_PAGE);
    let open_started = Instant::now();
    render(&mut rt, DATABASE_PAGE);
    assert!(rt.find("cell-0-title").is_some());
    println!(
        "logical input-ready open and first headless frame: {:.3}ms (one sample, OS cache uncontrolled)",
        milliseconds(open_started)
    );
    let gpu = rune::render::Gpu::new(rune::render::Gpu::instance(), None);
    let mut renderer = rune::render::Renderer::new(&gpu.device, rune::render::OFFSCREEN_FORMAT);
    for (name, ordinal) in [("top", 0usize), ("middle", 500_000), ("end", 999_999)] {
        let started = Instant::now();
        rt.update(|_, tx| {
            tx.scroll_to_instant(
                "db-body",
                0.,
                if ordinal == 999_999 {
                    100_000_000.
                } else {
                    ordinal as f32 * 38.
                },
            )
        });
        let mut ready = false;
        for _ in 0..5000 {
            rt.frame(1. / 120.);
            if let Some(window) = rt.state.tab().unwrap().table_window.as_ref() {
                if window.total == Some(1_000_000)
                    && window.visible.contains(&ordinal)
                    && ordinal >= window.start as usize
                    && ordinal < window.start as usize + window.window_len
                    && !window.loading
                {
                    let expected = rt
                        .state
                        .store
                        .record_window(
                            "bench-database",
                            window.start,
                            window.window_len as u32,
                            None,
                        )
                        .unwrap();
                    let moss_core::RecordWindowState::Ready(expected) = expected else {
                        panic!("rank unavailable")
                    };
                    let actual = &rt.state.tab().unwrap().records[..window.window_len];
                    assert_eq!(
                        actual.iter().map(|r| &r.id).collect::<Vec<_>>(),
                        expected.records.iter().map(|r| &r.id).collect::<Vec<_>>()
                    );
                    let local = ordinal - window.start as usize;
                    let record_id = actual[local].id.clone();
                    let original_title = actual[local].title.clone();
                    assert!(rt.find(&format!("cell-{local}-title")).is_some());
                    if name == "end" {
                        let nodes = rt.elements();
                        for key in [format!("cell-{local}-title"), "table-new".to_owned()] {
                            let node = nodes
                                .iter()
                                .find(|node| node.key.as_deref() == Some(key.as_str()))
                                .expect("end control mounted");
                            assert!(
                                node.visible[2] > 1. && node.visible[3] >= node.rect[3] - 4.,
                                "end control clipped: {key}: {:?} {:?}",
                                node.rect,
                                node.visible
                            );
                        }
                    }
                    println!(
                        "logical {name}: ready_ms={:.3} ordinal={ordinal} cached={}..{} id={} ",
                        milliseconds(started),
                        window.start,
                        window.start + window.window_len as u64,
                        actual[local].id
                    );
                    if name != "top" {
                        save_snapshot(
                            &mut rt,
                            &gpu,
                            &mut renderer,
                            &std::path::PathBuf::from(format!(
                                "/private/tmp/moss-table-perf-20261108/logical-{name}.png"
                            )),
                        );
                    }
                    if name == "end" {
                        click(&mut rt, &format!("cell-{local}-title"));
                        assert_eq!(
                            rt.state.editing_cell.as_ref().map(|cell| &cell.record_id),
                            Some(&record_id)
                        );
                        rt.state.cell.replace_text("Scratch identity check");
                        assert!(rt.state.finish_cell(false));
                        assert_eq!(
                            rt.state
                                .store
                                .get_record(&record_id)
                                .unwrap()
                                .unwrap()
                                .title,
                            "Scratch identity check"
                        );
                        rt.state
                            .store
                            .rename_record(&record_id, &original_title)
                            .unwrap();
                        assert_eq!(
                            rt.state
                                .store
                                .get_record(&record_id)
                                .unwrap()
                                .unwrap()
                                .title,
                            original_title
                        );
                        println!("logical end: UI edit saved and restored record {record_id}");
                    }
                    ready = true;
                    break;
                }
                assert!(window.error.is_none(), "{:?}", window.error);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(ready, "logical {name} did not become ready");
    }
}

#[test]
#[ignore = "requires a disposable 1,000-row fixture"]
fn thousand_row_table_interaction_benchmark() {
    let path = std::path::PathBuf::from(std::env::var_os("MOSS_TABLE_BENCH_DIR").expect("scratch fixture required"));
    assert!(path.starts_with("/tmp") || path.starts_with("/private/tmp"));
    let state = State::open(path, Some("bench-workspace")).unwrap();
    let text = rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES);
    let mut rt = Runtime::with_text(state, ui::view, (1440., 900.), Env { system_dark: false, scale: 2. }, text);
    rt.poll = Some(records::poll);
    rt.on_key = Some(key);
    rt.frame(0.);
    println!("HEADLESS CPU wall time only; 1000 stored records, 8 columns, 1440x900pt at 2x; OS cache uncontrolled; no GPU or native presentation.");
    for (name, wrap, freeze, group, filter) in [("plain", false, false, false, false), ("wrap-frozen", true, true, false, false), ("grouped", false, false, true, false), ("filter-sort", false, false, false, true)] {
        render(&mut rt, DOCUMENT_PAGE);
        let mut db = rt.state.store.get_database("bench-database").unwrap().unwrap();
        db.views[0].config = json!({"load_limit":999,"wrap_cells":wrap,"freeze_through":if freeze { json!("status") } else { Value::Null },"show_page_icon":true});
        db.views[0].query.group_by = group.then(|| "status".to_owned());
        db.views[0].query.filter = filter.then(|| FilterExpr::Rule(FilterRule { property_id:"status".into(), op:FilterOp::Eq, value:json!("Open") }));
        db.views[0].query.sorts = if filter { vec![SortRule { property_id:"title".into(), descending:false }] } else { vec![] };
        rt.state.store.save_database(&db).unwrap();
        let before = Instant::now();
        render(&mut rt, DATABASE_PAGE);
        println!("{name} open+headless frame: {:.3}ms loaded={}", milliseconds(before), rt.state.tab().unwrap().records.len());
        let body = rt.presented("db-body").unwrap().0;
        let (x,y) = (body[0]+200., body[1]+100.);
        let mut scroll = Vec::new();
        for _ in 0..180 {
            let start=Instant::now(); rt.wheel_xy(x,y,-4.,-36.); rt.frame(1./120.); scroll.push(milliseconds(start));
        }
        report(&format!("{name} scroll"), scroll, 8.33);
        if name == "plain" {
            let start = Instant::now();
            let mut reached = false;
            for step in 0..2000 {
                rt.wheel_xy(x, y, 0., -36.);
                rt.frame(1. / 120.);
                if step % 10 == 0 { std::thread::sleep(std::time::Duration::from_millis(1)); }
                if rt.state.tab().and_then(|tab| tab.table_window.as_ref().map(|w| (tab, w)))
                    .is_some_and(|(tab, w)| w.total == Some(1000) && w.visible.end == 1000 && !w.loading && tab.records.iter().take(w.window_len).any(|r| r.id == "record-000000999")) {
                    reached = true;
                    break;
                }
            }
            println!("plain double-speed traversal to row 1000: reached={reached}, elapsed={:.3}ms", milliseconds(start));
            assert!(reached);
        }
        rt.update(|_,tx| tx.scroll_to_instant("db-body",0.,0.)); rt.frame(0.);
        if name == "plain" {
            for _ in 0..200 {
                rt.frame(1. / 120.);
                if rt.state.tab().and_then(|tab| tab.table_window.as_ref()).is_some_and(|w| w.start == 0 && !w.loading) { break; }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        let handle = rt.presented("table-resize-title").unwrap().0;
        let (x,y)=(handle[0]+handle[2]/2.,handle[1]+handle[3]/2.);
        rt.pointer_down(x,y); rt.frame(0.);
        let mut drag=Vec::new();
        for n in 1..=60 { let start=Instant::now(); rt.pointer_move(x+n as f32,y); rt.frame(1./120.); drag.push(milliseconds(start)); }
        let start=Instant::now(); rt.pointer_up(x+60.,y); rt.frame(1./120.); let release=milliseconds(start);
        report(&format!("{name} resize drag"),drag,8.33); println!("{name} resize release {release:.3}ms");
        let title = rt.presented("table-header-title").unwrap().0;
        let status = rt.presented("table-header-status").unwrap().0;
        let (x,y)=(status[0]+30.,status[1]+status[3]/2.);
        rt.pointer_down(x,y); rt.frame(0.);
        let mut reorder=Vec::new();
        for n in 1..=60 { let start=Instant::now(); rt.pointer_move(x+(title[0]-x+10.)*n as f32/60.,y); rt.frame(1./120.); reorder.push(milliseconds(start)); }
        let start=Instant::now(); rt.pointer_up(title[0]+10.,y); rt.frame(1./120.); let release=milliseconds(start);
        report(&format!("{name} reorder drag"),reorder,8.33); println!("{name} reorder release {release:.3}ms");
        rt.update(|_,tx|tx.scroll_to_instant("db-body",0.,0.)); rt.frame(0.);
        if rt.find("cell-0-title").is_some() {
            click(&mut rt,"cell-0-title");
            let mut typing=Vec::new();
            for _ in 0..30 { let start=Instant::now(); rt.key(KeyEvent { key:Key::Text("x".into()), mods:Mods::default() }); rt.frame(1./120.); typing.push(milliseconds(start)); }
            report(&format!("{name} type and autosave"),typing,8.33);
            press(&mut rt, Key::Escape);
        }
    }
}

#[test]
#[ignore = "requires a disposable 1,000-row fixture"]
fn table_scroll_has_no_loading_frames() {
    let path = std::path::PathBuf::from(std::env::var_os("MOSS_TABLE_BENCH_DIR").expect("scratch fixture required"));
    let total: usize = std::env::var("MOSS_TABLE_BENCH_ROWS").unwrap_or_else(|_| "1000".into()).parse().unwrap();
    let speed: f32 = std::env::var("MOSS_TABLE_BENCH_SPEED").unwrap_or_else(|_| "160000".into()).parse().unwrap();
    assert!(path.starts_with("/tmp") || path.starts_with("/private/tmp"));
    let state = State::open(path, Some("bench-workspace")).unwrap();
    let text = rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES);
    let mut rt = Runtime::with_text(state, ui::view, (1440., 900.), Env { system_dark: false, scale: 2. }, text);
    rt.poll = Some(records::poll);
    rt.frame(0.);
    render(&mut rt, DATABASE_PAGE);
    let body = rt.presented("db-body").unwrap().0;
    let (x, y) = (body[0]+100., body[1]+100.);
    for _ in 0..10 {
        rt.wheel_xy(x, y, 0., -speed / 120.);
        rt.frame(1. / 120.);
        let nodes = rt.elements();
        assert!(!nodes.iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("table-pending-") || k == "table-window-loading") && e.visible[3] > 0.), "first scroll showed a loading row");
        assert!(nodes.iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("cell-") && k.ends_with("-title")) && e.visible[3] > 0.), "first scroll showed no real rows");
        let window = rt.state.tab().unwrap().table_window.as_ref().unwrap();
        let at = window.scroll.get();
        let first = ((at.offset[1] - 38.).max(0.) / 38.).floor() as usize;
        let last = (((at.offset[1] + at.viewport[1] - 38.).max(0.) / 38.).ceil() as usize)
            .min(window.total.unwrap_or(window.window_len as u64) as usize);
        for ordinal in first..last {
            let local = ordinal.checked_sub(window.start as usize).expect("first scroll outside cache");
            assert!(nodes.iter().any(|node| node.key.as_deref() == Some(format!("cell-{local}-title").as_str()) && node.visible[3] > 0.), "first scroll missed row {ordinal}");
        }
    }
    let wait_start = Instant::now();
    while rt.state.tab().unwrap().table_window.as_ref().unwrap().total != Some(total as u64) {
        assert!(wait_start.elapsed() < std::time::Duration::from_secs(5));
        rt.frame(1. / 120.);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 0.));
    rt.frame(0.);
    let mut frames = Vec::new();
    let mut pending = 0;
    let mut loading = 0;
    let mut empty = 0;
    let mut uncovered = 0;
    let mut distance = 0.;
    let mut previous_offset = rt.state.tab().unwrap().table_window.as_ref().unwrap().scroll.get().offset[1];
    let mut jumps = 0;
    let mut reversals = 0;
    let stress_started = Instant::now();
    let mut previous_input = stress_started - std::time::Duration::from_secs_f64(1. / 120.);
    let loops = std::env::var("MOSS_TABLE_BENCH_LOOPS").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    for direction in [-1., 1., -1., 1.].into_iter().cycle().take(4 * loops) {
        for _ in 0..26 {
            let start = Instant::now();
            let elapsed = start.duration_since(previous_input).max(std::time::Duration::from_secs_f64(1. / 120.));
            previous_input = start;
            let delta = speed * elapsed.as_secs_f32();
            rt.wheel_xy(x, y, 0., direction * delta);
            rt.frame(elapsed.as_secs_f64());
            frames.push(milliseconds(start));
            let nodes = rt.elements();
            if nodes.iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("table-pending-")) && e.visible[3] > 0.) { pending += 1; }
            if nodes.iter().any(|e| e.key.as_deref() == Some("table-window-loading") && e.visible[3] > 0.) { loading += 1; }
            if !nodes.iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("cell-") && k.ends_with("-title")) && e.visible[3] > 0.) { empty += 1; }
            let window = rt.state.tab().unwrap().table_window.as_ref().unwrap();
            let at = window.scroll.get();
            let movement = at.offset[1] - previous_offset;
            if (movement.abs() > delta + 4. || movement * direction > 4.) && std::env::var_os("MOSS_TABLE_BENCH_TRACE_SCROLL").is_some() {
                eprintln!("scroll delta={movement:.1} direction={direction} previous={previous_offset:.1} next={:.1} cache={}..{} visible={:?} loading={}", at.offset[1], window.start, window.start + window.window_len as u64, window.visible, window.loading);
            }
            if movement.abs() > delta + 4. { jumps += 1; }
            if movement * direction > 4. { reversals += 1; }
            distance += movement.abs();
            previous_offset = at.offset[1];
            let first = ((at.offset[1] - 38.).max(0.) / 38.).floor() as usize;
            let last = (((at.offset[1] + at.viewport[1] - 38.).max(0.) / 38.).ceil() as usize).min(total);
            if (first..last).any(|ordinal| {
                let top = 38. + ordinal as f32 * 38. - at.offset[1];
                if (top + 38.).min(at.viewport[1]) - top.max(0.) < 1. { return false; }
                let local = ordinal.saturating_sub(window.start as usize);
                ordinal < window.start as usize || !nodes.iter().any(|node| {
                    node.key.as_deref() == Some(format!("cell-{local}-title").as_str()) && node.visible[3] > 0.
                })
            }) {
                uncovered += 1;
                if std::env::var_os("MOSS_TABLE_BENCH_TRACE_SCROLL").is_some() {
                    eprintln!("uncovered offset={:.1} rows={first}..{last} cache={}..{}", at.offset[1], window.start, window.start + window.window_len as u64);
                    for ordinal in first..last {
                        let local = ordinal.saturating_sub(window.start as usize);
                        let cell = nodes.iter().find(|node| node.key.as_deref() == Some(format!("cell-{local}-title").as_str()));
                        if cell.is_none_or(|node| node.visible[3] <= 0.) {
                            eprintln!("  missing ordinal={ordinal} cell={:?}", cell.map(|node| (node.rect, node.visible)));
                        }
                    }
                }
            }
            for ordinal in first..last {
                if let Some(local) = ordinal.checked_sub(window.start as usize) {
                    if let Some(record) = rt.state.tab().unwrap().records.get(local) {
                        assert_eq!(record.id, format!("record-{ordinal:09}"));
                        if let Some(node) = nodes.iter().find(|node| node.key.as_deref() == Some(format!("cell-text-{local}-title").as_str()) && node.visible[3] > 0.) {
                            assert_eq!(node.text.as_deref(), Some(record.title.as_str()), "visible slot showed the wrong record");
                        }
                    }
                }
            }
            let remaining = std::time::Duration::from_secs_f64(1. / 120.).saturating_sub(start.elapsed());
            std::thread::sleep(remaining);
        }
    }
    let missed = frames.iter().filter(|ms| **ms > 1000. / 120.).count();
    report(&format!("{speed:.0}pt/s wheel+headless frame"), frames, 1000. / 120.);
    println!("{speed:.0}pt/s visible frames: pending={pending}, loading={loading}, no_real_rows={empty}, uncovered={uncovered}, jumps={jumps}, reversals={reversals}, distance={distance:.0}pt, observed_mean={:.0}pt/s", distance / stress_started.elapsed().as_secs_f32());
    assert!(distance > (speed * 0.3).min(100_000.), "test must actually scroll the table");
    assert_eq!((pending, loading, empty, uncovered), (0, 0, 0, 0), "scrolling must show real rows without loading frames");
    assert_eq!((jumps, reversals), (0, 0), "scrolling must not jump or move in the wrong direction");
    if std::env::var_os("MOSS_TABLE_BENCH_STRICT").is_some() {
        assert_eq!(missed, 0, "scroll frames must stay within the 120 Hz budget");
    }
}
