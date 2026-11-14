use super::*;

fn timeline_bounds(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .0
}

#[test]
fn timeline_fixed_column_blocks_hidden_grid_input_and_menus_stay_above_header() {
    let (mut rt, path) = database_runtime(3);
    rt.set_viewport((850., 700.), 1.);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.invalidate();
    rt.frame(0.);
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 600., 0.));
    rt.frame(0.);
    let fixed = timeline_bounds(&rt, "timeline-create-name");
    let count = rt.state.tab().unwrap().records.len();
    rt.pointer_down(fixed[0] + 20., fixed[1] + 18.);
    rt.pointer_move(fixed[0] + 100., fixed[1] + 18.);
    rt.pointer_up(fixed[0] + 100., fixed[1] + 18.);
    rt.frame(0.);
    assert_eq!(
        rt.state.tab().unwrap().records.len(),
        count,
        "hidden date grid must not create a record under the name column"
    );
    assert!(rt.state.timeline_drag.is_none());
    click(&mut rt, "db-views");
    rt.frame(1.);
    click(&mut rt, "view-search");
    press(&mut rt, Key::Text("Board".into()));
    assert_eq!(rt.state.view_search.text, "Board");
    click(&mut rt, "select-view-1");
    assert_eq!(rt.state.tab().unwrap().view, 1);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn timeline_keeps_names_fixed_and_dates_aligned_during_both_scroll_axes() {
    for (width, height, scale, dark) in [(850., 600., 1., false), (1100., 800., 2., true)] {
        let (mut s, path) = fixture();
        s.new_page(true, None);
        s.tabs[s.active].view = 3;
        let first = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        s.tabs[s.active].timeline_start = first;
        let db = s.tab().unwrap().database.as_ref().unwrap().clone();
        let date = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Date)
            .unwrap();
        for i in 0..36 {
            s.store
                .create_record(
                    &db.id,
                    &format!("Task {i:02}"),
                    &[(
                        date.id.clone(),
                        json!({"start":"2026-11-13", "end":"2026-11-28"}),
                    )]
                    .into(),
                )
                .unwrap();
        }
        assert!(s.query_records());
        let mut rt = Runtime::with_text(
            s,
            ui::view,
            (width, height),
            Env {
                system_dark: dark,
                scale,
            },
            rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
        );
        rt.on_key = Some(key);
        rt.frame(0.);
        let name = timeline_bounds(&rt, "timeline-name-0");
        let head = timeline_bounds(&rt, "timeline-name-header");
        let date = timeline_bounds(&rt, "timeline-date-2026-11-13");
        let bar = timeline_bounds(&rt, "timeline-bar-0");
        let controls = timeline_bounds(&rt, "timeline-controls");
        let footer = timeline_bounds(&rt, "timeline-new");
        rt.update(|_, tx| tx.scroll_to_instant("db-body", 240., 0.));
        rt.frame(0.);
        for (key, was) in [
            ("timeline-name-0", name),
            ("timeline-name-header", head),
            ("timeline-controls", controls),
            ("timeline-new", footer),
        ] {
            let now = timeline_bounds(&rt, key);
            assert!(
                (now[0] - was[0]).abs() < 0.1,
                "{key} moved: {was:?} to {now:?}"
            );
        }
        let date_after = timeline_bounds(&rt, "timeline-date-2026-11-13");
        let bar_after = timeline_bounds(&rt, "timeline-bar-0");
        assert!((date[0] - date_after[0] - 240.).abs() < 0.1);
        assert!((bar[0] - bar_after[0] - 240.).abs() < 0.1);
        assert!((bar_after[0] - date_after[0]).abs() < 0.1);
        rt.wheel_xy(name[0] + 20., name[1] + 18., -80., 0.);
        for _ in 0..60 {
            rt.frame(1. / 120.);
        }
        let date_wheel = timeline_bounds(&rt, "timeline-date-2026-11-13");
        assert!((date_after[0] - date_wheel[0] - 80.).abs() < 0.1);
        assert!((timeline_bounds(&rt, "timeline-name-0")[0] - name[0]).abs() < 0.1);
        rt.update(|_, tx| tx.scroll_to_instant("db-body", 600., 180.));
        rt.frame(0.);
        let name_after = timeline_bounds(&rt, "timeline-name-8");
        let track = timeline_bounds(&rt, "timeline-track-8");
        assert!((name_after[0] - name[0]).abs() < 0.1);
        assert!(
            (name_after[1] - track[1]).abs() < 0.1,
            "names and bars must share vertical position"
        );
        let axis = timeline_bounds(&rt, "timeline-axis");
        let body = timeline_bounds(&rt, "db-body");
        assert!(
            (axis[1] - body[1]).abs() < 0.1,
            "date ruler must stay at the top"
        );
        assert!((timeline_bounds(&rt, "timeline-name-header")[0] - head[0]).abs() < 0.1);
        rt.pointer_down(head[0] + 60., axis[1] + 28.);
        rt.pointer_up(head[0] + 60., axis[1] + 28.);
        rt.frame(0.);
        assert!(
            rt.state.record_peek.is_none(),
            "rows must not receive input through the fixed heading"
        );
        click(&mut rt, "timeline-open-8");
        assert_eq!(
            rt.state.record_peek.as_ref().unwrap().record.id,
            rt.state.tab().unwrap().records[8].id
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn timeline_dates_and_clipped_ranges_share_one_axis() {
    let (mut rt, path) = database_runtime(3);
    let first = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.tabs[rt.state.active].timeline_start = first;
    let date = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    for (start, end) in [("2026-10-29", "2026-11-03"), ("2026-11-29", "2026-12-10")] {
        let values = [(date.clone(), json!({"start": start, "end": end}))].into();
        rt.state.create_inline(values, false);
        rt.state.cell.replace_text(start);
        assert!(rt.state.finish_cell(false));
    }
    rt.invalidate();
    rt.frame(0.);
    let bounds = |key: &str| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
            .target
    };
    let day1 = bounds("timeline-date-2026-11-01");
    let day3 = bounds("timeline-date-2026-11-03");
    let day29 = bounds("timeline-date-2026-11-29");
    let track = bounds("timeline-track-0");
    let bar0 = bounds("timeline-bar-0");
    let bar1 = bounds("timeline-bar-1");
    assert!((track[0] - day1[0]).abs() < 0.1);
    assert!((bar0[0] - day1[0]).abs() < 0.1);
    assert!((bar0[0] + bar0[2] - day3[0] - day3[2]).abs() < 0.1);
    assert!((bar1[0] - day29[0]).abs() < 0.1);
    assert!((bar1[0] + bar1[2] - track[0] - track[2]).abs() < 0.1);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
