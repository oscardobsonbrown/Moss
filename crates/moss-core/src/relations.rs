use crate::{
    Database, PageKind, Property, PropertyKind, Record, Store, now, search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, TransactionBehavior, params};
use serde_json::{Value, json};
use std::collections::HashSet;

pub(crate) fn validate_relation_value(
    tx: &rusqlite::Transaction<'_>,
    property: &Property,
    value: &Value,
    workspace: &str,
) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    let ids = value
        .as_array()
        .ok_or_else(|| invalid("relation requires a list of record IDs"))?;
    let target_id = property
        .config
        .get("target_database_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("relation target missing"))?;
    let raw: String = tx
        .query_row("SELECT data FROM databases WHERE id=?1", [target_id], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| invalid("relation target missing"))?;
    let target_db: Database = decode(raw)?;
    let target_page = search::page(tx, &target_db.page_id)?
        .ok_or_else(|| invalid("relation target page missing"))?;
    if target_page.deleted_at.is_some()
        || target_page.kind != PageKind::Database
        || target_page.workspace_id != workspace
    {
        return Err(invalid("invalid relation target"));
    }
    let mut seen = HashSet::new();
    for id in ids {
        let id = id
            .as_str()
            .ok_or_else(|| invalid("relation requires record IDs"))?;
        if !seen.insert(id) {
            return Err(invalid("duplicate relation target"));
        }
        let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM records WHERE id=?1 AND database_id=?2 AND deleted_at IS NULL)", params![id, target_id], |r| r.get(0))?;
        if !valid {
            return Err(invalid(
                "relation target record missing or in another database",
            ));
        }
    }
    Ok(())
}

impl Store {
    pub fn search_relation_databases(
        &self,
        workspace_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<Database>> {
        if limit == 0 || limit > 200 {
            return Err(invalid("invalid search limit"));
        }
        let mut stmt = self.conn.prepare("SELECT d.data FROM databases d JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 AND p.deleted_at IS NULL AND instr(lower(json_extract(p.data,'$.title')),lower(?2))>0 ORDER BY json_extract(p.data,'$.title'), d.id LIMIT ?3")?;
        let rows = stmt.query_map(params![workspace_id, query, limit], |row| {
            decode(row.get(0)?)
        })?;
        rows.collect()
    }
    pub fn search_relation_records(
        &self,
        database_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<Record>> {
        if limit == 0 || limit > 200 {
            return Err(invalid("invalid search limit"));
        }
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let page = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if page.deleted_at.is_some() || page.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        let mut stmt = self.conn.prepare("SELECT data FROM records WHERE database_id=?1 AND deleted_at IS NULL AND instr(lower(title),lower(?2))>0 ORDER BY title,id LIMIT ?3")?;
        let rows = stmt.query_map(params![database_id, query, limit], |row| {
            decode(row.get(0)?)
        })?;
        rows.collect()
    }
    pub fn relation_records_by_ids(
        &self,
        database_id: &str,
        ids: &[String],
    ) -> Result<Vec<Record>> {
        if ids.len() > 200 {
            return Err(invalid("too many relation IDs"));
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        let marks = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT data FROM records WHERE database_id=? AND deleted_at IS NULL AND id IN ({marks})"
        );
        let mut args = vec![database_id.to_owned()];
        args.extend(ids.iter().cloned());
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), |row| decode(row.get(0)?))?;
        rows.collect()
    }
    pub fn set_relation_value_if_unchanged(
        &mut self,
        record_id: &str,
        expected_property: &Property,
        expected_value: &Value,
        ids: &[String],
    ) -> Result<Record> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
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
        let raw: String = tx
            .query_row(
                "SELECT data FROM databases WHERE id=?1",
                [&record.database_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("database missing"))?;
        let db: Database = decode(raw)?;
        let owner =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        let prop = db
            .properties
            .iter()
            .find(|p| p.id == expected_property.id)
            .ok_or_else(|| invalid("property missing"))?;
        if prop != expected_property || prop.kind != PropertyKind::Relation {
            return Err(invalid("relation changed in another editor"));
        }
        if record.values.get(&prop.id).unwrap_or(&Value::Null) != expected_value {
            return Err(invalid("relation value changed in another editor"));
        }
        validate_relation_value(&tx, prop, &json!(ids), &owner.workspace_id)?;
        record.values.insert(prop.id.clone(), json!(ids));
        record.updated_at = now();
        let values = encode(&record.values)?;
        tx.execute(
            "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
            params![values, encode(&record)?, record.id],
        )?;
        let page = match &record.page_id {
            Some(id) => search::page(&tx, id)?.ok_or_else(|| invalid("record page missing"))?,
            None => owner,
        };
        search::index(
            &tx,
            &format!("record:{}", record.id),
            &page,
            &format!("{} {values}", record.title),
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
}
