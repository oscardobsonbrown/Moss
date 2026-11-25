use crate::{key, poll, state::State, ui};
use moss_core::{Block, BlockKind, PageKind};
use rune::{Env, Runtime};
use std::time::Instant;

fn median(mut samples: Vec<f64>) -> (f64, f64, f64) {
    samples.sort_by(f64::total_cmp);
    let pct = |p: f64| samples[((samples.len() - 1) as f64 * p).ceil() as usize];
    (pct(0.50), pct(0.95), *samples.last().unwrap())
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.
}

fn make_blocks(page_id: &str, count: usize) -> Vec<Block> {
    let kinds = [
        BlockKind::Heading1,
        BlockKind::Paragraph,
        BlockKind::Paragraph,
        BlockKind::BulletedList,
        BlockKind::NumberedList,
        BlockKind::Todo,
        BlockKind::Quote,
        BlockKind::Heading2,
        BlockKind::Code,
        BlockKind::Callout,
        BlockKind::Paragraph,
        BlockKind::Divider,
    ];
    (0..count)
        .map(|i| {
            let mut block = Block::paragraph(
                page_id,
                if count == 1 {
                    "First paragraph on a clean coverless page.".to_owned()
                } else {
                    format!(
                        "Block {i}: offline personal workspace content for measuring coverless page rendering performance."
                    )
                },
                i as i64,
            );
            block.kind = if count == 1 {
                BlockKind::Paragraph
            } else {
                kinds[i % kinds.len()]
            };
            if block.kind == BlockKind::Todo {
                block.checked = i % 2 == 0;
            }
            block
        })
        .collect()
}

fn bench_case(label: &str, block_count: usize, sidebar_pages: usize) {
    let path = std::env::temp_dir().join(format!("moss-page-perf-{}", moss_core::new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    let workspace = state.workspace.id.clone();

    for i in 0..sidebar_pages {
        state
            .store
            .create_page(
                &workspace,
                None,
                &format!("Sidebar page {i:02}"),
                "",
                PageKind::Document,
            )
            .unwrap();
    }

    let page_a = state
        .store
        .create_page(&workspace, None, "Benchmark A", "", PageKind::Document)
        .unwrap();
    let page_b = state
        .store
        .create_page(&workspace, None, "Benchmark B", "", PageKind::Document)
        .unwrap();
    state
        .store
        .save_blocks(&page_a.id, &make_blocks(&page_a.id, block_count))
        .unwrap();
    state
        .store
        .save_blocks(&page_b.id, &make_blocks(&page_b.id, block_count))
        .unwrap();
    state.refresh_pages();
    state.open_page(&page_a.id, false);

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
    rt.on_key = Some(key);
    rt.poll = Some(poll);
    for _ in 0..4 {
        rt.frame(1. / 60.);
    }

    // 1. open_page (state + SQLite) & open_page + first frame
    let mut open_only = Vec::with_capacity(30);
    let mut open_and_frame = Vec::with_capacity(30);
    for i in 0..34 {
        let target = if i % 2 == 0 { &page_b.id } else { &page_a.id };
        let t0 = Instant::now();
        rt.update(|s, tx| {
            s.open_page(target, false);
            tx.instant();
        });
        let d_open = ms(t0);
        rt.frame(0.);
        let d_total = ms(t0);
        if i >= 4 {
            open_only.push(d_open);
            open_and_frame.push(d_total);
        }
    }

    // Focus block 0 for steady-state typing and caret blink benchmarks.
    rt.update(|s, tx| {
        s.focus_first_page_block(false);
        tx.instant();
    });
    rt.frame(1. / 60.);
    rt.frame(1. / 60.);

    // 2. Caret blink frame (500ms toggle)
    let mut blink = Vec::with_capacity(60);
    for i in 0..64 {
        let t0 = Instant::now();
        rt.state.caret.visible = !rt.state.caret.visible;
        rt.invalidate();
        rt.frame(1. / 60.);
        let d = ms(t0);
        if i >= 4 {
            blink.push(d);
        }
    }

    // 3. Typing 1 character in focused block 0 (UI rebuild + layout + paint)
    let mut typing = Vec::with_capacity(60);
    for i in 0..64 {
        let t0 = Instant::now();
        rt.update(|s, tx| {
            let edit = &mut s.tabs[s.active].blocks[0].edit;
            if i % 2 == 0 {
                edit.insert("x");
            } else {
                let len = edit.text.len();
                edit.set_selection(len.saturating_sub(1), len);
                edit.insert("");
            }
            tx.instant();
        });
        rt.frame(1. / 60.);
        let d = ms(t0);
        if i >= 4 {
            typing.push(d);
        }
    }

    // 4. Document scroll frames
    let scroll_key = format!("document-scroll-{}", rt.state.tab().unwrap().page.id);
    let mut doc_scroll = Vec::with_capacity(40);
    for i in 0..44 {
        let y = ((i % 20) as f32) * 32.;
        let t0 = Instant::now();
        rt.update(|_, tx| {
            tx.scroll_to_instant(&scroll_key, 0., y);
        });
        rt.frame(1. / 60.);
        let d = ms(t0);
        if i >= 4 {
            doc_scroll.push(d);
        }
    }

    // 5. Sidebar scroll frames (via programmatic update)
    let mut sidebar_scroll = Vec::with_capacity(40);
    for i in 0..44 {
        let y = ((i % 20) as f32) * 24.;
        let t0 = Instant::now();
        rt.update(|_, tx| {
            tx.scroll_to_instant("sidebar-scroll", 0., y);
        });
        rt.frame(1. / 60.);
        let d = ms(t0);
        if i >= 4 {
            sidebar_scroll.push(d);
        }
    }

    // 5b. Real wheel scroll (reading document unfocused & scrolling sidebar)
    rt.update(|s, tx| {
        for block in &mut s.tabs[s.active].blocks {
            block.edit.focused = false;
        }
        s.tabs[s.active].title.focused = false;
        tx.blur();
        tx.scroll_to_instant(&scroll_key, 0., 0.);
        tx.scroll_to_instant("sidebar-scroll", 0., 0.);
    });
    rt.frame(1. / 60.);
    rt.frame(1. / 60.);

    let commits_before_doc_wheel = rt.stats.commits;
    let mut doc_wheel = Vec::with_capacity(40);
    for i in 0..44 {
        let dy = if i % 20 < 10 { -32. } else { 32. };
        let t0 = Instant::now();
        rt.wheel(800., 450., dy);
        rt.frame(1. / 60.);
        let d = ms(t0);
        if i >= 4 {
            doc_wheel.push(d);
        }
    }
    let doc_wheel_commits = rt.stats.commits - commits_before_doc_wheel;

    let commits_before_sidebar_wheel = rt.stats.commits;
    let mut sidebar_wheel = Vec::with_capacity(40);
    for i in 0..44 {
        let dy = if i % 20 < 10 { -24. } else { 24. };
        let t0 = Instant::now();
        rt.wheel(120., 450., dy);
        rt.frame(1. / 60.);
        let d = ms(t0);
        if i >= 4 {
            sidebar_wheel.push(d);
        }
    }
    let sidebar_wheel_commits = rt.stats.commits - commits_before_sidebar_wheel;

    // Reset scroll for GPU capture
    rt.update(|_, tx| {
        tx.scroll_to_instant(&scroll_key, 0., 0.);
        tx.scroll_to_instant("sidebar-scroll", 0., 0.);
    });
    rt.frame(1. / 60.);

    // 6. Headless Metal GPU draw
    let instances = rt.instances.len();
    let gpu = rune::render::Gpu::headless();
    let mut renderer = rune::render::Renderer::new(&gpu.device, rune::render::OFFSCREEN_FORMAT);
    let target = rune::render::Offscreen::new(&gpu.device, 2880, 1800);
    let mut gpu_draw = Vec::with_capacity(30);
    for i in 0..34 {
        let t0 = Instant::now();
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
        let _ = target.read(&gpu.device);
        let d = ms(t0);
        if i >= 4 {
            gpu_draw.push(d);
        }
    }

    println!(
        "=== {label} (blocks={block_count}, sidebar_pages={sidebar_pages}, instances={instances}) ==="
    );
    for (name, samples) in [
        ("open_page (disk+state)", open_only),
        ("open_page + first frame", open_and_frame),
        ("caret_blink frame", blink),
        ("typing frame", typing),
        ("doc_scroll (update)", doc_scroll),
        ("sidebar_scroll (update)", sidebar_scroll),
        ("doc_wheel (real scroll)", doc_wheel),
        ("sidebar_wheel (real)", sidebar_wheel),
        ("gpu_draw (2880x1800)", gpu_draw),
    ] {
        let (p50, p95, max) = median(samples);
        println!("  {name:<26} p50={p50:7.3}ms  p95={p95:7.3}ms  max={max:7.3}ms");
    }
    println!("  wheel commits: doc={doc_wheel_commits}/44  sidebar={sidebar_wheel_commits}/44");

    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
#[ignore = "performance diagnostic"]
fn coverless_page_rendering_benchmark() {
    bench_case("1-block coverless page", 1, 50);
    bench_case("200-block coverless document", 200, 50);
}
