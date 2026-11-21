use crate::{
    Block, BlockKind, Page, Record, Store, now, search,
    store::{decode, encode, invalid},
};
use rusqlite::{Connection, OptionalExtension, Result, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryActor {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, PageKind, Property, PropertyKind, new_id};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn text_changes_coalesce_only_for_same_actor_within_window() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        let page = s
            .create_page(&workspace.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let actor = |id: &str| HistoryActor {
            id: id.into(),
            name: id.into(),
            kind: "person".into(),
        };
        s.set_history_actor(Some(actor("one"))).unwrap();
        let mut block = Block::paragraph(&page.id, "a", 0);
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        block.text = "ab".into();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        block.text = "abc".into();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        let history = s
            .page_history(&workspace.id, &page.id, None, None, 20)
            .unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].before.as_ref().unwrap()["text"], "a");
        assert_eq!(history[0].after.as_ref().unwrap()["text"], "abc");
        s.set_history_actor(Some(actor("two"))).unwrap();
        block.text = "abcd".into();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        assert_eq!(
            s.page_history(&workspace.id, &page.id, None, None, 20)
                .unwrap()
                .len(),
            3
        );
        s.conn.execute("UPDATE change_history SET occurred_at='2000-01-01T00:00:00Z' WHERE sequence=(SELECT MAX(sequence) FROM change_history)", []).unwrap();
        block.text = "abcde".into();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        assert_eq!(
            s.page_history(&workspace.id, &page.id, None, None, 20)
                .unwrap()
                .len(),
            4
        );
        block.depth = 1;
        block.text = "abcdef".into();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        assert_eq!(
            s.page_history(&workspace.id, &page.id, None, None, 20)
                .unwrap()
                .len(),
            5
        );
        let history = s
            .page_history(&workspace.id, &page.id, None, None, 20)
            .unwrap();
        s.restore_history_block(&workspace.id, &page.id, history[1].sequence, Some(&block))
            .unwrap();
        assert_eq!(
            s.page_history(&workspace.id, &page.id, None, None, 20)
                .unwrap()
                .len(),
            6
        );
    }

    #[test]
    fn history_is_scoped_paged_and_records_atomic_changes() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        let other = s.create_workspace("Two", "").unwrap();
        let page = s
            .create_page(&workspace.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let actor = HistoryActor {
            id: "agent-1".into(),
            name: "Codex".into(),
            kind: "agent".into(),
        };
        s.set_history_actor(Some(actor.clone())).unwrap();
        let mut block = Block::paragraph(&page.id, "Before", 0);
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        block.text = "After".into();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        s.rename_page_if_unchanged(&page.id, "Page", "Renamed")
            .unwrap();
        assert!(
            s.rename_page_if_unchanged(&page.id, "Page", "Lost")
                .is_err()
        );
        let entries = s
            .page_history(&workspace.id, &page.id, None, None, 200)
            .unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].property_id.as_deref(), Some("title"));
        assert_eq!(entries[1].before.as_ref().unwrap()["text"], "Before");
        assert_eq!(entries[1].after.as_ref().unwrap()["text"], "After");
        assert_eq!(entries[1].actor, Some(actor));
        let change_sequence = entries[1].sequence;
        let next = s
            .page_history(&workspace.id, &page.id, None, Some(entries[0].sequence), 1)
            .unwrap();
        assert_eq!(next[0].sequence, entries[1].sequence);
        assert_eq!(
            s.page_history(&workspace.id, &page.id, Some(&block.id), None, 20)
                .unwrap()
                .len(),
            2
        );
        assert!(s.page_history(&other.id, &page.id, None, None, 20).is_err());

        let mut stale = block.clone();
        stale.text = "stale".into();
        assert!(
            s.restore_history_block(&workspace.id, &page.id, change_sequence, Some(&stale))
                .is_err()
        );
        assert_eq!(s.get_blocks(&page.id).unwrap()[0].text, "After");
        let restored = s
            .restore_history_block(&workspace.id, &page.id, change_sequence, Some(&block))
            .unwrap();
        assert_eq!(restored[0].text, "Before");
        assert_eq!(
            s.page_history(&workspace.id, &page.id, Some(&block.id), None, 20)
                .unwrap()[0]
                .after
                .as_ref()
                .unwrap()["text"],
            "Before"
        );

        s.conn.execute_batch("CREATE TRIGGER fail_history BEFORE INSERT ON change_history BEGIN SELECT RAISE(ABORT, 'history unavailable'); END; CREATE TRIGGER fail_history_update BEFORE UPDATE ON change_history BEGIN SELECT RAISE(ABORT, 'history unavailable'); END;").unwrap();
        block.text = "Must not save".into();
        assert!(s.save_blocks(&page.id, &[block]).is_err());
        assert_eq!(s.get_blocks(&page.id).unwrap()[0].text, "Before");
        assert!(
            s.rename_page_if_unchanged(&page.id, "Renamed", "Must not save")
                .is_err()
        );
        assert_eq!(s.get_page(&page.id).unwrap().unwrap().title, "Renamed");
    }

    #[test]
    fn cell_history_records_values_and_rejects_failed_updates() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
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
                    id: "note".into(),
                    name: "Note".into(),
                    kind: PropertyKind::Text,
                    config: json!({}),
                },
            ],
            views: vec![],
        };
        let page = s
            .create_database_page(&workspace.id, None, "Tasks", "", db.clone())
            .unwrap();
        let record = s.create_record(&db.id, "Task", &BTreeMap::new()).unwrap();
        s.set_record_value_if_unchanged(&record.id, "note", &Value::Null, json!("New"))
            .unwrap();
        assert!(
            s.set_record_value_if_unchanged(&record.id, "note", &Value::Null, json!("Wrong"))
                .is_err()
        );
        let entries = s
            .page_history(&workspace.id, &page.id, Some(&record.id), None, 20)
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].property_id.as_deref(), Some("note"));
        assert_eq!(entries[0].before, Some(Value::Null));
        assert_eq!(entries[0].after, Some(json!("New")));
        assert_eq!(entries[0].actor, None);
        assert!(
            s.restore_history_record_value(
                &workspace.id,
                &page.id,
                entries[0].sequence,
                &json!("stale")
            )
            .is_err()
        );
        let restored = s
            .restore_history_record_value(
                &workspace.id,
                &page.id,
                entries[0].sequence,
                &json!("New"),
            )
            .unwrap();
        assert_eq!(restored.values["note"], Value::Null);
        let history = s
            .page_history(&workspace.id, &page.id, Some(&record.id), None, 20)
            .unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].after, Some(Value::Null));
    }

    #[test]
    fn direct_record_saves_record_changes_and_roll_back_with_history() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Tasks".into(),
            properties: vec![Property {
                id: "note".into(),
                name: "Note".into(),
                kind: PropertyKind::Text,
                config: json!({}),
            }],
            views: vec![],
        };
        let page = s
            .create_database_page(&workspace.id, None, "Tasks", "", db.clone())
            .unwrap();
        let mut record = s.create_record(&db.id, "Task", &BTreeMap::new()).unwrap();
        record.title = "Renamed".into();
        record.values.insert("note".into(), json!("Changed"));
        s.save_record(&record).unwrap();
        let entries = s
            .page_history(&workspace.id, &page.id, Some(&record.id), None, 20)
            .unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.entity_kind == "record_title"
            && e.before == Some(json!("Task"))
            && e.after == Some(json!("Renamed"))));
        assert!(
            entries
                .iter()
                .any(|e| e.property_id.as_deref() == Some("note")
                    && e.before == Some(Value::Null)
                    && e.after == Some(json!("Changed")))
        );
        s.conn.execute_batch("CREATE TRIGGER fail_history BEFORE INSERT ON change_history BEGIN SELECT RAISE(ABORT, 'history unavailable'); END;").unwrap();
        record.values.insert("note".into(), json!("Must not save"));
        assert!(s.save_record(&record).is_err());
        assert_eq!(
            s.get_record(&record.id).unwrap().unwrap().values["note"],
            json!("Changed")
        );
    }

    #[test]
    fn title_and_text_cell_typing_are_grouped_and_restore_is_separate() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        s.set_history_actor(Some(HistoryActor {
            id: "user".into(),
            name: "You".into(),
            kind: "person".into(),
        }))
        .unwrap();
        let page = s
            .create_page(&workspace.id, None, "a", "", PageKind::Document)
            .unwrap();
        s.rename_page_if_unchanged(&page.id, "a", "ab").unwrap();
        s.rename_page_if_unchanged(&page.id, "ab", "abc").unwrap();
        let history = s
            .page_history(&workspace.id, &page.id, None, None, 20)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].before, Some(json!("a")));
        assert_eq!(history[0].after, Some(json!("abc")));

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
                    id: "note".into(),
                    name: "Note".into(),
                    kind: PropertyKind::Text,
                    config: json!({}),
                },
            ],
            views: vec![],
        };
        let database_page = s
            .create_database_page(&workspace.id, None, "Tasks", "", db.clone())
            .unwrap();
        let record = s.create_record(&db.id, "Task", &BTreeMap::new()).unwrap();
        s.set_record_value_if_unchanged(&record.id, "note", &Value::Null, json!("a"))
            .unwrap();
        s.set_record_value_if_unchanged(&record.id, "note", &json!("a"), json!("ab"))
            .unwrap();
        let history = s
            .page_history(&workspace.id, &database_page.id, None, None, 20)
            .unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].before, Some(Value::Null));
        assert_eq!(history[0].after, Some(json!("ab")));
        s.restore_history_record_value(
            &workspace.id,
            &database_page.id,
            history[0].sequence,
            &json!("ab"),
        )
        .unwrap();
        assert_eq!(
            s.page_history(&workspace.id, &database_page.id, None, None, 20)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn record_lifecycle_is_attributed_archived_and_atomic() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Tasks".into(),
            properties: vec![],
            views: vec![],
        };
        let page = s
            .create_database_page(&workspace.id, None, "Tasks", "", db.clone())
            .unwrap();
        let actor = HistoryActor {
            id: "user".into(),
            name: "You".into(),
            kind: "person".into(),
        };
        s.set_history_actor(Some(actor.clone())).unwrap();
        let record = s.create_record(&db.id, "Task", &BTreeMap::new()).unwrap();
        let copy = s.duplicate_board_record(&db.id, &record.id).unwrap();
        s.delete_record(&record.id).unwrap();
        s.restore_record(&record.id).unwrap();
        s.ensure_record_page(&copy.id).unwrap();
        s.delete_record(&copy.id).unwrap();
        s.restore_record(&copy.id).unwrap();
        let entries = s
            .page_history(&workspace.id, &page.id, None, None, 20)
            .unwrap();
        for (id, kinds) in [
            (
                &record.id,
                ["record_created", "record_trashed", "record_restored"],
            ),
            (
                &copy.id,
                ["record_duplicated", "record_trashed", "record_restored"],
            ),
        ] {
            for kind in kinds {
                assert!(entries.iter().any(|entry| &entry.entity_id == id
                    && entry.entity_kind == kind
                    && entry.actor == Some(actor.clone())));
            }
        }
        let archive = s.export_workspace(&workspace.id).unwrap();
        let imported = s.import_workspace(&archive).unwrap();
        let imported_page = s
            .list_pages(&imported.id)
            .unwrap()
            .into_iter()
            .find(|p| p.kind == PageKind::Database)
            .unwrap();
        let imported_entries = s
            .page_history(&imported.id, &imported_page.id, None, None, 20)
            .unwrap();
        assert_eq!(imported_entries.len(), entries.len());
        assert!(imported_entries.iter().any(|entry| entry.entity_kind == "record_duplicated" && entry.entity_id != copy.id));

        s.conn.execute_batch("CREATE TRIGGER fail_lifecycle_history BEFORE INSERT ON change_history BEGIN SELECT RAISE(ABORT, 'history unavailable'); END;").unwrap();
        assert!(s.delete_record(&record.id).is_err());
        assert!(
            s.get_record(&record.id)
                .unwrap()
                .unwrap()
                .deleted_at
                .is_none()
        );
        assert!(s.delete_record(&copy.id).is_err());
        assert!(
            s.get_record(&copy.id)
                .unwrap()
                .unwrap()
                .deleted_at
                .is_none()
        );
        assert!(s.create_record(&db.id, "Failed", &BTreeMap::new()).is_err());
        assert!(s.duplicate_board_record(&db.id, &record.id).is_err());
    }

    #[test]
    fn discarded_record_drafts_do_not_leave_creation_events() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Tasks".into(),
            properties: vec![],
            views: vec![],
        };
        let db_page = s
            .create_database_page(&workspace.id, None, "Tasks", "", db.clone())
            .unwrap();
        let first = s.create_record(&db.id, "Draft", &BTreeMap::new()).unwrap();
        assert!(
            s.discard_untouched_item(Some(&first), None, &[], None)
                .unwrap()
        );
        let second = s
            .create_record(&db.id, "Draft page", &BTreeMap::new())
            .unwrap();
        let page = s.ensure_record_page(&second.id).unwrap();
        let second = s.get_record(&second.id).unwrap().unwrap();
        assert!(
            s.discard_untouched_item(Some(&second), Some(&page), &[], None)
                .unwrap()
        );
        assert!(
            s.page_history(&workspace.id, &db_page.id, None, None, 20)
                .unwrap()
                .is_empty()
        );
        let saved = s.create_record(&db.id, "Saved", &BTreeMap::new()).unwrap();
        s.delete_record(&saved.id).unwrap();
        let entries = s
            .page_history(&workspace.id, &db_page.id, Some(&saved.id), None, 20)
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.entity_kind == "record_created"));
        assert!(entries.iter().any(|e| e.entity_kind == "record_trashed"));
    }

    #[test]
    fn archive_preserves_history_and_remaps_removed_blocks() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        s.set_history_actor(Some(HistoryActor {
            id: "agent".into(),
            name: "Codex".into(),
            kind: "agent".into(),
        }))
        .unwrap();
        let page = s
            .create_page(&workspace.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let mut block = Block::paragraph(&page.id, "old", 0);
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        block.text = "new".into();
        s.save_blocks(&page.id, &[block.clone()]).unwrap();
        s.save_blocks(&page.id, &[]).unwrap();
        let archive = s.export_workspace(&workspace.id).unwrap();
        let imported = s.import_workspace(&archive).unwrap();
        let imported_page = s.list_pages(&imported.id).unwrap().remove(0);
        let entries = s
            .page_history(&imported.id, &imported_page.id, None, None, 20)
            .unwrap();
        assert_eq!(entries.len(), 3);
        assert_ne!(entries[0].entity_id, block.id);
        assert!(entries.iter().all(|entry| {
            entry.entity_id == entries[0].entity_id
                && entry
                    .actor
                    .as_ref()
                    .is_some_and(|actor| actor.kind == "agent")
        }));
        assert_eq!(
            entries[1].before.as_ref().unwrap()["page_id"],
            imported_page.id
        );
        assert_eq!(
            entries[1].before.as_ref().unwrap()["id"],
            entries[0].entity_id
        );
        assert_eq!(entries[1].before.as_ref().unwrap()["text"], "old");
    }

    #[test]
    fn comment_actor_type_is_explicit_and_survives_archive() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("One", "").unwrap();
        let page = s
            .create_page(&workspace.id, None, "Page", "", PageKind::Document)
            .unwrap();
        s.set_history_actor(Some(HistoryActor {
            id: "agent".into(),
            name: "Codex".into(),
            kind: "agent".into(),
        }))
        .unwrap();
        let thread = s
            .create_comment(
                &workspace.id,
                &page.id,
                crate::comments::CommentAnchor::Page,
                "Codex",
                "First",
            )
            .unwrap();
        s.reply_comment(&workspace.id, &thread.id, "Someone else", "Second")
            .unwrap();
        let messages = &s.list_comments(&workspace.id, &page.id).unwrap()[0].messages;
        assert_eq!(messages[0].kind.as_deref(), Some("agent"));
        assert_eq!(messages[1].kind, None);
        let archive = s.export_workspace(&workspace.id).unwrap();
        let imported = s.import_workspace(&archive).unwrap();
        let imported_page = s.list_pages(&imported.id).unwrap().remove(0);
        let messages = &s.list_comments(&imported.id, &imported_page.id).unwrap()[0].messages;
        assert_eq!(messages[0].kind.as_deref(), Some("agent"));
        assert_eq!(messages[1].kind, None);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub sequence: i64,
    pub workspace_id: String,
    pub page_id: String,
    pub entity_kind: String,
    pub entity_id: String,
    pub property_id: Option<String>,
    pub actor: Option<HistoryActor>,
    pub occurred_at: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

pub(crate) fn install_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS change_history (
        sequence INTEGER PRIMARY KEY AUTOINCREMENT,
        workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
        entity_kind TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        property_id TEXT,
        actor TEXT,
        occurred_at TEXT NOT NULL,
        before_json TEXT,
        after_json TEXT
    );
    CREATE INDEX IF NOT EXISTS change_history_page ON change_history(workspace_id, page_id, sequence DESC);
    CREATE INDEX IF NOT EXISTS change_history_entity ON change_history(workspace_id, page_id, entity_id, sequence DESC);")
}

pub(crate) fn record_change(
    tx: &Transaction<'_>,
    page: &Page,
    kind: &str,
    id: &str,
    property: Option<&str>,
    before: Option<&Value>,
    after: Option<&Value>,
    actor: Option<&HistoryActor>,
) -> Result<()> {
    if before == after {
        return Ok(());
    }
    tx.execute(
        "INSERT INTO change_history(workspace_id,page_id,entity_kind,entity_id,property_id,actor,occurred_at,before_json,after_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![page.workspace_id, page.id, kind, id, property, actor.map(encode).transpose()?, now(), before.map(encode).transpose()?, after.map(encode).transpose()?],
    )?;
    Ok(())
}

pub(crate) fn record_text_change(
    tx: &Transaction<'_>,
    page: &Page,
    kind: &str,
    id: &str,
    property: &str,
    before: &Value,
    after: &Value,
    actor: Option<&HistoryActor>,
    allow_coalescing: bool,
) -> Result<()> {
    if allow_coalescing && before.is_string() && after.is_string() && before != after {
        if let Some(actor) = actor {
            let last: Option<(i64, String, String, Option<String>, Option<String>, Option<String>, String, Option<String>)> = tx.query_row(
                "SELECT sequence,entity_kind,entity_id,property_id,actor,before_json,occurred_at,after_json FROM change_history WHERE workspace_id=?1 AND page_id=?2 ORDER BY sequence DESC LIMIT 1",
                params![page.workspace_id, page.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
            ).optional()?;
            if let Some((
                sequence,
                last_kind,
                last_id,
                last_property,
                last_actor,
                initial,
                at,
                latest,
            )) = last
            {
                let within = chrono::DateTime::parse_from_rfc3339(&at)
                    .ok()
                    .is_some_and(|time| {
                        (0..=3000).contains(
                            &chrono::Utc::now()
                                .signed_duration_since(time)
                                .num_milliseconds(),
                        )
                    });
                let initial = initial.map(decode::<Value>).transpose()?;
                let latest = latest.map(decode::<Value>).transpose()?;
                if within
                    && last_kind == kind
                    && last_id == id
                    && last_property.as_deref() == Some(property)
                    && last_actor.as_deref() == Some(encode(actor)?.as_str())
                    && initial.is_some()
                    && initial.as_ref() != Some(after)
                    && latest.as_ref() == Some(before)
                {
                    tx.execute(
                        "UPDATE change_history SET after_json=?1 WHERE sequence=?2",
                        params![encode(after)?, sequence],
                    )?;
                    return Ok(());
                }
            }
        }
    }
    record_change(
        tx,
        page,
        kind,
        id,
        Some(property),
        Some(before),
        Some(after),
        actor,
    )
}

pub(crate) fn record_block_changes(
    tx: &Transaction<'_>,
    page: &Page,
    saved: &[Block],
    actor: Option<&HistoryActor>,
    allow_coalescing: bool,
) -> Result<()> {
    let mut stmt = tx.prepare("SELECT id,data FROM blocks WHERE page_id=?1")?;
    let old = stmt
        .query_map([&page.id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>>>()?;
    let mut before: BTreeMap<String, Value> = old
        .into_iter()
        .map(|(id, data)| Ok((id, decode(data)?)))
        .collect::<Result<_>>()?;
    for block in saved {
        let previous = before.remove(&block.id);
        let next: Value = decode(encode(block)?)?;
        if let (true, Some(previous), Some(actor)) = (allow_coalescing, previous.as_ref(), actor) {
            if coalesce_text_change(tx, page, previous, &next, actor)? {
                continue;
            }
        }
        record_change(
            tx,
            page,
            "block",
            &block.id,
            None,
            previous.as_ref(),
            Some(&next),
            actor,
        )?;
    }
    for (id, previous) in before {
        record_change(tx, page, "block", &id, None, Some(&previous), None, actor)?;
    }
    Ok(())
}

fn coalesce_text_change(
    tx: &Transaction<'_>,
    page: &Page,
    before: &Value,
    after: &Value,
    actor: &HistoryActor,
) -> Result<bool> {
    let old: Block = decode(encode(before)?)?;
    let new: Block = decode(encode(after)?)?;
    if old.text == new.text {
        return Ok(false);
    }
    let strip_text = |mut block: Block| {
        block.text.clear();
        if let Some(config) = block.config.as_object_mut() {
            for key in ["html", "marks", "paragraphs", "soft_breaks"] {
                config.remove(key);
            }
        }
        block
    };
    if strip_text(old) != strip_text(new) {
        return Ok(false);
    }
    let last: Option<(i64, String, String, Option<String>, Option<String>, String)> = tx.query_row(
        "SELECT sequence,entity_kind,entity_id,actor,before_json,occurred_at FROM change_history WHERE workspace_id=?1 AND page_id=?2 ORDER BY sequence DESC LIMIT 1",
        params![page.workspace_id, page.id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    ).optional()?;
    let Some((sequence, kind, id, last_actor, first_before, at)) = last else {
        return Ok(false);
    };
    let Some(first_before) = first_before else {
        return Ok(false);
    };
    let within_window = chrono::DateTime::parse_from_rfc3339(&at)
        .ok()
        .is_some_and(|time| {
            let elapsed = chrono::Utc::now()
                .signed_duration_since(time)
                .num_milliseconds();
            (0..=3000).contains(&elapsed)
        });
    if kind != "block"
        || id != before["id"].as_str().unwrap_or("")
        || last_actor.as_deref() != Some(encode(actor)?.as_str())
        || !within_window
    {
        return Ok(false);
    }
    let first: Value = decode(first_before)?;
    if first == *after {
        return Ok(false);
    }
    let latest: Option<String> = tx.query_row(
        "SELECT after_json FROM change_history WHERE sequence=?1",
        [sequence],
        |row| row.get(0),
    )?;
    if latest.map(decode::<Value>).transpose()?.as_ref() != Some(before) {
        return Ok(false);
    }
    tx.execute(
        "UPDATE change_history SET after_json=?1 WHERE sequence=?2",
        params![encode(after)?, sequence],
    )?;
    Ok(true)
}

impl Store {
    /// Set the actor used for writes on this connection. None means unknown.
    pub fn set_history_actor(&mut self, actor: Option<HistoryActor>) -> Result<()> {
        if actor.as_ref().is_some_and(|a| {
            a.id.trim().is_empty()
                || a.name.trim().is_empty()
                || !matches!(a.kind.as_str(), "person" | "agent")
        }) {
            return Err(invalid("invalid history actor"));
        }
        self.history_actor = actor;
        Ok(())
    }

    fn history_entry(
        &self,
        workspace_id: &str,
        page_id: &str,
        sequence: i64,
    ) -> Result<HistoryEntry> {
        let cursor = sequence
            .checked_add(1)
            .ok_or_else(|| invalid("invalid history sequence"))?;
        self.page_history(workspace_id, page_id, None, Some(cursor), 1)?
            .into_iter()
            .find(|entry| entry.sequence == sequence)
            .ok_or_else(|| invalid("history entry missing"))
    }

    /// Restore one text block against the exact current block shown to the user.
    pub fn restore_history_block(
        &mut self,
        workspace_id: &str,
        page_id: &str,
        sequence: i64,
        expected: Option<&Block>,
    ) -> Result<Vec<Block>> {
        let entry = self.history_entry(workspace_id, page_id, sequence)?;
        if entry.entity_kind != "block" {
            return Err(invalid("history entry is not a block"));
        }
        let previous: Option<Block> = entry
            .before
            .map(|value| decode(encode(&value)?))
            .transpose()?;
        let safe = |block: &Block| {
            matches!(
                block.kind,
                BlockKind::Paragraph
                    | BlockKind::Heading1
                    | BlockKind::Heading2
                    | BlockKind::Heading3
                    | BlockKind::BulletedList
                    | BlockKind::NumberedList
                    | BlockKind::Todo
                    | BlockKind::Toggle
                    | BlockKind::Quote
                    | BlockKind::Callout
                    | BlockKind::Code
                    | BlockKind::Divider
            )
        };
        if previous.as_ref().is_some_and(|block| {
            block.id != entry.entity_id || block.page_id != page_id || !safe(block)
        }) || expected.is_some_and(|block| !safe(block))
        {
            return Err(invalid("this block type cannot be restored from history"));
        }
        let current = self.get_blocks(page_id)?;
        let actual = current.iter().find(|block| block.id == entry.entity_id);
        if actual != expected {
            return Err(invalid("block changed since the history preview"));
        }
        let mut next = current.clone();
        next.retain(|block| block.id != entry.entity_id);
        if let Some(block) = previous {
            let position = usize::try_from(block.sort_order)
                .unwrap_or(0)
                .min(next.len());
            next.insert(position, block);
        }
        for (index, block) in next.iter_mut().enumerate() {
            block.sort_order = index as i64;
        }
        let coalesce = self.history_coalesce;
        self.history_coalesce = false;
        let result = self.save_editor_blocks_if_unchanged(page_id, &current, &next);
        self.history_coalesce = coalesce;
        result
    }

    /// Restore one database property only if its current value still matches the preview.
    pub fn restore_history_record_value(
        &mut self,
        workspace_id: &str,
        page_id: &str,
        sequence: i64,
        expected: &Value,
    ) -> Result<Record> {
        let entry = self.history_entry(workspace_id, page_id, sequence)?;
        if entry.entity_kind != "record" {
            return Err(invalid("history entry is not a record"));
        }
        let property = entry
            .property_id
            .ok_or_else(|| invalid("history property missing"))?;
        let previous = entry
            .before
            .ok_or_else(|| invalid("previous value unavailable"))?;
        let coalesce = self.history_coalesce;
        self.history_coalesce = false;
        let result =
            self.set_record_value_if_unchanged(&entry.entity_id, &property, expected, previous);
        self.history_coalesce = coalesce;
        result
    }

    /// Read newest changes first. `before` is an exclusive sequence cursor.
    pub fn page_history(
        &self,
        workspace_id: &str,
        page_id: &str,
        entity_id: Option<&str>,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<HistoryEntry>> {
        self.page_history_filtered(workspace_id, page_id, entity_id, None, before, limit)
    }

    pub fn item_history(
        &self,
        workspace_id: &str,
        page_id: &str,
        entity_id: &str,
        property_id: Option<&str>,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<HistoryEntry>> {
        self.page_history_filtered(
            workspace_id,
            page_id,
            Some(entity_id),
            property_id,
            before,
            limit,
        )
    }

    fn page_history_filtered(
        &self,
        workspace_id: &str,
        page_id: &str,
        entity_id: Option<&str>,
        property_id: Option<&str>,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<HistoryEntry>> {
        let page = search::page(&self.conn, page_id)?.ok_or_else(|| invalid("page missing"))?;
        if page.workspace_id != workspace_id {
            return Err(invalid("page is not in this workspace"));
        }
        let mut stmt = self.conn.prepare("SELECT sequence, workspace_id, page_id, entity_kind, entity_id, property_id, actor, occurred_at, before_json, after_json FROM change_history WHERE workspace_id=?1 AND page_id=?2 AND (?3 IS NULL OR entity_id=?3) AND (?4 IS NULL OR property_id=?4) AND (?5 IS NULL OR sequence<?5) ORDER BY sequence DESC LIMIT ?6")?;
        stmt.query_map(
            params![
                workspace_id,
                page_id,
                entity_id,
                property_id,
                before,
                limit.clamp(1, 200)
            ],
            |row| {
                let actor: Option<String> = row.get(6)?;
                let before: Option<String> = row.get(8)?;
                let after: Option<String> = row.get(9)?;
                Ok(HistoryEntry {
                    sequence: row.get(0)?,
                    workspace_id: row.get(1)?,
                    page_id: row.get(2)?,
                    entity_kind: row.get(3)?,
                    entity_id: row.get(4)?,
                    property_id: row.get(5)?,
                    actor: actor.map(decode).transpose()?,
                    occurred_at: row.get(7)?,
                    before: before.map(decode).transpose()?,
                    after: after.map(decode).transpose()?,
                })
            },
        )?
        .collect()
    }
}
