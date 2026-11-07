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
}

fn invalid(message: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.to_owned())
}
fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}
fn decode<T: DeserializeOwned>(raw: String) -> Result<T> {
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

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        schema::init(&conn)?;
        Ok(Self { conn })
    }
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::init(&conn)?;
        Ok(Self { conn })
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
        collect_json(&mut self.conn.prepare("SELECT data FROM pages WHERE workspace_id=?1 AND deleted_at IS NULL ORDER BY json_extract(data,'$.created_at'),id")?, [workspace_id])
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
            created_at: t.clone(),
            updated_at: t,
            deleted_at: None,
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
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE pages SET parent_id=?1,data=?2 WHERE id=?3",
            params![page.parent_id, encode(page)?, page.id],
        )?;
        search::index(&tx, &format!("page:{}", page.id), page, &page.title)?;
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
    pub fn save_blocks(&mut self, page_id: &str, blocks: &[Block]) -> Result<()> {
        let page =
            search::page(&self.conn, page_id)?.ok_or_else(|| invalid("page does not exist"))?;
        if page.deleted_at.is_some() {
            return Err(invalid("page is in trash"));
        }
        let mut ids = std::collections::HashSet::new();
        for b in blocks {
            nonempty(&b.id)?;
            if b.page_id != page_id || !ids.insert(&b.id) {
                return Err(invalid("invalid or duplicate block"));
            }
        }
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM blocks WHERE page_id=?1", [page_id])?;
        for b in blocks {
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
        tx.commit()
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
        nonempty(&db.id)?;
        let p = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if p.kind != PageKind::Database || p.deleted_at.is_some() {
            return Err(invalid("invalid database page"));
        }
        let mut ids = std::collections::HashSet::new();
        for prop in &db.properties {
            nonempty(&prop.id)?;
            nonempty(&prop.name)?;
            if !ids.insert(&prop.id) {
                return Err(invalid("duplicate property"));
            }
        }
        let mut views = std::collections::HashSet::new();
        for v in &db.views {
            nonempty(&v.id)?;
            if !views.insert(&v.id) {
                return Err(invalid("duplicate view"));
            }
        }
        if let Some(old) = self.get_database(&db.id)? {
            if old.page_id != db.page_id {
                return Err(invalid("database page changed"));
            }
        }
        self.conn.execute("INSERT INTO databases(id,page_id,data) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET data=excluded.data",params![db.id,db.page_id,encode(db)?])?;
        Ok(())
    }
    pub fn save_record(&mut self, record: &Record) -> Result<()> {
        nonempty(&record.id)?;
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
            if p.workspace_id != owner.workspace_id || p.deleted_at.is_some() {
                return Err(invalid("record page outside workspace"));
            }
        }
        let old: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT database_id,page_id FROM records WHERE id=?1",
                [&record.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, pid)) = old {
            if id != record.database_id || pid != record.page_id {
                return Err(invalid("record owner changed"));
            }
        }
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO records(id,database_id,page_id,title,values_json,deleted_at,data) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET title=excluded.title,values_json=excluded.values_json,deleted_at=excluded.deleted_at,data=excluded.data",params![record.id,record.database_id,record.page_id,record.title,encode(&record.values)?,record.deleted_at,encode(record)?])?;
        let target = if let Some(pid) = &record.page_id {
            search::page(&tx, pid)?.ok_or_else(|| invalid("record page missing"))?
        } else {
            owner
        };
        tx.execute(
            "DELETE FROM search_fts WHERE key=?1",
            [format!("record:{}", record.id)],
        )?;
        if record.deleted_at.is_none() {
            search::index(
                &tx,
                &format!("record:{}", record.id),
                &target,
                &format!("{} {}", record.title, encode(&record.values)?),
            )?;
        }
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
        if query.sorts.len() > 8 {
            return Err(invalid("too many sort rules"));
        }
        for sort in &query.sorts {
            sql.push_str(&property_expr(&sort.property_id, &db, &mut args)?);
            sql.push_str(if sort.descending { " DESC, " } else { " ASC, " });
        }
        sql.push_str("id ASC LIMIT ? OFFSET ?");
        args.push(SqlValue::Integer(query.limit.min(1000) as i64));
        args.push(SqlValue::Integer(
            i64::try_from(query.offset).map_err(|_| invalid("offset too large"))?,
        ));
        collect_json(&mut self.conn.prepare(&sql)?, params_from_iter(args))
    }
    pub fn get_record(&self, id: &str) -> Result<Option<Record>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        raw.map(decode).transpose()
    }
    pub fn delete_record(&mut self, id: &str) -> Result<()> {
        let mut record = self
            .get_record(id)?
            .ok_or_else(|| invalid("record missing"))?;
        if record.deleted_at.is_none() {
            record.deleted_at = Some(now());
            self.save_record(&record)?;
        }
        Ok(())
    }
    pub fn restore_record(&mut self, id: &str) -> Result<()> {
        let mut record = self
            .get_record(id)?
            .ok_or_else(|| invalid("record missing"))?;
        if record.deleted_at.is_some() {
            record.deleted_at = None;
            self.save_record(&record)?;
        }
        Ok(())
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
        }
        tx.commit()
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

fn property_expr(id: &str, db: &Database, args: &mut Vec<SqlValue>) -> Result<String> {
    let prop = db
        .properties
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| invalid("unknown property"))?;
    if prop.kind == crate::PropertyKind::Title {
        Ok("title".into())
    } else {
        args.push(SqlValue::Text(id.into()));
        Ok("(SELECT value FROM json_each(records.values_json) WHERE key=? LIMIT 1)".into())
    }
}
fn filter_sql(
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
            let e = property_expr(&rule.property_id, db, args)?;
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
                        FilterOp::Eq => "=",
                        FilterOp::Ne => "<>",
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
