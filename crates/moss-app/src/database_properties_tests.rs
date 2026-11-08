use super::*;
use moss_core::PropertyKind;

#[test]
fn property_changes_keep_failed_drafts_on_conflict() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db_id = s.tab().unwrap().database.as_ref().unwrap().id.clone();
    s.open_database_property(None);
    s.database_property_input.replace_text("Priority");
    s.database_property_editor.as_mut().unwrap().kind = PropertyKind::Text;
    assert!(s.save_database_property());
    assert!(s.database_property_editor.is_none());
    let added = s
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.name == "Priority")
        .unwrap()
        .clone();
    s.open_database_property(Some(&added.id));
    s.database_property_input.replace_text("Draft rename");
    s.store
        .rename_property(&db_id, &added, "Changed elsewhere")
        .unwrap();
    assert!(!s.save_database_property());
    let editor = s.database_property_editor.as_ref().unwrap();
    assert_eq!(s.database_property_input.text, "Draft rename");
    assert!(editor.error.is_some());
    s.close_database_property();
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn title_property_delete_is_rejected_and_keeps_editor_open() {
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
    assert!(!s.delete_database_property());
    assert!(s.database_property_editor.as_ref().unwrap().error.is_some());
    assert!(
        s.tab()
            .unwrap()
            .database
            .as_ref()
            .unwrap()
            .properties
            .iter()
            .any(|p| p.id == title)
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn select_options_are_validated_without_discarding_draft() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.open_database_property(None);
    s.database_property_input.replace_text("Priority");
    s.database_property_editor.as_mut().unwrap().kind = PropertyKind::Select;
    s.database_property_options_input
        .replace_text("High, , Low");
    assert!(!s.save_database_property());
    assert_eq!(s.database_property_input.text, "Priority");
    assert_eq!(s.database_property_options_input.text, "High, , Low");
    assert!(s.database_property_editor.as_ref().unwrap().error.is_some());
    s.database_property_options_input.replace_text("High, Low");
    assert!(s.save_database_property());
    let property = s
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.name == "Priority")
        .unwrap();
    assert_eq!(property.kind, PropertyKind::Select);
    assert_eq!(property.config["options"].as_array().unwrap().len(), 2);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn deleting_a_filtered_property_refreshes_inactive_database_tabs() {
    use moss_core::{FilterExpr, FilterOp, FilterRule};
    use serde_json::json;
    use std::collections::BTreeMap;

    let (mut s, path) = fixture();
    s.new_page(true, None);
    let first = s.active;
    let db_id = s.tab().unwrap().database.as_ref().unwrap().id.clone();
    let mut db = s
        .store
        .add_property(&db_id, "Priority", PropertyKind::Text)
        .unwrap();
    let property = db
        .properties
        .iter()
        .find(|p| p.name == "Priority")
        .unwrap()
        .clone();
    let mut values = BTreeMap::new();
    values.insert(property.id.clone(), json!("High"));
    s.store.create_record(&db_id, "One", &values).unwrap();
    values.insert(property.id.clone(), json!("Low"));
    s.store.create_record(&db_id, "Two", &values).unwrap();
    db.views[0].query.filter = Some(FilterExpr::Rule(FilterRule {
        property_id: property.id.clone(),
        op: FilterOp::Eq,
        value: json!("High"),
    }));
    s.store.save_database(&db).unwrap();
    s.tabs[first].database = Some(db.clone());
    assert!(s.query_records());
    assert_eq!(s.tabs[first].records.len(), 1);
    let original_page = s.tabs[first].page.clone();
    s.new_page(true, None);
    let second = s.active;
    s.tabs[second].page = original_page;
    s.tabs[second].database = Some(db);
    assert!(s.query_records());
    assert_eq!(s.tabs[second].records.len(), 1);

    s.open_database_property(Some(&property.id));
    assert!(s.delete_database_property());
    assert_eq!(s.active, second);
    for index in [first, second] {
        assert_eq!(
            s.tabs[index].records.len(),
            2,
            "tab {index} must use its pruned filter"
        );
        assert!(
            s.tabs[index]
                .records
                .iter()
                .all(|r| !r.values.contains_key(&property.id))
        );
        assert!(
            s.tabs[index].database.as_ref().unwrap().views[0]
                .query
                .filter
                .is_none()
        );
    }
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn property_change_refresh_keeps_each_tabs_title_search() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let first = s.active;
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    s.store
        .create_record(&db.id, "Alpha", &Default::default())
        .unwrap();
    s.store
        .create_record(&db.id, "Beta", &Default::default())
        .unwrap();
    s.filter.replace_text("Alpha");
    assert!(s.query_records());
    let page = s.tabs[first].page.clone();
    s.new_page(true, None);
    let second = s.active;
    s.tabs[second].page = page;
    s.tabs[second].database = Some(db);
    s.filter.replace_text("Beta");
    assert!(s.query_records());
    s.filter.set_selection(0, 2);
    s.open_database_property(Some(&title));
    s.set_table_config("wrap_cells", json!(true));
    assert_eq!(s.tabs[first].title_search, "Alpha");
    assert_eq!(s.tabs[second].title_search, "Beta");
    assert_eq!(s.tabs[first].records[0].title, "Alpha");
    assert_eq!(s.tabs[second].records[0].title, "Beta");
    assert_eq!(s.filter.text, "Beta");
    assert_eq!(s.filter.range(), (0, 2));
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
