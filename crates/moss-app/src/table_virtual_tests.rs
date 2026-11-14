use super::*;

#[test]
fn virtual_table_keeps_scrolling_editing_and_footer_for_wrapped_frozen_rows() {
    for wrapped in [false, true] {
        let (mut rt, path) = database_runtime(0);
        let mut db = rt.state.tab().unwrap().database.clone().unwrap();
        let status = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Status)
            .unwrap()
            .id
            .clone();
        db.views[0].config = json!({"load_limit": 200, "wrap_cells": wrapped, "freeze_through": status, "calculations": if wrapped { json!({status.clone(): "count_all"}) } else { json!({}) }});
        if wrapped {
            db.views[0].query.group_by = Some(status.clone());
        }
        rt.state.store.save_database(&db).unwrap();
        rt.state.tabs[rt.state.active].database = Some(db.clone());
        for n in 0..180 {
            let title = if wrapped {
                format!("Row {n:03} {}", "long wrapped title ".repeat(9))
            } else {
                format!("Row {n:03}")
            };
            rt.state
                .store
                .create_record(
                    &db.id,
                    &title,
                    &std::collections::BTreeMap::from([(
                        status.clone(),
                        json!(if n % 2 == 0 { "Done" } else { "Not started" }),
                    )]),
                )
                .unwrap();
        }
        assert!(rt.state.query_records());
        rt.state.tabs[rt.state.active].table_window = None;
        rt.invalidate();
        rt.frame(0.);
        assert!(rt.find("table-virtual-rows").is_some());
        assert!(rt.find("cell-0-title").is_some());
        assert!(rt.find("cell-179-title").is_none());
        rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 100_000.));
        for _ in 0..20 {
            rt.frame(1. / 120.);
        }
        assert!(rt.find("cell-179-title").is_some());
        assert!(rt.find("table-new").is_some());
        if wrapped {
            assert!(rt.find("table-summaries").is_some());
        }
        let id = rt.state.tab().unwrap().records[179].id.clone();
        click(&mut rt, "cell-179-title");
        assert_eq!(rt.state.editing_cell.as_ref().unwrap().record_id, id);
        rt.state.cell.replace_text("Edited at the end");
        assert!(rt.state.finish_cell(false));
        assert_eq!(
            rt.state.store.get_record(&id).unwrap().unwrap().title,
            "Edited at the end"
        );
        rt.invalidate();
        rt.frame(0.);
        rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 100_000.));
        for _ in 0..10 {
            rt.frame(1. / 120.);
        }
        click(&mut rt, "table-new");
        assert!(rt.state.editing_cell.is_some());
        for _ in 0..5 {
            rt.frame(1. / 120.);
        }
        assert!(rt.find("database-cell-editor").is_some());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn logical_table_can_reach_last_row_of_million_with_bounded_nodes() {
    let (mut rt, path) = database_runtime(0);
    rt.poll = None;
    let database_id = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .id
        .clone();
    for index in 0..100 {
        rt.state
            .store
            .create_record(&database_id, &format!("Row {index}"), &Default::default())
            .unwrap();
    }
    assert!(rt.state.query_records());
    let count = rt.state.tab().unwrap().records.len();
    assert_eq!(count, 100);
    let tab = &mut rt.state.tabs[rt.state.active];
    tab.database.as_mut().unwrap().properties.push(Property {
        id: "many".into(),
        name: "Many".into(),
        kind: PropertyKind::MultiSelect,
        config: json!({}),
    });
    for index in [98, 99] {
        tab.records[index].title = "Long single-line title ".repeat(30);
        tab.records[index].values.insert(
            "many".into(),
            json!(["one", "two", "three", "four", "five", "six"]),
        );
    }
    let mut window = crate::table_window::TableWindow::new(count);
    window.total = Some(1_000_000);
    window.start = 999_900;
    window.visible = 999_970..1_000_000;
    rt.state.tabs[rt.state.active].table_window = Some(window);
    rt.invalidate();
    rt.frame(0.);
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 100_000_000.));
    rt.frame(0.);
    assert_eq!(rt.scroll_at_bottom("db-body"), Some(true));
    let header = rt
        .presented("table-header")
        .expect("header remains mounted")
        .0;
    let viewport = rt.presented("db-body").unwrap().0;
    assert!(
        (header[1] - viewport[1]).abs() <= 4.,
        "header should stay at the viewport top: {header:?} {viewport:?}"
    );
    let label = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("database-page-count"))
        .unwrap();
    assert_eq!(label.text.as_deref(), Some("1000000 records"));
    assert!(rt.find("db-next").is_none());
    assert!(rt.find("db-previous").is_none());
    let rect = rt.presented("cell-99-title").expect("last row mounted").0;
    let previous = rt.presented("cell-98-title").unwrap().0;
    assert!(
        (rect[3] - 38.).abs() <= 2.,
        "row height at the f32 end coordinate: {rect:?}"
    );
    assert!(
        (rect[1] - previous[1] - 38.).abs() <= 4.,
        "row spacing: {previous:?} {rect:?}"
    );
    assert!(
        rect[1] >= 0. && rect[1] < 1000.,
        "last row must be onscreen: {rect:?}"
    );
    assert!(rt.find("table-new").is_some());
    assert!(rt.find("cell-0-title").is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn logical_table_shows_placeholders_during_a_distant_window_load() {
    let (mut rt, path) = database_runtime(0);
    rt.poll = None;
    let db = rt.state.tab().unwrap().database.as_ref().unwrap().id.clone();
    for n in 0..100 { rt.state.store.create_record(&db, &format!("Row {n}"), &Default::default()).unwrap(); }
    assert!(rt.state.query_records());
    let mut window = crate::table_window::TableWindow::new(100);
    window.total = Some(1_000_000);
    window.visible = 500_000..500_040;
    rt.state.tabs[rt.state.active].table_window = Some(window);
    rt.invalidate();
    rt.frame(0.);
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 500_000. * 38.));
    rt.frame(0.);
    assert!(rt.find("table-pending-500010").is_some());
    assert!(rt.find("cell-0-title").is_none());
    assert!(rt.elements().len() < 5000, "placeholder window must remain bounded");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
