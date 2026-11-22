use super::*;
use moss_core::{PropertyKind, new_id};
use serde_json::json;
use std::path::PathBuf;

fn fixture(width: f32) -> (Runtime<State>, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-property-ui-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_name.replace_text("Property checks");
    state.new_page(true, None);
    if width == 850. {
        let mut db = state.tab().unwrap().database.clone().unwrap();
        db.properties.retain(|p| p.kind == PropertyKind::Title);
        state.store.save_database(&db).unwrap();
        state.tabs[state.active].database = Some(db);
    }
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        (width, 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    (rt, path)
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_move(x, y);
    rt.frame(1. / 120.);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..60 {
        rt.frame(1. / 120.);
    }
}

fn database(rt: &Runtime<State>) -> moss_core::Database {
    let id = &rt.state.tab().unwrap().database.as_ref().unwrap().id;
    rt.state.store.get_database(id).unwrap().unwrap()
}

#[test]
fn table_property_create_edit_rename_and_delete_persist() {
    let (mut rt, path) = fixture(1800.);
    click(&mut rt, "table-add-property");
    settle(&mut rt);
    assert!(rt.find("database-property-menu").is_some());
    assert!(rt.find("modal-scrim").is_none());
    click(&mut rt, "property-name-input");
    press(&mut rt, Key::Text("Notes".into()));
    click(&mut rt, "property-save");
    settle(&mut rt);
    assert!(rt.find("database-property-menu").is_none());
    let property = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.name == "Notes")
        .unwrap();
    assert_eq!(property.kind, PropertyKind::Text);
    let header = format!("table-header-{}", property.id);
    assert!(rt.find(&header).is_some());

    click(&mut rt, "table-new");
    press(&mut rt, Key::Text("Test row".into()));
    press(&mut rt, Key::Escape);
    let record_id = rt.state.tab().unwrap().records[0].id.clone();
    click(&mut rt, &format!("cell-0-{}", property.id));
    press(&mut rt, Key::Text("Stored text".into()));
    press(&mut rt, Key::Escape);
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&property.id],
        json!("Stored text")
    );

    click(&mut rt, &header);
    settle(&mut rt);
    click(&mut rt, "property-name-input");
    press(&mut rt, Key::Command('a'));
    press(&mut rt, Key::Text("Details".into()));
    click(&mut rt, "property-save");
    settle(&mut rt);
    assert!(
        database(&rt)
            .properties
            .iter()
            .any(|p| p.id == property.id && p.name == "Details")
    );
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&property.id],
        json!("Stored text")
    );

    click(&mut rt, &header);
    settle(&mut rt);
    click(&mut rt, "property-delete");
    click(&mut rt, "property-delete-cancel");
    assert!(database(&rt).properties.iter().any(|p| p.id == property.id));
    click(&mut rt, "property-delete");
    click(&mut rt, "property-delete-confirm");
    settle(&mut rt);
    assert!(rt.find("database-property-menu").is_none());
    assert!(!database(&rt).properties.iter().any(|p| p.id == property.id));
    assert!(
        !rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values
            .contains_key(&property.id)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn property_choices_validation_title_and_dismissal() {
    let (mut rt, path) = fixture(850.);
    click(&mut rt, "table-header-title");
    settle(&mut rt);
    let menu = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("database-property-menu") && !e.exiting)
        .unwrap();
    let [x, y, w, h] = menu.target;
    assert!(x >= 0. && y >= 0. && x + w <= 850. && y + h <= 700.);
    assert!(rt.find("property-delete").is_none());
    press(&mut rt, Key::Escape);
    settle(&mut rt);
    assert!(rt.find("database-property-menu").is_none());
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("table-header-title") && e.focused)
    );

    click(&mut rt, "table-header-title");
    settle(&mut rt);
    click(&mut rt, "property-name-input");
    press(&mut rt, Key::Command('a'));
    press(&mut rt, Key::Text("Task".into()));
    click(&mut rt, "property-save");
    settle(&mut rt);
    assert!(
        database(&rt)
            .properties
            .iter()
            .any(|p| p.kind == PropertyKind::Title && p.name == "Task")
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Task"))
    );

    click(&mut rt, "table-add-property");
    settle(&mut rt);
    for kind in [
        "text", "number", "select", "status", "date", "checkbox", "url", "email", "phone",
    ] {
        assert!(
            rt.find(&format!("property-type-{kind}")).is_some(),
            "missing {kind}"
        );
    }
    click(&mut rt, "property-save");
    assert!(rt.find("database-property-menu").is_some());
    assert!(
        rt.state
            .database_property_editor
            .as_ref()
            .unwrap()
            .error
            .is_some()
    );
    click(&mut rt, "property-name-input");
    press(&mut rt, Key::Text("Estimate".into()));
    click(&mut rt, "property-type-number");
    click(&mut rt, "property-save");
    settle(&mut rt);
    assert!(
        database(&rt)
            .properties
            .iter()
            .any(|p| p.name == "Estimate" && p.kind == PropertyKind::Number)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn select_property_options_are_available_to_a_record() {
    let (mut rt, path) = fixture(1800.);
    click(&mut rt, "table-add-property");
    settle(&mut rt);
    click(&mut rt, "property-name-input");
    press(&mut rt, Key::Text("Priority".into()));
    click(&mut rt, "property-type-select");
    click(&mut rt, "property-options-input");
    press(&mut rt, Key::Text("Low, Medium, High".into()));
    click(&mut rt, "property-save");
    settle(&mut rt);
    let property = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.name == "Priority")
        .unwrap();
    assert_eq!(property.kind, PropertyKind::Select);
    click(&mut rt, "table-new");
    press(&mut rt, Key::Text("Test row".into()));
    press(&mut rt, Key::Escape);
    let record_id = rt.state.tab().unwrap().records[0].id.clone();
    click(&mut rt, &format!("cell-0-{}", property.id));
    settle(&mut rt);
    for (i, name) in ["Low", "Medium", "High"].into_iter().enumerate() {
        let option = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&format!("property-option-{i}")) && !e.exiting)
            .unwrap();
        assert_eq!(option.label.as_deref(), Some(name));
    }
    click(&mut rt, "property-option-1");
    assert_eq!(
        rt.state
            .store
            .get_record(&record_id)
            .unwrap()
            .unwrap()
            .values[&property.id],
        json!("Medium")
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn table_header_changes_text_property_to_date_and_select_with_confirmation_rules() {
    let (mut rt, path) = fixture(1800.);
    let mut initial = database(&rt);
    initial.properties.retain(|p| p.kind == PropertyKind::Title);
    rt.state.store.save_database(&initial).unwrap();
    let db_id = initial.id;
    let db = rt
        .state
        .store
        .add_property(&db_id, "Departure Date", PropertyKind::Text)
        .unwrap();
    let date_prop = db
        .properties
        .iter()
        .find(|p| p.name == "Departure Date")
        .unwrap()
        .clone();
    let db = rt
        .state
        .store
        .add_property(&db_id, "Empty Note", PropertyKind::Text)
        .unwrap();
    let empty_prop = db
        .properties
        .iter()
        .find(|p| p.name == "Empty Note")
        .unwrap()
        .clone();
    let db = rt
        .state
        .store
        .add_property(&db_id, "Stage", PropertyKind::Text)
        .unwrap();
    let stage_prop = db
        .properties
        .iter()
        .find(|p| p.name == "Stage")
        .unwrap()
        .clone();
    let record = rt
        .state
        .store
        .create_record(
            &db_id,
            "Japan (Jan)",
            &std::collections::BTreeMap::from([
                (date_prop.id.clone(), json!("January 5, 2026")),
                (stage_prop.id.clone(), json!("Booked")),
            ]),
        )
        .unwrap();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.query_records();
    rt.frame(0.);

    // Title header does not expose a type changer.
    click(&mut rt, "table-header-title");
    settle(&mut rt);
    assert!(rt.find("property-type-trigger").is_none());
    press(&mut rt, Key::Escape);
    settle(&mut rt);

    // Non-empty Text -> Date shows confirmation, Cancel preserves Text, Confirm converts value and enables Date picker.
    click(&mut rt, &format!("table-header-{}", date_prop.id));
    settle(&mut rt);
    assert!(rt.find("property-type-trigger").is_some());
    click(&mut rt, "property-type-trigger");
    click(&mut rt, "property-type-date");
    assert!(rt.find("property-type-confirm").is_some());
    click(&mut rt, "property-type-cancel");
    assert_eq!(
        database(&rt)
            .properties
            .iter()
            .find(|p| p.id == date_prop.id)
            .unwrap()
            .kind,
        PropertyKind::Text
    );
    click(&mut rt, "property-type-date");
    click(&mut rt, "property-type-confirm");
    settle(&mut rt);
    assert!(rt.find("database-property-menu").is_none());
    assert_eq!(
        database(&rt)
            .properties
            .iter()
            .find(|p| p.id == date_prop.id)
            .unwrap()
            .kind,
        PropertyKind::Date
    );
    assert_eq!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .values[&date_prop.id],
        json!("2026-01-05")
    );
    click(&mut rt, &format!("cell-0-{}", date_prop.id));
    settle(&mut rt);
    assert!(rt.state.property_picker().is_some());
    press(&mut rt, Key::Escape);
    settle(&mut rt);

    // Empty Text -> Number applies immediately without confirmation.
    click(&mut rt, &format!("table-header-{}", empty_prop.id));
    settle(&mut rt);
    click(&mut rt, "property-type-trigger");
    click(&mut rt, "property-type-number");
    settle(&mut rt);
    assert!(rt.find("database-property-menu").is_none());
    assert_eq!(
        database(&rt)
            .properties
            .iter()
            .find(|p| p.id == empty_prop.id)
            .unwrap()
            .kind,
        PropertyKind::Number
    );

    // Lossless Text -> Select applies immediately and populates options from existing values.
    click(&mut rt, &format!("table-header-{}", stage_prop.id));
    settle(&mut rt);
    click(&mut rt, "property-type-trigger");
    click(&mut rt, "property-type-select");
    settle(&mut rt);
    assert!(rt.find("database-property-menu").is_none());
    let stage = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == stage_prop.id)
        .unwrap();
    assert_eq!(stage.kind, PropertyKind::Select);
    let stage_names: Vec<_> = stage.config["options"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(moss_core::option_name)
        .collect();
    assert_eq!(stage_names, vec!["Booked"]);
    let auto_color = moss_core::resolved_option_color(&stage, "Booked");
    assert!(moss_core::AUTO_OPTION_COLORS.contains(&auto_color));

    // Change the option color directly from the table cell picker via the pencil button.
    click(&mut rt, &format!("cell-0-{}", stage_prop.id));
    settle(&mut rt);
    click(&mut rt, "property-option-edit-0");
    click(&mut rt, "property-color-green");
    let stage = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == stage_prop.id)
        .unwrap();
    assert_eq!(moss_core::resolved_option_color(&stage, "Booked"), "green");
    press(&mut rt, Key::Escape);
    settle(&mut rt);

    // Change the option color from the column header Edit property menu.
    click(&mut rt, &format!("table-header-{}", stage_prop.id));
    settle(&mut rt);
    click(&mut rt, "editor-option-color-0");
    click(&mut rt, "editor-color-purple");
    let stage = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == stage_prop.id)
        .unwrap();
    assert_eq!(moss_core::resolved_option_color(&stage, "Booked"), "purple");
    press(&mut rt, Key::Escape);
    settle(&mut rt);

    // Convert Select -> Multi-select, toggle multiple tags, and verify colored tag pills.
    click(&mut rt, &format!("table-header-{}", stage_prop.id));
    settle(&mut rt);
    click(&mut rt, "property-type-trigger");
    click(&mut rt, "property-type-multi-select");
    settle(&mut rt);
    let stage = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == stage_prop.id)
        .unwrap();
    assert_eq!(stage.kind, PropertyKind::MultiSelect);
    assert_eq!(moss_core::resolved_option_color(&stage, "Booked"), "purple");
    click(&mut rt, &format!("cell-0-{}", stage_prop.id));
    settle(&mut rt);
    assert!(rt.find("property-create-new").is_none());
    click(&mut rt, "property-picker-input");
    press(&mut rt, Key::Text("Winter".into()));
    click(&mut rt, "property-option-create");
    assert_eq!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .values[&stage_prop.id],
        json!(["Booked", "Winter"])
    );
    let stage = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == stage_prop.id)
        .unwrap();
    assert!(moss_core::AUTO_OPTION_COLORS.contains(&moss_core::resolved_option_color(&stage, "Winter")));

    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn status_pencil_edits_name_and_color_create_new_and_date_format_and_relation_links_work() {
    let (mut rt, path) = fixture(1440.);
    let db_id = database(&rt).id;
    let status_id = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id;
    let date_id = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id;

    // Create a second database for testing relation open-in-new-tab links.
    rt.state.new_page(true, None);
    let target_db_id = database(&rt).id;
    let target_record = rt
        .state
        .store
        .create_record(&target_db_id, "Linked Item", &Default::default())
        .unwrap();
    rt.state.active = 0;

    let db = rt
        .state
        .store
        .save_advanced_property(
            &db_id,
            None,
            "Related",
            PropertyKind::Relation,
            json!({"target_database_id": target_db_id}),
            None,
        )
        .unwrap();
    let relation_id = db.properties.last().unwrap().id.clone();
    rt.state.tabs[0].database = Some(db);

    let mut values = std::collections::BTreeMap::new();
    values.insert(status_id.clone(), json!("In progress"));
    values.insert(date_id.clone(), json!("2026-11-12"));
    values.insert(relation_id.clone(), json!([target_record.id.clone()]));
    let record = rt.state.store.create_record(&db_id, "Task A", &values).unwrap();
    rt.state.query_records();
    rt.invalidate();
    settle(&mut rt);

    // 1. Edit Status name & color via the hover pencil icon and verify existing record updates.
    click(&mut rt, &format!("cell-0-{status_id}"));
    settle(&mut rt);
    assert!(rt.presented("property-create-new").is_none());
    assert!(rt.presented("property-clear").is_none());
    click(&mut rt, "property-option-edit-1");
    settle(&mut rt);
    rt.state
        .property_picker_mut()
        .unwrap()
        .option_editor
        .as_mut()
        .unwrap()
        .name_input
        .replace_text("Active");
    click(&mut rt, "property-color-purple");
    settle(&mut rt);
    assert_eq!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .values[&status_id],
        json!("Active")
    );
    let status_prop = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == status_id)
        .unwrap();
    assert_eq!(moss_core::resolved_option_color(&status_prop, "Active"), "purple");
    assert_eq!(moss_core::resolved_option_color(&status_prop, "Done"), "green");

    // 2. Type a new Status option name in the search input and create it inline.
    rt.state
        .property_picker_mut()
        .unwrap()
        .input
        .replace_text("Blocked");
    rt.invalidate();
    settle(&mut rt);
    click(&mut rt, "property-option-create");
    settle(&mut rt);
    assert_eq!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .values[&status_id],
        json!("Blocked")
    );
    let status_prop = database(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == status_id)
        .unwrap();
    assert!(moss_core::property_options(&status_prop).iter().any(|o| o.name == "Blocked"));

    // 3. Edit Date display format from the Date column header and from the Date picker.
    click(&mut rt, &format!("table-header-{date_id}"));
    settle(&mut rt);
    click(&mut rt, "property-date-format-trigger");
    click(&mut rt, "editor-date-format-year_month_day");
    settle(&mut rt);
    press(&mut rt, Key::Escape);
    settle(&mut rt);
    let date_cell_text = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&format!("cell-text-0-{date_id}")))
        .and_then(|e| e.text);
    assert_eq!(date_cell_text.as_deref(), Some("2026-11-12"));

    click(&mut rt, &format!("cell-0-{date_id}"));
    settle(&mut rt);
    let (sx, sy) = rt.find("date-scroll").unwrap();
    rt.wheel(sx, sy, -300.);
    settle(&mut rt);
    click(&mut rt, "date-format-trigger");
    settle(&mut rt);
    click(&mut rt, "picker-date-format-month_day_year_full");
    settle(&mut rt);
    press(&mut rt, Key::Escape);
    settle(&mut rt);
    let date_cell_text = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&format!("cell-text-0-{date_id}")))
        .and_then(|e| e.text);
    assert_eq!(date_cell_text.as_deref(), Some("November 12, 2026"));

    // 4. Click the underlined relation link directly inside the table cell to open in a new tab.
    let tabs_before = rt.state.tabs.len();
    click(&mut rt, &format!("relation-link-{relation_id}-0"));
    settle(&mut rt);
    assert_eq!(rt.state.tabs.len(), tabs_before + 1);
    assert_eq!(rt.state.tab().unwrap().page.title, "Linked Item");

    // 5. Open linked relation record in a new tab from the Relation picker too.
    let opened_tab = rt.state.active;
    rt.state.close_tab(opened_tab);
    rt.state.active = 0;
    rt.invalidate();
    settle(&mut rt);
    let tabs_before = rt.state.tabs.len();
    rt.state.open_relation_picker(&record.id, &relation_id);
    rt.invalidate();
    settle(&mut rt);
    click(&mut rt, &format!("relation-open-selected-{}", target_record.id));
    settle(&mut rt);
    assert_eq!(rt.state.tabs.len(), tabs_before + 1);
    assert_eq!(rt.state.tab().unwrap().page.title, "Linked Item");

    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
