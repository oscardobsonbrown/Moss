use crate::{
    Block, Database, Page, PageKind, PropertyKind, Record, Store, new_id, now, search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, TransactionBehavior, params};
use serde_json::{Value, json};
use std::collections::BTreeMap;

// Table cell text and rich text are content, not structural IDs.
fn remap_config(value: &mut Value, ids: &BTreeMap<String, String>, reference: bool) {
    match value {
        Value::String(id) if reference => {
            if let Some(new) = ids.get(id) {
                *id = new.clone();
            }
        }
        Value::Array(values) => {
            for value in values {
                remap_config(value, ids, reference);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key != "simple_table" {
                    remap_config(
                        value,
                        ids,
                        matches!(
                            key.as_str(),
                            "page_id"
                                | "block_id"
                                | "source_block_id"
                                | "target_block_id"
                                | "parent_block_id"
                                | "page_ids"
                                | "block_ids"
                        ),
                    );
                }
            }
        }
        _ => {}
    }
}

impl Store {
    /// Copy a live record and its optional page in one transaction.
    pub fn duplicate_board_record(&mut self, database_id: &str, record_id: &str) -> Result<Record> {
        let actor = self.history_actor.clone();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let raw: String = tx
            .query_row(
                "SELECT data FROM databases WHERE id=?1",
                [database_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("database missing"))?;
        let db: Database = decode(raw)?;
        let owner =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if db.id != database_id || owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("database is not available"));
        }
        let raw: String = tx
            .query_row(
                "SELECT data FROM records WHERE id=?1 AND database_id=?2 AND deleted_at IS NULL",
                params![record_id, database_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("record is not available in this database"))?;
        let source: Record = decode(raw)?;
        if source.id != record_id
            || source.database_id != database_id
            || source.deleted_at.is_some()
        {
            return Err(invalid("record is not available in this database"));
        }
        let stamp = now();
        let mut record = Record {
            id: new_id(),
            database_id: database_id.into(),
            page_id: None,
            title: format!(
                "{} (copy)",
                if source.title.is_empty() {
                    "Untitled"
                } else {
                    &source.title
                }
            ),
            values: BTreeMap::new(),
            created_at: stamp.clone(),
            updated_at: stamp.clone(),
            deleted_at: None,
        };
        for property in &db.properties {
            let value = match property.kind {
                PropertyKind::Title
                | PropertyKind::Formula
                | PropertyKind::Rollup
                | PropertyKind::UniqueId
                | PropertyKind::Button => None,
                PropertyKind::CreatedTime | PropertyKind::EditedTime => Some(json!(stamp)),
                _ => source.values.get(&property.id).cloned(),
            };
            if let Some(value) = value {
                if property.kind == PropertyKind::Relation {
                    crate::relations::validate_relation_value(
                        &tx,
                        property,
                        &value,
                        &owner.workspace_id,
                    )?;
                }
                record.values.insert(property.id.clone(), value);
            }
        }
        crate::history::record_change(
            &tx,
            &owner,
            "record_duplicated",
            &record.id,
            None,
            None,
            Some(&json!(record.title)),
            actor.as_ref(),
        )?;
        let target = if let Some(page_id) = &source.page_id {
            let mut page: Page =
                search::page(&tx, page_id)?.ok_or_else(|| invalid("record page missing"))?;
            if page.workspace_id != owner.workspace_id
                || page.kind != PageKind::Record
                || page.parent_id.as_deref() != Some(owner.id.as_str())
                || page.deleted_at.is_some()
                || page.title != source.title
                || page
                    .cover
                    .as_ref()
                    .is_some_and(|c| !crate::page_covers::valid_cover(c))
            {
                return Err(invalid("invalid record page"));
            }
            let mut blocks: Vec<Block> = tx
                .prepare("SELECT data FROM blocks WHERE page_id=?1 ORDER BY sort_order,id")?
                .query_map([page_id], |r| r.get::<_, String>(0))?
                .map(|r| decode(r?))
                .collect::<Result<_>>()?;
            let mut ids = BTreeMap::from([(page_id.clone(), new_id())]);
            for block in &blocks {
                if block.page_id != *page_id || ids.insert(block.id.clone(), new_id()).is_some() {
                    return Err(invalid("invalid record block"));
                }
            }
            page.id = ids[page_id].clone();
            page.title = record.title.clone();
            page.created_at = stamp.clone();
            page.sidebar_order = None;
            page.updated_at = stamp;
            tx.execute(
                "INSERT INTO pages(id,workspace_id,parent_id,deleted_at,data) VALUES(?1,?2,?3,NULL,?4)",
                params![page.id, page.workspace_id, page.parent_id, encode(&page)?],
            )?;
            search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
            for block in &mut blocks {
                block.id = ids[&block.id].clone();
                block.page_id = page.id.clone();
                remap_config(&mut block.config, &ids, false);
                tx.execute(
                    "INSERT INTO blocks(id,page_id,sort_order,data) VALUES(?1,?2,?3,?4)",
                    params![block.id, page.id, block.sort_order, encode(block)?],
                )?;
                search::index(
                    &tx,
                    &format!("block:{}", block.id),
                    &page,
                    &format!("{} {}", block.text, block.config),
                )?;
            }
            record.page_id = Some(page.id.clone());
            page
        } else {
            owner
        };
        tx.execute(
            "INSERT INTO records(id,database_id,page_id,title,values_json,deleted_at,data) VALUES(?1,?2,?3,?4,?5,NULL,?6)",
            params![record.id, record.database_id, record.page_id, record.title, encode(&record.values)?, encode(&record)?],
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
        let copied = decode(raw)?;
        tx.commit()?;
        Ok(copied)
    }
}

#[cfg(test)]
#[path = "board_copy_tests.rs"]
mod tests;
