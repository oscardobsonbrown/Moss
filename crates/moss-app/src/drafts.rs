use crate::state::{NewItemDraft, State};
use moss_core::{Page, Record};

impl State {
    pub(crate) fn track_new_record(&mut self, record: &Record) {
        self.new_item_drafts.insert(
            record.id.clone(),
            NewItemDraft {
                record: Some(record.clone()),
                page: None,
                blocks: Vec::new(),
                created_link: None,
            },
        );
    }

    pub(crate) fn track_new_page(&mut self, page: &Page) {
        self.new_item_drafts.insert(
            page.id.clone(),
            NewItemDraft {
                record: None,
                page: Some(page.clone()),
                blocks: Vec::new(),
                created_link: None,
            },
        );
    }

    pub(crate) fn track_new_child_page(&mut self, page: &Page, blocks: &[moss_core::Block]) {
        if let Some(link) = blocks.iter().find(|block| {
            block.kind == moss_core::BlockKind::ChildPage
                && block.config.get("page_id").and_then(|id| id.as_str()) == Some(&page.id)
        }) {
            self.track_new_page(page);
            if let Some(draft) = self.new_item_drafts.get_mut(&page.id) {
                draft.created_link = Some(link.clone());
            }
        }
    }

    pub(crate) fn attach_draft_page(&mut self, before: &Record, record: &Record, page: &Page) {
        if let Some(draft) = self.new_item_drafts.get_mut(&record.id) {
            if draft.page.is_none() && draft.record.as_ref() == Some(before) {
                draft.record = Some(record.clone());
                draft.page = Some(page.clone());
            }
        }
    }

    pub(crate) fn keep_draft_for_page(&mut self, page_id: &str) {
        if let Some(id) = self
            .new_item_drafts
            .iter()
            .find(|(_, draft)| draft.page.as_ref().is_some_and(|page| page.id == page_id))
            .map(|(id, _)| id.clone())
        {
            self.new_item_drafts.remove(&id);
            self.pending_empty_items.retain(|pending| pending != &id);
        }
    }

    pub(crate) fn discard_draft(&mut self, id: &str) -> bool {
        let Some(draft) = self.new_item_drafts.get(id) else {
            return true;
        };
        if draft.created_link.as_ref().is_some_and(|link| {
            self.tabs
                .iter()
                .any(|tab| tab.page.id == link.page_id && tab.dirty_blocks)
        }) {
            self.error = Some("Save the parent page before closing this draft.".into());
            return false;
        }
        match self.store.discard_untouched_item(
            draft.record.as_ref(),
            draft.page.as_ref(),
            &draft.blocks,
            draft.created_link.as_ref(),
        ) {
            Ok(discarded) => {
                if discarded {
                    let created_link = draft.created_link.clone();
                    let page_id = draft.page.as_ref().map(|page| page.id.clone());
                    let database_id = draft
                        .record
                        .as_ref()
                        .map(|record| record.database_id.clone());
                    for tab in &mut self.tabs {
                        tab.records.retain(|record| record.id != id);
                        tab.created_record_ids.retain(|created| created != id);
                        if let Some(link) = created_link
                            .as_ref()
                            .filter(|link| tab.page.id == link.page_id)
                        {
                            tab.blocks.retain(|block| block.block.id != link.id);
                            tab.page_links.remove(id);
                            if tab.blocks.is_empty() {
                                let block = moss_core::Block::paragraph(&tab.page.id, "", 0);
                                tab.blocks.push(crate::state::EditableBlock {
                                    edit: crate::rich_text::load(&block),
                                    block,
                                });
                            }
                        }
                    }
                    if let Some(database_id) = database_id {
                        crate::table_window::note_discarded(self, &database_id);
                    }
                    self.database_views.calendar.invalidate();
                    self.database_views.board.invalidate();
                    self.database_views.map.invalidate();
                    if let Some(page_id) = page_id {
                        let removed = self
                            .navigation_history
                            .iter()
                            .take(self.navigation_position + 1)
                            .filter(|id| **id == page_id)
                            .count();
                        self.navigation_history.retain(|id| id != &page_id);
                        self.navigation_position = self
                            .navigation_position
                            .saturating_sub(removed)
                            .min(self.navigation_history.len().saturating_sub(1));
                    }
                    self.refresh_pages();
                }
                self.new_item_drafts.remove(id);
                self.pending_empty_items.retain(|pending| pending != id);
                true
            }
            Err(error) => {
                self.error = Some(format!("Could not discard the empty item: {error}"));
                false
            }
        }
    }

    pub(crate) fn discard_draft_for_page(&mut self, page_id: &str) -> bool {
        let id = self
            .new_item_drafts
            .iter()
            .find(|(_, draft)| draft.page.as_ref().is_some_and(|page| page.id == page_id))
            .map(|(id, _)| id.clone());
        id.is_none_or(|id| self.discard_draft(&id))
    }

    pub(crate) fn discard_all_drafts(&mut self) {
        for id in self.new_item_drafts.keys().cloned().collect::<Vec<_>>() {
            if !self.discard_draft(&id) {
                break;
            }
        }
    }

    pub(crate) fn discard_closed_items(&mut self) -> bool {
        let pending = self.pending_empty_items.clone();
        let mut changed = false;
        for id in pending {
            let editing = self
                .editing_cell
                .as_ref()
                .is_some_and(|cell| cell.record_id == id)
                || self
                    .record_peek
                    .as_ref()
                    .is_some_and(|peek| peek.record.id == id)
                || self
                    .property_picker
                    .as_ref()
                    .is_some_and(|picker| picker.record_id == id)
                || self
                    .place_picker
                    .as_ref()
                    .is_some_and(|picker| picker.record_id == id)
                || self
                    .advanced_property
                    .relation_picker
                    .as_ref()
                    .is_some_and(|picker| picker.record_id == id)
                || self.tabs.iter().any(|tab| {
                    tab.record_context
                        .as_ref()
                        .is_some_and(|(_, record)| record.id == id)
                });
            if !editing && self.discard_draft(&id) {
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::new_id;

    #[test]
    fn new_pages_are_discarded_only_when_untouched() {
        let path = std::env::temp_dir().join(format!("moss-drafts-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.begin_page(None);
        let first = state.tab().unwrap().page.id.clone();
        state.begin_page(None);
        let second = state.tab().unwrap().page.id.clone();
        let first_tab = state
            .tabs
            .iter()
            .position(|tab| tab.page.id == first)
            .unwrap();
        state.close_tab(first_tab);
        assert!(state.store.get_page(&first).unwrap().is_none());
        state.tabs[state.active]
            .title
            .replace_text("Keep this page");
        state.save_title();
        state.close_tab(state.active);
        assert_eq!(
            state.store.get_page(&second).unwrap().unwrap().title,
            "Keep this page"
        );
        drop(state);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn inline_database_items_are_discarded_after_editing_ends() {
        let path = std::env::temp_dir().join(format!("moss-drafts-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(true, None);
        state.create_inline(Default::default(), false);
        let id = state.editing_cell.as_ref().unwrap().record_id.clone();
        assert!(state.finish_cell(false));
        assert!(state.discard_closed_items());
        assert!(state.store.get_record(&id).unwrap().is_none());
        state.create_inline(Default::default(), false);
        let id = state.editing_cell.as_ref().unwrap().record_id.clone();
        state.cell.replace_text("Keep this item");
        assert!(state.finish_cell(false));
        state.discard_closed_items();
        assert_eq!(
            state.store.get_record(&id).unwrap().unwrap().title,
            "Keep this item"
        );
        state.create_inline(Default::default(), false);
        let id = state.editing_cell.as_ref().unwrap().record_id.clone();
        state.cell.replace_text("Temporary text");
        assert!(state.save_cell());
        state.cell.replace_text("");
        assert!(state.finish_cell(false));
        state.discard_closed_items();
        assert!(
            state.store.get_record(&id).unwrap().is_some(),
            "an edited item must survive even if its title is cleared"
        );
        drop(state);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn edited_or_referenced_empty_pages_are_kept() {
        let path = std::env::temp_dir().join(format!("moss-drafts-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.begin_page(None);
        let page = state.tab().unwrap().page.id.clone();
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("A note");
        state.save_blocks();
        state.store.save_blocks(&page, &[]).unwrap();
        state.close_tab(state.active);
        assert!(state.store.get_page(&page).unwrap().is_some());
        state.begin_page(None);
        let page = state.tab().unwrap().page.id.clone();
        state
            .store
            .create_page(
                &state.workspace.id,
                Some(&page),
                "Child",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        state.close_tab(state.active);
        assert!(state.store.get_page(&page).unwrap().is_some());
        drop(state);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn empty_child_page_removes_only_its_own_link() {
        let path = std::env::temp_dir().join(format!("moss-drafts-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.begin_page(None);
        state.tabs[state.active].title.replace_text("Parent");
        state.save_title();
        let parent = state.tab().unwrap().page.id.clone();
        let target = state.tab().unwrap().blocks[0].block.id.clone();
        let expected = state.store.get_blocks(&parent).unwrap();
        let (child, links) = state
            .store
            .create_child_page_block(&state.workspace.id, &parent, &expected, &target, false)
            .unwrap();
        state.track_new_child_page(&child, &links);
        state.open_page(&child.id, false);
        state.open_page(&parent, false);
        assert!(state.store.get_page(&child.id).unwrap().is_none());
        assert!(
            state
                .store
                .get_blocks(&parent)
                .unwrap()
                .iter()
                .all(|block| block.kind != moss_core::BlockKind::ChildPage)
        );
        assert!(
            state
                .tab()
                .unwrap()
                .blocks
                .iter()
                .all(|block| block.block.kind != moss_core::BlockKind::ChildPage)
        );
        let target = state.tab().unwrap().blocks[0].block.id.clone();
        let expected = state.store.get_blocks(&parent).unwrap();
        let (child, mut links) = state
            .store
            .create_child_page_block(&state.workspace.id, &parent, &expected, &target, false)
            .unwrap();
        state.track_new_child_page(&child, &links);
        links[0].config["note"] = serde_json::json!("changed elsewhere");
        state.store.save_blocks(&parent, &links).unwrap();
        state.open_page(&child.id, false);
        state.open_page(&parent, false);
        assert!(
            state.store.get_page(&child.id).unwrap().is_some(),
            "a changed link must preserve its page"
        );
        drop(state);
        std::fs::remove_dir_all(path).unwrap();
    }
}
