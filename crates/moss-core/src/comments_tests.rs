use super::*;
use crate::{Block, Database, PageKind, Property, PropertyKind, new_id};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn threads_preserve_replies_and_deleted_or_changed_targets() {
    let mut store = Store::in_memory().unwrap();
    let w = store.create_workspace("A", "").unwrap();
    let other = store.create_workspace("B", "").unwrap();
    let page = store
        .create_page(&w.id, None, "Page", "", PageKind::Document)
        .unwrap();
    let mut block = Block::paragraph(&page.id, "café example", 0);
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    let anchor = CommentAnchor::Text {
        block_id: block.id.clone(),
        start: 0,
        end: 5,
        quote: "café".into(),
    };
    assert!(
        store
            .create_comment(
                &other.id,
                &page.id,
                anchor.clone(),
                "Oscar",
                "wrong workspace"
            )
            .is_err()
    );
    assert!(
        store
            .create_comment(
                &w.id,
                &page.id,
                CommentAnchor::Text {
                    block_id: block.id.clone(),
                    start: 0,
                    end: 4,
                    quote: "café".into()
                },
                "Oscar",
                "bad UTF-8"
            )
            .is_err()
    );
    let thread = store
        .create_comment(&w.id, &page.id, anchor, "Oscar", "Why?")
        .unwrap();
    assert!(
        store
            .reply_comment(&other.id, &thread.id, "Codex", "No")
            .is_err()
    );
    store
        .reply_comment(&w.id, &thread.id, "Codex", "Because.")
        .unwrap();
    store.resolve_comment(&w.id, &thread.id, true).unwrap();
    let loaded = store.list_comments(&w.id, &page.id).unwrap();
    assert_eq!(loaded[0].messages.len(), 2);
    assert!(loaded[0].resolved && loaded[0].anchor_available);
    store.resolve_comment(&w.id, &thread.id, false).unwrap();
    block.text = "café changed".into();
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    assert!(store.list_comments(&w.id, &page.id).unwrap()[0].anchor_available);
    block.text = "cafe changed".into();
    store.save_blocks(&page.id, &[block]).unwrap();
    let changed = store.list_comments(&w.id, &page.id).unwrap();
    assert!(!changed[0].resolved && !changed[0].anchor_available);
    let restored = Block::paragraph(&page.id, "café example", 0);
    store.save_blocks(&page.id, &[restored]).unwrap();
    assert!(!store.list_comments(&w.id, &page.id).unwrap()[0].anchor_available);
    store.save_blocks(&page.id, &[]).unwrap();
    assert_eq!(
        store.list_comments(&w.id, &page.id).unwrap()[0]
            .messages
            .len(),
        2
    );
    assert!(store.list_comments(&other.id, &page.id).unwrap().is_empty());
}

#[test]
fn disjoint_edits_shift_text_anchor_and_overlaps_never_reattach() {
    let mut store = Store::in_memory().unwrap();
    let w = store.create_workspace("A", "").unwrap();
    let page = store
        .create_page(&w.id, None, "Page", "", PageKind::Document)
        .unwrap();
    let mut block = Block::paragraph(&page.id, "one café three", 0);
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    store
        .create_comment(
            &w.id,
            &page.id,
            CommentAnchor::Text {
                block_id: block.id.clone(),
                start: 4,
                end: 9,
                quote: "café".into(),
            },
            "Oscar",
            "Check",
        )
        .unwrap();
    block.text = "zero one café three".into();
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    let thread = store.list_comments(&w.id, &page.id).unwrap().remove(0);
    assert!(thread.anchor_available);
    assert!(matches!(
        thread.anchor,
        CommentAnchor::Text {
            start: 9,
            end: 14,
            ..
        }
    ));
    block.text = "zero one cafe three".into();
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    block.text = "zero one café three".into();
    store.save_blocks(&page.id, &[block]).unwrap();
    assert!(!store.list_comments(&w.id, &page.id).unwrap()[0].anchor_available);
}

#[test]
fn cell_anchor_is_scoped_and_survives_record_deletion() {
    let mut store = Store::in_memory().unwrap();
    let w = store.create_workspace("A", "").unwrap();
    let db = Database {
        id: new_id(),
        page_id: String::new(),
        name: "Tasks".into(),
        properties: vec![Property {
            id: "title".into(),
            name: "Name".into(),
            kind: PropertyKind::Title,
            config: json!({}),
        }],
        views: vec![],
    };
    let page = store
        .create_database_page(&w.id, None, "Tasks", "", db.clone())
        .unwrap();
    let record = store
        .create_record(&db.id, "Task", &BTreeMap::new())
        .unwrap();
    let anchor = CommentAnchor::Cell {
        record_id: record.id.clone(),
        property_id: "title".into(),
    };
    store
        .create_comment(&w.id, &page.id, anchor, "Oscar", "Check")
        .unwrap();
    store.delete_record(&record.id).unwrap();
    assert!(!store.list_comments(&w.id, &page.id).unwrap()[0].anchor_available);
    store.restore_record(&record.id).unwrap();
    assert!(store.list_comments(&w.id, &page.id).unwrap()[0].anchor_available);
}

#[test]
fn comments_round_trip_in_json_and_binary_archives() {
    let root = std::env::temp_dir().join(format!("moss-comment-test-{}", new_id()));
    std::fs::create_dir_all(root.join("data")).unwrap();
    let mut store = Store::open(root.join("data/workspace.sqlite3")).unwrap();
    let w = store.create_workspace("A", "").unwrap();
    let page = store
        .create_page(&w.id, None, "Page", "", PageKind::Document)
        .unwrap();
    let mut block = Block::paragraph(&page.id, "hello world", 0);
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    let live = store
        .create_comment(
            &w.id,
            &page.id,
            CommentAnchor::Text {
                block_id: block.id.clone(),
                start: 0,
                end: 5,
                quote: "hello".into(),
            },
            "Oscar",
            "First",
        )
        .unwrap();
    store
        .reply_comment(&w.id, &live.id, "Codex", "Reply")
        .unwrap();
    let dead = store
        .create_comment(
            &w.id,
            &page.id,
            CommentAnchor::Text {
                block_id: block.id.clone(),
                start: 6,
                end: 11,
                quote: "world".into(),
            },
            "Oscar",
            "Second",
        )
        .unwrap();
    store.resolve_comment(&w.id, &dead.id, true).unwrap();
    block.text = "hello there".into();
    store.save_blocks(&page.id, &[block.clone()]).unwrap();
    block.text = "hello world".into();
    store.save_blocks(&page.id, &[block]).unwrap();
    assert!(
        !store
            .list_comments(&w.id, &page.id)
            .unwrap()
            .iter()
            .find(|t| t.id == dead.id)
            .unwrap()
            .anchor_available
    );
    let db = Database {
        id: new_id(),
        page_id: String::new(),
        name: "Tasks".into(),
        properties: vec![Property {
            id: new_id(),
            name: "Title".into(),
            kind: PropertyKind::Title,
            config: json!({}),
        }],
        views: vec![],
    };
    let db_page = store
        .create_database_page(&w.id, None, "Tasks", "", db.clone())
        .unwrap();
    let record = store
        .create_record(&db.id, "Task", &BTreeMap::new())
        .unwrap();
    store
        .create_comment(
            &w.id,
            &db_page.id,
            CommentAnchor::Cell {
                record_id: record.id.clone(),
                property_id: db.properties[0].id.clone(),
            },
            "Oscar",
            "Cell",
        )
        .unwrap();
    let json = store.export_workspace(&w.id).unwrap();
    let json_ws = store.import_workspace(&json).unwrap();
    let binary = root.join("workspace.moss");
    store.export_workspace_to_file(&w.id, &binary).unwrap();
    let binary_ws = store.import_workspace_from_file(&binary).unwrap();
    std::fs::remove_file(binary).unwrap();
    for imported in [json_ws, binary_ws] {
        let pages = store.list_pages(&imported.id).unwrap();
        let page2 = pages.iter().find(|p| p.title == "Page").unwrap();
        assert_ne!(page2.id, page.id);
        let block2 = store.get_blocks(&page2.id).unwrap().remove(0);
        let threads = store.list_comments(&imported.id, &page2.id).unwrap();
        let live2 = threads
            .iter()
            .find(|t| t.messages[0].text == "First")
            .unwrap();
        let dead2 = threads
            .iter()
            .find(|t| t.messages[0].text == "Second")
            .unwrap();
        assert_ne!(live2.id, live.id);
        assert_eq!(live2.messages.len(), 2);
        assert!(live2.anchor_available && !live2.resolved);
        assert!(
            matches!(&live2.anchor, CommentAnchor::Text { block_id, .. } if *block_id == block2.id)
        );
        assert!(!dead2.anchor_available && dead2.resolved);
        let imported_db_page = pages.iter().find(|p| p.title == "Tasks").unwrap();
        let imported_db = store
            .get_database_by_page(&imported_db_page.id)
            .unwrap()
            .unwrap();
        let cell = store
            .list_comments(&imported.id, &imported_db_page.id)
            .unwrap()
            .remove(0);
        assert!(cell.anchor_available);
        assert!(
            matches!(cell.anchor, CommentAnchor::Cell { ref record_id, ref property_id } if *record_id != record.id && *property_id == imported_db.properties[0].id)
        );
    }
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn commented_drafts_are_not_discarded() {
    let mut store = Store::in_memory().unwrap();
    let w = store.create_workspace("A", "").unwrap();
    let page = store
        .create_page(&w.id, None, "", "", PageKind::Document)
        .unwrap();
    store
        .create_comment(&w.id, &page.id, CommentAnchor::Page, "Oscar", "Keep")
        .unwrap();
    assert!(
        !store
            .discard_untouched_item(None, Some(&page), &[], None)
            .unwrap()
    );
    assert_eq!(store.list_comments(&w.id, &page.id).unwrap().len(), 1);

    let db = Database {
        id: new_id(),
        page_id: String::new(),
        name: "Tasks".into(),
        properties: vec![Property {
            id: new_id(),
            name: "Title".into(),
            kind: PropertyKind::Title,
            config: json!({}),
        }],
        views: vec![],
    };
    let db_page = store
        .create_database_page(&w.id, None, "Tasks", "", db.clone())
        .unwrap();
    let record = store.create_record(&db.id, "", &BTreeMap::new()).unwrap();
    store
        .create_comment(
            &w.id,
            &db_page.id,
            CommentAnchor::Cell {
                record_id: record.id.clone(),
                property_id: db.properties[0].id.clone(),
            },
            "Oscar",
            "Keep",
        )
        .unwrap();
    assert!(
        !store
            .discard_untouched_item(Some(&record), None, &[], None)
            .unwrap()
    );
    assert!(store.get_record(&record.id).unwrap().is_some());
}

#[test]
fn child_page_changes_leave_deleted_block_comments_unavailable() {
    let mut store = Store::in_memory().unwrap();
    let w = store.create_workspace("A", "").unwrap();
    let parent = store
        .create_page(&w.id, None, "Parent", "", PageKind::Document)
        .unwrap();
    let empty = Block::paragraph(&parent.id, "", 0);
    store.save_blocks(&parent.id, &[empty.clone()]).unwrap();
    let comment = store
        .create_comment(
            &w.id,
            &parent.id,
            CommentAnchor::Block {
                block_id: empty.id.clone(),
            },
            "Oscar",
            "Original",
        )
        .unwrap();
    let (child, blocks) = store
        .create_child_page_block(&w.id, &parent.id, &[empty.clone()], &empty.id, false)
        .unwrap();
    assert!(
        !store
            .list_comments(&w.id, &parent.id)
            .unwrap()
            .iter()
            .find(|t| t.id == comment.id)
            .unwrap()
            .anchor_available
    );
    let link = blocks[0].clone();
    let link_comment = store
        .create_comment(
            &w.id,
            &parent.id,
            CommentAnchor::Block {
                block_id: link.id.clone(),
            },
            "Oscar",
            "Link",
        )
        .unwrap();
    assert!(
        store
            .discard_untouched_item(None, Some(&child), &[], Some(&link))
            .unwrap()
    );
    let mut recreated = link;
    recreated.config = serde_json::json!({});
    recreated.kind = crate::BlockKind::Paragraph;
    store.save_blocks(&parent.id, &[recreated]).unwrap();
    assert!(
        !store
            .list_comments(&w.id, &parent.id)
            .unwrap()
            .iter()
            .find(|t| t.id == link_comment.id)
            .unwrap()
            .anchor_available
    );
}
