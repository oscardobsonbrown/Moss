use super::*;
use std::collections::BTreeMap;

fn belongs_to(rt: &Runtime<State>, child_key: &str, parent_key: &str) -> bool {
    let nodes = rt.elements();
    let Some(mut child) = nodes
        .iter()
        .position(|node| node.key.as_deref() == Some(child_key))
    else {
        return false;
    };
    while let Some(parent) = nodes[child].parent {
        if nodes[parent].key.as_deref() == Some(parent_key) {
            return true;
        }
        child = parent;
    }
    false
}

#[test]
fn board_uses_saved_select_group_and_creates_repeat_cards_in_that_group() {
    let (mut rt, path) = database_runtime(1);
    let id = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .id
        .clone();
    let mut db = rt
        .state
        .store
        .add_select_property(&id, "Priority", &["Alpha".into(), "Beta".into()])
        .unwrap();
    let group = db.properties.last().unwrap().id.clone();
    db.views[1].query.group_by = Some(group.clone());
    rt.state.store.save_database(&db).unwrap();
    let first = rt
        .state
        .store
        .create_record(&id, "A", &BTreeMap::from([(group.clone(), json!("Alpha"))]))
        .unwrap();
    let second = rt
        .state
        .store
        .create_record(&id, "B", &BTreeMap::from([(group.clone(), json!("Beta"))]))
        .unwrap();
    let empty = rt
        .state
        .store
        .create_record(&id, "Empty", &BTreeMap::new())
        .unwrap();
    rt.state.tabs[rt.state.active].database = Some(db);
    assert!(rt.state.query_records());
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("board-new-Alpha").is_some());
    assert!(rt.find("board-new-Beta").is_some());
    assert!(rt.find("board-new-").is_some());
    assert!(rt.find("board-new-In progress").is_none());
    for (record, column) in [(&first, 0), (&second, 1), (&empty, 2)] {
        assert!(belongs_to(
            &rt,
            &format!("board-card-{}", record.id),
            &format!("board-group-{column}")
        ));
    }
    click(&mut rt, "board-new-Alpha");
    let created = rt.state.editing_cell.as_ref().unwrap().record_id.clone();
    assert_eq!(
        rt.state.store.get_record(&created).unwrap().unwrap().values[&group],
        json!("Alpha")
    );
    press(&mut rt, Key::Text("First".into()));
    press(&mut rt, Key::Enter);
    let repeated = rt.state.editing_cell.as_ref().unwrap().record_id.clone();
    assert_eq!(
        rt.state
            .store
            .get_record(&repeated)
            .unwrap()
            .unwrap()
            .values[&group],
        json!("Alpha")
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn board_group_creation_keeps_scalar_value_types_and_empty_values() {
    for (kind, value, key) in [
        (PropertyKind::Number, json!(42), "board-new-42"),
        (PropertyKind::Checkbox, json!(true), "board-new-true"),
        (
            PropertyKind::Date,
            json!({"start":"2026-11-08","end":"2026-11-09"}),
            "board-new-8 November 2026 → 9 November 2026",
        ),
    ] {
        let (mut rt, path) = database_runtime(1);
        let id = rt
            .state
            .tab()
            .unwrap()
            .database
            .as_ref()
            .unwrap()
            .id
            .clone();
        let mut db = rt.state.store.add_property(&id, "Group", kind).unwrap();
        let property = db.properties.last().unwrap().id.clone();
        db.views[1].query.group_by = Some(property.clone());
        rt.state.store.save_database(&db).unwrap();
        rt.state
            .store
            .create_record(
                &id,
                "Existing",
                &BTreeMap::from([(property.clone(), value.clone())]),
            )
            .unwrap();
        rt.state.tabs[rt.state.active].database = Some(db);
        assert!(rt.state.query_records());
        rt.invalidate();
        rt.frame(0.);
        click(&mut rt, key);
        let created = rt.state.editing_cell.as_ref().unwrap().record_id.clone();
        assert_eq!(
            rt.state.store.get_record(&created).unwrap().unwrap().values[&property],
            value
        );
        assert!(rt.state.finish_cell(false));
        rt.invalidate();
        rt.frame(0.);
        click(&mut rt, "board-new-");
        let empty = rt.state.editing_cell.as_ref().unwrap().record_id.clone();
        assert!(rt.state.store.get_record(&empty).unwrap().unwrap().values[&property].is_null());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn board_does_not_silently_replace_missing_or_unsupported_grouping() {
    let (mut rt, path) = database_runtime(1);
    let mut db = rt.state.tab().unwrap().database.clone().unwrap();
    for group in [
        "missing".to_owned(),
        db.properties
            .iter()
            .find(|p| p.kind == PropertyKind::Title)
            .unwrap()
            .id
            .clone(),
    ] {
        db.views[1].query.group_by = Some(group);
        rt.state.tabs[rt.state.active].database = Some(db.clone());
        rt.invalidate();
        rt.frame(0.);
        assert!(rt.elements().iter().any(|node| {
            node.text
                .as_deref()
                .is_some_and(|text| text.contains("group") || text.contains("Board"))
        }));
        assert!(rt.find("board-new-In progress").is_none());
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
