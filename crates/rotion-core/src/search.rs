use crate::{Page, SearchHit};
use rusqlite::{Connection, OptionalExtension, Result, params};

pub(crate) fn index(conn: &Connection, key: &str, page: &Page, body: &str) -> Result<()> {
    conn.execute("DELETE FROM search_fts WHERE key=?1", [key])?;
    conn.execute(
        "INSERT INTO search_fts(key,page_id,workspace_id,body) VALUES (?1,?2,?3,?4)",
        params![key, page.id, page.workspace_id, body],
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
    let expr = terms.join(" AND ");
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
}
