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
    let path = std::env::temp_dir().join(format!("rotion-test-{}.sqlite", new_id()));
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
    let path = std::env::temp_dir().join(format!("rotion-expiry-{}.sqlite", new_id()));
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
