use crate::{
    PageCover, Store,
    store::{decode, encode},
};
use rusqlite::{Result, params};
use std::collections::BTreeMap;

// Make the bounded requested IDs drive primary-key lookups, not a workspace scan.
const PREVIEW_SQL: &str = "SELECT r.id, COALESCE((
                SELECT substr(json_extract(b.data, '$.text'), 1, 512)
                FROM blocks b WHERE b.page_id=p.id
                ORDER BY b.sort_order, b.id LIMIT 1
             ), '')
             FROM json_each(?2) requested
             CROSS JOIN records r ON r.id=requested.value
             CROSS JOIN databases d ON d.id=r.database_id
             CROSS JOIN pages owner ON owner.id=d.page_id
             LEFT JOIN pages p ON p.id=r.page_id
             WHERE owner.workspace_id=?1 AND owner.deleted_at IS NULL
               AND r.deleted_at IS NULL
               AND (r.page_id IS NULL OR (p.workspace_id=?1 AND p.deleted_at IS NULL))";

impl Store {
    /// Read covers for the requested live records in one bounded query.
    pub fn record_page_covers(
        &self,
        workspace_id: &str,
        record_ids: &[String],
    ) -> Result<BTreeMap<String, PageCover>> {
        if record_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut statement = self.conn.prepare_cached("SELECT r.id, json_extract(p.data, '$.cover') FROM json_each(?2) requested CROSS JOIN records r ON r.id=requested.value CROSS JOIN databases d ON d.id=r.database_id CROSS JOIN pages owner ON owner.id=d.page_id JOIN pages p ON p.id=r.page_id WHERE owner.workspace_id=?1 AND owner.deleted_at IS NULL AND r.deleted_at IS NULL AND p.workspace_id=?1 AND p.deleted_at IS NULL AND p.parent_id=owner.id AND json_extract(p.data, '$.kind')='record' AND json_type(p.data, '$.cover')='object'")?;
        statement
            .query_map(params![workspace_id, encode(&record_ids)?], |row| {
                let raw: String = row.get(1)?;
                Ok((row.get(0)?, decode(raw)?))
            })?
            .collect()
    }

    /// Read bounded first-block text for visible Gallery records in one query.
    pub fn record_page_previews(
        &self,
        workspace_id: &str,
        record_ids: &[String],
    ) -> Result<BTreeMap<String, String>> {
        if record_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut statement = self.conn.prepare_cached(PREVIEW_SQL)?;
        statement
            .query_map(params![workspace_id, encode(&record_ids)?], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_query_uses_requested_record_ids_first() {
        let store = Store::in_memory().unwrap();
        let mut query = store
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {PREVIEW_SQL}"))
            .unwrap();
        let plan: Vec<String> = query
            .query_map(params!["workspace", "[\"one\"]"], |row| row.get(3))
            .unwrap()
            .collect::<Result<_>>()
            .unwrap();
        assert!(
            plan.first().is_some_and(|s| s.contains("SCAN requested")),
            "{plan:?}"
        );
        assert!(
            plan.iter()
                .any(|s| s.contains("SEARCH r USING INDEX sqlite_autoindex_records_1")),
            "{plan:?}"
        );
    }
}
