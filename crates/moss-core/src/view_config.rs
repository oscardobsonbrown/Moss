use crate::{
    Database, PageKind, Store, ViewKind, search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, TransactionBehavior, params};
use serde_json::Value;

impl Store {
    /// Apply a config edit to the current saved view while holding the write lock.
    pub fn edit_view_config(
        &mut self,
        database_id: &str,
        view_id: &str,
        expected_kind: ViewKind,
        edit: impl FnOnce(&mut Value) -> std::result::Result<(), String>,
    ) -> Result<Database> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let raw: String = tx
            .query_row(
                "SELECT data FROM databases WHERE id=?1",
                [database_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| invalid("database missing"))?;
        let mut db: Database = decode(raw)?;
        let owner =
            search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        let view = db
            .views
            .iter_mut()
            .find(|view| view.id == view_id)
            .ok_or_else(|| invalid("view missing"))?;
        if view.kind != expected_kind {
            return Err(invalid(
                "view layout changed in another editor; reopen its settings",
            ));
        }
        edit(&mut view.config).map_err(|message| invalid(&message))?;
        tx.execute(
            "UPDATE databases SET data=?1 WHERE id=?2",
            params![encode(&db)?, db.id],
        )?;
        tx.commit()?;
        Ok(db)
    }
}
