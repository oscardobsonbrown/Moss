use super::*;
use moss_core::{PropertyKind, new_id};
use serde_json::json;
use std::path::PathBuf;

fn fixture() -> (Runtime<State>, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-advanced-ui-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_name.replace_text("Source");
    s.new_page(true, None);
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1800., 800.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.poll = Some(crate::records::poll);
    rt.frame(0.);
    (rt, path)
}
fn click(rt: &mut Runtime<State>, id: &str) {
    let (x, y) = rt.find(id).unwrap_or_else(|| panic!("missing {id}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}
fn click_visible(rt: &mut Runtime<State>, id: &str) {
    let item = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(id) && !e.exiting)
        .unwrap();
    let [x, y, w, h] = item.visible;
    assert!(w > 0. && h > 0., "{id} has no visible area");
    rt.pointer_down(x + w / 2., y + h / 2.);
    rt.pointer_up(x + w / 2., y + h / 2.);
    rt.frame(1. / 120.);
}
fn keypress(rt: &mut Runtime<State>, key: Key) {
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
fn type_choice(rt: &mut Runtime<State>, kind: &str) {
    let (x, y) = rt.find("property-type-text").unwrap();
    rt.wheel(x, y, -500.);
    rt.frame(1. / 120.);
    click(rt, &format!("property-type-{kind}"));
}
fn add_editor(rt: &mut Runtime<State>, name: &str, kind: &str) {
    click(rt, "table-add-property");
    settle(rt);
    click(rt, "property-name-input");
    keypress(rt, Key::Text(name.into()));
    type_choice(rt, kind);
}
fn current_db(rt: &Runtime<State>) -> moss_core::Database {
    rt.state.tab().unwrap().database.clone().unwrap()
}

#[test]
fn formula_validation_and_edit_refresh_visible_result() {
    let (mut rt, path) = fixture();
    let id = current_db(&rt).id;
    let db = rt
        .state
        .store
        .add_property(&id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db
        .properties
        .iter()
        .find(|p| p.name == "Amount")
        .unwrap()
        .id
        .clone();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.create_inline(Default::default(), false);
    let record = rt.state.tab().unwrap().records[0].id.clone();
    rt.state.finish_cell(false);
    rt.state
        .store
        .set_record_value_if_unchanged(&record, &amount, &serde_json::Value::Null, json!(7))
        .unwrap();
    rt.state.query_records();
    rt.frame(0.);
    add_editor(&mut rt, "Double", "formula");
    click(&mut rt, "formula-expression");
    keypress(&mut rt, Key::Text("prop(".into()));
    click(&mut rt, "property-save");
    assert!(
        rt.state
            .database_property_editor
            .as_ref()
            .unwrap()
            .error
            .is_some()
    );
    assert_eq!(
        rt.state
            .database_property_editor
            .as_ref()
            .unwrap()
            .advanced
            .expression
            .text,
        "prop("
    );
    click(&mut rt, "formula-expression");
    keypress(&mut rt, Key::Command('a'));
    keypress(&mut rt, Key::Text("prop(\"Amount\") * 2".into()));
    click(&mut rt, "property-save");
    settle(&mut rt);
    let formula = current_db(&rt)
        .properties
        .iter()
        .find(|p| p.name == "Double")
        .unwrap()
        .id
        .clone();
    let value = &rt.state.tab().unwrap().records[0].values[&formula];
    assert_eq!(value.as_f64(), Some(14.));
    click(&mut rt, &format!("cell-0-{amount}"));
    keypress(&mut rt, Key::Command('a'));
    keypress(&mut rt, Key::Text("9".into()));
    keypress(&mut rt, Key::Escape);
    settle(&mut rt);
    assert_eq!(
        rt.state.tab().unwrap().records[0].values[&formula].as_f64(),
        Some(18.)
    );
    let before = rt.state.editing_cell.is_some();
    click_visible(&mut rt, &format!("cell-0-{formula}"));
    assert_eq!(rt.state.editing_cell.is_some(), before);
    click_visible(&mut rt, &format!("table-header-{formula}"));
    settle(&mut rt);
    assert!(
        rt.state.database_property_editor.is_some(),
        "formula header did not open: {:?}",
        rt.state.error
    );
    assert_eq!(
        rt.state
            .database_property_editor
            .as_ref()
            .unwrap()
            .advanced
            .expression
            .text,
        "prop(\"Amount\") * 2"
    );
    click(&mut rt, "formula-expression");
    keypress(&mut rt, Key::Command('a'));
    keypress(&mut rt, Key::Text("prop(\"Amount\") * 3".into()));
    click(&mut rt, "property-save");
    settle(&mut rt);
    assert_eq!(
        rt.state.tab().unwrap().records[0].values[&formula].as_f64(),
        Some(27.)
    );
    drop(rt);
    let reopened = moss_core::Store::open(path.join("workspace.sqlite3")).unwrap();
    let db = reopened.get_database(&id).unwrap().unwrap();
    assert_eq!(
        db.properties
            .iter()
            .find(|p| p.id == formula)
            .unwrap()
            .config["expression"],
        "prop(\"Amount\") * 3"
    );
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn relation_and_rollup_update_across_databases() {
    let (mut rt, path) = fixture();
    let source_page = rt.state.tab().unwrap().page.id.clone();
    rt.state.new_name.replace_text("Target");
    rt.state.new_page(true, None);
    let target_page = rt.state.tab().unwrap().page.id.clone();
    let target_id = current_db(&rt).id;
    let db = rt
        .state
        .store
        .add_property(&target_id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db
        .properties
        .iter()
        .find(|p| p.name == "Amount")
        .unwrap()
        .id
        .clone();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.create_inline(Default::default(), false);
    let target_record = rt.state.tab().unwrap().records[0].id.clone();
    rt.state.cell.replace_text("Target row");
    rt.state.finish_cell(false);
    rt.state
        .store
        .set_record_value_if_unchanged(&target_record, &amount, &serde_json::Value::Null, json!(7))
        .unwrap();
    rt.state.query_records();
    rt.state.open_page(&source_page, false);
    rt.state.create_inline(Default::default(), false);
    rt.state.cell.replace_text("Source row");
    rt.state.finish_cell(false);
    rt.frame(0.);

    add_editor(&mut rt, "Link", "relation");
    click(&mut rt, &format!("relation-database-{target_id}"));
    click(&mut rt, "property-save");
    settle(&mut rt);
    let relation = current_db(&rt)
        .properties
        .iter()
        .find(|p| p.name == "Link")
        .unwrap()
        .id
        .clone();
    click(&mut rt, &format!("cell-0-{relation}"));
    settle(&mut rt);
    click(&mut rt, &format!("relation-record-{target_record}"));
    let source_record = rt.state.tab().unwrap().records[0].id.clone();
    let expected_property = current_db(&rt)
        .properties
        .into_iter()
        .find(|p| p.id == relation)
        .unwrap();
    rt.state
        .store
        .set_relation_value_if_unchanged(
            &source_record,
            &expected_property,
            &serde_json::Value::Null,
            &[],
        )
        .unwrap();
    click(&mut rt, "relation-apply");
    assert!(rt.state.relation_picker().unwrap().save_failed);
    rt.update(|s, tx| {
        s.open_page(&target_page, false);
        tx.instant();
    });
    rt.frame(0.);
    assert_eq!(rt.state.tab().unwrap().page.id, source_page);
    assert!(rt.find("relation-picker").is_some());
    click(&mut rt, "relation-discard");
    click(&mut rt, &format!("cell-0-{relation}"));
    settle(&mut rt);
    click(&mut rt, &format!("relation-record-{target_record}"));
    click(&mut rt, "relation-apply");
    settle(&mut rt);
    assert_eq!(
        rt.state.tab().unwrap().records[0].values[&relation],
        json!([target_record])
    );

    let target_tab = rt
        .state
        .tabs
        .iter()
        .position(|tab| tab.page.id == target_page)
        .unwrap();
    rt.update(|s, tx| {
        s.close_tab(target_tab);
        tx.instant();
    });
    rt.frame(1. / 120.);
    assert!(!rt.state.tabs.iter().any(|tab| tab.page.id == target_page));
    let cell = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&format!("cell-0-{relation}")))
        .unwrap();
    let [x, y, w, h] = cell.visible;
    assert!(w > 0. && h > 0.);
    rt.wheel_xy(x + w / 2., y + h / 2., -250., 0.);
    rt.frame(1. / 120.);
    assert!(rt.elements().iter().any(|e| e.key.as_deref()
        == Some(&format!("relation-link-text-{relation}-0"))
        && e.text.as_deref() == Some("Target row")
        && e.visible[2] > 0.
        && e.visible[2] + 1. >= e.rect[2]
        && e.visible[3] > 0.));
    rt.update(|s, tx| {
        s.store
            .rename_record(&target_record, "Renamed target")
            .unwrap();
        tx.instant();
    });
    rt.frame(1. / 120.);
    assert!(rt.elements().iter().any(|e| e.key.as_deref()
        == Some(&format!("relation-link-text-{relation}-0"))
        && e.text.as_deref() == Some("Renamed target")
        && e.visible[2] > 0.
        && e.visible[2] + 1. >= e.rect[2]
        && e.visible[3] > 0.));

    add_editor(&mut rt, "Total", "rollup");
    click(&mut rt, &format!("rollup-relation-{relation}"));
    let first_property = rt
        .state
        .database_property_editor
        .as_ref()
        .unwrap()
        .advanced
        .target_properties[0]
        .id
        .clone();
    let (x, y) = rt
        .find(&format!("rollup-property-{first_property}"))
        .unwrap();
    rt.wheel(x, y, -400.);
    rt.frame(1. / 120.);
    click(&mut rt, &format!("rollup-property-{amount}"));
    assert_eq!(
        rt.state
            .database_property_editor
            .as_ref()
            .unwrap()
            .advanced
            .target_property_id
            .as_deref(),
        Some(amount.as_str())
    );
    let (x, y) = rt.find("rollup-operation-original").unwrap();
    rt.wheel(x, y, -160.);
    rt.frame(1. / 120.);
    click(&mut rt, "rollup-operation-sum");
    assert_eq!(
        rt.state
            .database_property_editor
            .as_ref()
            .unwrap()
            .advanced
            .operation,
        "sum"
    );
    click(&mut rt, "property-save");
    settle(&mut rt);
    assert!(
        rt.state.database_property_editor.is_none(),
        "rollup save failed: {:?}",
        rt.state
            .database_property_editor
            .as_ref()
            .and_then(|e| e.error.as_ref())
    );
    let total = current_db(&rt)
        .properties
        .iter()
        .find(|p| p.name == "Total")
        .unwrap()
        .id
        .clone();
    assert_eq!(
        rt.state.tab().unwrap().records[0].values[&total].as_f64(),
        Some(7.)
    );

    rt.update(|s, tx| {
        s.open_page(&target_page, false);
        tx.instant();
    });
    rt.frame(0.);
    click(&mut rt, &format!("cell-0-{amount}"));
    keypress(&mut rt, Key::Command('a'));
    keypress(&mut rt, Key::Text("9".into()));
    keypress(&mut rt, Key::Escape);
    rt.update(|s, tx| {
        s.open_page(&source_page, false);
        tx.instant();
    });
    rt.frame(0.);
    settle(&mut rt);
    assert_eq!(
        rt.state.tab().unwrap().records[0].values[&total].as_f64(),
        Some(9.)
    );
    rt.update(|s, tx| {
        s.store.delete_record(&target_record).unwrap();
        tx.instant();
    });
    rt.frame(1. / 120.);
    assert!(
        !rt.state
            .tab()
            .unwrap()
            .relation_titles
            .contains_key(&target_record)
    );
    assert!(rt.elements().iter().any(|e| e.key.as_deref()
        == Some(&format!("relation-link-text-{relation}-0"))
        && e.text.as_deref() == Some("1 linked page")));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn visible_calculation_refresh_preserves_the_active_cell_draft() {
    let (mut rt, path) = fixture();
    let db_id = current_db(&rt).id;
    let db = rt
        .state
        .store
        .add_property(&db_id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db
        .properties
        .iter()
        .find(|p| p.name == "Amount")
        .unwrap()
        .id
        .clone();
    let db = rt
        .state
        .store
        .save_advanced_property(
            &db_id,
            None,
            "Double",
            PropertyKind::Formula,
            json!({"expression":"prop(\"Amount\") * 2"}),
            None,
        )
        .unwrap();
    let formula = db
        .properties
        .iter()
        .find(|p| p.name == "Double")
        .unwrap()
        .id
        .clone();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.create_inline(Default::default(), false);
    let first = rt.state.tab().unwrap().records[0].id.clone();
    rt.state.cell.replace_text("First");
    rt.state.finish_cell(false);
    rt.state.create_inline(Default::default(), false);
    let second = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .find(|r| r.id != first)
        .unwrap()
        .id
        .clone();
    rt.state.cell.replace_text("Second");
    rt.state.finish_cell(false);
    for id in [&first, &second] {
        rt.state
            .store
            .set_record_value_if_unchanged(id, &amount, &serde_json::Value::Null, json!(7))
            .unwrap();
    }
    rt.state.query_records();
    rt.update(|_, tx| tx.instant());
    rt.frame(0.);
    let index = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .position(|r| r.id == first)
        .unwrap();
    click(&mut rt, &format!("cell-{index}-{amount}"));
    let selected = rt.state.editing_cell.as_ref().unwrap().record_id.clone();
    let text = rt.state.cell.text.clone();
    rt.update(|s, tx| {
        s.store
            .set_record_value_if_unchanged(&second, &amount, &json!(7), json!(9))
            .unwrap();
        tx.instant();
    });
    rt.frame(1. / 120.);
    assert_eq!(rt.state.editing_cell.as_ref().unwrap().record_id, selected);
    assert_eq!(rt.state.cell.text, text);
    assert!(rt.state.cell.focused);
    assert!(rt.find("database-cell-editor").is_some());
    let updated = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .find(|r| r.id == second)
        .unwrap();
    assert_eq!(updated.values[&formula].as_f64(), Some(18.));
    assert!(rt.state.computed_refresh_deferred);
    let count = rt.state.store.local_change_count();
    assert!(!crate::computed_refresh::poll(&mut rt.state));
    assert_eq!(rt.state.store.local_change_count(), count);
    keypress(&mut rt, Key::Escape);
    rt.frame(1. / 120.);
    assert!(!rt.state.computed_refresh_deferred);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn refresh_keeps_each_open_database_title_search() {
    let (mut rt, path) = fixture();
    let source_page = rt.state.tab().unwrap().page.id.clone();
    for title in ["Alpha keep", "Alpha other"] {
        rt.state.create_inline(Default::default(), false);
        rt.state.cell.replace_text(title);
        rt.state.finish_cell(false);
    }
    rt.state.filter.replace_text("keep");
    rt.state.query_records();
    rt.state.new_name.replace_text("Target");
    rt.state.new_page(true, None);
    let target_page = rt.state.tab().unwrap().page.id.clone();
    let mut target_other = String::new();
    for title in ["Beta keep", "Beta other"] {
        rt.state.create_inline(Default::default(), false);
        let id = rt.state.editing_cell.as_ref().unwrap().record_id.clone();
        if title == "Beta other" {
            target_other = id;
        }
        rt.state.cell.replace_text(title);
        rt.state.finish_cell(false);
    }
    rt.state.filter.replace_text("Beta");
    rt.state.query_records();
    rt.state.open_page(&source_page, false);
    rt.update(|s, tx| {
        s.store
            .rename_record(&target_other, "Gamma changed")
            .unwrap();
        tx.instant();
    });
    rt.frame(1. / 120.);
    let source = rt
        .state
        .tabs
        .iter()
        .find(|t| t.page.id == source_page)
        .unwrap();
    let target = rt
        .state
        .tabs
        .iter()
        .find(|t| t.page.id == target_page)
        .unwrap();
    assert_eq!(source.title_search, "keep");
    assert_eq!(
        source
            .records
            .iter()
            .map(|r| r.title.as_str())
            .collect::<Vec<_>>(),
        ["Alpha keep"]
    );
    assert_eq!(target.title_search, "Beta");
    assert_eq!(
        target
            .records
            .iter()
            .map(|r| r.title.as_str())
            .collect::<Vec<_>>(),
        ["Beta keep"]
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn refresh_updates_peek_values_without_replacing_unsaved_page_edits() {
    let (mut rt, path) = fixture();
    let db_id = current_db(&rt).id;
    let db = rt
        .state
        .store
        .add_property(&db_id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db
        .properties
        .iter()
        .find(|p| p.name == "Amount")
        .unwrap()
        .id
        .clone();
    let db = rt
        .state
        .store
        .save_advanced_property(
            &db_id,
            None,
            "Double",
            PropertyKind::Formula,
            json!({"expression":"prop(\"Amount\") * 2"}),
            None,
        )
        .unwrap();
    let formula = db
        .properties
        .iter()
        .find(|p| p.name == "Double")
        .unwrap()
        .id
        .clone();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.create_inline(Default::default(), false);
    let id = rt.state.tab().unwrap().records[0].id.clone();
    rt.state.cell.replace_text("Saved title");
    rt.state.finish_cell(false);
    rt.state.open_record(0);
    rt.update(|s, tx| {
        let peek = s.record_peek.as_mut().unwrap();
        peek.title.replace_text("Unsaved title");
        peek.dirty_title = true;
        peek.blocks[0].edit.replace_text("Unsaved body");
        peek.dirty_blocks = true;
        s.store
            .set_record_value_if_unchanged(&id, &amount, &serde_json::Value::Null, json!(7))
            .unwrap();
        tx.instant();
    });
    rt.frame(1. / 120.);
    let peek = rt.state.record_peek.as_ref().unwrap();
    assert_eq!(peek.record.values[&formula].as_f64(), Some(14.));
    assert_eq!(peek.title.text, "Unsaved title");
    assert_eq!(peek.blocks[0].edit.text, "Unsaved body");
    assert!(peek.dirty_title && peek.dirty_blocks);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
