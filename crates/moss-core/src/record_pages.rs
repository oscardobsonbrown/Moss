use crate::store::{decode, encode, invalid};
use crate::{Block, Database, Page, PageKind, Record, Store, new_id, now, search};
use rusqlite::{Connection, OptionalExtension, Result, params};
use serde_json::Value;
use std::collections::BTreeMap;

impl Store {
    pub fn discard_untouched_item(
        &mut self,
        record: Option<&Record>,
        page: Option<&Page>,
        blocks: &[Block],
        created_link: Option<&Block>,
    ) -> Result<bool> {
        let tx = self.conn.transaction()?;
        if let Some(record) = record {
            let current: Option<String> = tx
                .query_row(
                    "SELECT data FROM records WHERE id=?1",
                    [&record.id],
                    |row| row.get(0),
                )
                .optional()?;
            let in_use: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM calculation_edges WHERE target_id=?1 AND source_id<>?1) OR EXISTS(SELECT 1 FROM records r, json_tree(r.values_json) value WHERE r.id<>?1 AND instr(r.values_json,?1)>0 AND value.type='text' AND value.value=?1)", [&record.id], |row| row.get(0))?;
            if current.map(decode::<Record>).transpose()?.as_ref() != Some(record)
                || in_use
                || record.page_id.as_deref() != page.map(|p| p.id.as_str())
            {
                return Ok(false);
            }
        }
        if let Some(page) = page {
            if let Some(link) = created_link {
                let current: Option<String> = tx
                    .query_row("SELECT data FROM blocks WHERE id=?1", [&link.id], |row| {
                        row.get(0)
                    })
                    .optional()?;
                if current.map(decode::<Block>).transpose()?.as_ref() != Some(link)
                    || link.kind != crate::BlockKind::ChildPage
                    || link.config.get("page_id").and_then(Value::as_str) != Some(&page.id)
                    || page.parent_id.as_deref() != Some(&link.page_id)
                {
                    return Ok(false);
                }
            }
            let current: Option<String> = tx
                .query_row("SELECT data FROM pages WHERE id=?1", [&page.id], |row| {
                    row.get(0)
                })
                .optional()?;
            let current_blocks: Vec<Block> = {
                let mut query =
                    tx.prepare("SELECT data FROM blocks WHERE page_id=?1 ORDER BY sort_order,id")?;
                let rows = query.query_map([&page.id], |row| row.get::<_, String>(0))?;
                rows.map(|row| decode(row?)).collect::<Result<_>>()?
            };
            let in_use: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM pages WHERE parent_id=?1) OR EXISTS(SELECT 1 FROM databases WHERE page_id=?1) OR EXISTS(SELECT 1 FROM blocks b, json_tree(b.data) value WHERE b.page_id<>?1 AND b.id<>?2 AND instr(b.data,?1)>0 AND value.type='text' AND value.value=?1)", params![page.id, created_link.map(|link| link.id.as_str()).unwrap_or("")], |row| row.get(0))?;
            if current.map(decode::<Page>).transpose()?.as_ref() != Some(page)
                || current_blocks != blocks
                || in_use
            {
                return Ok(false);
            }
            if let Some(link) = created_link {
                tx.execute("DELETE FROM blocks WHERE id=?1", [&link.id])?;
            }
            tx.execute("DELETE FROM pages WHERE id=?1", [&page.id])?;
        } else if let Some(record) = record {
            tx.execute("DELETE FROM records WHERE id=?1", [&record.id])?;
        } else {
            return Ok(false);
        }
        tx.commit()?;
        Ok(true)
    }
    /// Create a record with caller-supplied property defaults.
    pub fn create_record(
        &mut self,
        database_id: &str,
        title: &str,
        values: &BTreeMap<String, Value>,
    ) -> Result<Record> {
        let t = now();
        let record = Record {
            id: new_id(),
            database_id: database_id.into(),
            page_id: None,
            title: title.into(),
            values: values.clone(),
            created_at: t.clone(),
            updated_at: t,
            deleted_at: None,
        };
        self.save_record(&record)?;
        self.get_record(&record.id)?
            .ok_or_else(|| invalid("record missing"))
    }

    /// Create the record's page once and link it in the same transaction.
    pub fn ensure_record_page(&mut self, record_id: &str) -> Result<Page> {
        let tx = self.conn.transaction()?;
        let raw: String = tx
            .query_row("SELECT data FROM records WHERE id=?1", [record_id], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or_else(|| invalid("record missing"))?;
        let mut record: Record = decode(raw)?;
        if record.deleted_at.is_some() {
            return Err(invalid("record in trash"));
        }
        let raw_db: String = tx
            .query_row(
                "SELECT data FROM databases WHERE id=?1",
                [&record.database_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("database missing"))?;
        let db: Database = decode(raw_db)?;
        let owner =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        if let Some(id) = &record.page_id {
            let page = search::page(&tx, id)?.ok_or_else(|| invalid("record page missing"))?;
            if page.workspace_id != owner.workspace_id
                || page.kind != PageKind::Record
                || page.parent_id.as_deref() != Some(owner.id.as_str())
                || page.deleted_at.is_some()
            {
                return Err(invalid("invalid record page"));
            }
            tx.commit()?;
            return Ok(page);
        }
        let t = now();
        let page = Page {
            id: new_id(),
            workspace_id: owner.workspace_id.clone(),
            parent_id: Some(owner.id.clone()),
            title: record.title.clone(),
            icon: String::new(),
            kind: PageKind::Record,
            cover: None,
            created_at: t.clone(),
            sidebar_order: None,
            updated_at: t.clone(),
            deleted_at: None,
            settings: Default::default(),
        };
        tx.execute(
            "INSERT INTO pages(id,workspace_id,parent_id,deleted_at,data) VALUES(?1,?2,?3,NULL,?4)",
            params![page.id, page.workspace_id, page.parent_id, encode(&page)?],
        )?;
        record.page_id = Some(page.id.clone());
        record.updated_at = t;
        tx.execute(
            "UPDATE records SET page_id=?1,data=?2 WHERE id=?3",
            params![page.id, encode(&record)?, record.id],
        )?;
        search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
        search::index(
            &tx,
            &format!("record:{}", record.id),
            &page,
            &format!("{} {}", record.title, encode(&record.values)?),
        )?;
        tx.commit()?;
        Ok(page)
    }

    /// Rename a record and its linked page atomically.
    pub fn rename_record(&mut self, record_id: &str, title: &str) -> Result<Record> {
        let tx = self.conn.transaction()?;
        let raw: String = tx
            .query_row("SELECT data FROM records WHERE id=?1", [record_id], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or_else(|| invalid("record missing"))?;
        let mut record: Record = decode(raw)?;
        if record.deleted_at.is_some() {
            return Err(invalid("record in trash"));
        }
        let raw_db: String = tx
            .query_row(
                "SELECT data FROM databases WHERE id=?1",
                [&record.database_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("database missing"))?;
        let db: Database = decode(raw_db)?;
        let owner =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() {
            return Err(invalid("database in trash"));
        }
        let stamp = now();
        record.title = title.into();
        record.updated_at = stamp.clone();
        let target = if let Some(id) = &record.page_id {
            let mut page = search::page(&tx, id)?.ok_or_else(|| invalid("record page missing"))?;
            if page.workspace_id != owner.workspace_id
                || page.kind != PageKind::Record
                || page.parent_id.as_deref() != Some(owner.id.as_str())
                || page.deleted_at.is_some()
            {
                return Err(invalid("invalid record page"));
            }
            page.title = title.into();
            page.updated_at = stamp;
            tx.execute(
                "UPDATE pages SET data=?1 WHERE id=?2",
                params![encode(&page)?, page.id],
            )?;
            search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
            page
        } else {
            owner
        };
        tx.execute(
            "UPDATE records SET title=?1,data=?2 WHERE id=?3",
            params![record.title, encode(&record)?, record.id],
        )?;
        search::index(
            &tx,
            &format!("record:{}", record.id),
            &target,
            &format!("{} {}", record.title, encode(&record.values)?),
        )?;
        crate::calculation::refresh(&tx, std::slice::from_ref(&record.id))?;
        let raw: String =
            tx.query_row("SELECT data FROM records WHERE id=?1", [&record.id], |r| {
                r.get(0)
            })?;
        record = decode(raw)?;
        tx.commit()?;
        Ok(record)
    }
    pub fn delete_record(&mut self, id: &str) -> Result<()> {
        let record = self
            .get_record(id)?
            .ok_or_else(|| invalid("record missing"))?;
        if record.deleted_at.is_some() {
            return Ok(());
        }
        if let Some(page) = record.page_id {
            return self.trash(&page, false);
        }
        let tx = self.conn.transaction()?;
        set_record_trash(&tx, id, id, false, &now())?;
        crate::calculation::refresh(&tx, &[id.to_owned()])?;
        tx.commit()
    }
    pub fn restore_record(&mut self, id: &str) -> Result<()> {
        let record = self
            .get_record(id)?
            .ok_or_else(|| invalid("record missing"))?;
        if record.deleted_at.is_none() {
            return Ok(());
        }
        if let Some(page) = record.page_id {
            return self.trash(&page, true);
        }
        let tx = self.conn.transaction()?;
        let db_raw: String = tx.query_row(
            "SELECT data FROM databases WHERE id=?1",
            [&record.database_id],
            |r| r.get(0),
        )?;
        let db: Database = decode(db_raw)?;
        let owner =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() {
            return Err(invalid("restore database first"));
        }
        set_record_trash(&tx, id, id, true, &now())?;
        crate::calculation::refresh(&tx, &[id.to_owned()])?;
        tx.commit()
    }
    pub fn delete_page(&mut self, id: &str) -> Result<()> {
        self.trash(id, false)
    }
    pub fn restore_page(&mut self, id: &str) -> Result<()> {
        self.trash(id, true)
    }
    fn trash(&mut self, id: &str, restore: bool) -> Result<()> {
        let root = search::page(&self.conn, id)?.ok_or_else(|| invalid("page missing"))?;
        if restore {
            if root.deleted_at.is_none() {
                return Ok(());
            }
            let tag: Option<String> =
                self.conn
                    .query_row("SELECT trash_root FROM pages WHERE id=?1", [id], |r| {
                        r.get(0)
                    })?;
            if tag.as_deref() != Some(id) {
                return Err(invalid("restore trash root first"));
            }
            if let Some(pid) = &root.parent_id {
                let p = search::page(&self.conn, pid)?.ok_or_else(|| invalid("parent missing"))?;
                if p.deleted_at.is_some() {
                    return Err(invalid("restore parent first"));
                }
            }
        } else if root.deleted_at.is_some() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        let ids: Vec<String> = {
            let mut s=tx.prepare("WITH RECURSIVE subtree(id) AS (SELECT id FROM pages WHERE id=?1 UNION ALL SELECT p.id FROM pages p JOIN subtree s ON p.parent_id=s.id) SELECT id FROM subtree")?;
            let rows = s.query_map([id], |r| r.get(0))?;
            rows.collect::<Result<_>>()?
        };
        let stamp = now();
        let mut changed_records = Vec::new();
        for child in ids {
            let mut p = search::page(&tx, &child)?.ok_or_else(|| invalid("page missing"))?;
            if restore {
                let tag: Option<String> =
                    tx.query_row("SELECT trash_root FROM pages WHERE id=?1", [&child], |r| {
                        r.get(0)
                    })?;
                if tag.as_deref() != Some(id) {
                    continue;
                }
                p.deleted_at = None;
            } else {
                if p.deleted_at.is_some() {
                    continue;
                }
                p.deleted_at = Some(stamp.clone());
            }
            tx.execute(
                "UPDATE pages SET deleted_at=?1,trash_root=?2,data=?3 WHERE id=?4",
                params![
                    p.deleted_at,
                    if restore { None } else { Some(id) },
                    encode(&p)?,
                    child
                ],
            )?;
            let record_ids: Vec<String> = {
                let mut statement = tx.prepare("SELECT r.id FROM records r JOIN databases d ON d.id=r.database_id WHERE r.page_id=?1 OR d.page_id=?1")?;
                let rows = statement.query_map([&child], |row| row.get(0))?;
                rows.collect::<Result<_>>()?
            };
            for record_id in record_ids {
                set_record_trash(&tx, &record_id, id, restore, &stamp)?;
                changed_records.push(record_id);
            }
        }
        crate::calculation::refresh(&tx, &changed_records)?;
        tx.commit()
    }
}

// The root marker prevents a parent restore from reviving previously trashed records.
fn set_record_trash(
    conn: &Connection,
    id: &str,
    root: &str,
    restore: bool,
    stamp: &str,
) -> Result<()> {
    let raw: String = conn.query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))?;
    let mut record: Record = decode(raw)?;
    let tag: Option<String> = conn
        .query_row(
            "SELECT root_id FROM record_trash_roots WHERE record_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    if restore {
        if record.deleted_at.is_none() || tag.as_deref().is_some_and(|t| t != root) {
            return Ok(());
        }
        // Legacy deleted records have no marker. Only explicit record restore revives them.
        if tag.is_none() && root != id {
            return Ok(());
        }
        record.deleted_at = None;
        conn.execute("DELETE FROM record_trash_roots WHERE record_id=?1", [id])?;
    } else {
        if record.deleted_at.is_some() {
            return Ok(());
        }
        record.deleted_at = Some(stamp.into());
        conn.execute(
            "INSERT INTO record_trash_roots(record_id,root_id) VALUES(?1,?2)",
            params![id, root],
        )?;
    }
    conn.execute(
        "UPDATE records SET deleted_at=?1,data=?2 WHERE id=?3",
        params![record.deleted_at, encode(&record)?, id],
    )?;
    if restore {
        let raw: String = conn.query_row(
            "SELECT data FROM databases WHERE id=?1",
            [&record.database_id],
            |r| r.get(0),
        )?;
        let db: Database = decode(raw)?;
        let page_id = record.page_id.as_deref().unwrap_or(&db.page_id);
        let page = search::page(conn, page_id)?.ok_or_else(|| invalid("record page missing"))?;
        search::index(
            conn,
            &format!("record:{}", record.id),
            &page,
            &format!("{} {}", record.title, encode(&record.values)?),
        )?;
    } else {
        search::remove(conn, &format!("record:{}", record.id))?;
    }
    Ok(())
}
