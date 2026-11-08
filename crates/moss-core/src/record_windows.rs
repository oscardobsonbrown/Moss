use crate::{
    PageKind, Record, Store, search,
    store::{decode, invalid},
};
use rusqlite::{Connection, OptionalExtension, Result, TransactionBehavior, params};

const BUCKET: usize = 1024;
const MAX_BUCKET: u64 = 2048;
const MAX_WINDOW: u32 = 1000;

#[derive(Debug)]
pub struct RecordWindow {
    pub total: u64,
    pub revision: u64,
    pub start: u64,
    pub records: Vec<Record>,
}

#[derive(Debug)]
pub enum RecordWindowState {
    Ready(RecordWindow),
    Pending,
    Stale { revision: u64 },
}

pub(crate) fn install_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS record_rank_meta(version INTEGER NOT NULL);")?;
    let version: Option<i64> = conn
        .query_row("SELECT version FROM record_rank_meta LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()?;
    if version.is_some_and(|version| version >= 1) {
        return Ok(());
    }
    let result = conn.execute_batch(r#"
        SAVEPOINT record_rank_migration;
        CREATE TABLE IF NOT EXISTS record_rank_state (
            database_id TEXT PRIMARY KEY, revision INTEGER NOT NULL, ready INTEGER NOT NULL, total INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS record_rank_buckets (
            database_id TEXT NOT NULL, lower_id TEXT NOT NULL, count INTEGER NOT NULL,
            PRIMARY KEY(database_id, lower_id)
        ) WITHOUT ROWID;
        DROP TRIGGER IF EXISTS record_rank_insert;
        DROP TRIGGER IF EXISTS record_rank_delete;
        DROP TRIGGER IF EXISTS record_rank_update;
        DROP TRIGGER IF EXISTS record_rank_database_delete;
        CREATE TRIGGER record_rank_insert AFTER INSERT ON records BEGIN
            INSERT INTO record_rank_state SELECT NEW.database_id,0,0,0 WHERE NOT EXISTS (SELECT 1 FROM record_rank_state WHERE database_id=NEW.database_id);
            UPDATE record_rank_state SET revision=revision+1,
                total=total+CASE WHEN ready=1 AND NEW.deleted_at IS NULL THEN 1 ELSE 0 END
                WHERE database_id=NEW.database_id;
            UPDATE record_rank_buckets SET count=count+1 WHERE NEW.deleted_at IS NULL
                AND database_id=NEW.database_id
                AND (SELECT ready FROM record_rank_state WHERE database_id=NEW.database_id)=1
                AND lower_id=(SELECT lower_id FROM record_rank_buckets WHERE database_id=NEW.database_id AND lower_id<=NEW.id ORDER BY lower_id DESC LIMIT 1);
        END;
        CREATE TRIGGER record_rank_delete AFTER DELETE ON records BEGIN
            INSERT INTO record_rank_state SELECT OLD.database_id,0,0,0 WHERE NOT EXISTS (SELECT 1 FROM record_rank_state WHERE database_id=OLD.database_id);
            UPDATE record_rank_state SET revision=revision+1,
                total=total-CASE WHEN ready=1 AND OLD.deleted_at IS NULL THEN 1 ELSE 0 END
                WHERE database_id=OLD.database_id;
            UPDATE record_rank_buckets SET count=count-1 WHERE OLD.deleted_at IS NULL
                AND database_id=OLD.database_id
                AND (SELECT ready FROM record_rank_state WHERE database_id=OLD.database_id)=1
                AND lower_id=(SELECT lower_id FROM record_rank_buckets WHERE database_id=OLD.database_id AND lower_id<=OLD.id ORDER BY lower_id DESC LIMIT 1);
        END;
        CREATE TRIGGER record_rank_update AFTER UPDATE ON records BEGIN
            INSERT INTO record_rank_state SELECT OLD.database_id,0,0,0 WHERE NOT EXISTS (SELECT 1 FROM record_rank_state WHERE database_id=OLD.database_id);
            INSERT INTO record_rank_state SELECT NEW.database_id,0,0,0 WHERE NOT EXISTS (SELECT 1 FROM record_rank_state WHERE database_id=NEW.database_id);
            UPDATE record_rank_state SET revision=revision+1,
                total=total-CASE WHEN ready=1 AND OLD.deleted_at IS NULL THEN 1 ELSE 0 END
                           +CASE WHEN ready=1 AND OLD.database_id=NEW.database_id AND NEW.deleted_at IS NULL THEN 1 ELSE 0 END
                WHERE database_id=OLD.database_id;
            UPDATE record_rank_state SET revision=revision+1,
                total=total+CASE WHEN ready=1 AND NEW.deleted_at IS NULL THEN 1 ELSE 0 END
                WHERE database_id=NEW.database_id AND OLD.database_id<>NEW.database_id;
            UPDATE record_rank_buckets SET count=count-1 WHERE OLD.deleted_at IS NULL
                AND (OLD.id<>NEW.id OR OLD.database_id<>NEW.database_id OR OLD.deleted_at IS NOT NEW.deleted_at)
                AND database_id=OLD.database_id
                AND (SELECT ready FROM record_rank_state WHERE database_id=OLD.database_id)=1
                AND lower_id=(SELECT lower_id FROM record_rank_buckets WHERE database_id=OLD.database_id AND lower_id<=OLD.id ORDER BY lower_id DESC LIMIT 1);
            UPDATE record_rank_buckets SET count=count+1 WHERE NEW.deleted_at IS NULL
                AND (OLD.id<>NEW.id OR OLD.database_id<>NEW.database_id OR OLD.deleted_at IS NOT NEW.deleted_at)
                AND database_id=NEW.database_id
                AND (SELECT ready FROM record_rank_state WHERE database_id=NEW.database_id)=1
                AND lower_id=(SELECT lower_id FROM record_rank_buckets WHERE database_id=NEW.database_id AND lower_id<=NEW.id ORDER BY lower_id DESC LIMIT 1);
        END;
        CREATE TRIGGER record_rank_database_delete AFTER DELETE ON databases BEGIN
            DELETE FROM record_rank_buckets WHERE database_id=OLD.id;
            DELETE FROM record_rank_state WHERE database_id=OLD.id;
        END;
        DELETE FROM record_rank_meta;
        INSERT INTO record_rank_meta VALUES(1);
        RELEASE record_rank_migration;
    "#);
    if result.is_err() {
        let _ =
            conn.execute_batch("ROLLBACK TO record_rank_migration; RELEASE record_rank_migration;");
    }
    result
}

fn valid_database(conn: &Connection, database_id: &str) -> Result<()> {
    let raw: String = conn.query_row(
        "SELECT data FROM databases WHERE id=?1",
        [database_id],
        |row| row.get(0),
    )?;
    let db: crate::Database = decode(raw)?;
    let page = search::page(conn, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
    if page.deleted_at.is_some() || page.kind != PageKind::Database {
        return Err(invalid("database is not available"));
    }
    Ok(())
}

impl Store {
    /// Build a bounded-memory rank index. Call on a background write connection.
    pub fn build_record_rank(&mut self, database_id: &str) -> Result<()> {
        self.build_record_rank_with(database_id, || {})
    }

    fn build_record_rank_with(
        &mut self,
        database_id: &str,
        after_scan: impl FnOnce(),
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        valid_database(&tx, database_id)?;
        let mut buckets: Vec<(String, i64)> = vec![(String::new(), 0)];
        let mut after = String::new();
        loop {
            let ids: Vec<String> = {
                let mut statement = tx.prepare("SELECT id FROM records WHERE database_id=?1 AND deleted_at IS NULL AND id>?2 ORDER BY id LIMIT ?3")?;
                let rows = statement
                    .query_map(params![database_id, after, BUCKET as u32], |row| row.get(0))?;
                rows.collect::<Result<_>>()?
            };
            if ids.is_empty() {
                break;
            }
            if buckets.last().unwrap().1 > 0 {
                buckets.push((ids[0].clone(), 0));
            }
            buckets.last_mut().unwrap().1 = ids.len() as i64;
            after = ids.last().unwrap().clone();
        }
        let total: i64 = buckets.iter().map(|(_, count)| count).sum();
        after_scan();
        // Upgrading this snapshot fails if a concurrent connection wrote during the scan.
        tx.execute("INSERT INTO record_rank_state(database_id,revision,ready,total) VALUES(?1,0,1,?2) ON CONFLICT(database_id) DO UPDATE SET ready=1,total=excluded.total", params![database_id, total])?;
        tx.execute(
            "DELETE FROM record_rank_buckets WHERE database_id=?1",
            [database_id],
        )?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO record_rank_buckets(database_id,lower_id,count) VALUES(?1,?2,?3)",
            )?;
            for (lower, count) in buckets {
                insert.execute(params![database_id, lower, count])?;
            }
        }
        tx.commit()
    }

    /// Read a bounded ID-ordered window and its exact live count from one snapshot.
    pub fn record_window(
        &self,
        database_id: &str,
        start: u64,
        limit: u32,
        expected_revision: Option<u64>,
    ) -> Result<RecordWindowState> {
        if limit == 0 || limit > MAX_WINDOW {
            return Err(invalid("window limit must be between 1 and 1000"));
        }
        let tx = self.conn.unchecked_transaction()?;
        valid_database(&tx, database_id)?;
        let state: Option<(i64, bool, i64)> = tx
            .query_row(
                "SELECT revision,ready,total FROM record_rank_state WHERE database_id=?1",
                [database_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((revision, true, total)) = state else {
            return Ok(RecordWindowState::Pending);
        };
        let revision = u64::try_from(revision).map_err(|_| invalid("invalid rank revision"))?;
        let total = u64::try_from(total).map_err(|_| invalid("invalid rank total"))?;
        if expected_revision.is_some_and(|expected| expected != revision) {
            return Ok(RecordWindowState::Stale { revision });
        }
        let start = if start >= total {
            total.saturating_sub(u64::from(limit))
        } else {
            start
        };
        if total == 0 {
            return Ok(RecordWindowState::Ready(RecordWindow {
                total,
                revision,
                start: 0,
                records: Vec::new(),
            }));
        }
        let mut base = 0u64;
        let mut bucket = None;
        {
            let mut statement = tx.prepare("SELECT lower_id,count FROM record_rank_buckets WHERE database_id=?1 ORDER BY lower_id")?;
            let mut rows = statement.query([database_id])?;
            while let Some(row) = rows.next()? {
                let lower: String = row.get(0)?;
                let count = u64::try_from(row.get::<_, i64>(1)?)
                    .map_err(|_| invalid("invalid bucket count"))?;
                if start < base.saturating_add(count) {
                    bucket = Some((lower, count, start - base));
                    break;
                }
                base = base.saturating_add(count);
            }
        }
        let Some((lower, count, offset)) = bucket else {
            return Err(invalid("rank index is inconsistent"));
        };
        if count > MAX_BUCKET {
            return Ok(RecordWindowState::Pending);
        }
        let records = {
            let mut statement = tx.prepare("SELECT data FROM records WHERE database_id=?1 AND deleted_at IS NULL AND id>=?2 ORDER BY id LIMIT ?3 OFFSET ?4")?;
            let rows = statement
                .query_map(params![database_id, lower, limit, offset as i64], |row| {
                    row.get::<_, String>(0)
                })?;
            rows.map(|row| decode(row?)).collect::<Result<Vec<_>>>()?
        };
        Ok(RecordWindowState::Ready(RecordWindow {
            total,
            revision,
            start,
            records,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Database, DatabaseView, Property, PropertyKind, RecordQuery, ViewKind, new_id, now,
        store::encode,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn fixture() -> (Store, String) {
        fixture_with_store(Store::in_memory().unwrap())
    }

    fn fixture_with_store(mut store: Store) -> (Store, String) {
        let ws = store.create_workspace("Windows", "").unwrap();
        let page = store
            .create_database_page(
                &ws.id,
                None,
                "Items",
                "",
                Database {
                    id: new_id(),
                    page_id: String::new(),
                    name: "Items".into(),
                    properties: vec![Property {
                        id: "title".into(),
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
                },
            )
            .unwrap();
        let db = store.get_database_by_page(&page.id).unwrap().unwrap();
        (store, db.id)
    }

    fn insert_ids(store: &mut Store, db: &str, ids: impl IntoIterator<Item = String>) {
        let tx = store.conn.transaction().unwrap();
        {
            let mut insert = tx.prepare("INSERT INTO records(id,database_id,page_id,title,values_json,deleted_at,data) VALUES(?1,?2,NULL,?3,'{}',NULL,?4)").unwrap();
            for id in ids {
                let record = Record {
                    id: id.clone(),
                    database_id: db.into(),
                    page_id: None,
                    title: id.clone(),
                    values: BTreeMap::new(),
                    created_at: now(),
                    updated_at: now(),
                    deleted_at: None,
                };
                insert
                    .execute(params![id, db, record.title, encode(&record).unwrap()])
                    .unwrap();
            }
        }
        tx.commit().unwrap();
    }

    fn ready(state: RecordWindowState) -> RecordWindow {
        match state {
            RecordWindowState::Ready(window) => window,
            other => panic!("expected ready: {other:?}"),
        }
    }

    #[test]
    fn skewed_ids_support_middle_bottom_and_live_mutations() {
        let (mut store, db) = fixture();
        insert_ids(
            &mut store,
            &db,
            (0..3500).map(|n| format!("same-prefix-{n:07}")),
        );
        assert!(matches!(
            store.record_window(&db, 1500, 100, None).unwrap(),
            RecordWindowState::Pending
        ));
        store.build_record_rank(&db).unwrap();
        for start in [0, 1023, 1750, 3499, 9000] {
            let window = ready(store.record_window(&db, start, 100, None).unwrap());
            assert_eq!(window.total, 3500);
            let expected = store
                .query_records(
                    &db,
                    &RecordQuery {
                        offset: window.start,
                        limit: 100,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(window.records, expected);
        }
        let revision = ready(store.record_window(&db, 0, 10, None).unwrap()).revision;
        insert_ids(
            &mut store,
            &db,
            ["!new-first".to_owned(), "zz-new-last".to_owned()],
        );
        assert!(matches!(
            store.record_window(&db, 0, 10, Some(revision)).unwrap(),
            RecordWindowState::Stale { .. }
        ));
        let first = ready(store.record_window(&db, 0, 10, None).unwrap());
        assert_eq!(first.total, 3502);
        assert_eq!(first.records[0].id, "!new-first");
        store.delete_record("same-prefix-0000000").unwrap();
        assert_eq!(
            ready(store.record_window(&db, 0, 10, None).unwrap()).total,
            3501
        );
        store.restore_record("same-prefix-0000000").unwrap();
        assert_eq!(
            ready(store.record_window(&db, 0, 10, None).unwrap()).total,
            3502
        );
    }

    #[test]
    fn oversized_bucket_requires_rebuild_instead_of_an_unbounded_scan() {
        let (mut store, db) = fixture();
        store.build_record_rank(&db).unwrap();
        insert_ids(
            &mut store,
            &db,
            (0..2050).map(|n| format!("same-prefix-{n:07}")),
        );
        assert!(matches!(
            store.record_window(&db, 1000, 50, None).unwrap(),
            RecordWindowState::Pending
        ));
        store.build_record_rank(&db).unwrap();
        let page = ready(store.record_window(&db, 1000, 50, None).unwrap());
        assert_eq!(page.total, 2050);
        assert_eq!(page.records.len(), 50);
        assert_eq!(page.records[0].id, "same-prefix-0001000");
    }

    #[test]
    fn concurrent_write_during_build_does_not_publish_stale_counts() {
        let path = std::env::temp_dir().join(format!("moss-rank-test-{}.sqlite3", new_id()));
        let (mut store, db) = fixture_with_store(Store::open(&path).unwrap());
        insert_ids(&mut store, &db, ["first".to_owned()]);
        store.build_record_rank(&db).unwrap();
        let mut other = Store::open(&path).unwrap();
        let result = store.build_record_rank_with(&db, || {
            insert_ids(&mut other, &db, ["second".to_owned()]);
        });
        assert!(result.is_err(), "the old snapshot must not be published");
        let current = ready(store.record_window(&db, 0, 10, None).unwrap());
        assert_eq!(current.total, 2);
        assert_eq!(current.records.len(), 2);
        drop(other);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn upsert_and_typed_value_edits_preserve_existing_rank_and_record() {
        let (mut store, db) = fixture();
        let db = store
            .add_property(&db, "Notes", PropertyKind::Text)
            .unwrap();
        let property = db.properties.iter().find(|p| p.name == "Notes").unwrap();
        let mut record = store
            .create_record(&db.id, "Original", &BTreeMap::new())
            .unwrap();
        store.build_record_rank(&db.id).unwrap();
        record.values.insert(property.id.clone(), json!("first"));
        store.save_record(&record).unwrap();
        let changed = store
            .set_record_value_if_unchanged(
                &record.id,
                &property.id,
                &json!("first"),
                json!("second"),
            )
            .unwrap();
        assert_eq!(changed.values[&property.id], json!("second"));
        assert_eq!(
            store.get_record(&record.id).unwrap().unwrap().title,
            "Original"
        );
        let window = ready(store.record_window(&db.id, 0, 10, None).unwrap());
        assert_eq!(window.total, 1);
        assert_eq!(window.records[0].values[&property.id], json!("second"));
    }

    #[test]
    fn opening_an_unversioned_rank_index_replaces_old_triggers_without_losing_data() {
        let path = std::env::temp_dir().join(format!("moss-rank-migration-{}.sqlite3", new_id()));
        let (mut store, db) = fixture_with_store(Store::open(&path).unwrap());
        let mut record = store.create_record(&db, "Kept", &BTreeMap::new()).unwrap();
        store.build_record_rank(&db).unwrap();
        store.conn.execute_batch("DROP TABLE record_rank_meta; DROP TRIGGER record_rank_update; CREATE TRIGGER record_rank_update AFTER UPDATE ON records BEGIN INSERT OR IGNORE INTO record_rank_state VALUES(NEW.database_id,0,0,0); END;").unwrap();
        drop(store);
        let mut reopened = Store::open(&path).unwrap();
        record.title = "Still kept".into();
        reopened.save_record(&record).unwrap();
        let window = ready(reopened.record_window(&db, 0, 10, None).unwrap());
        assert_eq!(window.total, 1);
        assert_eq!(window.records[0].title, "Still kept");
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}
