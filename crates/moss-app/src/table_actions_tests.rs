use super::*;
use moss_core::{FilterExpr, PropertyKind};
use serde_json::json;

#[test]
fn header_actions_persist_query_settings_and_relative_insertion() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    let view = db.views[0].id.clone();
    s.open_database_property(Some(&title));
    s.set_table_sort(Some(true));
    s.open_database_property(Some(&title));
    s.table_filter_input.replace_text("Alpha");
    s.set_table_filter();
    s.open_database_property(Some(&title));
    s.set_table_group();
    s.open_database_property(Some(&title));
    s.set_table_config("wrap_cells", json!(true));
    s.open_database_property(Some(&title));
    s.set_table_config("freeze_through", json!(title));
    s.open_database_property(Some(&title));
    s.insert_table_property(true);
    s.database_property_input.replace_text("Details");
    assert!(s.save_database_property());
    s.open_database_property(Some(&title));
    s.insert_table_property(false);
    s.database_property_input.replace_text("Before title");
    assert!(s.save_database_property());
    let saved = s.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(saved.properties[0].name, "Before title");
    assert_eq!(saved.properties[1].id, title);
    assert_eq!(saved.properties[2].name, "Details");
    let saved_view = saved.views.iter().find(|v| v.id == view).unwrap();
    assert_eq!(saved_view.query.sorts[0].property_id, title);
    assert!(saved_view.query.sorts[0].descending);
    assert!(matches!(saved_view.query.filter, Some(FilterExpr::Rule(_))));
    assert_eq!(saved_view.query.group_by.as_deref(), Some(title.as_str()));
    assert_eq!(saved_view.config["wrap_cells"], true);
    assert_eq!(saved_view.config["freeze_through"], title);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn conflicting_table_view_change_keeps_menu_and_draft() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    s.open_database_property(Some(&title));
    let view = &db.views[0];
    s.store
        .edit_view_config(&db.id, &view.id, view.kind, |config| {
            config["wrap_cells"] = json!(true);
            Ok(())
        })
        .unwrap();
    s.set_table_config("wrap_cells", json!(false));
    assert!(s.database_property_editor.as_ref().unwrap().error.is_some());
    assert_eq!(
        s.store.get_database(&db.id).unwrap().unwrap().views[0].config["wrap_cells"],
        true
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn wrapped_rows_remain_aligned_when_editing_and_frozen_columns_stay_visible() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let title_id = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    for n in 0..5 {
        s.store
            .add_property(&db.id, &format!("Extra {n}"), PropertyKind::Text)
            .unwrap();
    }
    let new_db = s.store.get_database(&db.id).unwrap().unwrap();
    s.tabs[s.active].database = Some(new_db);
    let long = "A long title with multiple words that must wrap when the row is displayed and while it is being edited";
    s.store
        .create_record(&db.id, long, &Default::default())
        .unwrap();
    assert!(s.query_records());
    s.open_database_property(Some(&title_id));
    s.set_table_config("wrap_cells", json!(true));
    let property = db
        .properties
        .iter()
        .find(|p| p.kind != PropertyKind::Title)
        .unwrap()
        .clone();
    s.open_database_property(Some(&property.id));
    s.set_table_config("freeze_through", json!(property.id));
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (850., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    let elem = |rt: &Runtime<State>, key: &str| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
    };
    let title = elem(&rt, "cell-0-title");
    assert!(title.target[3] > 38., "wrapped title: {:?}", title.target);
    let other = elem(&rt, &format!("cell-0-{}", property.id));
    assert!(
        (title.target[3] - other.target[3]).abs() < 1.,
        "cells should share row height"
    );
    click(&mut rt, "cell-0-title");
    let editing = elem(&rt, "cell-0-title");
    assert!(
        (editing.target[3] - title.target[3]).abs() < 2.,
        "rest {:?}, edit {:?}",
        title.target,
        editing.target
    );
    press(&mut rt, Key::Escape);
    let viewport = elem(&rt, "db-body").rect;
    rt.wheel_xy(viewport[0] + 100., viewport[1] + 50., -250., 0.);
    rt.frame(1. / 60.);
    assert!(
        elem(&rt, "table-header-title").stuck > 0.,
        "title header must stick while scrolling"
    );
    assert!(
        elem(&rt, &format!("table-header-{}", property.id)).stuck > 0.,
        "second frozen header must stick while scrolling"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calculation_setting_preserves_other_columns_and_detects_same_column_conflict() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    let other = db
        .properties
        .iter()
        .find(|p| p.kind != PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    s.open_database_property(Some(&title));
    s.store
        .edit_view_config(&db.id, &db.views[0].id, db.views[0].kind, |config| {
            config["calculations"] = json!({other.clone(): "count_all"});
            Ok(())
        })
        .unwrap();
    s.set_table_calculation(Some("count_values"));
    let saved = s.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(
        saved.views[0].config["calculations"][&title],
        "count_values"
    );
    assert_eq!(saved.views[0].config["calculations"][&other], "count_all");
    s.open_database_property(Some(&title));
    s.store
        .edit_view_config(&db.id, &db.views[0].id, db.views[0].kind, |config| {
            config["calculations"][&title] = json!("count_unique");
            Ok(())
        })
        .unwrap();
    s.set_table_calculation(Some("count_empty"));
    assert!(s.database_property_editor.as_ref().unwrap().error.is_some());
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn footer_summaries_cover_all_matching_rows_not_only_the_loaded_page() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let db = s
        .store
        .add_property(&db.id, "Amount", PropertyKind::Number)
        .unwrap();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    let amount = db
        .properties
        .iter()
        .find(|p| p.name == "Amount")
        .unwrap()
        .id
        .clone();
    s.tabs[s.active].database = Some(db.clone());
    for n in 1..=150 {
        let values = std::collections::BTreeMap::from([(amount.clone(), json!(n))]);
        s.store
            .create_record(&db.id, &format!("Row {n}"), &values)
            .unwrap();
    }
    assert!(s.query_records());
    assert_eq!(s.tab().unwrap().records.len(), 100);
    s.open_database_property(Some(&title));
    s.set_table_calculation(Some("count_all"));
    s.open_database_property(Some(&amount));
    s.set_table_calculation(Some("sum"));
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    for _ in 0..200 {
        rt.update(|s, tx| {
            crate::table_summaries::poll(s, tx);
        });
        rt.frame(1. / 120.);
        if matches!(
            rt.state.tab().unwrap().summary_values.get(&amount),
            Some(crate::table_summaries::SummaryValue::Ready(_))
        ) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let tab = rt.state.tab().unwrap();
    assert!(
        matches!(tab.summary_values.get(&title), Some(crate::table_summaries::SummaryValue::Ready(v)) if v == &json!(150))
    );
    assert!(
        matches!(tab.summary_values.get(&amount), Some(crate::table_summaries::SummaryValue::Ready(v)) if v.as_f64() == Some(11325.))
    );
    rt.update(|_, tx| tx.scroll_to_bottom_instant("db-body"));
    rt.frame(1. / 120.);
    assert!(rt.find(&format!("table-summary-{amount}")).is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn header_filter_menu_applies_contains_and_removes_its_rule() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    for name in ["Alpha", "Beta", "Alphabet"] {
        s.store
            .create_record(&db.id, name, &Default::default())
            .unwrap();
    }
    assert!(s.query_records());
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, "table-header-title");
    click(&mut rt, "table-filter");
    click(&mut rt, "table-filter-op-contains");
    click(&mut rt, "table-filter-value");
    press(&mut rt, Key::Text("Alpha".into()));
    click(&mut rt, "table-filter-apply");
    assert_eq!(rt.state.tab().unwrap().records.len(), 2);
    click(&mut rt, "table-header-title");
    click(&mut rt, "table-filter");
    click(&mut rt, "table-filter-remove");
    assert_eq!(rt.state.tab().unwrap().records.len(), 3);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn tab_traversal_follows_inserted_column_order() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let title = s
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    s.open_database_property(Some(&title));
    s.insert_table_property(false);
    s.database_property_input.replace_text("Before");
    assert!(s.save_database_property());
    let before = s.tab().unwrap().database.as_ref().unwrap().properties[0]
        .id
        .clone();
    s.create_inline(Default::default(), false);
    s.cell.replace_text("Row");
    assert!(s.finish_cell(false));
    let record = s.tab().unwrap().records[0].id.clone();
    s.selected_cell = Some((record.clone(), Some(before)));
    assert!(s.table_selection_key(&KeyEvent {
        key: Key::Tab,
        mods: Mods::default()
    }));
    assert_eq!(s.selected_cell, Some((record, None)));
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calculation_menu_is_reachable_in_a_small_window() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 600.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, "table-header-title");
    let (x, y) = rt.find("property-menu-scroll").unwrap();
    rt.wheel(x, y, -300.);
    rt.frame(1. / 60.);
    click(&mut rt, "table-calculate");
    click(&mut rt, "table-calculate-count_all");
    assert!(
        rt.state.tab().unwrap().database.as_ref().unwrap().views[0].config["calculations"]
            .is_object()
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn removing_property_filter_keeps_unrelated_filters() {
    use moss_core::{FilterOp, FilterRule};
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    let other = db
        .properties
        .iter()
        .find(|p| p.kind != PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    let unrelated = FilterExpr::Rule(FilterRule {
        property_id: other,
        op: FilterOp::IsNotEmpty,
        value: json!(null),
    });
    let mut changed = db.clone();
    changed.views[0].query.filter = Some(unrelated.clone());
    s.store.save_database(&changed).unwrap();
    s.tabs[s.active].database = Some(changed);
    for text in ["one", "two"] {
        s.open_database_property(Some(&title));
        s.table_filter_input.replace_text(text);
        s.set_table_filter();
    }
    s.open_database_property(Some(&title));
    s.clear_table_filter();
    assert_eq!(
        s.tab().unwrap().database.as_ref().unwrap().views[0]
            .query
            .filter,
        Some(unrelated)
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
