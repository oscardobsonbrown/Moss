use rusqlite::{Connection, Result};

pub(crate) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
        CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE TABLE IF NOT EXISTS pages(id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE, parent_id TEXT REFERENCES pages(id) ON DELETE CASCADE, deleted_at TEXT, trash_root TEXT, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE INDEX IF NOT EXISTS pages_workspace ON pages(workspace_id, deleted_at);
        CREATE INDEX IF NOT EXISTS pages_parent ON pages(parent_id);
        CREATE TABLE IF NOT EXISTS blocks(id TEXT PRIMARY KEY, page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE, sort_order INTEGER NOT NULL, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE INDEX IF NOT EXISTS blocks_page ON blocks(page_id, sort_order);
        CREATE TABLE IF NOT EXISTS databases(id TEXT PRIMARY KEY, page_id TEXT NOT NULL UNIQUE REFERENCES pages(id) ON DELETE CASCADE, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY, database_id TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE, page_id TEXT REFERENCES pages(id) ON DELETE CASCADE, title TEXT NOT NULL, values_json TEXT NOT NULL CHECK(json_valid(values_json)), deleted_at TEXT, data TEXT NOT NULL CHECK(json_valid(data)));
        CREATE INDEX IF NOT EXISTS records_database ON records(database_id, deleted_at, id);
        CREATE INDEX IF NOT EXISTS records_database_title ON records(database_id, deleted_at, title, id);
        CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(key UNINDEXED, page_id UNINDEXED, workspace_id UNINDEXED, body, tokenize='unicode61');
        CREATE TRIGGER IF NOT EXISTS page_search_delete AFTER DELETE ON pages BEGIN DELETE FROM search_fts WHERE page_id=old.id; END;
        CREATE TRIGGER IF NOT EXISTS block_search_delete AFTER DELETE ON blocks BEGIN DELETE FROM search_fts WHERE key='block:' || old.id; END;
        CREATE TRIGGER IF NOT EXISTS record_search_delete AFTER DELETE ON records BEGIN DELETE FROM search_fts WHERE key='record:' || old.id; END;"
    )?;
    conn.execute("DELETE FROM pages WHERE deleted_at IS NOT NULL AND julianday(deleted_at) <= julianday('now', '-30 days')", [])?;
    conn.execute("DELETE FROM records WHERE deleted_at IS NOT NULL AND julianday(deleted_at) <= julianday('now', '-30 days')", [])?;
    Ok(())
}
