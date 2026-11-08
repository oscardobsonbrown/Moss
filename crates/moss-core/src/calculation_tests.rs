use crate::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn database(s: &mut Store, workspace: &Workspace, name: &str) -> Database {
    let definition = Database {
        id: new_id(),
        page_id: String::new(),
        name: name.into(),
        properties: vec![Property {
            id: new_id(),
            name: "Name".into(),
            kind: PropertyKind::Title,
            config: json!({}),
        }],
        views: vec![DatabaseView {
            id: new_id(),
            name: "Table".into(),
            kind: ViewKind::Table,
            query: RecordQuery::default(),
            config: json!({}),
        }],
    };
    let page = s
        .create_database_page(&workspace.id, None, name, "", definition)
        .unwrap();
    s.get_database_by_page(&page.id).unwrap().unwrap()
}
#[test]
fn formulas_rollups_recalculate_across_relations_and_survive_renames_trash_and_reopen() {
    let path = std::env::temp_dir().join(format!("moss-calculation-{}.sqlite", new_id()));
    let mut s = Store::open(&path).unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let target = database(&mut s, &w, "Target");
    let target = s
        .add_property(&target.id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = target.properties[1].clone();
    let target = s
        .save_advanced_property(
            &target.id,
            None,
            "Double",
            PropertyKind::Formula,
            json!({"expression":"prop(\"Amount\") * 2"}),
            None,
        )
        .unwrap();
    let double = target.properties[2].clone();
    let row = s
        .create_record(
            &target.id,
            "Item",
            &BTreeMap::from([(amount.id.clone(), json!(7))]),
        )
        .unwrap();
    assert_eq!(row.values[&double.id], json!(14.0));
    let source = database(&mut s, &w, "Source");
    let source = s
        .save_advanced_property(
            &source.id,
            None,
            "Linked",
            PropertyKind::Relation,
            json!({"target_database_id": target.id}),
            None,
        )
        .unwrap();
    let relation = source.properties[1].clone();
    let source = s.save_advanced_property(&source.id, None, "Total", PropertyKind::Rollup, json!({"relation_property_id":relation.id, "target_property_id":double.id, "operation":"sum"}), None).unwrap();
    let total = source.properties[2].clone();
    let source_row = s
        .create_record(&source.id, "Source row", &BTreeMap::new())
        .unwrap();
    s.set_relation_value_if_unchanged(&source_row.id, &relation, &Value::Null, &[row.id.clone()])
        .unwrap();
    assert_eq!(
        s.get_record(&source_row.id).unwrap().unwrap().values[&total.id],
        json!(14.0)
    );
    s.set_record_value_if_unchanged(&row.id, &amount.id, &json!(7), json!(9))
        .unwrap();
    assert_eq!(
        s.get_record(&source_row.id).unwrap().unwrap().values[&total.id],
        json!(18.0)
    );
    let target = s.rename_property(&target.id, &amount, "Renamed").unwrap();
    let existing_formula = target
        .properties
        .iter()
        .find(|p| p.id == double.id)
        .unwrap()
        .clone();
    s.save_advanced_property(
        &target.id,
        Some(&existing_formula),
        "Double renamed",
        PropertyKind::Formula,
        json!({"expression":"prop(\"Amount\") * 2"}),
        None,
    )
    .unwrap();
    s.set_record_value_if_unchanged(&row.id, &amount.id, &json!(9), json!(10))
        .unwrap();
    assert_eq!(
        s.get_record(&source_row.id).unwrap().unwrap().values[&total.id],
        json!(20.0)
    );
    s.delete_record(&row.id).unwrap();
    assert!(s.get_record(&source_row.id).unwrap().unwrap().values[&total.id].is_null());
    s.restore_record(&row.id).unwrap();
    assert_eq!(
        s.get_record(&source_row.id).unwrap().unwrap().values[&total.id],
        json!(20.0)
    );
    let query = RecordQuery {
        filter: Some(FilterExpr::Rule(FilterRule {
            property_id: total.id.clone(),
            op: FilterOp::Gt,
            value: json!(15),
        })),
        ..RecordQuery::default()
    };
    assert_eq!(s.query_records(&source.id, &query).unwrap().len(), 1);
    drop(s);
    let s = Store::open(&path).unwrap();
    assert_eq!(
        s.get_record(&source_row.id).unwrap().unwrap().values[&total.id],
        json!(20.0)
    );
    drop(s);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cycles_are_rejected_and_formula_errors_propagate_after_property_deletion() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let db = database(&mut s, &w, "Test");
    let db = s
        .add_property(&db.id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db.properties[1].clone();
    let db = s
        .save_advanced_property(
            &db.id,
            None,
            "A",
            PropertyKind::Formula,
            json!({"expression":"prop(\"Amount\") * 2"}),
            None,
        )
        .unwrap();
    let a = db.properties[2].clone();
    let db = s
        .save_advanced_property(
            &db.id,
            None,
            "B",
            PropertyKind::Formula,
            json!({"expression":"prop(\"A\") + 1"}),
            None,
        )
        .unwrap();
    let b = db.properties[3].clone();
    let row = s
        .create_record(
            &db.id,
            "row",
            &BTreeMap::from([(amount.id.clone(), json!(3))]),
        )
        .unwrap();
    assert_eq!(row.values[&b.id], json!(7.0));
    assert!(
        s.save_advanced_property(
            &db.id,
            Some(&a),
            "A",
            PropertyKind::Formula,
            json!({"expression":"prop(\"B\") + 1"}),
            None
        )
        .is_err()
    );
    assert_eq!(s.get_database(&db.id).unwrap().unwrap(), db);
    s.delete_property(&db.id, &amount).unwrap();
    let row = s.get_record(&row.id).unwrap().unwrap();
    assert!(row.values[&a.id].get("_moss_error").is_some());
    assert!(row.values[&b.id].get("_moss_error").is_some());
}

#[test]
fn relations_reject_wrong_targets_and_schema_changes_without_writing() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let target = database(&mut s, &w, "Target");
    let other = database(&mut s, &w, "Other");
    let source = database(&mut s, &w, "Source");
    let source = s
        .save_advanced_property(
            &source.id,
            None,
            "Link",
            PropertyKind::Relation,
            json!({"target_database_id": target.id}),
            None,
        )
        .unwrap();
    let relation = source.properties[1].clone();
    let row = s
        .create_record(&source.id, "row", &BTreeMap::new())
        .unwrap();
    let wrong = s
        .create_record(&other.id, "wrong", &BTreeMap::new())
        .unwrap();
    assert!(
        s.set_relation_value_if_unchanged(&row.id, &relation, &Value::Null, &[wrong.id])
            .is_err()
    );
    assert!(
        s.get_record(&row.id)
            .unwrap()
            .unwrap()
            .values
            .get(&relation.id)
            .is_none()
    );
    let target_row = s
        .create_record(&target.id, "target", &BTreeMap::new())
        .unwrap();
    s.set_relation_value_if_unchanged(&row.id, &relation, &Value::Null, &[target_row.id])
        .unwrap();
    assert!(
        s.save_advanced_property(
            &source.id,
            Some(&relation),
            "Link",
            PropertyKind::Relation,
            json!({"target_database_id": other.id}),
            None
        )
        .is_err()
    );
    let mut changed = source.clone();
    changed.properties[1].config["target_database_id"] = json!(other.id);
    assert!(s.save_database(&changed).is_err());
    changed = source.clone();
    changed.properties[1].kind = PropertyKind::Text;
    assert!(s.save_database(&changed).is_err());
}

#[test]
fn error_values_do_not_match_numeric_filters_and_sort_after_numbers() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let db = database(&mut s, &w, "Test");
    let db = s
        .add_property(&db.id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db.properties[1].clone();
    let db = s
        .save_advanced_property(
            &db.id,
            None,
            "Divide",
            PropertyKind::Formula,
            json!({"expression":"10 / prop(\"Amount\")"}),
            None,
        )
        .unwrap();
    let formula = db.properties[2].clone();
    let bad = s
        .create_record(
            &db.id,
            "bad",
            &BTreeMap::from([(amount.id.clone(), json!(0))]),
        )
        .unwrap();
    let good = s
        .create_record(
            &db.id,
            "good",
            &BTreeMap::from([(amount.id.clone(), json!(2))]),
        )
        .unwrap();
    assert!(bad.values[&formula.id].get("_moss_error").is_some());
    let filter = FilterExpr::Rule(FilterRule {
        property_id: formula.id.clone(),
        op: FilterOp::Gt,
        value: json!(1),
    });
    let filtered = s
        .query_records(
            &db.id,
            &RecordQuery {
                filter: Some(filter),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        filtered.iter().map(|r| &r.id).collect::<Vec<_>>(),
        vec![&good.id]
    );
    let sorted = s
        .query_records(
            &db.id,
            &RecordQuery {
                sorts: vec![SortRule {
                    property_id: formula.id,
                    descending: true,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        sorted.iter().map(|r| &r.id).collect::<Vec<_>>(),
        vec![&good.id, &bad.id]
    );
}

#[test]
fn propagation_failure_rolls_back_source_and_dependent_values() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let db = database(&mut s, &w, "Test");
    let db = s
        .add_property(&db.id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db.properties[1].clone();
    let db = s
        .save_advanced_property(
            &db.id,
            None,
            "Double",
            PropertyKind::Formula,
            json!({"expression":"prop(\"Amount\") * 2"}),
            None,
        )
        .unwrap();
    let computed = db.properties[2].clone();
    let row = s
        .create_record(
            &db.id,
            "row",
            &BTreeMap::from([(amount.id.clone(), json!(3))]),
        )
        .unwrap();
    // Reject the second update: input value was written first, computed value is written second.
    s.conn.execute_batch(&format!("CREATE TRIGGER fail_calculation BEFORE UPDATE OF values_json ON records WHEN json_extract(NEW.values_json, '$.\"{}\"')=8 BEGIN SELECT RAISE(ABORT, 'forced'); END;", computed.id)).unwrap();
    assert!(
        s.set_record_value_if_unchanged(&row.id, &amount.id, &json!(3), json!(4))
            .is_err()
    );
    assert_eq!(s.get_record(&row.id).unwrap().unwrap(), row);
    s.conn
        .execute_batch("DROP TRIGGER fail_calculation")
        .unwrap();
}

#[test]
fn record_title_updates_propagate_through_title_formula_and_rollup() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let target = database(&mut s, &w, "Target");
    let target = s
        .save_advanced_property(
            &target.id,
            None,
            "Upper",
            PropertyKind::Formula,
            json!({"expression":"upper(prop(\"Name\"))"}),
            None,
        )
        .unwrap();
    let formula = target.properties[1].clone();
    let target_record = s
        .create_record(&target.id, "before", &BTreeMap::new())
        .unwrap();
    let source = database(&mut s, &w, "Source");
    let source = s
        .save_advanced_property(
            &source.id,
            None,
            "Link",
            PropertyKind::Relation,
            json!({"target_database_id":target.id}),
            None,
        )
        .unwrap();
    let link = source.properties[1].clone();
    let source = s.save_advanced_property(&source.id, None, "Related", PropertyKind::Rollup, json!({"relation_property_id":link.id, "target_property_id":formula.id, "operation":"original"}), None).unwrap();
    let rollup = source.properties[2].clone();
    let source_record = s
        .create_record(&source.id, "source", &BTreeMap::new())
        .unwrap();
    s.set_relation_value_if_unchanged(
        &source_record.id,
        &link,
        &Value::Null,
        &[target_record.id.clone()],
    )
    .unwrap();
    assert_eq!(
        s.get_record(&source_record.id).unwrap().unwrap().values[&rollup.id],
        json!(["BEFORE"])
    );
    s.rename_record(&target_record.id, "after").unwrap();
    assert_eq!(
        s.get_record(&source_record.id).unwrap().unwrap().values[&rollup.id],
        json!(["AFTER"])
    );
}

#[test]
fn imported_workspace_remaps_calculation_bindings_and_rebuilds_relation_edges() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Export", "").unwrap();
    let target = database(&mut s, &w, "Target");
    let target = s
        .add_property(&target.id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = target.properties[1].clone();
    let target = s
        .save_advanced_property(
            &target.id,
            None,
            "Double",
            PropertyKind::Formula,
            json!({"expression":"prop(\"Amount\") * 2"}),
            None,
        )
        .unwrap();
    let double = target.properties[2].clone();
    let row = s
        .create_record(
            &target.id,
            "target row",
            &BTreeMap::from([(amount.id.clone(), json!(3))]),
        )
        .unwrap();
    let source = database(&mut s, &w, "Source");
    let source = s
        .save_advanced_property(
            &source.id,
            None,
            "Link",
            PropertyKind::Relation,
            json!({"target_database_id":target.id}),
            None,
        )
        .unwrap();
    let relation = source.properties[1].clone();
    let source = s.save_advanced_property(&source.id, None, "Total", PropertyKind::Rollup, json!({"relation_property_id":relation.id, "target_property_id":double.id, "operation":"sum"}), None).unwrap();
    let source_row = s
        .create_record(&source.id, "source row", &BTreeMap::new())
        .unwrap();
    s.set_relation_value_if_unchanged(&source_row.id, &relation, &Value::Null, &[row.id.clone()])
        .unwrap();
    let archive = s.export_workspace(&w.id).unwrap();
    let imported = s.import_workspace(&archive).unwrap();
    let pages = s.list_pages(&imported.id).unwrap();
    let target = s
        .get_database_by_page(&pages.iter().find(|p| p.title == "Target").unwrap().id)
        .unwrap()
        .unwrap();
    let source = s
        .get_database_by_page(&pages.iter().find(|p| p.title == "Source").unwrap().id)
        .unwrap()
        .unwrap();
    let target_row = s
        .query_records(&target.id, &RecordQuery::default())
        .unwrap()
        .remove(0);
    let source_row = s
        .query_records(&source.id, &RecordQuery::default())
        .unwrap()
        .remove(0);
    let amount = target
        .properties
        .iter()
        .find(|p| p.name == "Amount")
        .unwrap();
    let total = source
        .properties
        .iter()
        .find(|p| p.name == "Total")
        .unwrap();
    s.set_record_value_if_unchanged(&target_row.id, &amount.id, &json!(3), json!(5))
        .unwrap();
    assert_eq!(
        s.get_record(&source_row.id).unwrap().unwrap().values[&total.id],
        json!(10.0)
    );
}

#[test]
fn malformed_legacy_calculations_report_errors_without_blocking_unrelated_edits() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let mut db = database(&mut s, &w, "Test");
    let broken_formula = Property {
        id: new_id(),
        name: "Broken".into(),
        kind: PropertyKind::Formula,
        config: json!({"expression":"prop(\"Missing\") + 1"}),
    };
    let broken_relation = Property {
        id: new_id(),
        name: "Link".into(),
        kind: PropertyKind::Relation,
        config: json!({"target_database_id":"missing"}),
    };
    let broken_rollup = Property {
        id: new_id(),
        name: "Broken rollup".into(),
        kind: PropertyKind::Rollup,
        config: json!({"relation_property_id":broken_relation.id, "target_property_id":"missing", "operation":"sum"}),
    };
    db.properties.extend([
        broken_formula.clone(),
        broken_relation,
        broken_rollup.clone(),
    ]);
    s.conn
        .execute(
            "UPDATE databases SET data=?1 WHERE id=?2",
            rusqlite::params![crate::store::encode(&db).unwrap(), db.id],
        )
        .unwrap();
    let row = s.create_record(&db.id, "row", &BTreeMap::new()).unwrap();
    assert!(row.values[&broken_formula.id].get("_moss_error").is_some());
    assert!(row.values[&broken_rollup.id].get("_moss_error").is_some());
    let db = s
        .save_advanced_property(
            &db.id,
            None,
            "Good",
            PropertyKind::Formula,
            json!({"expression":"42"}),
            None,
        )
        .unwrap();
    let good = db.properties.last().unwrap();
    assert_eq!(
        s.get_record(&row.id).unwrap().unwrap().values[&good.id],
        json!(42.0)
    );
}

#[test]
fn relation_search_is_bounded_and_generic_writes_validate_targets() {
    let mut s = Store::in_memory().unwrap();
    let workspace = s.create_workspace("Test", "").unwrap();
    let foreign_workspace = s.create_workspace("Other", "").unwrap();
    let target = database(&mut s, &workspace, "Target database");
    let foreign = database(&mut s, &foreign_workspace, "Target foreign");
    let source = database(&mut s, &workspace, "Source");
    let source = s
        .save_advanced_property(
            &source.id,
            None,
            "Relation",
            PropertyKind::Relation,
            json!({"target_database_id":target.id}),
            None,
        )
        .unwrap();
    let prop = source.properties[1].clone();
    let good = s
        .create_record(&target.id, "Alpha target", &BTreeMap::new())
        .unwrap();
    let wrong = s
        .create_record(&foreign.id, "Alpha foreign", &BTreeMap::new())
        .unwrap();
    let mut source_row = s
        .create_record(&source.id, "Source row", &BTreeMap::new())
        .unwrap();
    let databases = s
        .search_relation_databases(&workspace.id, "target", 10)
        .unwrap();
    assert_eq!(databases.len(), 1);
    assert_eq!(databases[0].id, target.id);
    assert_eq!(
        s.search_relation_records(&target.id, "target", 1).unwrap()[0].id,
        good.id
    );
    assert!(s.search_relation_records(&target.id, "", 201).is_err());
    source_row.values.insert(prop.id.clone(), json!([wrong.id]));
    assert!(s.save_record(&source_row).is_err());
    assert!(
        s.get_record(&source_row.id)
            .unwrap()
            .unwrap()
            .values
            .get(&prop.id)
            .is_none()
    );
    source_row.values.insert(prop.id.clone(), json!([good.id]));
    s.save_record(&source_row).unwrap();
    assert_eq!(
        s.relation_records_by_ids(&target.id, &[good.id.clone(), "missing".into()])
            .unwrap()[0]
            .id,
        good.id
    );
}

#[test]
fn generic_schema_deletion_cleans_values_and_preserves_title() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let db = database(&mut s, &w, "Test");
    let mut db = s
        .add_property(&db.id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = db.properties[1].clone();
    let row = s
        .create_record(
            &db.id,
            "row",
            &BTreeMap::from([(amount.id.clone(), json!(9))]),
        )
        .unwrap();
    db.views[0].query.group_by = Some(amount.id.clone());
    db.views[0].config = json!({"freeze_through":amount.id, "other":"keep"});
    db.properties.remove(1);
    s.save_database(&db).unwrap();
    let saved = s.get_database(&db.id).unwrap().unwrap();
    assert!(saved.views[0].query.group_by.is_none());
    assert!(saved.views[0].config.get("freeze_through").is_none());
    assert_eq!(saved.views[0].config["other"], "keep");
    assert!(
        !s.get_record(&row.id)
            .unwrap()
            .unwrap()
            .values
            .contains_key(&amount.id)
    );
    db.properties.clear();
    assert!(s.save_database(&db).is_err());
    assert_eq!(s.get_database(&db.id).unwrap().unwrap(), saved);
}

#[test]
fn date_filters_and_sorts_use_range_start_and_preserve_scalar_dates() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let db = database(&mut s, &w, "Test");
    let db = s.add_property(&db.id, "When", PropertyKind::Date).unwrap();
    let date = db.properties[1].id.clone();
    let early = s
        .create_record(
            &db.id,
            "early",
            &BTreeMap::from([(date.clone(), json!("2026-11-01"))]),
        )
        .unwrap();
    let range = s
        .create_record(
            &db.id,
            "range",
            &BTreeMap::from([(
                date.clone(),
                json!({"start":"2026-11-07", "end":"2026-11-09"}),
            )]),
        )
        .unwrap();
    let timed = s
        .create_record(
            &db.id,
            "timed",
            &BTreeMap::from([(
                date.clone(),
                json!({"start":"2026-11-07T14:30:00+08:00", "end":"2026-11-08T10:00:00+08:00"}),
            )]),
        )
        .unwrap();
    let filter = FilterExpr::Rule(FilterRule {
        property_id: date.clone(),
        op: FilterOp::Eq,
        value: json!("2026-11-07"),
    });
    let result = s
        .query_records(
            &db.id,
            &RecordQuery {
                filter: Some(filter),
                ..Default::default()
            },
        )
        .unwrap();
    let ids: std::collections::BTreeSet<_> = result.iter().map(|r| &r.id).collect();
    assert_eq!(
        ids,
        std::collections::BTreeSet::from([&range.id, &timed.id])
    );
    let result = s
        .query_records(
            &db.id,
            &RecordQuery {
                sorts: vec![SortRule {
                    property_id: date,
                    descending: false,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.iter().map(|r| &r.id).collect::<Vec<_>>(),
        vec![&early.id, &range.id, &timed.id]
    );
}

#[test]
fn deleting_rollup_target_property_marks_related_rows_as_errors() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Test", "").unwrap();
    let target = database(&mut s, &w, "Target");
    let target = s
        .add_property(&target.id, "Amount", PropertyKind::Number)
        .unwrap();
    let amount = target.properties[1].clone();
    let row = s
        .create_record(
            &target.id,
            "row",
            &BTreeMap::from([(amount.id.clone(), json!(4))]),
        )
        .unwrap();
    let source = database(&mut s, &w, "Source");
    let source = s
        .save_advanced_property(
            &source.id,
            None,
            "Link",
            PropertyKind::Relation,
            json!({"target_database_id":target.id}),
            None,
        )
        .unwrap();
    let link = source.properties[1].clone();
    let source = s.save_advanced_property(&source.id, None, "Total", PropertyKind::Rollup, json!({"relation_property_id":link.id, "target_property_id":amount.id, "operation":"sum"}), None).unwrap();
    let rollup = source.properties[2].clone();
    let source_row = s
        .create_record(&source.id, "source", &BTreeMap::new())
        .unwrap();
    s.set_relation_value_if_unchanged(&source_row.id, &link, &Value::Null, &[row.id])
        .unwrap();
    s.delete_property(&target.id, &amount).unwrap();
    let source_row = s.get_record(&source_row.id).unwrap().unwrap();
    assert!(source_row.values[&rollup.id].get("_moss_error").is_some());
}

#[test]
fn unsupported_formula_is_rejected_without_creating_a_property() {
    let mut s = Store::in_memory().unwrap();
    let workspace = s.create_workspace("Test", "").unwrap();
    let db = database(&mut s, &workspace, "Test");
    assert!(
        s.save_advanced_property(
            &db.id,
            None,
            "Clock",
            PropertyKind::Formula,
            json!({"expression":"now()"}),
            None
        )
        .is_err()
    );
    assert_eq!(s.get_database(&db.id).unwrap().unwrap(), db);
}
