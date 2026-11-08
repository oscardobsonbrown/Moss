use crate::{Block, BlockKind, Store, store::encode};
use rusqlite::{Result, params};
use std::collections::BTreeMap;

// The requested IDs must drive the indexed lookups, even in a large workspace.
const RECORD_APPEARANCE_SQL: &str = "SELECT r.id, COALESCE(json_extract(p.data, '$.icon'), ''),
               EXISTS(SELECT 1 FROM blocks b WHERE b.page_id=p.id AND
                   (json_extract(b.data, '$.kind') IN ('divider','image','video','audio','pdf','file','bookmark','embed','table_of_contents','synced_block','child_page','simple_table')
                    OR length(trim(COALESCE(json_extract(b.data, '$.text'), ''), char(9,10,11,12,13,32,133,160,5760,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8232,8233,8239,8287,12288))) > 0))
             FROM json_each(?2) requested
             CROSS JOIN records r ON r.id=requested.value
             CROSS JOIN databases d ON d.id=r.database_id
             CROSS JOIN pages owner ON owner.id=d.page_id
             LEFT JOIN pages p ON p.id=r.page_id
             WHERE owner.workspace_id=?1 AND owner.deleted_at IS NULL
               AND r.deleted_at IS NULL
               AND (r.page_id IS NULL OR (p.workspace_id=?1 AND p.deleted_at IS NULL
                    AND p.parent_id=owner.id AND json_extract(p.data, '$.kind')='record'))";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordPageAppearance {
    pub icon: String,
    pub has_body_content: bool,
}

pub fn blocks_have_content(blocks: &[Block]) -> bool {
    blocks.iter().any(|block| {
        matches!(
            block.kind,
            BlockKind::Divider
                | BlockKind::Image
                | BlockKind::Video
                | BlockKind::Audio
                | BlockKind::Pdf
                | BlockKind::File
                | BlockKind::Bookmark
                | BlockKind::Embed
                | BlockKind::TableOfContents
                | BlockKind::SyncedBlock
                | BlockKind::ChildPage
                | BlockKind::SimpleTable
        ) || !block.text.trim().is_empty()
    })
}

impl Store {
    /// Read page icons for a bounded record page without per-row storage access.
    pub fn record_page_icons(
        &self,
        workspace_id: &str,
        record_ids: &[String],
    ) -> Result<BTreeMap<String, String>> {
        Ok(self
            .record_page_appearance(workspace_id, record_ids)?
            .into_iter()
            .map(|(id, appearance)| (id, appearance.icon))
            .collect())
    }

    /// Read custom icons and body presence for a bounded set of live record pages.
    pub fn record_page_appearance(
        &self,
        workspace_id: &str,
        record_ids: &[String],
    ) -> Result<BTreeMap<String, RecordPageAppearance>> {
        if record_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        self.conn
            .prepare_cached(RECORD_APPEARANCE_SQL)?
            .query_map(params![workspace_id, encode(&record_ids)?], |row| {
                Ok((
                    row.get(0)?,
                    RecordPageAppearance {
                        icon: row.get(1)?,
                        has_body_content: row.get(2)?,
                    },
                ))
            })?
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, DatabaseView, Property, PropertyKind, RecordQuery, ViewKind, new_id};
    use serde_json::json;

    #[test]
    fn appearance_query_drives_record_primary_key_from_requested_ids() {
        let store = Store::in_memory().unwrap();
        let sql = format!("EXPLAIN QUERY PLAN {RECORD_APPEARANCE_SQL}");
        let mut statement = store.conn.prepare(&sql).unwrap();
        let plan: Vec<String> = statement
            .query_map(params!["workspace", "[\"one\"]"], |row| row.get(3))
            .unwrap()
            .collect::<Result<_>>()
            .unwrap();
        assert!(
            plan.first()
                .is_some_and(|line| line.contains("SCAN requested")),
            "{plan:?}"
        );
        assert!(
            plan.iter()
                .any(|line| line.contains("SEARCH r USING INDEX sqlite_autoindex_records_1")),
            "{plan:?}"
        );
    }

    #[test]
    fn icons_are_batched_and_exclude_foreign_or_trashed_pages() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Icons", "").unwrap();
        let other = store.create_workspace("Other", "").unwrap();
        let page = store
            .create_database_page(
                &workspace.id,
                None,
                "Items",
                "",
                Database {
                    id: new_id(),
                    page_id: String::new(),
                    name: "Items".into(),
                    properties: vec![Property {
                        id: "title".into(),
                        name: "Name".into(),
                        kind: PropertyKind::Title,
                        config: json!({}),
                    }],
                    views: vec![DatabaseView {
                        id: new_id(),
                        name: "Table".into(),
                        kind: ViewKind::Table,
                        query: RecordQuery::default(),
                        config: json!({}),
                    }],
                },
            )
            .unwrap();
        let db = store.get_database_by_page(&page.id).unwrap().unwrap();
        let first = store
            .create_record(&db.id, "One", &BTreeMap::new())
            .unwrap();
        let second = store
            .create_record(&db.id, "Two", &BTreeMap::new())
            .unwrap();
        let mut linked = store.ensure_record_page(&first.id).unwrap();
        linked.icon = "🌱".into();
        store.update_page(&linked).unwrap();
        let ids = vec![first.id.clone(), second.id.clone(), "missing".into()];
        let icons = store.record_page_icons(&workspace.id, &ids).unwrap();
        assert_eq!(icons.get(&first.id).map(String::as_str), Some("🌱"));
        assert_eq!(icons.get(&second.id).map(String::as_str), Some(""));
        assert_eq!(icons.len(), 2);
        assert!(store.record_page_icons(&other.id, &ids).unwrap().is_empty());
        store.delete_page(&linked.id).unwrap();
        assert!(
            !store
                .record_page_icons(&workspace.id, &ids)
                .unwrap()
                .contains_key(&first.id)
        );
    }

    #[test]
    fn appearance_distinguishes_empty_text_and_real_body_without_creating_pages() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Icons", "").unwrap();
        let other = store.create_workspace("Other", "").unwrap();
        let page = store
            .create_database_page(
                &workspace.id,
                None,
                "Items",
                "",
                Database {
                    id: new_id(),
                    page_id: String::new(),
                    name: "Items".into(),
                    properties: vec![Property {
                        id: "title".into(),
                        name: "Name".into(),
                        kind: PropertyKind::Title,
                        config: json!({}),
                    }],
                    views: vec![DatabaseView {
                        id: new_id(),
                        name: "Table".into(),
                        kind: ViewKind::Table,
                        query: RecordQuery::default(),
                        config: json!({}),
                    }],
                },
            )
            .unwrap();
        let db = store.get_database_by_page(&page.id).unwrap().unwrap();
        let linked = store
            .create_record(&db.id, "Title only", &BTreeMap::new())
            .unwrap();
        let unlinked = store
            .create_record(&db.id, "Unlinked", &BTreeMap::new())
            .unwrap();
        let mut record_page = store.ensure_record_page(&linked.id).unwrap();
        record_page.icon = "🌱".into();
        store.update_page(&record_page).unwrap();
        let ids = vec![linked.id.clone(), unlinked.id.clone()];
        let read = |store: &Store| store.record_page_appearance(&workspace.id, &ids).unwrap();
        assert!(!read(&store)[&linked.id].has_body_content);
        assert_eq!(read(&store)[&linked.id].icon, "🌱");
        assert!(!read(&store)[&unlinked.id].has_body_content);
        assert!(
            store
                .get_record(&unlinked.id)
                .unwrap()
                .unwrap()
                .page_id
                .is_none()
        );
        let blank = Block::paragraph(&record_page.id, " \t\n\u{00a0}", 0);
        store
            .save_blocks(&record_page.id, &[blank.clone()])
            .unwrap();
        assert!(!read(&store)[&linked.id].has_body_content);
        let mut heading = blank.clone();
        heading.kind = BlockKind::Heading1;
        heading.text = "A heading".into();
        store.save_blocks(&record_page.id, &[heading]).unwrap();
        assert!(read(&store)[&linked.id].has_body_content);
        let mut media = blank.clone();
        media.kind = BlockKind::Image;
        media.text.clear();
        store.save_blocks(&record_page.id, &[media]).unwrap();
        assert!(read(&store)[&linked.id].has_body_content);
        store.save_blocks(&record_page.id, &[blank]).unwrap();
        assert!(!read(&store)[&linked.id].has_body_content);
        assert!(
            store
                .record_page_appearance(&other.id, &ids)
                .unwrap()
                .is_empty()
        );
        store.delete_page(&record_page.id).unwrap();
        assert!(!read(&store).contains_key(&linked.id));
    }
}
