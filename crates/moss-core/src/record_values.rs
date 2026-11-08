use crate::{
    Database, PageKind, Record, Store, now, search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, TransactionBehavior, params};
use serde_json::Value;

impl Store {
    /// Change one value under a write lock, preserving concurrent changes to other properties.
    pub fn set_record_value_if_unchanged(
        &mut self,
        record_id: &str,
        property_id: &str,
        expected: &Value,
        value: Value,
    ) -> Result<Record> {
        self.set_record_value_if_unchanged_kind(record_id, property_id, expected, value, None)
    }

    pub(crate) fn set_record_value_if_unchanged_kind(
        &mut self,
        record_id: &str,
        property_id: &str,
        expected: &Value,
        value: Value,
        required_kind: Option<crate::PropertyKind>,
    ) -> Result<Record> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let raw: String = tx
            .query_row("SELECT data FROM records WHERE id=?1", [record_id], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or_else(|| invalid("record missing"))?;
        let mut record: Record = decode(raw)?;
        if record.deleted_at.is_some() {
            return Err(invalid("record in trash"));
        }
        if record.values.get(property_id).unwrap_or(&Value::Null) != expected {
            return Err(invalid(
                "property changed in another editor; reopen it before applying this draft",
            ));
        }
        let raw: String = tx
            .query_row(
                "SELECT data FROM databases WHERE id=?1",
                [&record.database_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("database missing"))?;
        let db: Database = decode(raw)?;
        let property = db
            .properties
            .iter()
            .find(|p| p.id == property_id)
            .ok_or_else(|| invalid("unknown record property"))?;
        if required_kind.is_some_and(|kind| property.kind != kind) {
            return Err(invalid("property type changed"));
        }
        if matches!(
            property.kind,
            crate::PropertyKind::Formula
                | crate::PropertyKind::Rollup
                | crate::PropertyKind::Relation
        ) {
            return Err(invalid("this property requires its dedicated editor"));
        }
        let owner =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        let target = if let Some(id) = &record.page_id {
            let page = search::page(&tx, id)?.ok_or_else(|| invalid("record page missing"))?;
            if page.deleted_at.is_some()
                || page.kind != PageKind::Record
                || page.workspace_id != owner.workspace_id
                || page.parent_id.as_deref() != Some(owner.id.as_str())
                || page.title != record.title
            {
                return Err(invalid("invalid record page"));
            }
            page
        } else {
            owner
        };
        record.values.insert(property_id.to_owned(), value);
        record.updated_at = now();
        let values = encode(&record.values)?;
        tx.execute(
            "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
            params![values, encode(&record)?, record.id],
        )?;
        search::index(
            &tx,
            &format!("record:{}", record.id),
            &target,
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
