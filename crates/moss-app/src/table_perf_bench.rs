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
                assert_eq!(rt.state.tab().unwrap().records.len(), limit as usize);
            }
            opens.push(milliseconds(before));
            let rect = rt.presented("db-body").unwrap().0;
            let (x, y) = (
                (rect[0] + rect[0].max((rect[0] + rect[2]).min(1440.))) * 0.5,
                (rect[1] + rect[1].max((rect[1] + rect[3]).min(900.))) * 0.5,
            );
            for step in 0..4 {
                let before = Instant::now();
                rt.wheel_xy(x, y, if step % 2 == 0 { -22. } else { 17. }, -28.);
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
                    rt.wheel_xy(x, y, -12., -120.);
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
