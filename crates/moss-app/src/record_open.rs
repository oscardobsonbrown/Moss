use crate::state::{EditableBlock, PeekMode, RecordPeek, State};
use moss_core::{Block, Database};
use rune::TextEdit;

impl State {
    pub(crate) fn record_database(&self, record_id: &str) -> Option<&Database> {
        self.record_peek
            .as_ref()
            .filter(|p| p.record.id == record_id)
            .map(|p| &p.database)
            .or_else(|| {
                self.tab().and_then(|t| {
                    t.record_context
                        .as_ref()
                        .filter(|(_, r)| r.id == record_id)
                        .map(|(db, _)| db)
                })
            })
            .or_else(|| self.tab().and_then(|t| t.database.as_ref()))
    }

    pub fn open_database_record(&mut self, database_id: &str, record_id: &str, open_mode: &str) {
        let result = (|| -> Result<_, Box<dyn std::error::Error>> {
            let db = self
                .store
                .get_database(database_id)?
                .ok_or("Database was not found")?;
            let owner = self
                .store
                .get_page(&db.page_id)?
                .ok_or("Database page was not found")?;
            if owner.workspace_id != self.workspace.id || owner.deleted_at.is_some() {
                return Err("Database is not available in this workspace".into());
            }
            let record = self
                .store
                .get_record(record_id)?
                .ok_or("Record was not found")?;
            if record.database_id != database_id || record.deleted_at.is_some() {
                return Err("Record is not available in this database".into());
            }
            Ok(db)
        })();
        let database = match result {
            Ok(database) => database,
            Err(error) => {
                self.error = Some(format!("Could not open the page: {error}"));
                return;
            }
        };
        if !self.finish_cell(false) || !self.close_peek() {
            return;
        }
        let full = matches!(open_mode, "full" | "full_page");
        if full && !self.flush_current() {
            return;
        }
        let result = (|| -> Result<RecordPeek, Box<dyn std::error::Error>> {
            let page = self.store.ensure_record_page(record_id)?;
            let record = self
                .store
                .get_record(record_id)?
                .ok_or("Record was not found")?;
            let mut blocks = self.store.get_blocks(&page.id)?;
            if blocks.is_empty() {
                blocks.push(Block::paragraph(&page.id, "", 0));
            }
            let mut title = TextEdit::new(&record.title);
            title.focused = true;
            Ok(RecordPeek {
                database,
                record,
                page,
                title,
                mode: if matches!(open_mode, "center" | "center_peek") {
                    PeekMode::Center
                } else {
                    PeekMode::Side
                },
                blocks: blocks
                    .into_iter()
                    .map(|block| {
                        let edit = crate::rich_text::load(&block);
                        EditableBlock { block, edit }
                    })
                    .collect(),
                dirty_title: false,
                dirty_blocks: false,
            })
        })();
        match result {
            Ok(peek) => {
                let page_id = peek.page.id.clone();
                self.sync_record(&peek.record);
                if full {
                    self.open_page(&page_id, true);
                } else {
                    self.record_peek = Some(peek);
                    self.modal = None;
                    self.record_sidebar_recent(&page_id);
                }
            }
            Err(error) => self.error = Some(format!("Could not open the page: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::new_id;
    #[test]
    fn opening_a_widget_record_keeps_the_active_source_and_uses_the_record_schema() {
        let path = std::env::temp_dir().join(format!("moss-source-open-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let target_db = s.tab().unwrap().database.clone().unwrap();
        let record = s
            .store
            .create_record(&target_db.id, "Target", &Default::default())
            .unwrap();
        s.new_page(true, None);
        let active = s.tab().unwrap().page.id.clone();
        s.open_database_record(&target_db.id, &record.id, "side");
        assert_eq!(s.tab().unwrap().page.id, active);
        assert_eq!(s.record_peek.as_ref().unwrap().database.id, target_db.id);
        let property = target_db
            .properties
            .iter()
            .find(|p| p.kind == moss_core::PropertyKind::Status)
            .unwrap();
        s.open_property_picker(&record.id, &property.id);
        assert!(s.property_picker.is_some());
        s.dismiss_property_picker();
        s.close_peek();
        let wrong = s.tab().unwrap().database.as_ref().unwrap().id.clone();
        s.open_database_record(&wrong, &record.id, "side");
        assert!(s.record_peek.is_none());
        assert!(s.error.is_some());
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
}
