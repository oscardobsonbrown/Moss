use super::*;
use crate::{
    Database, DatabaseView, FilterExpr, FilterOp, FilterRule, Property, PropertyKind, RecordQuery,
    SortRule, ViewKind, new_id,
};
use serde_json::json;

fn fixture() -> (Store, Database) {
    fixture_types(PropertyKind::Status, PropertyKind::Checkbox)
}
fn fixture_types(primary: PropertyKind, secondary: PropertyKind) -> (Store, Database) {
    let mut store = Store::in_memory().unwrap();
    let ws = store.create_workspace("Board", "🗂").unwrap();
    let db = Database {
        id: new_id(),
        page_id: String::new(),
        name: "Tasks".into(),
        properties: vec![
            Property {
                id: "title".into(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            },
            Property {
                id: "status".into(),
                name: "Status".into(),
                kind: primary,
                config: if matches!(primary, PropertyKind::Status | PropertyKind::Select) {
                    json!({"options":[{"name":"Alpha","color":"red"},{"name":"Beta","color":"blue"}]})
                } else {
                    json!({})
                },
            },
            Property {
                id: "priority".into(),
                name: "Priority".into(),
                kind: secondary,
                config: json!({}),
            },
            Property {
                id: "notes".into(),
                name: "Notes".into(),
                kind: PropertyKind::Text,
                config: json!({}),
            },
        ],
        views: vec![DatabaseView {
            id: new_id(),
            name: "Board".into(),
            kind: ViewKind::Board,
            query: RecordQuery {
                group_by: Some("status".into()),
                limit: 1,
                offset: 200,
                ..Default::default()
            },
            config: json!({"board_subgroup_by":"priority"}),
        }],
    };
    let page = store
        .create_database_page(&ws.id, None, "Tasks", "", db.clone())
        .unwrap();
    let db = store.get_database_by_page(&page.id).unwrap().unwrap();
    (store, db)
}
fn values(status: Value, priority: Value) -> BTreeMap<String, Value> {
    [
        ("status".into(), status),
        ("priority".into(), priority),
        ("notes".into(), json!("keep")),
    ]
    .into()
}
fn projection(store: &Store, db: &Database) -> BoardProjection {
    store
        .query_board(&db.id, &db.views[0].id, "", &BTreeMap::new())
        .unwrap()
}

#[test]
fn board_projection_counts_full_query_and_windows_each_typed_bucket() {
    let (mut store, db) = fixture();
    for i in 0..260 {
        store
            .create_record(
                &db.id,
                &format!("Task {i:03}"),
                &values(
                    json!(if i % 2 == 0 { "Alpha" } else { "Beta" }),
                    json!(i % 3 == 0),
                ),
            )
            .unwrap();
    }
    let p = projection(&store, &db);
    assert_eq!(p.total, 260);
    assert!(!p.groups_truncated);
    assert!(
        p.buckets
            .iter()
            .all(|bucket| bucket.records.len() <= BOARD_PAGE_SIZE)
    );
    assert_eq!(p.buckets.iter().map(|b| b.total).sum::<u64>(), 260);
    let bucket = p.buckets.iter().find(|b| b.total > 50).unwrap();
    let key = bucket_key(&bucket.group, &bucket.subgroup);
    let old: Vec<_> = bucket.records.iter().map(|r| &r.id).collect();
    let p2 = store
        .query_board(&db.id, &db.views[0].id, "", &[(key, 50)].into())
        .unwrap();
    let next = p2
        .buckets
        .iter()
        .find(|b| b.group == bucket.group && b.subgroup == bucket.subgroup)
        .unwrap();
    assert_eq!(next.offset, 50);
    assert!(next.records.iter().all(|r| !old.contains(&&r.id)));
    assert_eq!(next.total, bucket.total);
    let search = store
        .query_board(&db.id, &db.views[0].id, "Task 00", &BTreeMap::new())
        .unwrap();
    assert_eq!(search.total, 10);
}
#[test]
fn board_move_updates_both_axes_atomically_and_preserves_other_changes() {
    let (mut store, db) = fixture();
    let record = store
        .create_record(&db.id, "Task", &values(json!("Alpha"), json!(true)))
        .unwrap();
    store
        .set_record_value_if_unchanged(&record.id, "notes", &json!("keep"), json!("concurrent"))
        .unwrap();
    let updated = store
        .move_board_record(
            &db.id,
            &db.views[0],
            &record,
            &db.properties[1],
            Some(&db.properties[2]),
            json!("Beta"),
            Value::Null,
            None,
            record.id.as_bytes(),
        )
        .unwrap();
    assert_eq!(updated.values["status"], json!("Beta"));
    assert!(updated.values["priority"].is_null());
    assert_eq!(updated.values["notes"], json!("concurrent"));
    assert!(
        store
            .move_board_record(
                &db.id,
                &db.views[0],
                &record,
                &db.properties[1],
                Some(&db.properties[2]),
                json!("Alpha"),
                json!(false),
                None,
                record.id.as_bytes()
            )
            .is_err()
    );
    assert_eq!(store.get_record(&record.id).unwrap().unwrap(), updated);
}
#[test]
fn board_manual_rank_keeps_untouched_cards_in_order_and_rejects_stale_position() {
    let (mut store, mut db) = fixture();
    db.views[0].config = json!({});
    store.save_database(&db).unwrap();
    let mut records = (0..3)
        .map(|i| {
            store
                .create_record(
                    &db.id,
                    &i.to_string(),
                    &values(json!("Alpha"), json!(false)),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    records.sort_by(|a, b| a.id.cmp(&b.id));
    let p = projection(&store, &db);
    let record = &records[2];
    let rank = p.order[&record.id].clone();
    store
        .move_board_record(
            &db.id,
            &db.views[0],
            record,
            &db.properties[1],
            None,
            json!("Alpha"),
            Value::Null,
            Some(&records[1].id),
            &rank,
        )
        .unwrap();
    let p = projection(&store, &db);
    let ids: Vec<_> = p
        .buckets
        .iter()
        .find(|b| b.group == json!("Alpha"))
        .unwrap()
        .records
        .iter()
        .map(|r| r.id.clone())
        .collect();
    assert_eq!(
        ids,
        vec![
            records[0].id.clone(),
            records[2].id.clone(),
            records[1].id.clone()
        ]
    );
    assert!(
        store
            .move_board_record(
                &db.id,
                &db.views[0],
                record,
                &db.properties[1],
                None,
                json!("Alpha"),
                Value::Null,
                None,
                &rank
            )
            .is_err()
    );
    db.views[0].query.sorts = vec![SortRule {
        property_id: "title".into(),
        descending: false,
    }];
    store.save_database(&db).unwrap();
    assert!(
        store
            .move_board_record(
                &db.id,
                &db.views[0],
                record,
                &db.properties[1],
                None,
                json!("Alpha"),
                Value::Null,
                None,
                &p.order[&record.id]
            )
            .is_err()
    );
    store
        .move_board_record(
            &db.id,
            &db.views[0],
            record,
            &db.properties[1],
            None,
            json!("Beta"),
            Value::Null,
            None,
            &p.order[&record.id],
        )
        .unwrap();
}
#[test]
fn board_group_rename_covers_all_records_and_retains_option_metadata() {
    let (mut store, mut db) = fixture();
    db.views[0].config["board_hidden_groups"] = json!([group_key(&json!("Alpha"))]);
    db.views[0].query.filter = Some(FilterExpr::Rule(FilterRule {
        property_id: "status".into(),
        op: FilterOp::Eq,
        value: json!("Alpha"),
    }));
    let mut unrelated = db.views[0].clone();
    unrelated.id = new_id();
    unrelated.query.group_by = Some("priority".into());
    unrelated.config = json!({"board_group_order":[group_key(&json!("Alpha"))]});
    db.views.push(unrelated);
    store.save_database(&db).unwrap();
    for i in 0..270 {
        store
            .create_record(
                &db.id,
                &i.to_string(),
                &values(json!("Alpha"), json!(false)),
            )
            .unwrap();
    }
    let db = store
        .save_board_group_name(&db.id, &db.properties[1], Some(&json!("Alpha")), "Renamed")
        .unwrap();
    assert_eq!(
        db.views[0].config["board_hidden_groups"],
        json!([group_key(&json!("Renamed"))])
    );
    assert_eq!(
        db.views[1].config["board_group_order"],
        json!([group_key(&json!("Alpha"))])
    );
    assert!(
        matches!(&db.views[0].query.filter,Some(FilterExpr::Rule(rule)) if rule.value==json!("Renamed"))
    );
    assert_eq!(
        db.properties[1].config["options"][0],
        json!({"name":"Renamed","color":"red"})
    );
    let p = projection(&store, &db);
    assert_eq!(
        p.buckets
            .iter()
            .filter(|b| b.group == json!("Renamed"))
            .map(|b| b.total)
            .sum::<u64>(),
        270
    );
    assert!(
        store
            .save_board_group_name(&db.id, &db.properties[1], None, "Beta")
            .is_err()
    );
    let db = store
        .save_board_group_name(&db.id, &db.properties[1], None, "Later")
        .unwrap();
    assert_eq!(
        db.properties[1].config["options"].as_array().unwrap().len(),
        3
    );
}
#[test]
fn board_groups_and_counts_respect_filter_with_off_page_matches() {
    let (mut store, mut db) = fixture();
    for i in 0..8 {
        store
            .create_record(
                &db.id,
                &i.to_string(),
                &values(json!(if i < 5 { "Alpha" } else { "Beta" }), json!(false)),
            )
            .unwrap();
    }
    db.views[0].query.filter = Some(FilterExpr::Rule(FilterRule {
        property_id: "status".into(),
        op: FilterOp::Eq,
        value: json!("Beta"),
    }));
    store.save_database(&db).unwrap();
    let p = projection(&store, &db);
    assert_eq!(p.total, 3);
    assert!(p.buckets.iter().all(|b| b.group == json!("Beta")));
}

#[test]
fn board_manual_order_survives_archive_id_remapping() {
    let (mut store, mut db) = fixture();
    db.views[0].config = json!({});
    store.save_database(&db).unwrap();
    let mut records = (0..4)
        .map(|i| {
            store
                .create_record(
                    &db.id,
                    &format!("Card {i}"),
                    &values(json!("Alpha"), json!(false)),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    records.sort_by(|a, b| a.id.cmp(&b.id));
    let moved = &records[3];
    store
        .move_board_record(
            &db.id,
            &db.views[0],
            moved,
            &db.properties[1],
            None,
            json!("Alpha"),
            Value::Null,
            Some(&records[1].id),
            moved.id.as_bytes(),
        )
        .unwrap();
    let original = projection(&store, &db)
        .buckets
        .into_iter()
        .find(|b| b.group == json!("Alpha"))
        .unwrap()
        .records
        .into_iter()
        .map(|r| r.title)
        .collect::<Vec<_>>();
    let workspace = store.get_page(&db.page_id).unwrap().unwrap().workspace_id;
    let archive = store.export_workspace(&workspace).unwrap();
    let imported = store.import_workspace(&archive).unwrap();
    let page = store
        .list_pages(&imported.id)
        .unwrap()
        .into_iter()
        .find(|p| p.kind == PageKind::Database)
        .unwrap();
    let restored = store.get_database_by_page(&page.id).unwrap().unwrap();
    let restored = projection(&store, &restored)
        .buckets
        .into_iter()
        .find(|b| b.group == json!("Alpha"))
        .unwrap()
        .records
        .into_iter()
        .map(|r| r.title)
        .collect::<Vec<_>>();
    assert_eq!(restored, original);
}

#[test]
fn board_totals_cover_all_pairs_when_card_bucket_windows_are_capped() {
    let (mut store, db) = fixture_types(PropertyKind::Text, PropertyKind::Text);
    for group in 0..9 {
        for sub in 0..32 {
            store
                .create_record(
                    &db.id,
                    "Task",
                    &values(
                        json!(format!("Group {group}")),
                        json!(format!("Lane {sub}")),
                    ),
                )
                .unwrap();
        }
    }
    let p = projection(&store, &db);
    assert_eq!(p.total, 288);
    assert!(p.groups_truncated);
    assert_eq!(p.buckets.len(), 256);
    assert_eq!(p.group_totals.values().sum::<u64>(), 288);
    assert_eq!(p.group_totals[&group_key(&json!("Group 8"))], 32);
    // 31 named lanes plus the empty lane fit; retained lane counts include every primary group.
    assert!(p.subgroup_totals.values().all(|count| *count == 9));
}

fn eq(id: &str, value: Value) -> FilterExpr {
    FilterExpr::Rule(FilterRule {
        property_id: id.into(),
        op: FilterOp::Eq,
        value,
    })
}
#[test]
fn board_creation_inherits_and_rules_and_rejects_conflicting_columns() {
    let (_, mut db) = fixture();
    db.views[0].query.filter = Some(FilterExpr::All(vec![
        eq("status", json!("Alpha")),
        eq("priority", json!(true)),
        eq("notes", json!("Required")),
    ]));
    let context = [("status".into(), json!("Alpha"))].into();
    let result = creation_values(&db, &db.views[0], context).unwrap();
    assert_eq!(result["status"], json!("Alpha"));
    assert_eq!(result["priority"], json!(true));
    assert_eq!(result["notes"], json!("Required"));
    assert!(creation_values(&db, &db.views[0], [("status".into(), json!("Beta"))].into()).is_err());
    db.views[0].query.filter = Some(FilterExpr::All(vec![
        eq("notes", json!("A")),
        eq("notes", json!("B")),
    ]));
    assert!(creation_values(&db, &db.views[0], BTreeMap::new()).is_err());
}
#[test]
fn board_creation_inherits_only_common_or_values_without_choosing_a_branch() {
    let (_, mut db) = fixture();
    db.views[0].query.filter = Some(FilterExpr::Any(vec![
        FilterExpr::All(vec![
            eq("status", json!("Alpha")),
            eq("notes", json!("Common")),
            eq("priority", json!(true)),
        ]),
        FilterExpr::All(vec![
            eq("status", json!("Beta")),
            eq("notes", json!("Common")),
            eq("priority", json!(false)),
        ]),
    ]));
    let result = creation_values(
        &db,
        &db.views[0],
        [("status".into(), json!("Alpha"))].into(),
    )
    .unwrap();
    assert_eq!(result["notes"], json!("Common"));
    assert_eq!(result["status"], json!("Alpha"));
    assert!(
        !result.contains_key("priority"),
        "do not choose a branch even when the column matches one"
    );
    assert!(
        creation_values(
            &db,
            &db.views[0],
            [("status".into(), json!("Other"))].into()
        )
        .is_err()
    );
}
#[test]
fn board_creation_preserves_date_context_and_does_not_force_computed_or_title_fields() {
    let (_, mut db) = fixture();
    db.properties.push(Property {
        id: "date".into(),
        name: "Date".into(),
        kind: PropertyKind::Date,
        config: json!({}),
    });
    db.properties.push(Property {
        id: "formula".into(),
        name: "Formula".into(),
        kind: PropertyKind::Formula,
        config: json!({"expression":"1"}),
    });
    db.views[0].query.filter = Some(FilterExpr::All(vec![
        eq("date", json!("2026-11-08")),
        eq("notes", json!("")),
        eq("formula", json!(99)),
        eq("title", json!("Heading")),
    ]));
    let date = json!({"start":"2026-11-08T14:00:00Z","end":"2026-11-10T14:00:00Z"});
    let context = [("date".into(), date.clone()), ("notes".into(), Value::Null)].into();
    let result = creation_values(&db, &db.views[0], context).unwrap();
    assert_eq!(result["date"], date);
    assert_eq!(result["notes"], json!(""));
    assert!(!result.contains_key("formula"));
    assert!(!result.contains_key("title"));
    assert!(creation_values(&db, &db.views[0], [("formula".into(), json!(9))].into()).is_err());
}
#[test]
fn board_numeric_groups_merge_integer_floats_and_rank_fractional_values_exactly() {
    let (mut store, mut db) = fixture_types(PropertyKind::Number, PropertyKind::Checkbox);
    db.views[0].config = json!({});
    store.save_database(&db).unwrap();
    store
        .create_record(&db.id, "Integer", &values(json!(42), json!(false)))
        .unwrap();
    store
        .create_record(&db.id, "Float", &values(json!(42.0), json!(false)))
        .unwrap();
    let p = projection(&store, &db);
    assert_eq!(p.groups.iter().filter(|v| **v == json!(42)).count(), 1);
    assert_eq!(
        p.buckets
            .iter()
            .find(|b| b.group == json!(42))
            .unwrap()
            .total,
        2
    );
    let fraction = 1.2345678901234567;
    let mut cards = (0..3)
        .map(|i| {
            store
                .create_record(
                    &db.id,
                    &format!("Precise {i}"),
                    &values(json!(fraction), json!(false)),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    store
        .move_board_record(
            &db.id,
            &db.views[0],
            &cards[2],
            &db.properties[1],
            None,
            json!(fraction),
            Value::Null,
            Some(&cards[1].id),
            cards[2].id.as_bytes(),
        )
        .unwrap();
    let p = projection(&store, &db);
    let ids = p
        .buckets
        .iter()
        .find(|b| b.group == json!(fraction))
        .unwrap()
        .records
        .iter()
        .map(|r| r.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            cards[0].id.clone(),
            cards[2].id.clone(),
            cards[1].id.clone()
        ]
    );
}

#[test]
fn board_option_metadata_is_bounded_and_filtered_late_options_still_have_cards() {
    let (mut store, mut db) = fixture();
    db.properties[1].config =
        json!({"options":(0..10_000).map(|i|format!("Option {i}")).collect::<Vec<_>>()});
    db.views[0].config = json!({});
    db.views[0].query.filter = Some(eq("status", json!("Option 9999")));
    store.save_database(&db).unwrap();
    let record = store
        .create_record(
            &db.id,
            "Late option",
            &values(json!("Option 9999"), json!(false)),
        )
        .unwrap();
    let loaded = project_loaded(&db, &db.views[0], std::slice::from_ref(&record)).unwrap();
    assert!(loaded.groups_truncated);
    assert!(loaded.groups.len() <= MAX_GROUPS + 1);
    assert!(loaded.groups.contains(&json!("Option 9999")));
    let p = projection(&store, &db);
    assert!(p.groups_truncated);
    assert!(p.groups.len() <= MAX_GROUPS + 1);
    assert_eq!(p.total, 1);
    assert_eq!(p.buckets[0].records[0].id, record.id);
}
