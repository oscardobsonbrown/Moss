use super::*;
use std::path::PathBuf;

fn fixture() -> (State, PathBuf, String, String, String) {
    let path = std::env::temp_dir().join(format!("moss-record-atomic-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    let mut db = s.tab().unwrap().database.clone().unwrap();
    let text_id = new_id();
    let checkbox_id = new_id();
    db.properties.push(Property {
        id: text_id.clone(),
        name: "Notes".into(),
        kind: PropertyKind::Text,
        config: json!({}),
    });
    db.properties.push(Property {
        id: checkbox_id.clone(),
        name: "Done".into(),
        kind: PropertyKind::Checkbox,
        config: json!({}),
    });
    s.store.save_database(&db).unwrap();
    s.tabs[s.active].database = Some(db);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let id = s.tab().unwrap().records[0].id.clone();
    (s, path, id, text_id, checkbox_id)
}

#[test]
fn scalar_autosaves_preserve_other_properties_and_hold_conflicting_draft() {
    let (mut s, path, id, text, checkbox) = fixture();
    let mut other = Store::open(path.join("workspace.sqlite3")).unwrap();
    s.edit_record_property(&id, Some(text.clone()));
    s.cell.replace_text("first");
    assert!(s.save_cell());
    other
        .set_record_value_if_unchanged(&id, &checkbox, &Value::Null, json!(true))
        .unwrap();
    s.cell.replace_text("second");
    assert!(s.save_cell());
    let saved = other.get_record(&id).unwrap().unwrap();
    assert_eq!(saved.values[&text], "second");
    assert_eq!(saved.values[&checkbox], true);
    assert_eq!(s.tab().unwrap().records[0].values[&checkbox], true);
    other
        .set_record_value_if_unchanged(&id, &text, &json!("second"), json!("other window"))
        .unwrap();
    s.cell.replace_text("my draft");
    assert!(!s.save_cell());
    assert!(!s.finish_cell(false));
    assert_eq!(s.cell.text, "my draft");
    assert_eq!(
        s.editing_cell.as_ref().unwrap().expected_value,
        json!("second")
    );
    assert_eq!(
        other.get_record(&id).unwrap().unwrap().values[&text],
        "other window"
    );
    drop(other);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn checkbox_uses_the_displayed_value_and_reports_a_conflict() {
    let (mut s, path, id, text, checkbox) = fixture();
    let mut other = Store::open(path.join("workspace.sqlite3")).unwrap();
    other
        .set_record_value_if_unchanged(&id, &text, &Value::Null, json!("other edit"))
        .unwrap();
    s.toggle_table_checkbox(&id, &checkbox);
    let saved = other.get_record(&id).unwrap().unwrap();
    assert_eq!(saved.values[&checkbox], true);
    assert_eq!(saved.values[&text], "other edit");
    assert_eq!(s.tab().unwrap().records[0].values[&text], "other edit");
    other
        .set_record_value_if_unchanged(&id, &checkbox, &json!(true), json!(false))
        .unwrap();
    s.toggle_table_checkbox(&id, &checkbox);
    assert!(s.error.is_some());
    assert_eq!(
        other.get_record(&id).unwrap().unwrap().values[&checkbox],
        false
    );
    assert_eq!(s.tab().unwrap().records[0].values[&checkbox], true);
    drop(other);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn status_picker_keeps_conflicting_choice_until_discarded() {
    let (mut s, path, id, _, _) = fixture();
    let status = s
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let mut other = Store::open(path.join("workspace.sqlite3")).unwrap();
    s.open_property_picker(&id, &status);
    other
        .set_record_value_if_unchanged(&id, &status, &json!("Not started"), json!("Done"))
        .unwrap();
    assert!(!s.choose_property_value("In progress"));
    assert!(s.property_picker().unwrap().save_failed);
    assert_eq!(s.cell.text, "In progress");
    assert_eq!(
        s.editing_cell.as_ref().unwrap().expected_value,
        json!("Not started")
    );
    s.dismiss_cell_picker();
    assert!(s.property_picker().is_some());
    assert_eq!(
        other.get_record(&id).unwrap().unwrap().values[&status],
        "Done"
    );
    other.delete_record(&id).unwrap();
    s.discard_failed_property_change();
    assert!(s.property_picker().is_some());
    assert_eq!(s.cell.text, "In progress");
    assert!(
        s.property_picker()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("Could not reload")
    );
    other.restore_record(&id).unwrap();
    s.discard_failed_property_change();
    assert!(s.property_picker().is_none());
    assert_eq!(s.tab().unwrap().records[0].values[&status], "Done");
    drop(other);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn inline_conflict_retains_draft_until_user_reloads() {
    let (mut s, path, id, text, _) = fixture();
    let mut other = Store::open(path.join("workspace.sqlite3")).unwrap();
    s.edit_record_property(&id, Some(text.clone()));
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(crate::key);
    rt.frame(0.);
    other
        .set_record_value_if_unchanged(&id, &text, &Value::Null, json!("other window"))
        .unwrap();
    rt.key(rune::KeyEvent {
        key: rune::Key::Text("my draft".into()),
        mods: Default::default(),
    });
    rt.frame(1. / 120.);
    assert_eq!(rt.state.cell.text, "my draft");
    assert!(rt.state.editing_cell.as_ref().unwrap().save_failed);
    let click = |rt: &mut rune::Runtime<State>, key: &str| {
        let (x, y) = rt.find(key).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
    };
    click(&mut rt, "cell-retry");
    assert_eq!(rt.state.cell.text, "my draft");
    assert!(rt.state.editing_cell.as_ref().unwrap().save_failed);
    assert_eq!(
        other.get_record(&id).unwrap().unwrap().values[&text],
        "other window"
    );
    click(&mut rt, "cell-discard");
    assert!(rt.state.editing_cell.is_none());
    assert_eq!(
        rt.state.tab().unwrap().records[0].values[&text],
        "other window"
    );
    assert!(rt.state.select_database_view(1));
    drop(rt);
    drop(other);
    std::fs::remove_dir_all(path).unwrap();
}
