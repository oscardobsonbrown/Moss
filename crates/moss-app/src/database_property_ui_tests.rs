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
