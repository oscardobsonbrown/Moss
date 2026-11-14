use crate::{Page, Store, store::decode};
use rusqlite::{Result, params};

impl Store {
    /// Return a title match only when it is unique in this workspace.
    pub fn page_for_unique_title(&self, workspace: &str, title: &str) -> Result<Option<Page>> {
        let mut statement = self.conn.prepare("SELECT data FROM pages WHERE workspace_id=?1 AND deleted_at IS NULL AND json_extract(data,'$.title')=?2 COLLATE NOCASE LIMIT 2")?;
        let pages = statement
            .query_map(params![workspace, title], |row| decode(row.get(0)?))?
            .collect::<Result<Vec<Page>>>()?;
        Ok(if pages.len() == 1 {
            pages.into_iter().next()
        } else {
            None
        })
    }
    /// Resolve one imported source path without guessing between duplicate pages.
    pub fn page_for_import_path(&self, workspace: &str, path: &str) -> Result<Option<Page>> {
        let mut statement = self.conn.prepare("SELECT DISTINCT p.data FROM pages p JOIN blocks b ON b.page_id=p.id WHERE p.workspace_id=?1 AND p.deleted_at IS NULL AND json_extract(p.data,'$.kind') <> 'record' AND json_extract(b.data,'$.config.import_source_path')=?2 LIMIT 2")?;
        let pages = statement
            .query_map(params![workspace, path], |row| decode(row.get(0)?))?
            .collect::<Result<Vec<Page>>>()?;
        Ok(if pages.len() == 1 {
            pages.into_iter().next()
        } else {
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Block, PageKind};
    #[test]
    fn resolves_imported_paths_only_within_the_requested_workspace() {
        let mut store = Store::in_memory().unwrap();
        let a = store.create_workspace("A", "").unwrap();
        let b = store.create_workspace("B", "").unwrap();
        let page = store
            .create_page(&a.id, None, "Target", "", PageKind::Document)
            .unwrap();
        let mut block = Block::paragraph(&page.id, "", 0);
        block.config = serde_json::json!({"import_source_path":"Notes/Target.md"});
        store.save_blocks(&page.id, &[block]).unwrap();
        assert_eq!(
            store
                .page_for_import_path(&a.id, "Notes/Target.md")
                .unwrap()
                .unwrap()
                .id,
            page.id
        );
        assert!(
            store
                .page_for_import_path(&b.id, "Notes/Target.md")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .page_for_unique_title(&a.id, "target")
                .unwrap()
                .unwrap()
                .id,
            page.id
        );
        store
            .create_page(&a.id, None, "Target", "", PageKind::Document)
            .unwrap();
        assert!(
            store
                .page_for_unique_title(&a.id, "Target")
                .unwrap()
                .is_none()
        );
        let plan: String = store.conn.query_row("EXPLAIN QUERY PLAN SELECT p.data FROM blocks b JOIN pages p ON p.id=b.page_id WHERE json_extract(b.data,'$.config.import_source_path')=?1", ["Notes/Target.md"], |r| r.get(3)).unwrap();
        assert!(plan.contains("blocks_import_source"), "{plan}");
    }
}
