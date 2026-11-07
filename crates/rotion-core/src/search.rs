use crate::{Page, SearchHit};
use rusqlite::{Connection, OptionalExtension, Result, params};

pub(crate) fn index(conn: &Connection, key: &str, page: &Page, body: &str) -> Result<()> {
    remove(conn, key)?;
    conn.execute(
        "INSERT INTO search_fts(key,page_id,workspace_id,body) VALUES (?1,?2,?3,?4)",
        params![key, page.id, page.workspace_id, body],
    )?;
    Ok(())
}

pub(crate) fn remove(conn: &Connection, key: &str) -> Result<()> {
    let expression = format!("key : \"{}\"", key.replace('"', "\"\""));
    conn.execute(
        "DELETE FROM search_fts WHERE search_fts MATCH ?1 AND key=?2",
        params![expression, key],
    )?;
    Ok(())
}

pub(crate) fn search(
    conn: &Connection,
    query: &str,
    workspace: Option<&str>,
    limit: u32,
) -> Result<Vec<SearchHit>> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() || limit == 0 {
        return Ok(vec![]);
    }
    let expr = format!("body : ({})", terms.join(" AND "));
    let mut stmt = conn.prepare("SELECT p.data, f.body FROM (SELECT page_id, min(rowid) AS rid FROM search_fts WHERE search_fts MATCH ?1 AND (?2 IS NULL OR workspace_id=?2) GROUP BY page_id) hits JOIN search_fts f ON f.rowid=hits.rid JOIN pages p ON p.id=hits.page_id WHERE p.deleted_at IS NULL AND (?2 IS NULL OR p.workspace_id=?2) ORDER BY p.id LIMIT ?3")?;
    let rows = stmt.query_map(params![expr, workspace, limit.min(500)], |r| {
        let raw: String = r.get(0)?;
        let page: Page = serde_json::from_str(&raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(SearchHit {
            page_id: page.id,
            workspace_id: page.workspace_id,
            title: page.title,
            snippet: r.get::<_, String>(1)?.chars().take(240).collect(),
            kind: page.kind,
        })
    })?;
    rows.collect()
}

pub(crate) fn page(conn: &Connection, id: &str) -> Result<Option<Page>> {
    let raw: Option<String> = conn
        .query_row("SELECT data FROM pages WHERE id=?1", [id], |r| r.get(0))
        .optional()?;
    raw.map(|s| {
        serde_json::from_str(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use crate::{PageKind, Store};

    #[test]
    fn quoted_fts_input_is_literal_and_workspace_scoped() {
        let mut store = Store::in_memory().unwrap();
        let first = store.create_workspace("One", "").unwrap();
        let second = store.create_workspace("Two", "").unwrap();
        store
            .create_page(&first.id, None, "alpha OR beta", "", PageKind::Document)
            .unwrap();
        store
            .create_page(&second.id, None, "alpha OR beta", "", PageKind::Document)
            .unwrap();
        assert_eq!(
            store
                .search("alpha OR beta", Some(&first.id), 10)
                .unwrap()
                .len(),
            1
        );
        for input in ["\"", "alpha \"", "(alpha)", "NEAR(alpha)", "*"] {
            assert!(store.search(input, Some(&first.id), 10).is_ok());
        }
    }
    #[test]
    fn repeated_saves_and_deletes_remove_old_block_text() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("One", "").unwrap();
        let page = store
            .create_page(&workspace.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let mut block = crate::Block::paragraph(&page.id, "oldterm", 0);
        block.id = "quoted\"keyuniqueterm".into();
        store.save_blocks(&page.id, &[block.clone()]).unwrap();
        assert_eq!(
            store
                .search("oldterm", Some(&workspace.id), 10)
                .unwrap()
                .len(),
            1
        );
        assert!(
            store
                .search("keyuniqueterm", Some(&workspace.id), 10)
                .unwrap()
                .is_empty()
        );
        block.text = "newterm".into();
        for _ in 0..3 {
            store.save_blocks(&page.id, &[block.clone()]).unwrap();
        }
        assert!(
            store
                .search("oldterm", Some(&workspace.id), 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .search("newterm", Some(&workspace.id), 10)
                .unwrap()
                .len(),
            1
        );
        store.save_blocks(&page.id, &[]).unwrap();
        assert!(
            store
                .search("newterm", Some(&workspace.id), 10)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn legacy_fts_migration_keeps_rows_and_indexes_keys() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE VIRTUAL TABLE search_fts USING fts5(key UNINDEXED, page_id UNINDEXED, workspace_id UNINDEXED, body, tokenize='unicode61');").unwrap();
        conn.execute("INSERT INTO search_fts(key,page_id,workspace_id,body) VALUES (?1,'page','workspace','legacytext')", ["block:quote\"id"]).unwrap();
        crate::schema::init(&conn).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM search_fts WHERE search_fts MATCH 'body:legacytext'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        super::remove(&conn, "block:quote\"id").unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM search_fts", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        crate::schema::init(&conn).unwrap();
    }
}
