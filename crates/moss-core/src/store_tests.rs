use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn database(store: &mut Store, workspace: &str) -> Database {
    let page = store
        .create_page(workspace, None, "Tasks", "", PageKind::Database)
        .unwrap();
    let db = Database {
        id: new_id(),
        page_id: page.id,
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
                kind: PropertyKind::Status,
                config: json!({}),
            },
        ],
        views: vec![],
    };
    store.save_database(&db).unwrap();
    db
}

#[test]
fn checking_a_todo_updates_only_that_block_and_validates_its_owner() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Tasks", "").unwrap();
    let page = store
        .create_page(&workspace.id, None, "Page", "", PageKind::Document)
        .unwrap();
    let other = store
        .create_page(&workspace.id, None, "Other", "", PageKind::Document)
        .unwrap();
    let mut todo = Block::paragraph(&page.id, "Keep this text", 0);
    todo.kind = BlockKind::Todo;
    todo.config = json!({"custom": "keep"});
    let paragraph = Block::paragraph(&page.id, "Other block", 1);
    store
        .save_blocks(&page.id, &[todo.clone(), paragraph.clone()])
        .unwrap();
    let changes = store.local_change_count();
    store.set_todo_checked(&page.id, &todo.id, true).unwrap();
    assert_eq!(store.local_change_count() - changes, 2);
    todo.checked = true;
    assert_eq!(
        store.get_blocks(&page.id).unwrap(),
        vec![todo.clone(), paragraph.clone()]
    );
    assert!(store.set_todo_checked(&other.id, &todo.id, false).is_err());
    assert!(
        store
            .set_todo_checked(&page.id, &paragraph.id, true)
            .is_err()
    );
    assert!(store.set_todo_checked(&page.id, "missing", true).is_err());
    store
        .conn
        .execute(
            "UPDATE pages SET deleted_at='2026-01-01' WHERE id=?1",
            [&page.id],
        )
        .unwrap();
    assert!(store.set_todo_checked(&page.id, &todo.id, false).is_err());
}

#[test]
fn gallery_previews_are_bounded_ordered_and_workspace_scoped() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Preview", "").unwrap();
    let other_workspace = store.create_workspace("Other", "").unwrap();
    let db = database(&mut store, &workspace.id);
    let other_db = database(&mut store, &other_workspace.id);
    let record = store
        .create_record(&db.id, "Body", &BTreeMap::new())
        .unwrap();
    let page = store.ensure_record_page(&record.id).unwrap();
    store
        .save_blocks(
            &page.id,
            &[
                Block::paragraph(&page.id, "Later block", 1),
                Block::paragraph(&page.id, "界".repeat(600), 0),
            ],
        )
        .unwrap();
    let empty = store
        .create_record(&db.id, "Empty first", &BTreeMap::new())
        .unwrap();
    let empty_page = store.ensure_record_page(&empty.id).unwrap();
    store
        .save_blocks(
            &empty_page.id,
            &[
                Block::paragraph(&empty_page.id, "", 0),
                Block::paragraph(&empty_page.id, "Do not skip the first block", 1),
            ],
        )
        .unwrap();
    let no_page = store
        .create_record(&db.id, "No page", &BTreeMap::new())
        .unwrap();
    let other = store
        .create_record(&other_db.id, "Private", &BTreeMap::new())
        .unwrap();
    let other_page = store.ensure_record_page(&other.id).unwrap();
    let cover = PageCover::Color {
        color: "#b8c5d6".into(),
    };
    store
        .set_page_cover_if_unchanged(&workspace.id, &page.id, &None, Some(cover.clone()))
        .unwrap();
    store
        .set_page_cover_if_unchanged(
            &other_workspace.id,
            &other_page.id,
            &None,
            Some(cover.clone()),
        )
        .unwrap();
    store
        .save_blocks(
            &other_page.id,
            &[Block::paragraph(&other_page.id, "Other workspace", 0)],
        )
        .unwrap();
    let ids = vec![
        record.id.clone(),
        empty.id.clone(),
        no_page.id.clone(),
        other.id.clone(),
        "missing".into(),
    ];
    let previews = store.record_page_previews(&workspace.id, &ids).unwrap();
    let covers = store.record_page_covers(&workspace.id, &ids).unwrap();
    assert_eq!(covers.len(), 1);
    assert_eq!(covers.get(&record.id), Some(&cover));
    assert_eq!(previews.len(), 3);
    assert_eq!(previews[&record.id], "界".repeat(512));
    assert_eq!(previews[&empty.id], "");
    assert_eq!(previews[&no_page.id], "");
    assert!(
        store
            .record_page_previews(&workspace.id, &[])
            .unwrap()
            .is_empty()
    );
    store.delete_record(&record.id).unwrap();
    assert!(
        store
            .record_page_covers(&workspace.id, &ids)
            .unwrap()
            .is_empty()
    );
    assert!(
        !store
            .record_page_previews(&workspace.id, &ids)
            .unwrap()
            .contains_key(&record.id)
    );
    store.delete_page(&db.page_id).unwrap();
    assert!(
        store
            .record_page_previews(&workspace.id, &ids)
            .unwrap()
            .is_empty()
    );
}
fn record(db: &Database, title: &str, status: &str) -> Record {
    let mut values = BTreeMap::new();
    values.insert("status".into(), json!(status));
    let t = now();
    Record {
        id: new_id(),
        database_id: db.id.clone(),
        page_id: None,
        title: title.into(),
        values,
        created_at: t.clone(),
        updated_at: t,
        deleted_at: None,
    }
}
#[test]
fn durability_search_and_trash() {
    let path = std::env::temp_dir().join(format!("moss-test-{}.sqlite", new_id()));
    let (wid, pid) = {
        let mut s = Store::open(&path).unwrap();
        let w = s.create_workspace("One", "📁").unwrap();
        let p = s
            .create_page(&w.id, None, "Guide", "", PageKind::Document)
            .unwrap();
        s.save_blocks(
            &p.id,
            &[Block::paragraph(&p.id, "telescope observations", 0)],
        )
        .unwrap();
        assert_eq!(s.search("telescope", Some(&w.id), 10).unwrap().len(), 1);
        (w.id, p.id)
    };
    {
        let mut s = Store::open(&path).unwrap();
        assert_eq!(s.get_page(&pid).unwrap().unwrap().title, "Guide");
        s.delete_page(&pid).unwrap();
        assert!(s.search("telescope", Some(&wid), 10).unwrap().is_empty());
        assert_eq!(s.list_trash(&wid).unwrap().len(), 1);
        s.restore_page(&pid).unwrap();
        assert_eq!(s.search("telescope", Some(&wid), 10).unwrap().len(), 1);
    }
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
}
#[test]
fn filtered_paged_query_and_workspace_boundaries() {
    let mut s = Store::in_memory().unwrap();
    let a = s.create_workspace("A", "").unwrap();
    let b = s.create_workspace("B", "").unwrap();
    let parent = s
        .create_page(&a.id, None, "Parent", "", PageKind::Document)
        .unwrap();
    assert!(
        s.create_page(&b.id, Some(&parent.id), "Wrong", "", PageKind::Document)
            .is_err()
    );
    let db = database(&mut s, &a.id);
    for title in ["Zebra", "Alpha", "Beta"] {
        s.save_record(&record(&db, title, "Open")).unwrap();
    }
    s.save_record(&record(&db, "Done", "Done")).unwrap();
    let q = RecordQuery {
        filter: Some(FilterExpr::Rule(FilterRule {
            property_id: "status".into(),
            op: FilterOp::Eq,
            value: json!("Open"),
        })),
        sorts: vec![SortRule {
            property_id: "title".into(),
            descending: false,
        }],
        limit: 1,
        offset: 1,
        group_by: None,
    };
    assert_eq!(s.query_records(&db.id, &q).unwrap()[0].title, "Beta");
    assert!(s.search("Alpha", Some(&b.id), 10).unwrap().is_empty());
    assert_eq!(s.search("Alpha", Some(&a.id), 10).unwrap().len(), 1);
    let bad = RecordQuery {
        filter: Some(FilterExpr::Rule(FilterRule {
            property_id: "x') OR 1--".into(),
            op: FilterOp::Eq,
            value: json!(1),
        })),
        ..RecordQuery::default()
    };
    assert!(s.query_records(&db.id, &bad).is_err());
}
#[test]
fn block_replacement_is_atomic_and_descendant_trash_restores() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("A", "").unwrap();
    let p = s
        .create_page(&w.id, None, "Parent", "", PageKind::Document)
        .unwrap();
    let c = s
        .create_page(&w.id, Some(&p.id), "Child", "", PageKind::Document)
        .unwrap();
    let block = Block::paragraph(&c.id, "original", 0);
    s.save_blocks(&c.id, &[block.clone()]).unwrap();
    let mut bad = block.clone();
    bad.page_id = p.id.clone();
    assert!(s.save_blocks(&c.id, &[bad]).is_err());
    assert_eq!(s.get_blocks(&c.id).unwrap(), vec![block]);
    s.delete_page(&p.id).unwrap();
    assert!(s.list_pages(&w.id).unwrap().is_empty());
    assert!(s.restore_page(&c.id).is_err());
    s.restore_page(&p.id).unwrap();
    assert_eq!(s.list_pages(&w.id).unwrap().len(), 2);
}

#[test]
fn record_trash_search_and_seed_data() {
    let mut s = Store::in_memory().unwrap();
    let workspaces = s.seed_demo_workspaces().unwrap();
    assert_eq!(workspaces.len(), 5);
    let pages = s.list_pages(&workspaces[0].id).unwrap();
    let database_page = pages.iter().find(|p| p.kind == PageKind::Database).unwrap();
    let db = s.get_database_by_page(&database_page.id).unwrap().unwrap();
    assert_eq!(db.views.len(), ViewKind::ALL.len());
    let rows = s.query_records(&db.id, &RecordQuery::default()).unwrap();
    assert_eq!(rows.len(), 6);
    let first = &rows[0];
    s.delete_record(&first.id).unwrap();
    assert_eq!(
        s.query_records(&db.id, &RecordQuery::default())
            .unwrap()
            .len(),
        5
    );
    s.restore_record(&first.id).unwrap();
    assert_eq!(
        s.query_records(&db.id, &RecordQuery::default())
            .unwrap()
            .len(),
        6
    );
    assert_eq!(s.seed_demo_workspaces().unwrap().len(), 5);
}

#[test]
fn expired_trash_is_removed_on_reopen() {
    let path = std::env::temp_dir().join(format!("moss-expiry-{}.sqlite", new_id()));
    let pid = {
        let mut s = Store::open(&path).unwrap();
        let w = s.create_workspace("A", "").unwrap();
        let p = s
            .create_page(&w.id, None, "Expired", "", PageKind::Document)
            .unwrap();
        s.delete_page(&p.id).unwrap();
        s.conn
            .execute(
                "UPDATE pages SET deleted_at='2000-01-01T00:00:00Z' WHERE id=?1",
                [&p.id],
            )
            .unwrap();
        p.id
    };
    let s = Store::open(&path).unwrap();
    assert!(s.get_page(&pid).unwrap().is_none());
    drop(s);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
}

#[test]
fn old_pages_without_settings_keep_their_archive_encoding() {
    let old = json!({"id":"p","workspace_id":"w","parent_id":null,"title":"Old page","icon":"","kind":"document","created_at":"2026-01-01","updated_at":"2026-01-01","deleted_at":null});
    let page: Page = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(page.settings, PageSettings::default());
    assert_eq!(serde_json::to_value(page).unwrap(), old);
}

#[test]
fn record_page_is_created_once_hidden_and_renamed_atomically() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Records", "").unwrap();
    let db = database(&mut store, &workspace.id);
    let values = BTreeMap::from([("status".into(), json!("Open"))]);
    let mut record = store.create_record(&db.id, "Original", &values).unwrap();
    let page = store.ensure_record_page(&record.id).unwrap();
    assert_eq!(store.ensure_record_page(&record.id).unwrap().id, page.id);
    assert_eq!(
        store.get_record_by_page(&page.id).unwrap().unwrap().id,
        record.id
    );
    assert!(store.get_record_by_page(&db.page_id).unwrap().is_none());
    assert_eq!(page.kind, PageKind::Record);
    assert_eq!(page.parent_id.as_deref(), Some(db.page_id.as_str()));
    assert!(
        !store
            .list_pages(&workspace.id)
            .unwrap()
            .iter()
            .any(|p| p.id == page.id)
    );
    assert_eq!(
        store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .page_id
            .as_deref(),
        Some(page.id.as_str())
    );
    let block = Block::paragraph(&page.id, "Unique record content", 0);
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    assert_eq!(store.get_blocks(&page.id).unwrap(), vec![block]);
    assert_eq!(
        store
            .search("Unique record content", Some(&workspace.id), 10)
            .unwrap()[0]
            .page_id,
        page.id
    );
    let renamed = store.rename_record(&record.id, "Revised").unwrap();
    assert_eq!(renamed.title, "Revised");
    assert_eq!(store.get_page(&page.id).unwrap().unwrap().title, "Revised");
    assert_eq!(
        store.get_record(&record.id).unwrap().unwrap().values,
        values
    );
    record.page_id = Some(page.id.clone());
    record.title = "Unsafe change".into();
    assert!(store.save_record(&record).is_err());
    assert_eq!(store.get_page(&page.id).unwrap().unwrap().title, "Revised");
    record.database_id = new_id();
    assert!(store.save_record(&record).is_err());
    assert!(
        store
            .create_record(
                &db.id,
                "Bad",
                &BTreeMap::from([("unknown".into(), json!(true))])
            )
            .is_err()
    );
}

#[test]
fn record_and_page_trash_stay_in_sync_with_database_cascade() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Trash", "").unwrap();
    let db = database(&mut store, &workspace.id);
    let first = store
        .create_record(&db.id, "First", &BTreeMap::new())
        .unwrap();
    let second = store
        .create_record(&db.id, "Second", &BTreeMap::new())
        .unwrap();
    let unlinked = store
        .create_record(&db.id, "Third", &BTreeMap::new())
        .unwrap();
    let first_page = store.ensure_record_page(&first.id).unwrap();
    let second_page = store.ensure_record_page(&second.id).unwrap();
    store
        .save_blocks(
            &first_page.id,
            &[Block::paragraph(&first_page.id, "firstblockunique", 0)],
        )
        .unwrap();
    store.delete_record(&first.id).unwrap();
    assert!(
        store
            .get_page(&first_page.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(
        store
            .search("firstblockunique", Some(&workspace.id), 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .query_records(&db.id, &RecordQuery::default())
            .unwrap()
            .len(),
        2
    );
    store.restore_record(&first.id).unwrap();
    assert_eq!(
        store
            .search("firstblockunique", Some(&workspace.id), 10)
            .unwrap()
            .len(),
        1
    );
    store.delete_page(&first_page.id).unwrap();
    assert!(
        store
            .get_record(&first.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    store.restore_page(&first_page.id).unwrap();
    assert!(
        store
            .get_record(&first.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none()
    );
    store.delete_record(&first.id).unwrap();
    store.delete_page(&db.page_id).unwrap();
    assert!(
        store
            .get_record(&second.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(
        store
            .get_record(&unlinked.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(
        store
            .get_page(&second_page.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(store.restore_record(&second.id).is_err());
    store.restore_page(&db.page_id).unwrap();
    assert!(
        store
            .get_record(&first.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(
        store
            .get_page(&first_page.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(
        store
            .get_record(&second.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none()
    );
    assert!(
        store
            .get_record(&unlinked.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none()
    );
    assert_eq!(
        store
            .query_records(&db.id, &RecordQuery::default())
            .unwrap()
            .len(),
        2
    );
    store.restore_record(&first.id).unwrap();
    assert_eq!(
        store
            .query_records(&db.id, &RecordQuery::default())
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn archives_use_the_moss_format() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Archive", "").unwrap();
    let archive = store.export_workspace(&workspace.id).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&archive).unwrap();
    assert_eq!(value["format"], "moss-workspace");
    let imported = store.import_workspace(&value.to_string()).unwrap();
    assert_eq!(imported.name, workspace.name);
    value["format"] = serde_json::json!("invalid format-workspace");
    assert!(store.import_workspace(&value.to_string()).is_err());
}

#[test]
fn archive_preserves_record_trash_roots() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Archive trash", "").unwrap();
    let db = database(&mut store, &workspace.id);
    let first = store
        .create_record(&db.id, "Already deleted", &BTreeMap::new())
        .unwrap();
    let second = store
        .create_record(&db.id, "Deleted with database", &BTreeMap::new())
        .unwrap();
    store.ensure_record_page(&first.id).unwrap();
    store.ensure_record_page(&second.id).unwrap();
    store.delete_record(&first.id).unwrap();
    store.delete_page(&db.page_id).unwrap();
    let archive = store.export_workspace(&workspace.id).unwrap();
    let copy = store.import_workspace(&archive).unwrap();
    let trashed_db = store
        .list_trash(&copy.id)
        .unwrap()
        .into_iter()
        .find(|p| p.kind == PageKind::Database)
        .unwrap();
    store.restore_page(&trashed_db.id).unwrap();
    let copied_db = store.get_database_by_page(&trashed_db.id).unwrap().unwrap();
    let records = store
        .query_records(&copied_db.id, &RecordQuery::default())
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].title, "Deleted with database");
}

#[test]
fn record_page_lookup_reports_duplicate_legacy_links() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Duplicates", "").unwrap();
    let db = database(&mut store, &workspace.id);
    let first = store
        .create_record(&db.id, "First", &BTreeMap::new())
        .unwrap();
    let second = store
        .create_record(&db.id, "Second", &BTreeMap::new())
        .unwrap();
    let page = store.ensure_record_page(&first.id).unwrap();
    store
        .conn
        .execute(
            "UPDATE records SET page_id=?1 WHERE id=?2",
            rusqlite::params![page.id, second.id],
        )
        .unwrap();
    assert!(store.get_record_by_page(&page.id).is_err());
}
