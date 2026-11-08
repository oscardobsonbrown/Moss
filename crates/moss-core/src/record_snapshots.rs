use crate::{
    Record, Store,
    store::{decode, invalid},
};
use rusqlite::{Result, params};

impl Store {
    /// Read a bounded set of live records from one database, without changing their order in the UI.
    pub fn record_snapshots(&self, database_id: &str, ids: &[String]) -> Result<Vec<Record>> {
        if ids.len() > 1000 {
            return Err(invalid("too many record IDs"));
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = serde_json::to_string(ids).map_err(|error| invalid(&error.to_string()))?;
        let mut stmt = self.conn.prepare(
            "SELECT r.data FROM records r JOIN databases d ON d.id=r.database_id \
             JOIN pages p ON p.id=d.page_id WHERE r.database_id=?1 AND r.deleted_at IS NULL \
             AND p.deleted_at IS NULL AND r.id IN (SELECT value FROM json_each(?2))",
        )?;
        let rows = stmt.query_map(params![database_id, ids], |row| decode(row.get(0)?))?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RecordQuery;

    #[test]
    fn snapshots_are_scoped_and_bounded() {
        let mut store = Store::in_memory().unwrap();
        let workspaces = store.seed_demo_workspaces().unwrap();
        let first = store
            .search_relation_databases(&workspaces[0].id, "", 10)
            .unwrap()
            .remove(0);
        let second = store
            .search_relation_databases(&workspaces[1].id, "", 10)
            .unwrap()
            .remove(0);
        let a = store
            .query_records(&first.id, &RecordQuery::default())
            .unwrap()
            .remove(0);
        let b = store
            .query_records(&second.id, &RecordQuery::default())
            .unwrap()
            .remove(0);
        let found = store
            .record_snapshots(&first.id, &[a.id.clone(), b.id])
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, a.id);
        assert!(
            store
                .record_snapshots(&first.id, &vec!["x".into(); 1001])
                .is_err()
        );
    }
}
