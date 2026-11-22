use super::*;

fn fixture_with_record() -> (
    Runtime<State>,
    std::path::PathBuf,
    moss_core::Database,
    String,
) {
    let (mut rt, path) = database_runtime(0);
    rt.state.create_inline(Default::default(), false);
    rt.state.cell.replace_text("Fixture item");
    assert!(rt.state.finish_cell(false));
    let tab = rt.state.tab().unwrap();
    let db = tab.database.clone().unwrap();
    let id = tab.records[0].id.clone();
    rt.state.open_database_record(&db.id, &id, "center");
    rt.invalidate();
    rt.frame(0.3);
    rt.frame(0.3);
    (rt, path, db, id)
}

#[test]
fn metadata_menu_edits_visibility_and_keeps_the_record_open() {
    let (mut rt, path, db, _) = fixture_with_record();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let anchor = format!("peek-property-name-{status}");
    click(&mut rt, &anchor);
    click(&mut rt, "metadata-rename");
    assert!(rt.state.record_peek.is_some());
    assert!(rt.find("property-name-input").is_some());
    rt.state
        .database_property_editor
        .as_mut()
        .unwrap()
        .name_input
        .replace_text("Progress");
    click(&mut rt, "property-save");
    assert!(rt.state.record_peek.is_some());
    assert_eq!(
        rt.state
            .record_peek
            .as_ref()
            .unwrap()
            .database
            .properties
            .iter()
            .find(|p| p.id == status)
            .unwrap()
            .name,
        "Progress"
    );
    click(&mut rt, &anchor);
    click(&mut rt, "metadata-visibility");
    click(&mut rt, "metadata-visibility-hidden");
    assert!(rt.find(&anchor).is_none());
    assert!(rt.find("peek-hidden-properties").is_some());
    assert!(rt.state.close_peek());
    rt.state.open_record(0);
    rt.invalidate();
    rt.frame(0.3);
    assert!(rt.find(&anchor).is_none());
    click(&mut rt, "peek-hidden-properties");
    rt.frame(0.3);
    click(&mut rt, &format!("metadata-layout-{status}"));
    assert!(
        rt.state
            .metadata_menu
            .as_ref()
            .is_some_and(|m| m.pane == crate::database_properties::MetadataPane::Visibility),
        "metadata menu closed or did not change pane, open={}",
        rt.state.metadata_menu.is_some()
    );
    click(&mut rt, "metadata-visibility-always");
    assert!(rt.find(&anchor).is_some());
    click(&mut rt, &anchor);
    click(&mut rt, "metadata-delete");
    assert!(rt.find("property-delete-confirm").is_some());
    click(&mut rt, "property-delete-cancel");
    assert!(
        rt.state
            .store
            .get_database(&db.id)
            .unwrap()
            .unwrap()
            .properties
            .iter()
            .any(|p| p.id == status)
    );
    press(&mut rt, Key::Escape);
    assert!(rt.state.database_property_editor.is_none());
    assert!(rt.state.record_peek.is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_hover_reveals_the_drag_handle() {
    let (mut rt, path, db, _) = fixture_with_record();
    let property = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap();
    let anchor = format!("peek-property-name-{}", property.id);
    let grip = format!("peek-property-grip-{}", property.id);
    rt.pointer_move(10., 50.);
    rt.frame(0.3);
    let opacity = |rt: &Runtime<State>| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&grip))
            .unwrap()
            .opacity
    };
    assert!(opacity(&rt) < 0.05);
    let r = rt.presented(&anchor).unwrap().0;
    rt.pointer_move(r[0] + 8., r[1] + 12.);
    rt.frame(0.3);
    assert!(opacity(&rt) > 0.95);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_drag_reorders_record_properties_only() {
    let (mut rt, path, db, _) = fixture_with_record();
    let props: Vec<_> = db
        .properties
        .iter()
        .filter(|p| p.kind != PropertyKind::Title)
        .collect();
    let first = format!("peek-property-name-{}", props[0].id);
    let last = format!("peek-property-name-{}", props.last().unwrap().id);
    let a = rt.presented(&first).unwrap().0;
    let b = rt.presented(&last).unwrap().0;
    rt.pointer_down(a[0] + 8., a[1] + 10.);
    rt.pointer_move(a[0] + 8., a[1] + 16.);
    rt.frame(0.02);
    rt.pointer_move(b[0] + 8., b[1] + b[3] - 2.);
    rt.frame(0.02);
    assert!(rt.find("metadata-drop-marker").is_some());
    rt.pointer_up(b[0] + 8., b[1] + b[3] - 2.);
    rt.frame(0.3);
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    let layout = moss_core::record_layout::layout(&saved).unwrap();
    assert_eq!(layout.order.last(), Some(&props[0].id));
    assert_eq!(
        saved.properties.iter().map(|p| &p.id).collect::<Vec<_>>(),
        db.properties.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
    assert!(rt.state.metadata_menu.is_none());
    click(&mut rt, &last);
    assert!(rt.find("metadata-menu").is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn page_property_order_is_shared_by_database_views() {
    let (mut rt, path, db, record_id) = fixture_with_record();
    rt.poll = Some(poll);
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
    for (id, value) in [
        (&status, json!("In progress")),
        (&date, json!("2026-11-10")),
    ] {
        let record = rt.state.store.get_record(&record_id).unwrap().unwrap();
        let old = record.values.get(id).cloned().unwrap_or(json!(null));
        let saved = rt
            .state
            .store
            .set_record_value_if_unchanged(&record_id, id, &old, value)
            .unwrap();
        rt.state.sync_record(&saved);
    }
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state
        .move_metadata_property(&db.id, &date, Some(&status));
    rt.invalidate();
    rt.frame(0.3);
    let top = |rt: &Runtime<State>, key: &str| {
        rt.presented(key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .0
    };
    assert!(
        top(&rt, &format!("peek-property-row-{date}"))[1]
            < top(&rt, &format!("peek-property-row-{status}"))[1]
    );
    assert!(rt.state.close_peek());
    rt.invalidate();
    rt.frame(0.3);
    assert!(
        top(&rt, &format!("table-header-{date}"))[0]
            < top(&rt, &format!("table-header-{status}"))[0]
    );
    rt.state.selected_cell = Some((record_id.clone(), None));
    let tab_key = KeyEvent {
        key: Key::Tab,
        mods: Mods::default(),
    };
    assert!(rt.state.table_selection_key(&tab_key));
    assert_eq!(
        rt.state.selected_cell.as_ref().unwrap().1.as_deref(),
        Some(date.as_str())
    );
    assert!(rt.state.table_selection_key(&tab_key));
    assert_eq!(
        rt.state.selected_cell.as_ref().unwrap().1.as_deref(),
        Some(status.as_str())
    );
    for (index, date_key, status_key, axis) in [
        (
            1,
            format!("board-value-{record_id}-{date}"),
            format!("board-value-{record_id}-{status}"),
            1,
        ),
        (
            4,
            format!("gallery-value-0-{date}"),
            format!("gallery-value-0-{status}"),
            1,
        ),
        (
            5,
            format!("list-value-0-{date}"),
            format!("list-value-0-{status}"),
            0,
        ),
    ] {
        assert!(rt.state.select_database_view(index));
        rt.invalidate();
        rt.frame(0.3);
        rt.frame(0.3);
        assert!(
            top(&rt, &date_key)[axis] < top(&rt, &status_key)[axis],
            "view {index}"
        );
    }
    assert!(rt.state.select_database_view(2));
    rt.invalidate();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        rt.frame(1. / 120.);
        if rt
            .find(&format!("calendar-value-{record_id}-{date}-2026-11-10"))
            .is_some()
        {
            break;
        }
        assert!(std::time::Instant::now() < until, "calendar did not load");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        top(
            &rt,
            &format!("calendar-value-{record_id}-{date}-2026-11-10")
        )[1] < top(
            &rt,
            &format!("calendar-value-{record_id}-{status}-2026-11-10")
        )[1]
    );
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let saved = reopened.store.get_database(&db.id).unwrap().unwrap();
    let order = moss_core::record_layout::properties(&saved);
    assert!(order.iter().position(|p| p.id == date) < order.iter().position(|p| p.id == status));
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_layout_reorders_with_accessible_controls() {
    let (mut rt, path, db, _) = fixture_with_record();
    let props: Vec<_> = db
        .properties
        .iter()
        .filter(|p| p.kind != PropertyKind::Title)
        .collect();
    click(&mut rt, &format!("peek-property-name-{}", props[0].id));
    rt.frame(0.3);
    click(&mut rt, "metadata-layout");
    click(&mut rt, &format!("metadata-layout-down-{}", props[0].id));
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    let layout = moss_core::record_layout::layout(&saved).unwrap();
    assert_eq!(layout.order[0], props[1].id);
    assert_eq!(layout.order[1], props[0].id);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_empty_visibility_and_full_page_controls_work() {
    let (mut rt, path, db, record_id) = fixture_with_record();
    let db = rt
        .state
        .store
        .add_property(&db.id, "Place", PropertyKind::Place)
        .unwrap();
    rt.state.apply_database_property_change(db.clone(), None);
    let place = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Place)
        .unwrap()
        .id
        .clone();
    rt.invalidate();
    rt.frame(0.3);
    click(&mut rt, &format!("peek-property-name-{place}"));
    rt.frame(0.3);
    click(&mut rt, "metadata-visibility");
    click(&mut rt, "metadata-visibility-hide_empty");
    assert!(rt.find(&format!("peek-property-name-{place}")).is_none());
    assert!(rt.state.close_peek());
    rt.state.open_database_record(&db.id, &record_id, "full");
    rt.invalidate();
    rt.frame(0.3);
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    click(&mut rt, &format!("page-property-{status}"));
    assert!(rt.find("property-picker").is_some());
    press(&mut rt, Key::Escape);
    assert!(rt.find("page-hidden-properties").is_some());
    click(&mut rt, "page-hidden-properties");
    rt.frame(0.3);
    click(&mut rt, &format!("metadata-layout-{place}"));
    click(&mut rt, "metadata-visibility-always");
    assert!(rt.find(&format!("page-property-name-{place}")).is_some());
    click(&mut rt, &format!("page-property-name-{place}"));
    rt.frame(0.3);
    click(&mut rt, "metadata-edit");
    assert!(rt.find("property-name-input").is_some());
    assert!(rt.state.tab().unwrap().record_context.is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_layout_does_not_overwrite_a_newer_change() {
    let (mut rt, path, db, _) = fixture_with_record();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let mut newer = moss_core::record_layout::RecordLayout::default();
    newer.visibility.insert(status.clone(), "hide_empty".into());
    rt.state
        .store
        .set_record_layout(&db.id, None, &newer)
        .unwrap();
    let mut stale = moss_core::record_layout::RecordLayout::default();
    stale.visibility.insert(status.clone(), "hidden".into());
    assert!(!rt.state.set_metadata_layout(&db.id, stale));
    assert!(
        rt.state
            .error
            .as_deref()
            .unwrap_or("")
            .contains("another window")
    );
    let stored = rt.state.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(moss_core::record_layout::layout(&stored).unwrap(), newer);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_layout_rejects_invalid_properties_and_modes() {
    let (mut rt, path, db, _) = fixture_with_record();
    let property = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap();
    for layout in [
        moss_core::record_layout::RecordLayout {
            order: vec![property.id.clone(), property.id.clone()],
            ..Default::default()
        },
        moss_core::record_layout::RecordLayout {
            order: vec!["missing".into()],
            ..Default::default()
        },
        moss_core::record_layout::RecordLayout {
            visibility: [(property.id.clone(), "invalid".into())].into(),
            ..Default::default()
        },
    ] {
        assert!(
            rt.state
                .store
                .set_record_layout(&db.id, None, &layout)
                .is_err()
        );
    }
    assert_eq!(rt.state.store.get_database(&db.id).unwrap().unwrap(), db);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn metadata_rename_preserves_formula_configuration() {
    let (mut rt, path, db, _) = fixture_with_record();
    let updated = rt
        .state
        .store
        .save_advanced_property(
            &db.id,
            None,
            "Total",
            PropertyKind::Formula,
            json!({"expression":"1 + 2"}),
            None,
        )
        .unwrap();
    let formula = updated
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Formula)
        .unwrap()
        .clone();
    rt.state.apply_database_property_change(updated, None);
    rt.invalidate();
    rt.frame(0.3);
    click(&mut rt, &format!("peek-property-name-{}", formula.id));
    rt.frame(0.3);
    click(&mut rt, "metadata-rename");
    assert!(rt.find("formula-expression-input").is_none());
    rt.state
        .database_property_editor
        .as_mut()
        .unwrap()
        .name_input
        .replace_text("Renamed total");
    click(&mut rt, "property-save");
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    let renamed = saved
        .properties
        .iter()
        .find(|p| p.id == formula.id)
        .unwrap();
    assert_eq!(renamed.name, "Renamed total");
    assert_eq!(renamed.config, formula.config);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn full_page_record_adds_edits_typed_properties_and_changes_property_type() {
    let (mut rt, path) = database_runtime(0);
    rt.state.create_inline(Default::default(), false);
    rt.state.cell.replace_text("Austria");
    assert!(rt.state.finish_cell(false));
    let db_id = rt.state.tab().unwrap().database.as_ref().unwrap().id.clone();
    let db = rt
        .state
        .store
        .add_property(&db_id, "Done", PropertyKind::Checkbox)
        .unwrap();
    let checkbox = db.properties.last().unwrap().id.clone();
    let db = rt
        .state
        .store
        .add_property(&db_id, "Days", PropertyKind::Number)
        .unwrap();
    let days = db.properties.last().unwrap().id.clone();
    let record_id = rt.state.tab().unwrap().records[0].id.clone();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.open_database_record(&db_id, &record_id, "full_page");
    rt.invalidate();
    rt.frame(0.3);
    assert!(rt.state.tab().unwrap().database.is_none());
    assert!(rt.state.tab().unwrap().record_context.is_some());

    // Checkbox toggles on a full-page record.
    click(&mut rt, &format!("page-property-{checkbox}"));
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&checkbox],
        json!(true)
    );

    // Number input saves a typed JSON number on a full-page record.
    click(&mut rt, &format!("page-property-{days}"));
    rt.state.cell.replace_text("14");
    assert!(rt.state.finish_cell(false));
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&days],
        json!(14.0)
    );

    // Add a new property directly from the full-page record view.
    click(&mut rt, "page-add-property");
    rt.frame(0.3);
    assert!(rt.find("database-property-menu").is_some());
    rt.state
        .database_property_editor
        .as_mut()
        .unwrap()
        .name_input
        .replace_text("Departure");
    click(&mut rt, "property-save");
    rt.frame(0.3);
    let updated_db = rt.state.store.get_database(&db_id).unwrap().unwrap();
    let departure = updated_db
        .properties
        .iter()
        .find(|p| p.name == "Departure")
        .unwrap()
        .id
        .clone();
    assert!(rt.find(&format!("page-property-name-{departure}")).is_some());

    // Change the new property's type from Text to Date directly from the full-page record.
    click(&mut rt, &format!("page-property-name-{departure}"));
    rt.frame(0.3);
    click(&mut rt, "metadata-edit");
    rt.frame(0.3);
    click(&mut rt, "property-type-trigger");
    click(&mut rt, "property-type-date");
    rt.frame(0.3);
    let updated_db = rt.state.store.get_database(&db_id).unwrap().unwrap();
    assert_eq!(
        updated_db
            .properties
            .iter()
            .find(|p| p.id == departure)
            .unwrap()
            .kind,
        PropertyKind::Date
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
