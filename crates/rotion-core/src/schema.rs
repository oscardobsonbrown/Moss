use rusqlite::{Connection, Result};

pub(crate) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
        CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE TABLE IF NOT EXISTS pages(id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE, parent_id TEXT REFERENCES pages(id) ON DELETE CASCADE, deleted_at TEXT, trash_root TEXT, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE INDEX IF NOT EXISTS pages_workspace ON pages(workspace_id, deleted_at);
        CREATE INDEX IF NOT EXISTS pages_parent ON pages(parent_id);
        CREATE INDEX IF NOT EXISTS pages_expired_trash ON pages(deleted_at) WHERE deleted_at IS NOT NULL;
        CREATE TABLE IF NOT EXISTS blocks(id TEXT PRIMARY KEY, page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE, sort_order INTEGER NOT NULL, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE INDEX IF NOT EXISTS blocks_page ON blocks(page_id, sort_order);
        CREATE TABLE IF NOT EXISTS databases(id TEXT PRIMARY KEY, page_id TEXT NOT NULL UNIQUE REFERENCES pages(id) ON DELETE CASCADE, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY, database_id TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE, page_id TEXT REFERENCES pages(id) ON DELETE CASCADE, title TEXT NOT NULL, values_json TEXT NOT NULL CHECK(json_valid(values_json)), deleted_at TEXT, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE TABLE IF NOT EXISTS record_trash_roots(record_id TEXT PRIMARY KEY REFERENCES records(id) ON DELETE CASCADE, root_id TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS records_database ON records(database_id, deleted_at, id);
        CREATE INDEX IF NOT EXISTS records_database_title ON records(database_id, deleted_at, title, id);
        CREATE INDEX IF NOT EXISTS records_expired_trash ON records(deleted_at) WHERE deleted_at IS NOT NULL;
        CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(key UNINDEXED, page_id UNINDEXED, workspace_id UNINDEXED, body, tokenize='unicode61');
        CREATE TRIGGER IF NOT EXISTS page_search_delete AFTER DELETE ON pages BEGIN DELETE FROM search_fts WHERE page_id=old.id; END;
        CREATE TRIGGER IF NOT EXISTS block_search_delete AFTER DELETE ON blocks BEGIN DELETE FROM search_fts WHERE key='block:' || old.id; END;
        CREATE TRIGGER IF NOT EXISTS record_search_delete AFTER DELETE ON records BEGIN DELETE FROM search_fts WHERE key='record:' || old.id; END;"
    )?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        // Rebuild the FTS table once, retaining rowids and all existing content.
        // The old schema and its triggers remain intact if this transaction fails.
        conn.execute_batch(r#"
            BEGIN IMMEDIATE;
            DROP TRIGGER IF EXISTS page_search_delete;
            DROP TRIGGER IF EXISTS block_search_delete;
            DROP TRIGGER IF EXISTS record_search_delete;
            ALTER TABLE search_fts RENAME TO search_fts_old;
            CREATE VIRTUAL TABLE search_fts USING fts5(key, page_id UNINDEXED, workspace_id UNINDEXED, body, tokenize='unicode61');
            INSERT INTO search_fts(rowid,key,page_id,workspace_id,body)
                SELECT rowid,key,page_id,workspace_id,body FROM search_fts_old;
            DROP TABLE search_fts_old;
            CREATE TRIGGER page_search_delete AFTER DELETE ON pages BEGIN
                DELETE FROM search_fts WHERE search_fts MATCH ('key : "' || replace('page:' || old.id, '"', '""') || '"') AND key='page:' || old.id;
            END;
            CREATE TRIGGER block_search_delete AFTER DELETE ON blocks BEGIN
                DELETE FROM search_fts WHERE search_fts MATCH ('key : "' || replace('block:' || old.id, '"', '""') || '"') AND key='block:' || old.id;
            END;
            CREATE TRIGGER record_search_delete AFTER DELETE ON records BEGIN
                DELETE FROM search_fts WHERE search_fts MATCH ('key : "' || replace('record:' || old.id, '"', '""') || '"') AND key='record:' || old.id;
            END;
            PRAGMA user_version=1;
            COMMIT;
        "#)?;
    }
    conn.execute("DELETE FROM pages WHERE deleted_at IS NOT NULL AND julianday(deleted_at) <= julianday('now', '-30 days')", [])?;
    conn.execute("DELETE FROM records WHERE deleted_at IS NOT NULL AND julianday(deleted_at) <= julianday('now', '-30 days')", [])?;
    Ok(())
}
