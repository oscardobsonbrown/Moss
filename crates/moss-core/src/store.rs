use crate::{
    Block, Database, FilterExpr, FilterOp, Page, PageKind, Record, RecordQuery, SearchHit,
    Workspace, new_id, now,
};
use crate::{schema, search};
use rusqlite::{
    Connection, OptionalExtension, Result, params, params_from_iter, types::Value as SqlValue,
};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub struct Store {
    pub(crate) conn: Connection,
    pub(crate) history_actor: Option<crate::history::HistoryActor>,
    pub(crate) history_coalesce: bool,
}

pub(crate) fn invalid(message: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.to_owned())
}
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}
pub(crate) fn decode<T: DeserializeOwned>(raw: String) -> Result<T> {
    serde_json::from_str(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}
fn collect_json<T: DeserializeOwned>(
    stmt: &mut rusqlite::Statement<'_>,
    args: impl rusqlite::Params,
) -> Result<Vec<T>> {
    stmt.query_map(args, |r| r.get::<_, String>(0))?
        .map(|r| decode(r?))
        .collect()
}
fn nonempty(s: &str) -> Result<()> {
    if s.trim().is_empty() {
        Err(invalid("empty identifier or name"))
    } else {
        Ok(())
    }
}

fn validate_database_definition(db: &Database) -> Result<()> {
    nonempty(&db.id)?;
    let mut ids = std::collections::HashSet::new();
    for prop in &db.properties {
        nonempty(&prop.id)?;
        nonempty(&prop.name)?;
        if !ids.insert(&prop.id) {
            return Err(invalid("duplicate property"));
        }
    }
    let mut views = std::collections::HashSet::new();
    for view in &db.views {
        nonempty(&view.id)?;
        if !views.insert(&view.id) {
            return Err(invalid("duplicate view"));
        }
    }
    Ok(())
}

impl Store {
    /// Return a handle that can interrupt a running SQLite operation on this connection.
    pub fn interrupt_handle(&self) -> rusqlite::InterruptHandle {
        self.conn.get_interrupt_handle()
    }
    /// Count writes issued by this connection, including writes later rolled back.
    pub fn local_change_count(&self) -> u64 {
        self.conn.total_changes()
    }
    /// Detect commits made by other SQLite connections.
    pub fn external_data_version(&self) -> Result<i64> {
        self.conn
            .pragma_query_value(None, "data_version", |row| row.get(0))
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        schema::init(&conn)?;
        Ok(Self {
            conn,
            history_actor: None,
            history_coalesce: true,
        })
    }
    /// Open an existing store for reads without schema changes or cleanup writes.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.pragma_update(None, "query_only", true)?;
        Ok(Self {
            conn,
            history_actor: None,
            history_coalesce: true,
        })
    }
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::init(&conn)?;
        Ok(Self {
            conn,
            history_actor: None,
            history_coalesce: true,
        })
    }
    pub fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        collect_json(
            &mut self
                .conn
                .prepare("SELECT data FROM workspaces ORDER BY json_extract(data,'$.name'), id")?,
            [],
        )
    }
    pub fn create_workspace(&mut self, name: &str, icon: &str) -> Result<Workspace> {
        nonempty(name)?;
        let w = Workspace {
            id: new_id(),
            name: name.into(),
            icon: icon.into(),
            created_at: now(),
        };
        self.conn.execute(
            "INSERT INTO workspaces(id,data) VALUES(?1,?2)",
            params![w.id, encode(&w)?],
        )?;
        Ok(w)
    }
    pub fn list_pages(&self, workspace_id: &str) -> Result<Vec<Page>> {
        collect_json(&mut self.conn.prepare("SELECT data FROM pages WHERE workspace_id=?1 AND deleted_at IS NULL AND json_extract(data,'$.kind') <> 'record' ORDER BY json_extract(data,'$.created_at'),id")?, [workspace_id])
    }
    pub fn get_page(&self, id: &str) -> Result<Option<Page>> {
        search::page(&self.conn, id)
    }
    pub fn create_page(
        &mut self,
        workspace_id: &str,
        parent_id: Option<&str>,
        title: &str,
        icon: &str,
        kind: PageKind,
    ) -> Result<Page> {
        nonempty(workspace_id)?;
        let t = now();
        let page = Page {
            id: new_id(),
            workspace_id: workspace_id.into(),
            parent_id: parent_id.map(str::to_owned),
            title: title.into(),
            icon: icon.into(),
            kind,
            cover: None,
            created_at: t.clone(),
            sidebar_order: None,
            updated_at: t,
            deleted_at: None,
            settings: Default::default(),
        };
        self.validate_page(&page, false)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO pages(id,workspace_id,parent_id,deleted_at,data) VALUES(?1,?2,?3,NULL,?4)",
            params![page.id, page.workspace_id, page.parent_id, encode(&page)?],
        )?;
        search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
        tx.commit()?;
        Ok(page)
    }
    /// Create a page and its database definition together. The input page_id must be empty.
    pub fn create_database_page(
        &mut self,
        workspace_id: &str,
        parent_id: Option<&str>,
        title: &str,
        icon: &str,
        mut definition: Database,
    ) -> Result<Page> {
        if !definition.page_id.is_empty() {
            return Err(invalid("new database definition already has a page"));
        }
        validate_database_definition(&definition)?;
        let stamp = now();
        let page = Page {
            id: new_id(),
            workspace_id: workspace_id.into(),
            parent_id: parent_id.map(str::to_owned),
            title: title.into(),
            icon: icon.into(),
            kind: PageKind::Database,
            cover: None,
            created_at: stamp.clone(),
            sidebar_order: None,
            updated_at: stamp,
            deleted_at: None,
            settings: Default::default(),
        };
        self.validate_page(&page, false)?;
        definition.page_id = page.id.clone();
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(id) = parent_id {
            let parent = search::page(&tx, id)?.ok_or_else(|| invalid("parent missing"))?;
            if parent.deleted_at.is_some() || parent.workspace_id != workspace_id {
                return Err(invalid("invalid parent"));
            }
        }
        tx.execute(
            "INSERT INTO pages(id,workspace_id,parent_id,deleted_at,data) VALUES(?1,?2,?3,NULL,?4)",
            params![page.id, page.workspace_id, page.parent_id, encode(&page)?],
        )?;
        tx.execute(
            "INSERT INTO databases(id,page_id,data) VALUES(?1,?2,?3)",
            params![definition.id, definition.page_id, encode(&definition)?],
        )?;
        for property in definition
            .properties
            .iter()
            .filter(|p| p.kind == crate::PropertyKind::Relation)
        {
            crate::relations::validate_relation_value(
                &tx,
                property,
                &serde_json::json!([]),
                &page.workspace_id,
            )?;
        }
        if definition.properties.iter().any(|p| {
            matches!(
                p.kind,
                crate::PropertyKind::Formula | crate::PropertyKind::Rollup
            )
        }) {
            crate::advanced_properties::graph(&tx, &page.workspace_id, &definition, None)?;
        }
        search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
        tx.commit()?;
        Ok(page)
    }
    /// Convert a live empty document into a database without changing its identity.
    pub fn create_database_on_page(
        &mut self,
        page_id: &str,
        mut definition: Database,
    ) -> Result<Page> {
        if !definition.page_id.is_empty() {
            return Err(invalid("new database definition already has a page"));
        }
        validate_database_definition(&definition)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page missing"))?;
        if page.deleted_at.is_some() || page.kind != PageKind::Document {
            return Err(invalid("page is not a live document"));
        }
        let has_blocks: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM blocks WHERE page_id=?1)",
            [page_id],
            |row| row.get(0),
        )?;
        if has_blocks {
            return Err(invalid("document has content"));
        }
        definition.page_id = page.id.clone();
        definition.name = if page.title.is_empty() {
            "Untitled database".into()
        } else {
            page.title.clone()
        };
        page.kind = PageKind::Database;
        page.icon = "▦".into();
        page.updated_at = now();
        tx.execute(
            "UPDATE pages SET data=?1 WHERE id=?2",
            params![encode(&page)?, page.id],
        )?;
        tx.execute(
            "INSERT INTO databases(id,page_id,data) VALUES(?1,?2,?3)",
            params![definition.id, definition.page_id, encode(&definition)?],
        )?;
        for property in definition
            .properties
            .iter()
            .filter(|p| p.kind == crate::PropertyKind::Relation)
        {
            crate::relations::validate_relation_value(
                &tx,
                property,
                &serde_json::json!([]),
                &page.workspace_id,
            )?;
        }
        if definition.properties.iter().any(|p| {
            matches!(
                p.kind,
                crate::PropertyKind::Formula | crate::PropertyKind::Rollup
            )
        }) {
            crate::advanced_properties::graph(&tx, &page.workspace_id, &definition, None)?;
        }
        search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
        tx.commit()?;
        Ok(page)
    }
    fn validate_page(&self, page: &Page, existing: bool) -> Result<()> {
        nonempty(&page.id)?;
        nonempty(&page.workspace_id)?;
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
            [&page.workspace_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(invalid("workspace does not exist"));
        }
        if let Some(parent) = &page.parent_id {
            let p = search::page(&self.conn, parent)?
                .ok_or_else(|| invalid("parent does not exist"))?;
            if p.workspace_id != page.workspace_id || p.deleted_at.is_some() || parent == &page.id {
                return Err(invalid("invalid parent"));
            }
            let cycle:bool=self.conn.query_row("WITH RECURSIVE parents(id,parent_id) AS (SELECT id,parent_id FROM pages WHERE id=?1 UNION ALL SELECT p.id,p.parent_id FROM pages p JOIN parents x ON p.id=x.parent_id) SELECT EXISTS(SELECT 1 FROM parents WHERE id=?2)",params![parent,page.id],|r|r.get(0))?;
            if cycle {
                return Err(invalid("parent cycle"));
            }
        }
        if existing {
            let old = search::page(&self.conn, &page.id)?
                .ok_or_else(|| invalid("page does not exist"))?;
            if old.kind == PageKind::Record
                && (old.title != page.title || old.parent_id != page.parent_id)
            {
                return Err(invalid(
                    "record title or parent must be changed through the record API",
                ));
            }
            if old.workspace_id != page.workspace_id
                || old.kind != page.kind
                || old.created_at != page.created_at
                || old.deleted_at != page.deleted_at
            {
                return Err(invalid("immutable page fields changed"));
            }
        }
        Ok(())
    }
    pub fn update_page(&mut self, page: &Page) -> Result<()> {
        self.validate_page(page, true)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = search::page(&tx, &page.id)?.ok_or_else(|| invalid("page does not exist"))?;
        let mut page = page.clone();
        page.sidebar_order = if page.parent_id == current.parent_id {
            current.sidebar_order
        } else {
            None
        };
        tx.execute(
            "UPDATE pages SET parent_id=?1,data=?2 WHERE id=?3",
            params![page.parent_id, encode(&page)?, page.id],
        )?;
        search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
        tx.commit()
    }
    pub fn get_blocks(&self, page_id: &str) -> Result<Vec<Block>> {
        collect_json(
            &mut self
                .conn
                .prepare("SELECT data FROM blocks WHERE page_id=?1 ORDER BY sort_order,id")?,
            [page_id],
        )
    }
    /// Replace blocks exactly as provided, including explicit image-asset changes.
    /// Editors with potentially stale image state must use `save_editor_blocks`.
    pub fn save_blocks(&mut self, page_id: &str, blocks: &[Block]) -> Result<()> {
        self.write_blocks(page_id, blocks, false, None).map(|_| ())
    }
    /// Save text and layout edits while keeping current assets for existing image blocks.
    /// Use the guarded image attachment methods to replace or clear an image.
    pub fn save_editor_blocks(&mut self, page_id: &str, blocks: &[Block]) -> Result<Vec<Block>> {
        self.write_blocks(page_id, blocks, true, None)
    }
    /// Save an editor snapshot only when the stored blocks still match its base.
    pub fn save_editor_blocks_if_unchanged(
        &mut self,
        page_id: &str,
        expected: &[Block],
        blocks: &[Block],
    ) -> Result<Vec<Block>> {
        self.write_blocks(page_id, blocks, true, Some(expected))
    }
    /// Update a task without rewriting or reindexing the page's other blocks.
    pub fn set_todo_checked(&mut self, page_id: &str, block_id: &str, checked: bool) -> Result<()> {
        nonempty(page_id)?;
        nonempty(block_id)?;
        let actor = self.history_actor.clone();
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page does not exist"))?;
        let before: Option<String> = tx.query_row("SELECT data FROM blocks WHERE id=?1 AND page_id=?2 AND json_extract(data,'$.kind')='todo' AND EXISTS(SELECT 1 FROM pages WHERE id=?2 AND deleted_at IS NULL)", params![block_id, page_id], |row| row.get(0)).optional()?;
        let before: serde_json::Value =
            decode(before.ok_or_else(|| invalid("task or page is not available"))?)?;
        let changed = tx.execute(
            "UPDATE blocks SET data=json_set(data,'$.checked',json(?3)) WHERE id=?1 AND page_id=?2 AND json_extract(data,'$.kind')='todo' AND EXISTS(SELECT 1 FROM pages WHERE id=?2 AND deleted_at IS NULL)",
            params![block_id, page_id, if checked { "true" } else { "false" }],
        )?;
        if changed != 1 {
            return Err(invalid("task or page is not available"));
        }
        let after: serde_json::Value = decode(tx.query_row(
            "SELECT data FROM blocks WHERE id=?1",
            [block_id],
            |row| row.get(0),
        )?)?;
        crate::history::record_change(
            &tx,
            &page,
            "block",
            block_id,
            None,
            Some(&before),
            Some(&after),
            actor.as_ref(),
        )?;
        tx.commit()
    }
    fn write_blocks(
        &mut self,
        page_id: &str,
        blocks: &[Block],
        preserve_images: bool,
        expected: Option<&[Block]>,
    ) -> Result<Vec<Block>> {
        let mut ids = std::collections::HashSet::new();
        for b in blocks {
            nonempty(&b.id)?;
            if b.page_id != page_id || !ids.insert(&b.id) {
                return Err(invalid("invalid or duplicate block"));
            }
        }
        let actor = self.history_actor.clone();
        let coalesce = self.history_coalesce;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page does not exist"))?;
        if page.deleted_at.is_some() {
            return Err(invalid("page is in trash"));
        }
        if let Some(expected) = expected {
            let current: Vec<Block> = collect_json(
                &mut tx
                    .prepare("SELECT data FROM blocks WHERE page_id=?1 ORDER BY sort_order,id")?,
                [page_id],
            )?;
            let mut base = expected.to_vec();
            let mut current = current;
            if preserve_images {
                for block in base
                    .iter_mut()
                    .chain(current.iter_mut())
                    .filter(|b| b.kind == crate::BlockKind::Image)
                {
                    if let Some(config) = block.config.as_object_mut() {
                        config.remove("asset");
                    }
                }
            }
            if current != base {
                return Err(invalid(
                    "page changed in another editor; reload before saving",
                ));
            }
        }
        let mut saved = blocks.to_vec();
        if preserve_images && saved.iter().any(|b| b.kind == crate::BlockKind::Image) {
            let mut statement = tx.prepare(
                "SELECT data FROM blocks WHERE page_id=?1 AND json_extract(data,'$.kind')='image'",
            )?;
            let existing = statement
                .query_map([page_id], |row| decode::<Block>(row.get(0)?))?
                .collect::<Result<Vec<_>>>()?;
            let existing: std::collections::HashMap<_, _> = existing
                .into_iter()
                .map(|block| (block.id.clone(), block))
                .collect();
            for block in saved
                .iter_mut()
                .filter(|b| b.kind == crate::BlockKind::Image)
            {
                if let Some(previous) = existing.get(&block.id) {
                    if block.config.is_null() {
                        block.config = serde_json::json!({});
                    }
                    let config = block
                        .config
                        .as_object_mut()
                        .ok_or_else(|| invalid("invalid image block settings"))?;
                    match previous.config.get("asset") {
                        Some(asset) => {
                            config.insert("asset".into(), asset.clone());
                        }
                        None => {
                            config.remove("asset");
                        }
                    }
                }
            }
        }
        crate::history::record_block_changes(&tx, &page, &saved, actor.as_ref(), coalesce)?;
        crate::comments::update_block_anchors(&tx, page_id, &saved)?;
        tx.execute("DELETE FROM blocks WHERE page_id=?1", [page_id])?;
        for b in &saved {
            tx.execute(
                "INSERT INTO blocks(id,page_id,sort_order,data) VALUES(?1,?2,?3,?4)",
                params![b.id, page_id, b.sort_order, encode(b)?],
            )?;
            search::index(
                &tx,
                &format!("block:{}", b.id),
                &page,
                &format!("{} {}", b.text, b.config),
            )?;
        }
        tx.commit()?;
        Ok(saved)
    }
    pub fn get_database(&self, id: &str) -> Result<Option<Database>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT data FROM databases WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        raw.map(decode).transpose()
    }
    pub fn get_database_by_page(&self, page_id: &str) -> Result<Option<Database>> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT data FROM databases WHERE page_id=?1",
                [page_id],
                |r| r.get(0),
            )
            .optional()?;
        raw.map(decode).transpose()
    }
    pub fn save_database(&mut self, db: &Database) -> Result<()> {
        validate_database_definition(db)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let page =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if page.kind != PageKind::Database || page.deleted_at.is_some() {
            return Err(invalid("invalid database page"));
        }
        let old: Option<String> = tx
            .query_row("SELECT data FROM databases WHERE id=?1", [&db.id], |row| {
                row.get(0)
            })
            .optional()?;
        let old: Option<Database> = old.map(decode).transpose()?;
        if old.as_ref().is_some_and(|old| old.page_id != db.page_id) {
            return Err(invalid("database page changed"));
        }
        if let Some(old) = &old {
            for previous in &old.properties {
                if let Some(next) = db.properties.iter().find(|p| p.id == previous.id) {
                    if previous.kind != next.kind {
                        return Err(invalid("changing property type requires a migration"));
                    }
                    if previous.kind == crate::PropertyKind::Relation
                        && previous.config.get("target_database_id")
                            != next.config.get("target_database_id")
                    {
                        let has_values: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM records r, json_each(r.values_json) v WHERE r.database_id=?1 AND v.key=?2 AND v.value IS NOT NULL AND v.value <> '[]')", params![db.id, previous.id], |row| row.get(0))?;
                        if has_values {
                            return Err(invalid(
                                "clear relation values before changing its target",
                            ));
                        }
                    }
                }
            }
        }
        let schema_changed = old
            .as_ref()
            .is_none_or(|old| old.properties != db.properties);
        let removed: Vec<_> = old
            .as_ref()
            .into_iter()
            .flat_map(|old| &old.properties)
            .filter(|old| !db.properties.iter().any(|p| p.id == old.id))
            .collect();
        if removed
            .iter()
            .any(|property| property.kind == crate::PropertyKind::Title)
        {
            return Err(invalid("title property cannot be deleted"));
        }
        let mut updated = db.clone();
        for property in &removed {
            for view in &mut updated.views {
                crate::properties::prune_view(view, &property.id);
            }
        }
        tx.execute("INSERT INTO databases(id,page_id,data) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET data=excluded.data", params![updated.id, updated.page_id, encode(&updated)?])?;
        if schema_changed {
            for property in updated
                .properties
                .iter()
                .filter(|p| p.kind == crate::PropertyKind::Relation)
            {
                let unchanged = old
                    .as_ref()
                    .and_then(|old| old.properties.iter().find(|p| p.id == property.id))
                    .is_some_and(|old| old.config == property.config);
                if unchanged {
                    continue;
                }
                let target = property
                    .config
                    .get("target_database_id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| invalid("relation target missing"))?;
                let target_raw: String = tx
                    .query_row("SELECT data FROM databases WHERE id=?1", [target], |row| {
                        row.get(0)
                    })
                    .optional()?
                    .ok_or_else(|| invalid("relation target missing"))?;
                let target_db: Database = decode(target_raw)?;
                let target_page = search::page(&tx, &target_db.page_id)?
                    .ok_or_else(|| invalid("relation target missing"))?;
                if target_page.deleted_at.is_some()
                    || target_page.kind != PageKind::Database
                    || target_page.workspace_id != page.workspace_id
                {
                    return Err(invalid("invalid relation target"));
                }
            }
            crate::advanced_properties::graph(&tx, &page.workspace_id, &updated, None)?;
            let mut after = String::new();
            loop {
                let ids: Vec<String> = {
                    let mut q = tx.prepare("SELECT id FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256")?;
                    let rows = q.query_map(params![db.id, after], |row| row.get(0))?;
                    rows.collect::<Result<Vec<_>>>()?
                };
                if ids.is_empty() {
                    break;
                }
                after = ids.last().unwrap().clone();
                if !removed.is_empty() {
                    for id in &ids {
                        let raw: String =
                            tx.query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
                                row.get(0)
                            })?;
                        let mut record: Record = decode(raw)?;
                        let mut changed = false;
                        for property in &removed {
                            changed |= record.values.remove(&property.id).is_some();
                        }
                        if changed {
                            let values = encode(&record.values)?;
                            tx.execute(
                                "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
                                params![values, encode(&record)?, record.id],
                            )?;
                            if record.deleted_at.is_none() {
                                let target = search::page(
                                    &tx,
                                    record.page_id.as_deref().unwrap_or(&db.page_id),
                                )?
                                .ok_or_else(|| invalid("record page missing"))?;
                                search::index(
                                    &tx,
                                    &format!("record:{}", record.id),
                                    &target,
                                    &format!("{} {values}", record.title),
                                )?;
                            }
                        }
                    }
                }
                crate::calculation::refresh(&tx, &ids)?;
            }
        }
        tx.commit()
    }
    pub fn save_record(&mut self, record: &Record) -> Result<()> {
        nonempty(&record.id)?;
        let actor = self.history_actor.clone();
        let coalesce = self.history_coalesce;
        let db = self
            .get_database(&record.database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        if record
            .values
            .keys()
            .any(|key| !db.properties.iter().any(|p| &p.id == key))
        {
            return Err(invalid("unknown record property"));
        }
        for timestamp in [&record.created_at, &record.updated_at]
            .into_iter()
            .chain(record.deleted_at.iter())
        {
            chrono::DateTime::parse_from_rfc3339(timestamp)
                .map_err(|_| invalid("invalid record timestamp"))?;
        }
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() {
            return Err(invalid("database in trash"));
        }
        if let Some(pid) = &record.page_id {
            let p = search::page(&self.conn, pid)?.ok_or_else(|| invalid("record page missing"))?;
            if p.workspace_id != owner.workspace_id
                || p.kind != PageKind::Record
                || p.parent_id.as_deref() != Some(owner.id.as_str())
                || p.deleted_at.is_some()
            {
                return Err(invalid("invalid record page"));
            }
            if p.title != record.title {
                return Err(invalid("use rename_record to change a linked record title"));
            }
        }
        let old: Option<(String, Option<String>, Option<String>)> = self
            .conn
            .query_row(
                "SELECT database_id,page_id,deleted_at FROM records WHERE id=?1",
                [&record.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((id, pid, deleted_at)) = old {
            if id != record.database_id || pid != record.page_id || deleted_at != record.deleted_at
            {
                return Err(invalid("record owner changed"));
            }
        } else if record.page_id.is_some() {
            return Err(invalid("link record pages with ensure_record_page"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT data FROM records WHERE id=?1",
                [&record.id],
                |row| row.get(0),
            )
            .optional()?;
        let previous: Option<Record> = previous.map(decode).transpose()?;
        for property in db
            .properties
            .iter()
            .filter(|p| p.kind == crate::PropertyKind::Relation)
        {
            let value = record
                .values
                .get(&property.id)
                .unwrap_or(&serde_json::Value::Null);
            let old = previous
                .as_ref()
                .and_then(|r| r.values.get(&property.id))
                .unwrap_or(&serde_json::Value::Null);
            if value != old {
                crate::relations::validate_relation_value(
                    &tx,
                    property,
                    value,
                    &owner.workspace_id,
                )?;
            }
        }
        if let Some(previous) = &previous {
            crate::history::record_change(
                &tx,
                &owner,
                "record_title",
                &record.id,
                None,
                Some(&serde_json::json!(previous.title)),
                Some(&serde_json::json!(record.title)),
                actor.as_ref(),
            )?;
            for property in &db.properties {
                if matches!(
                    property.kind,
                    crate::PropertyKind::Formula
                        | crate::PropertyKind::Rollup
                        | crate::PropertyKind::Title
                ) {
                    continue;
                }
                let before = previous
                    .values
                    .get(&property.id)
                    .unwrap_or(&serde_json::Value::Null);
                let after = record
                    .values
                    .get(&property.id)
                    .unwrap_or(&serde_json::Value::Null);
                if property.kind == crate::PropertyKind::Relation {
                    crate::history::record_change(
                        &tx,
                        &owner,
                        "relation",
                        &record.id,
                        Some(&property.id),
                        Some(before),
                        Some(after),
                        actor.as_ref(),
                    )?;
                } else {
                    crate::history::record_text_change(
                        &tx,
                        &owner,
                        "record",
                        &record.id,
                        &property.id,
                        before,
                        after,
                        actor.as_ref(),
                        coalesce
                            && matches!(
                                property.kind,
                                crate::PropertyKind::Text
                                    | crate::PropertyKind::Url
                                    | crate::PropertyKind::Email
                                    | crate::PropertyKind::Phone
                            ),
                    )?;
                }
            }
        } else {
            crate::history::record_change(
                &tx,
                &owner,
                "record_created",
                &record.id,
                None,
                None,
                Some(&serde_json::json!(record.title)),
                actor.as_ref(),
            )?;
        }
        tx.execute("INSERT INTO records(id,database_id,page_id,title,values_json,deleted_at,data) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET title=excluded.title,values_json=excluded.values_json,deleted_at=excluded.deleted_at,data=excluded.data",params![record.id,record.database_id,record.page_id,record.title,encode(&record.values)?,record.deleted_at,encode(record)?])?;
        let target = if let Some(pid) = &record.page_id {
            search::page(&tx, pid)?.ok_or_else(|| invalid("record page missing"))?
        } else {
            owner
        };
        search::remove(&tx, &format!("record:{}", record.id))?;
        if record.deleted_at.is_none() {
            search::index(
                &tx,
                &format!("record:{}", record.id),
                &target,
                &format!("{} {}", record.title, encode(&record.values)?),
            )?;
        }
        crate::calculation::refresh(&tx, std::slice::from_ref(&record.id))?;
        tx.commit()
    }
    pub fn query_records(&self, database_id: &str, query: &RecordQuery) -> Result<Vec<Record>> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() {
            return Err(invalid("database in trash"));
        }
        let mut args = vec![SqlValue::Text(database_id.into())];
        let mut sql =
            String::from("SELECT data FROM records WHERE database_id=? AND deleted_at IS NULL");
        if let Some(f) = &query.filter {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(f, &db, &mut args, 0)?);
        }
        sql.push_str(" ORDER BY ");
        if let Some(group) = &query.group_by {
            sql.push_str(&property_expr(group, &db, &mut args)?);
            sql.push_str(" ASC NULLS LAST, ");
        }
        if query.sorts.len() > 8 {
            return Err(invalid("too many sort rules"));
        }
        for sort in &query.sorts {
            sql.push_str(&property_expr(&sort.property_id, &db, &mut args)?);
            sql.push_str(if sort.descending {
                " DESC NULLS LAST, "
            } else {
                " ASC NULLS LAST, "
            });
        }
        sql.push_str("id ASC LIMIT ? OFFSET ?");
        args.push(SqlValue::Integer(query.limit.min(1000) as i64));
        args.push(SqlValue::Integer(
            i64::try_from(query.offset).map_err(|_| invalid("offset too large"))?,
        ));
        collect_json(&mut self.conn.prepare(&sql)?, params_from_iter(args))
    }
    /// Find the record linked to a page. Duplicate links are reported as invalid data.
    pub fn get_record_by_page(&self, page_id: &str) -> Result<Option<Record>> {
        let mut records: Vec<Record> = collect_json(
            &mut self
                .conn
                .prepare("SELECT data FROM records WHERE page_id=?1 LIMIT 2")?,
            [page_id],
        )?;
        if records.len() > 1 {
            return Err(invalid("multiple records linked to page"));
        }
        Ok(records.pop())
    }
    pub fn get_record(&self, id: &str) -> Result<Option<Record>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        raw.map(decode).transpose()
    }
    pub fn list_trash(&self, workspace_id: &str) -> Result<Vec<Page>> {
        collect_json(&mut self.conn.prepare("SELECT data FROM pages WHERE workspace_id=?1 AND deleted_at IS NOT NULL AND id=trash_root ORDER BY deleted_at DESC")?,[workspace_id])
    }
    pub fn search(
        &self,
        query: &str,
        workspace: Option<&str>,
        limit: u32,
    ) -> Result<Vec<SearchHit>> {
        search::search(&self.conn, query, workspace, limit)
    }
}

pub(crate) fn property_expr(id: &str, db: &Database, args: &mut Vec<SqlValue>) -> Result<String> {
    let prop = db
        .properties
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| invalid("unknown property"))?;
    if prop.kind == crate::PropertyKind::Title {
        Ok("title".into())
    } else {
        args.push(SqlValue::Text(id.into()));
        if prop.kind == crate::PropertyKind::Date {
            Ok("(SELECT CASE WHEN type='object' THEN json_extract(value,'$.start') ELSE value END FROM json_each(records.values_json) WHERE key=? LIMIT 1)".into())
        } else if matches!(
            prop.kind,
            crate::PropertyKind::Formula | crate::PropertyKind::Rollup
        ) {
            Ok("(SELECT CASE WHEN type='object' THEN CASE WHEN json_type(value,'$._moss_error') IS NOT NULL THEN NULL ELSE value END ELSE value END FROM json_each(records.values_json) WHERE key=? LIMIT 1)".into())
        } else {
            Ok("(SELECT value FROM json_each(records.values_json) WHERE key=? LIMIT 1)".into())
        }
    }
}
pub(crate) fn filter_sql(
    f: &FilterExpr,
    db: &Database,
    args: &mut Vec<SqlValue>,
    depth: usize,
) -> Result<String> {
    if depth > 12 {
        return Err(invalid("filter too deep"));
    }
    match f {
        FilterExpr::All(items) | FilterExpr::Any(items) => {
            if items.len() > 100 {
                return Err(invalid("too many filters"));
            }
            if items.is_empty() {
                return Ok(if matches!(f, FilterExpr::All(_)) {
                    "1"
                } else {
                    "0"
                }
                .into());
            }
            let parts: Vec<String> = items
                .iter()
                .map(|x| filter_sql(x, db, args, depth + 1))
                .collect::<Result<_>>()?;
            Ok(format!(
                "({})",
                parts.join(if matches!(f, FilterExpr::All(_)) {
                    " AND "
                } else {
                    " OR "
                })
            ))
        }
        FilterExpr::Rule(rule) => {
            let mut e = property_expr(&rule.property_id, db, args)?;
            let date_only = db
                .properties
                .iter()
                .any(|p| p.id == rule.property_id && p.kind == crate::PropertyKind::Date)
                && rule
                    .value
                    .as_str()
                    .is_some_and(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok())
                && matches!(
                    rule.op,
                    FilterOp::Eq
                        | FilterOp::Ne
                        | FilterOp::Gt
                        | FilterOp::Ge
                        | FilterOp::Lt
                        | FilterOp::Le
                );
            if date_only {
                e = format!("substr({e},1,10)");
            }
            match rule.op {
                FilterOp::IsEmpty => Ok(format!("(COALESCE({e}, '')='')")),
                FilterOp::IsNotEmpty => Ok(format!("(COALESCE({e}, '')<>'')")),
                op => {
                    let v = match &rule.value {
                        serde_json::Value::Null => SqlValue::Null,
                        serde_json::Value::Bool(b) => SqlValue::Integer(*b as i64),
                        serde_json::Value::Number(n) => n
                            .as_i64()
                            .map(SqlValue::Integer)
                            .or_else(|| n.as_f64().map(SqlValue::Real))
                            .ok_or_else(|| invalid("invalid number"))?,
                        serde_json::Value::String(s) => SqlValue::Text(s.clone()),
                        _ => SqlValue::Text(encode(&rule.value)?),
                    };
                    args.push(v);
                    let operator = match op {
                        FilterOp::Eq => "IS",
                        FilterOp::Ne => "IS NOT",
                        FilterOp::Gt => ">",
                        FilterOp::Ge => ">=",
                        FilterOp::Lt => "<",
                        FilterOp::Le => "<=",
                        FilterOp::Contains => "LIKE",
                        FilterOp::NotContains => "NOT LIKE",
                        _ => unreachable!(),
                    };
                    if matches!(op, FilterOp::Contains | FilterOp::NotContains) {
                        let last = args.last_mut().unwrap();
                        if let SqlValue::Text(s) = last {
                            *s = format!(
                                "%{}%",
                                s.replace('\\', "\\\\")
                                    .replace('%', "\\%")
                                    .replace('_', "\\_")
                            );
                        }
                    }
                    Ok(format!(
                        "({e} {operator} ?{})",
                        if matches!(op, FilterOp::Contains | FilterOp::NotContains) {
                            " ESCAPE '\\'"
                        } else {
                            ""
                        }
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod query_regression_tests {
    use super::*;
    use crate::{Database, FilterRule, Property, PropertyKind, SortRule};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn missing_values_sort_last_in_both_directions_and_null_filters_work() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Test", "").unwrap();
        let page = store
            .create_page(&workspace.id, None, "Records", "", PageKind::Database)
            .unwrap();
        let db = Database {
            id: new_id(),
            page_id: page.id,
            name: "Records".into(),
            properties: vec![Property {
                id: "rank".into(),
                name: "Rank".into(),
                kind: PropertyKind::Number,
                config: json!({}),
            }],
            views: vec![],
        };
        store.save_database(&db).unwrap();
        for (name, rank) in [("low", Some(1)), ("high", Some(2)), ("missing", None)] {
            let mut values = BTreeMap::new();
            if let Some(rank) = rank {
                values.insert("rank".into(), json!(rank));
            }
            let t = now();
            store
                .save_record(&Record {
                    id: new_id(),
                    database_id: db.id.clone(),
                    page_id: None,
                    title: name.into(),
                    values,
                    created_at: t.clone(),
                    updated_at: t,
                    deleted_at: None,
                })
                .unwrap();
        }
        for (descending, expected) in [
            (false, vec!["low", "high", "missing"]),
            (true, vec!["high", "low", "missing"]),
        ] {
            let q = RecordQuery {
                sorts: vec![SortRule {
                    property_id: "rank".into(),
                    descending,
                }],
                ..RecordQuery::default()
            };
            let names: Vec<String> = store
                .query_records(&db.id, &q)
                .unwrap()
                .into_iter()
                .map(|r| r.title)
                .collect();
            assert_eq!(names, expected);
            let first = RecordQuery {
                limit: 1,
                ..q.clone()
            };
            let second = RecordQuery {
                limit: 1,
                offset: 1,
                ..q
            };
            assert_eq!(
                store.query_records(&db.id, &first).unwrap()[0].title,
                expected[0]
            );
            assert_eq!(
                store.query_records(&db.id, &second).unwrap()[0].title,
                expected[1]
            );
        }
        let q = RecordQuery {
            filter: Some(FilterExpr::Rule(FilterRule {
                property_id: "rank".into(),
                op: FilterOp::Eq,
                value: json!(null),
            })),
            ..RecordQuery::default()
        };
        assert_eq!(store.query_records(&db.id, &q).unwrap()[0].title, "missing");
    }
}
