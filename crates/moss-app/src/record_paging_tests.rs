use super::*;
use moss_core::{PropertyKind, RecordQuery, SortRule};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn next_previous_seek_and_stale_filter_keep_the_current_page() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    for n in 0..230 {
        s.store
            .create_record(&db.id, &format!("Row {n}"), &BTreeMap::new())
            .unwrap();
    }
    assert!(s.query_records());
    let first = s.tab().unwrap().records.clone();
    assert!(s.tab().unwrap().paging_next.is_some());
    s.move_record_page(100);
    let expected = s
        .store
        .query_records(
            &db.id,
            &RecordQuery {
                offset: 100,
                limit: 100,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(s.tab().unwrap().records, expected);
    assert!(s.tab().unwrap().has_more_records);
    s.move_record_page(0);
    assert_eq!(s.tab().unwrap().records, first);
    s.filter.replace_text("Changed search");
    s.move_record_page(100);
    assert_eq!(s.tab().unwrap().offset, 0);
    assert_eq!(s.tab().unwrap().records, first);
    assert!(
        s.error
            .as_deref()
            .is_some_and(|error| error.contains("cursor"))
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn unsupported_property_sort_uses_the_existing_offset_query() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let db = s
        .store
        .add_property(&db.id, "Score", PropertyKind::Number)
        .unwrap();
    let score = db
        .properties
        .iter()
        .find(|p| p.name == "Score")
        .unwrap()
        .id
        .clone();
    for n in 0..140 {
        let values = if n % 4 == 0 {
            BTreeMap::new()
        } else {
            BTreeMap::from([(score.clone(), json!(n % 7))])
        };
        s.store
            .create_record(&db.id, &format!("Row {n}"), &values)
            .unwrap();
    }
    let mut updated = db.clone();
    updated.views[0].query.sorts = vec![SortRule {
        property_id: score,
        descending: true,
    }];
    s.store.save_database(&updated).unwrap();
    s.tabs[s.active].database = Some(updated.clone());
    assert!(s.query_records());
    assert!(s.tab().unwrap().paging_next.is_none());
    s.move_record_page(100);
    let mut expected_query = updated.views[0].query.clone();
    expected_query.offset = 100;
    expected_query.limit = 100;
    assert_eq!(
        s.tab().unwrap().records,
        s.store.query_records(&db.id, &expected_query).unwrap()
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn title_ties_and_search_cursor_survive_tab_switches() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let mut db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let title = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .id
        .clone();
    db.views[0].query.sorts = vec![SortRule {
        property_id: title,
        descending: false,
    }];
    s.store.save_database(&db).unwrap();
    s.tabs[s.active].database = Some(db.clone());
    for _ in 0..205 {
        s.store
            .create_record(&db.id, "Same title", &BTreeMap::new())
            .unwrap();
    }
    s.filter.replace_text("Same");
    assert!(s.query_records());
    let first = s.tab().unwrap().records.clone();
    let database_page = s.tab().unwrap().page.id.clone();
    s.new_page(false, None);
    s.open_page(&database_page, false);
    assert_eq!(s.filter.text, "Same");
    assert!(s.tab().unwrap().paging_next.is_some());
    s.move_record_page(100);
    let second = s.tab().unwrap().records.clone();
    assert_eq!(second.len(), 100);
    assert!(first.iter().all(|a| second.iter().all(|b| a.id != b.id)));
    s.move_record_page(0);
    assert_eq!(s.tab().unwrap().records, first);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
