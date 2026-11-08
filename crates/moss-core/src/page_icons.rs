use crate::{
    Page, PageKind, Store, now, search,
    store::{encode, invalid},
};
use regex::Regex;
use rusqlite::{OptionalExtension, Result, TransactionBehavior, params};
use std::sync::OnceLock;

// Accept one pictographic grapheme, a flag, or a keycap. The limit bounds
// malformed sequences while allowing multi-person ZWJ emoji.
fn valid_icon(icon: &str) -> bool {
    if icon.is_empty() {
        return true;
    }
    if icon.len() > 64 {
        return false;
    }
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| {
            Regex::new(concat!(
                r"\A(?:",
                r"\p{Extended_Pictographic}\x{FE0F}?[\x{1F3FB}-\x{1F3FF}]?",
                r"(?:\x{200D}\p{Extended_Pictographic}\x{FE0F}?[\x{1F3FB}-\x{1F3FF}]?)*",
                r"|\p{Regional_Indicator}{2}",
                r"|[0-9#*]\x{FE0F}?\x{20E3}",
                r"|\x{1F3F4}[\x{E0020}-\x{E007E}]+\x{E007F}",
                r")\z"
            ))
            .expect("valid icon expression")
        })
        .is_match(icon)
}

impl Store {
    /// Change one page icon while preserving newer changes to other fields.
    pub fn set_page_icon_if_unchanged(
        &mut self,
        workspace_id: &str,
        page_id: &str,
        expected_icon: &str,
        new_icon: &str,
    ) -> Result<Page> {
        if !valid_icon(new_icon) {
            return Err(invalid("icon must be one emoji or empty"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut page = search::page(&tx, page_id)?.ok_or_else(|| invalid("page missing"))?;
        if page.workspace_id != workspace_id || page.deleted_at.is_some() {
            return Err(invalid("page is not live in this workspace"));
        }
        if page.kind == PageKind::Record {
            let owner: Option<String> = tx.query_row(
                "SELECT d.page_id FROM records r JOIN databases d ON d.id=r.database_id JOIN pages p ON p.id=d.page_id WHERE r.page_id=?1 AND r.deleted_at IS NULL AND p.deleted_at IS NULL AND p.workspace_id=?2",
                params![page_id, workspace_id], |r| r.get(0),
            ).optional()?;
            if owner.as_deref() != page.parent_id.as_deref() || owner.is_none() {
                return Err(invalid("record page is not linked to a live database"));
            }
        }
        if page.icon != expected_icon {
            return Err(invalid("icon changed in another editor"));
        }
        page.icon = new_icon.to_owned();
        page.updated_at = now();
        tx.execute(
            "UPDATE pages SET data=?1 WHERE id=?2",
            params![encode(&page)?, page.id],
        )?;
        tx.commit()?;
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Block, Database, DatabaseView, PageFont, Property, PropertyKind, ViewKind, new_id,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn emoji_validation_accepts_single_sequences_and_rejects_text() {
        for value in [
            "",
            "📄",
            "👍🏽",
            "👩🏽‍💻",
            "👨‍👩‍👧‍👦",
            "🇦🇺",
            "1️⃣",
            "❤️",
            "🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f}",
        ] {
            assert!(valid_icon(value), "expected {value:?}");
        }
        for value in [
            "hello",
            "https://example.com/icon",
            "📄📄",
            "🇦",
            "a\n",
            "😀\u{200d}a",
            "123",
            "👩\u{200d}",
        ] {
            assert!(!valid_icon(value), "unexpected {value:?}");
        }
        assert!(!valid_icon(&"👩‍".repeat(30)));
    }

    #[test]
    fn update_preserves_latest_page_data_and_rejects_conflicts() {
        let mut s = Store::in_memory().unwrap();
        let w = s.create_workspace("Workspace", "").unwrap();
        let other = s.create_workspace("Other", "").unwrap();
        let mut page = s
            .create_page(&w.id, None, "Before", "📄", PageKind::Document)
            .unwrap();
        let body = Block::paragraph(&page.id, "retained", 0);
        s.save_blocks(&page.id, std::slice::from_ref(&body))
            .unwrap();
        page.title = "Latest title".into();
        page.settings.font = PageFont::Mono;
        s.update_page(&page).unwrap();
        let updated = s
            .set_page_icon_if_unchanged(&w.id, &page.id, "📄", "👩🏽‍💻")
            .unwrap();
        assert_eq!(updated.title, "Latest title");
        assert_eq!(updated.settings.font, PageFont::Mono);
        assert_eq!(s.get_blocks(&page.id).unwrap()[0].id, body.id);
        assert!(
            s.set_page_icon_if_unchanged(&w.id, &page.id, "📄", "✅")
                .is_err()
        );
        assert!(
            s.set_page_icon_if_unchanged(&other.id, &page.id, "👩🏽‍💻", "✅")
                .is_err()
        );
        assert!(
            s.set_page_icon_if_unchanged(&w.id, &page.id, "👩🏽‍💻", "abc")
                .is_err()
        );
        assert_eq!(s.get_page(&page.id).unwrap().unwrap().icon, "👩🏽‍💻");
        assert_eq!(
            s.set_page_icon_if_unchanged(&w.id, &page.id, "👩🏽‍💻", "")
                .unwrap()
                .icon,
            ""
        );
        s.delete_page(&page.id).unwrap();
        assert!(
            s.set_page_icon_if_unchanged(&w.id, &page.id, "", "✅")
                .is_err()
        );
    }

    #[test]
    fn linked_record_page_uses_same_guard_and_rejects_trash() {
        let mut s = Store::in_memory().unwrap();
        let w = s.create_workspace("Workspace", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Tasks".into(),
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
                query: Default::default(),
                config: json!({}),
            }],
        };
        s.create_database_page(&w.id, None, "Tasks", "", db.clone())
            .unwrap();
        let record = s.create_record(&db.id, "Task", &BTreeMap::new()).unwrap();
        let page = s.ensure_record_page(&record.id).unwrap();
        let saved = s
            .set_page_icon_if_unchanged(&w.id, &page.id, "", "🚀")
            .unwrap();
        assert_eq!(saved.icon, "🚀");
        assert_eq!(s.get_record(&record.id).unwrap().unwrap().title, "Task");
        s.delete_record(&record.id).unwrap();
        assert!(
            s.set_page_icon_if_unchanged(&w.id, &page.id, "🚀", "✅")
                .is_err()
        );
    }
}
