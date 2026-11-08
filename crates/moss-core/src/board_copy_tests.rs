use super::*;
use crate::{BlockKind, DatabaseView, Property, ViewKind};

fn fixture() -> (Store, Database) {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Board copy", "").unwrap();
    let database = Database {
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
                id: "points".into(),
                name: "Points".into(),
                kind: PropertyKind::Number,
                config: json!({}),
            },
            Property {
                id: "notes".into(),
                name: "Notes".into(),
                kind: PropertyKind::Text,
                config: json!({}),
            },
            Property {
                id: "formula".into(),
                name: "Double".into(),
                kind: PropertyKind::Formula,
                config: json!({"expression":"prop(\"Points\") * 2"}),
            },
            Property {
                id: "created".into(),
                name: "Created".into(),
                kind: PropertyKind::CreatedTime,
                config: json!({}),
            },
            Property {
                id: "edited".into(),
                name: "Edited".into(),
                kind: PropertyKind::EditedTime,
                config: json!({}),
            },
            Property {
                id: "unique".into(),
                name: "ID".into(),
                kind: PropertyKind::UniqueId,
                config: json!({}),
            },
        ],
        views: vec![DatabaseView {
            id: new_id(),
            name: "Board".into(),
            kind: ViewKind::Board,
            query: Default::default(),
            config: json!({}),
        }],
    };
    store
        .create_database_page(&workspace.id, None, "Tasks", "", database.clone())
        .unwrap();
    let database = store.get_database(&database.id).unwrap().unwrap();
    (store, database)
}

fn counts(store: &Store) -> (i64, i64, i64) {
    (
        store
            .conn
            .query_row("SELECT count(*) FROM records", [], |r| r.get(0))
            .unwrap(),
        store
            .conn
            .query_row("SELECT count(*) FROM pages", [], |r| r.get(0))
            .unwrap(),
        store
            .conn
            .query_row("SELECT count(*) FROM blocks", [], |r| r.get(0))
            .unwrap(),
    )
}

#[test]
fn board_copy_keeps_page_body_and_appearance_with_independent_ids() {
    let (mut store, db) = fixture();
    let values = BTreeMap::from([
        ("points".into(), json!(6)),
        ("notes".into(), json!("Original notes")),
        ("created".into(), json!("2000-01-01T00:00:00+00:00")),
        ("edited".into(), json!("2000-01-01T00:00:00+00:00")),
        ("unique".into(), json!("TASK-17")),
    ]);
    let source = store.create_record(&db.id, "Plan trip", &values).unwrap();
    let mut source_page = store.ensure_record_page(&source.id).unwrap();
    source_page.icon = "🧭".into();
    source_page.settings.small_text = true;
    source_page.cover = Some(crate::PageCover::Image {
        asset: "a".repeat(64),
        position: 0.3,
    });
    store.update_page(&source_page).unwrap();
    let child = store
        .create_page(
            &source_page.workspace_id,
            Some(&source_page.id),
            "Existing child",
            "",
            PageKind::Document,
        )
        .unwrap();
    let mut text = Block::paragraph(&source_page.id, "Body marker", 0);
    text.config = json!({"html":"<p><strong>Body marker</strong></p>","literal_id":text.id});
    let mut table = Block::paragraph(&source_page.id, "", 1);
    table.kind = BlockKind::SimpleTable;
    table.config = json!({"simple_table":{"version":1,"rows":[[text.id]],"header_row":true,"header_column":false}});
    let mut sync = Block::paragraph(&source_page.id, "", 2);
    sync.kind = BlockKind::SyncedBlock;
    sync.config = json!({"source_block_id":text.id,"page_id":source_page.id,"block_ids":[text.id]});
    let mut link = Block::paragraph(&source_page.id, "", 3);
    link.kind = BlockKind::ChildPage;
    link.config = json!({"page_id":child.id});
    store
        .save_blocks(
            &source_page.id,
            &[text.clone(), table.clone(), sync.clone(), link.clone()],
        )
        .unwrap();
    // A stale cached calculation must not be copied as the new record's result.
    let mut stale = store.get_record(&source.id).unwrap().unwrap();
    stale.values.insert("formula".into(), json!(999));
    store
        .conn
        .execute(
            "UPDATE records SET data=?1, values_json=?2 WHERE id=?3",
            params![
                encode(&stale).unwrap(),
                encode(&stale.values).unwrap(),
                source.id
            ],
        )
        .unwrap();
    let copied = store.duplicate_board_record(&db.id, &source.id).unwrap();
    assert_ne!(copied.id, source.id);
    assert_eq!(copied.title, "Plan trip (copy)");
    assert_eq!(copied.values["notes"], "Original notes");
    assert_eq!(copied.values["formula"].as_f64(), Some(12.));
    assert!(!copied.values.contains_key("unique"));
    assert_eq!(copied.values["created"], json!(copied.created_at));
    assert_eq!(copied.values["edited"], json!(copied.updated_at));
    let page = store
        .get_page(copied.page_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    assert_ne!(page.id, source_page.id);
    assert_eq!(page.parent_id, Some(db.page_id.clone()));
    assert_eq!(page.icon, source_page.icon);
    assert_eq!(page.cover, source_page.cover);
    assert_eq!(page.settings, source_page.settings);
    let blocks = store.get_blocks(&page.id).unwrap();
    assert_eq!(blocks.len(), 4);
    assert!(blocks.iter().all(|b| b.page_id == page.id));
    for (new, old) in blocks.iter().zip([&text, &table, &sync, &link]) {
        assert_ne!(new.id, old.id);
    }
    assert_eq!(blocks[0].text, text.text);
    assert_eq!(blocks[0].config["literal_id"], text.id);
    assert_eq!(
        blocks[1].config, table.config,
        "table cell text is not an ID reference"
    );
    assert_eq!(blocks[2].config["source_block_id"], blocks[0].id);
    assert_eq!(blocks[2].config["block_ids"], json!([blocks[0].id]));
    assert_eq!(blocks[2].config["page_id"], page.id);
    assert_eq!(blocks[3].config["page_id"], child.id);
    assert_eq!(
        store.get_page(&child.id).unwrap().unwrap().parent_id,
        Some(source_page.id.clone())
    );
    assert!(
        store
            .search("Body marker", Some(&page.workspace_id), 50)
            .unwrap()
            .iter()
            .any(|hit| hit.page_id == page.id)
    );
    store
        .set_record_value_if_unchanged(
            &copied.id,
            "notes",
            &json!("Original notes"),
            json!("New notes"),
        )
        .unwrap();
    assert_eq!(
        store.get_record(&source.id).unwrap().unwrap().values["notes"],
        "Original notes"
    );
    assert_eq!(store.get_blocks(&source_page.id).unwrap()[0].id, text.id);
}

#[test]
fn board_copy_without_body_stays_unmaterialized_and_rejects_wrong_or_deleted_sources() {
    let (mut store, db) = fixture();
    let source = store
        .create_record(&db.id, "Blank", &Default::default())
        .unwrap();
    let before = counts(&store);
    let copy = store.duplicate_board_record(&db.id, &source.id).unwrap();
    assert!(copy.page_id.is_none());
    assert_eq!(counts(&store), (before.0 + 1, before.1, before.2));
    let before = counts(&store);
    assert!(
        store
            .duplicate_board_record("another-db", &source.id)
            .is_err()
    );
    assert_eq!(counts(&store), before);
    store.delete_record(&source.id).unwrap();
    assert!(store.duplicate_board_record(&db.id, &source.id).is_err());
    assert_eq!(counts(&store), before);
    store.delete_page(&db.page_id).unwrap();
    assert!(store.duplicate_board_record(&db.id, &copy.id).is_err());
}

#[test]
fn board_copy_rolls_back_page_blocks_and_search_when_record_insert_fails() {
    let (mut store, db) = fixture();
    let source = store
        .create_record(&db.id, "Atomic", &Default::default())
        .unwrap();
    let page = store.ensure_record_page(&source.id).unwrap();
    store
        .save_blocks(
            &page.id,
            &[Block::paragraph(&page.id, "Rollback marker", 0)],
        )
        .unwrap();
    let before = counts(&store);
    store.conn.execute_batch("CREATE TRIGGER copy_failure BEFORE INSERT ON records WHEN NEW.title LIKE '% (copy)' BEGIN SELECT RAISE(ABORT,'test record insert failure'); END;").unwrap();
    assert!(store.duplicate_board_record(&db.id, &source.id).is_err());
    assert_eq!(counts(&store), before);
    assert!(
        store
            .search("Atomic copy", Some(&page.workspace_id), 50)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .search("Rollback marker", Some(&page.workspace_id), 50)
            .unwrap()
            .iter()
            .filter(|h| h.page_id == page.id)
            .count(),
        1
    );
}

#[test]
fn board_copy_keeps_relation_targets_and_recomputes_rollups() {
    let (mut store, db) = fixture();
    let db = store
        .save_advanced_property(
            &db.id,
            None,
            "Related",
            PropertyKind::Relation,
            json!({"target_database_id":db.id}),
            None,
        )
        .unwrap();
    let relation = db.properties.last().unwrap().id.clone();
    let db = store
        .save_advanced_property(
            &db.id,
            None,
            "Related points",
            PropertyKind::Rollup,
            json!({"relation_property_id":relation,"target_property_id":"points","operation":"sum"}),
            None,
        )
        .unwrap();
    let rollup = db.properties.last().unwrap().id.clone();
    let target = store
        .create_record(&db.id, "Target", &[("points".into(), json!(7))].into())
        .unwrap();
    let source = store
        .create_record(
            &db.id,
            "Source",
            &[(relation.clone(), json!([target.id]))].into(),
        )
        .unwrap();
    let copy = store.duplicate_board_record(&db.id, &source.id).unwrap();
    assert_eq!(copy.values[&relation], json!([target.id]));
    assert_eq!(copy.values[&rollup].as_f64(), Some(7.));
    let edges: i64 = store
        .conn
        .query_row(
            "SELECT count(*) FROM calculation_edges WHERE source_id=?1 AND target_id=?2",
            params![copy.id, target.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(edges, 1);
    store.delete_record(&target.id).unwrap();
    let before = counts(&store);
    assert!(store.duplicate_board_record(&db.id, &source.id).is_err());
    assert_eq!(counts(&store), before);
}
