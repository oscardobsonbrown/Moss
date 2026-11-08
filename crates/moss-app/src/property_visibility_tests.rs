use super::*;

#[test]
fn list_visibility_persists_without_changing_other_views_or_unknown_ids() {
    let (mut rt, path) = database_runtime(5);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    let mut db = rt.state.tab().unwrap().database.clone().unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    db.views[5].config = json!({"hidden_property_ids": ["unknown-property"], "unrelated": "keep"});
    rt.state.store.save_database(&db).unwrap();
    rt.state.tabs[rt.state.active].database = Some(db.clone());
    let mut record = rt.state.tab().unwrap().records[0].clone();
    record.values.insert(date.clone(), json!("2026-11-07"));
    rt.state.store.save_record(&record).unwrap();
    assert!(rt.state.query_records());
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, "list-properties-button");
    assert!(rt.find("list-properties-search").is_some());
    click(&mut rt, &format!("list-property-{status}"));
    assert!(rt.state.property_visibility_open);
    assert!(
        rt.elements()
            .iter()
            .all(|e| e.key.as_deref() != Some(&format!("list-value-0-{status}")))
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some(&format!("list-value-0-{date}")))
    );
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(saved.views[5].config["unrelated"], "keep");
    let hidden = saved.views[5].config["hidden_property_ids"]
        .as_array()
        .unwrap();
    assert!(hidden.contains(&json!(status)) && hidden.contains(&json!("unknown-property")));
    assert!(!hidden.contains(&json!(title)));
    assert_eq!(saved.views[0].config, db.views[0].config);
    click(&mut rt, "list-properties-hide-all");
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    assert!(
        saved.views[5].config["hidden_property_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(date))
    );
    click(&mut rt, "list-properties-show-all");
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(
        saved.views[5].config["hidden_property_ids"],
        json!(["unknown-property"])
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some(&format!("list-value-0-{status}")))
    );
    click(&mut rt, &format!("list-property-{status}"));
    click(&mut rt, "list-properties-close");
    assert!(!rt.state.property_visibility_open);
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let saved = reopened.store.get_database(&db.id).unwrap().unwrap();
    assert!(
        saved.views[5].config["hidden_property_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(status))
    );
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_visibility_save_keeps_panel_and_previous_state() {
    let (mut rt, path) = database_runtime(5);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    click(&mut rt, "list-properties-button");
    rt.state.store.delete_page(&db.page_id).unwrap();
    click(&mut rt, &format!("list-property-{status}"));
    assert!(rt.state.property_visibility_open);
    assert!(rt.state.error.as_deref().unwrap().contains("not saved"));
    assert_eq!(
        rt.state.tab().unwrap().database.as_ref().unwrap().views[5].config,
        db.views[5].config
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn visibility_search_filters_rows_and_shortcut_closes_panel() {
    let (mut rt, path) = database_runtime(5);
    let db = rt.state.tab().unwrap().database.as_ref().unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    click(&mut rt, "list-properties-button");
    press(&mut rt, Key::Text("Status".into()));
    assert!(rt.find(&format!("list-property-{status}")).is_some());
    assert!(rt.find(&format!("list-property-{date}")).is_none());
    press(&mut rt, Key::Command('k'));
    assert!(!rt.state.property_visibility_open);
    assert!(!rt.state.property_visibility_search.focused);
    assert!(matches!(rt.state.modal, Some(state::Modal::Search)));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn visibility_merges_changes_from_another_window_by_view_id() {
    let (mut rt, path) = database_runtime(5);
    let original = rt.state.tab().unwrap().database.clone().unwrap();
    let view_id = original.views[5].id.clone();
    let status = original
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let mut newer = original.clone();
    newer.views[0].name = "Edited elsewhere".into();
    newer.views[5].config = json!({"unrelated": "preserved"});
    newer.views.swap(0, 5);
    rt.state.store.save_database(&newer).unwrap();
    click(&mut rt, "list-properties-button");
    click(&mut rt, &format!("list-property-{status}"));
    let saved = rt.state.store.get_database(&original.id).unwrap().unwrap();
    assert_eq!(
        saved
            .views
            .iter()
            .find(|v| v.id == original.views[0].id)
            .unwrap()
            .name,
        "Edited elsewhere"
    );
    let list = saved.views.iter().find(|v| v.id == view_id).unwrap();
    assert_eq!(list.config["unrelated"], "preserved");
    assert!(
        list.config["hidden_property_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(status))
    );
    assert_eq!(
        rt.state.tab().unwrap().database.as_ref().unwrap().views[rt.state.tab().unwrap().view].id,
        view_id
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_settings_merge_latest_view_config_and_keep_other_edits() {
    let (mut rt, path) = database_runtime(4);
    let old = rt.state.tab().unwrap().database.clone().unwrap();
    let gallery_id = old.views[4].id.clone();
    let mut newer = old.clone();
    newer.views[0].name = "Other window edit".into();
    newer.views[4].config = json!({"unrelated": "keep", "gallery_preview": "none"});
    newer.views.swap(0, 4);
    rt.state.store.save_database(&newer).unwrap();
    click(&mut rt, "gallery-settings-button");
    click(&mut rt, "gallery-size-small");
    let saved = rt.state.store.get_database(&old.id).unwrap().unwrap();
    assert_eq!(
        saved
            .views
            .iter()
            .find(|v| v.id == old.views[0].id)
            .unwrap()
            .name,
        "Other window edit"
    );
    let gallery = saved.views.iter().find(|v| v.id == gallery_id).unwrap();
    assert_eq!(gallery.config["unrelated"], "keep");
    assert_eq!(gallery.config["gallery_preview"], "none");
    assert_eq!(gallery.config["gallery_card_size"], "small");
    assert_eq!(
        rt.state.tab().unwrap().database.as_ref().unwrap().views[rt.state.tab().unwrap().view].id,
        gallery_id
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_setting_rejects_view_changed_in_another_window() {
    let (mut rt, path) = database_runtime(4);
    let mut db = rt.state.tab().unwrap().database.clone().unwrap();
    let id = db.views[4].id.clone();
    db.views[4].kind = ViewKind::Table;
    rt.state.store.save_database(&db).unwrap();
    click(&mut rt, "gallery-settings-button");
    click(&mut rt, "gallery-size-small");
    assert!(rt.state.gallery_settings_open);
    assert!(rt.state.error.as_deref().unwrap().contains("not saved"));
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(
        saved.views.iter().find(|v| v.id == id).unwrap().config,
        db.views[4].config
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn visibility_applies_to_table_board_gallery_and_preserves_date_views() {
    for view_index in 0..6 {
        let (mut rt, path) = database_runtime(view_index);
        let mut db = rt.state.tab().unwrap().database.clone().unwrap();
        let title = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Title)
            .unwrap()
            .id
            .clone();
        let status = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Status)
            .unwrap()
            .id
            .clone();
        let date = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Date)
            .unwrap()
            .id
            .clone();
        let date_value = rt.state.tab().unwrap().calendar_month + chrono::Duration::days(5);
        let mut values = std::collections::BTreeMap::new();
        values.insert(status.clone(), json!("Not started"));
        values.insert(date.clone(), json!(date_value.to_string()));
        let record = rt
            .state
            .store
            .create_record(&db.id, "Visible title", &values)
            .unwrap();
        db.views[view_index].config = json!({"hidden_property_ids": if view_index == 2 || view_index == 3 { vec![status.clone(), title.clone(), date.clone()] } else { vec![status.clone(), title.clone()] }, "freeze_through": status, "calculations": {date.clone(): "count_values", status.clone(): "count_all"}});
        rt.state.store.save_database(&db).unwrap();
        let active = rt.state.active;
        rt.state.tabs[active].database = Some(db.clone());
        let record_id = record.id.clone();
        rt.state.tabs[active].records = vec![record];
        rt.state.tabs[active].timeline_start = date_value - chrono::Duration::days(2);
        rt.state.tabs[active].table_window = None;
        rt.invalidate();
        rt.frame(0.);
        assert!(ui::property_visibility::visible(&db, view_index, &title));
        assert!(!ui::property_visibility::visible(&db, view_index, &status));
        let keys: Vec<_> = rt.elements().into_iter().filter_map(|e| e.key).collect();
        match view_index {
            0 => {
                assert!(keys.contains(&"table-header-title".to_string()));
                assert!(!keys.contains(&format!("table-header-{status}")));
                assert!(keys.contains(&format!("table-header-{date}")));
                assert!(!keys.contains(&format!("cell-0-{status}")));
                assert!(keys.contains(&format!("cell-0-{date}")));
                assert!(!keys.contains(&format!("table-summary-{status}")));
                assert!(keys.contains(&format!("table-summary-{date}")));
                let title_rect = rt.presented("table-header-title").unwrap().0;
                let date_rect = rt.presented(&format!("table-header-{date}")).unwrap().0;
                assert!((date_rect[0] - title_rect[0] - 280.).abs() < 1.);
            }
            1 => {
                assert!(!keys.contains(&format!("board-value-{record_id}-{status}")));
                assert!(keys.contains(&format!("board-value-{record_id}-{date}")));
            }
            2 => assert!(keys.contains(&"calendar-0".to_owned())),
            3 => assert!(keys.contains(&"timeline-bar-0".to_owned())),
            4 => {
                assert!(!keys.contains(&format!("gallery-value-0-{status}")));
                assert!(keys.contains(&format!("gallery-value-0-{date}")));
            }
            5 => {
                assert!(!keys.contains(&format!("list-value-0-{status}")));
                assert!(keys.contains(&format!("list-value-0-{date}")));
            }
            _ => unreachable!(),
        }
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn visibility_saves_per_view_for_all_six_layouts() {
    let (mut rt, path) = database_runtime(0);
    let mut db = rt.state.tab().unwrap().database.clone().unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    for view in &mut db.views {
        view.config = json!({"hidden_property_ids": ["unrecognized-id"], "custom": "preserved"});
    }
    rt.state.store.save_database(&db).unwrap();
    let active = rt.state.active;
    rt.state.tabs[active].database = Some(db.clone());
    for index in 0..6 {
        rt.state.tabs[active].view = index;
        ui::property_visibility::set_visibility(&mut rt.state, Some(&status), true);
        assert!(rt.state.error.is_none());
        let stored = rt.state.store.get_database(&db.id).unwrap().unwrap();
        for (other_index, view) in stored.views.iter().enumerate() {
            assert_eq!(view.config["custom"], "preserved");
            let hidden = view.config["hidden_property_ids"].as_array().unwrap();
            assert!(hidden.contains(&json!("unrecognized-id")));
            assert_eq!(hidden.contains(&json!(status)), other_index <= index);
        }
    }
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let db = reopened.store.get_database(&db.id).unwrap().unwrap();
    assert!(db.views.iter().take(6).all(|view| {
        view.config["hidden_property_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(status))
    }));
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn view_settings_properties_action_saves_and_reopens_for_each_layout() {
    let (mut rt, path) = database_runtime(0);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    for index in 0..6 {
        let active = rt.state.active;
        rt.state.tabs[active].view = index;
        rt.invalidate();
        rt.frame(0.);
        click(&mut rt, "db-view-settings");
        click(&mut rt, "view-properties");
        assert!(rt.find("list-property-visibility").is_some());
        click(&mut rt, &format!("list-property-{status}"));
        assert!(rt.state.error.is_none());
        assert!(rt.state.property_visibility_open);
        click(&mut rt, "list-properties-close");
    }
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let db = reopened.store.get_database(&db.id).unwrap().unwrap();
    assert!(db.views.iter().take(6).all(|view| {
        view.config["hidden_property_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(status))
    }));
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
