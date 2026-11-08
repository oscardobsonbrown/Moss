use crate::{
    Page, PageKind, Store, now, search,
    store::{encode, invalid},
};
use rusqlite::{Result, TransactionBehavior, params};

impl Store {
    pub fn set_page_settings_if_unchanged(
        &mut self,
        workspace_id: &str,
        page_id: &str,
        expected: crate::PageSettings,
        next: crate::PageSettings,
    ) -> Result<Page> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page missing"))?;
        if page.workspace_id != workspace_id || page.deleted_at.is_some() {
            return Err(invalid("page is not live in this workspace"));
        }
        if page.settings != expected {
            return Err(invalid("page appearance changed in another editor"));
        }
        page.settings = next;
        page.updated_at = now();
        tx.execute(
            "UPDATE pages SET data=?1 WHERE id=?2",
            params![encode(&page)?, page.id],
        )?;
        tx.commit()?;
        Ok(page)
    }

    /// Rename a live page only if its title still matches the editor's starting title.
    pub fn rename_page_if_unchanged(
        &mut self,
        page_id: &str,
        expected_title: &str,
        new_title: &str,
    ) -> Result<Page> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page missing"))?;
        if page.deleted_at.is_some() || page.kind == PageKind::Record {
            return Err(invalid("page is not a live document or database"));
        }
        if page.title != expected_title {
            return Err(invalid("title changed in another editor"));
        }
        page.title = new_title.to_owned();
        page.updated_at = now();
        tx.execute(
            "UPDATE pages SET data=?1 WHERE id=?2",
            params![encode(&page)?, page.id],
        )?;
        search::index(&tx, &format!("page:{}", page.id), &page, &page.title)?;
        tx.commit()?;
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Block, PageKind, new_id};

    #[test]
    fn rename_preserves_latest_metadata_and_block_search_across_reopen() {
        let path = std::env::temp_dir().join(format!("moss-title-{}.sqlite", new_id()));
        let mut first = Store::open(&path).unwrap();
        let workspace = first.create_workspace("Test", "").unwrap();
        let page = first
            .create_page(&workspace.id, None, "Before", "📄", PageKind::Document)
            .unwrap();
        first
            .save_blocks(
                &page.id,
                &[Block::paragraph(&page.id, "bodyuniquemarker", 0)],
            )
            .unwrap();
        let mut second = Store::open(&path).unwrap();
        let mut newer = second.get_page(&page.id).unwrap().unwrap();
        newer.settings.small_text = true;
        newer.icon = "⭐".into();
        newer.cover = Some(crate::PageCover::Color {
            color: "#aabbcc".into(),
        });
        second.update_page(&newer).unwrap();
        let updated = first
            .rename_page_if_unchanged(&page.id, "Before", "After")
            .unwrap();
        assert!(updated.settings.small_text);
        assert_eq!(updated.icon, "⭐");
        assert_eq!(updated.cover, newer.cover);
        assert_eq!(updated.title, "After");
        assert!(
            first
                .search("bodyuniquemarker", Some(&workspace.id), 10)
                .unwrap()
                .iter()
                .any(|h| h.page_id == page.id)
        );
        assert!(
            first
                .search("After", Some(&workspace.id), 10)
                .unwrap()
                .iter()
                .any(|h| h.page_id == page.id)
        );
        assert!(
            first
                .rename_page_if_unchanged(&page.id, "Before", "Stale")
                .is_err()
        );
        drop(first);
        drop(second);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.get_page(&page.id).unwrap().unwrap(), updated);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn appearance_preserves_new_cover_and_rejects_stale_settings() {
        let mut s = Store::in_memory().unwrap();
        let w = s.create_workspace("Test", "").unwrap();
        let p = s
            .create_page(&w.id, None, "Before", "", PageKind::Document)
            .unwrap();
        let cover = Some(crate::PageCover::Color {
            color: "#aabbcc".into(),
        });
        s.set_page_cover_if_unchanged(&w.id, &p.id, &None, cover.clone())
            .unwrap();
        let settings = crate::PageSettings {
            small_text: true,
            ..Default::default()
        };
        let saved = s
            .set_page_settings_if_unchanged(&w.id, &p.id, p.settings, settings)
            .unwrap();
        assert_eq!(saved.cover, cover);
        assert_eq!(saved.title, "Before");
        assert!(
            s.set_page_settings_if_unchanged(&w.id, &p.id, p.settings, Default::default())
                .is_err()
        );
        assert_eq!(s.get_page(&p.id).unwrap().unwrap(), saved);
    }

    #[test]
    fn failed_write_and_trashed_or_record_page_leave_title_unchanged() {
        let mut s = Store::in_memory().unwrap();
        let workspace = s.create_workspace("Test", "").unwrap();
        let page = s
            .create_page(&workspace.id, None, "Before", "", PageKind::Document)
            .unwrap();
        s.conn.execute_batch("CREATE TRIGGER fail_page_rename BEFORE UPDATE ON pages BEGIN SELECT RAISE(ABORT, 'forced failure'); END;").unwrap();
        assert!(
            s.rename_page_if_unchanged(&page.id, "Before", "After")
                .is_err()
        );
        assert_eq!(s.get_page(&page.id).unwrap().unwrap(), page);
        assert!(
            s.search("Before", Some(&workspace.id), 10)
                .unwrap()
                .iter()
                .any(|h| h.page_id == page.id)
        );
        s.conn
            .execute_batch("DROP TRIGGER fail_page_rename")
            .unwrap();
        s.delete_page(&page.id).unwrap();
        assert!(
            s.rename_page_if_unchanged(&page.id, "Before", "After")
                .is_err()
        );
        let record = s
            .create_page(&workspace.id, None, "Record", "", PageKind::Record)
            .unwrap();
        assert!(
            s.rename_page_if_unchanged(&record.id, "Record", "After")
                .is_err()
        );
    }
}
