use crate::state::{EditableBlock, State};
use moss_core::{Block, history::HistoryEntry};
use serde_json::Value;

#[derive(Default)]
pub struct HistoryState {
    pub open: bool,
    pub page_id: Option<String>,
    pub entity_id: Option<String>,
    pub property_id: Option<String>,
    pub entries: Vec<HistoryEntry>,
    pub selected: Option<HistoryEntry>,
    pub expected_block: Option<Block>,
    pub expected_value: Option<Value>,
    pub error: Option<String>,
    pub has_more: bool,
    pub live: bool,
    pub next_refresh: f64,
}

impl State {
    pub fn open_history(&mut self, page_id: &str) {
        if !self.comments.input.text.trim().is_empty() {
            self.comments.error = Some("Finish the current comment first.".into());
            return;
        }
        self.comments.open = false;
        self.history = HistoryState {
            open: true,
            page_id: Some(page_id.to_owned()),
            ..Default::default()
        };
        self.load_more_history();
    }

    pub fn open_item_history(&mut self, page_id: &str, entity_id: &str, property_id: Option<&str>) {
        if !self.comments.input.text.trim().is_empty() {
            self.comments.error = Some("Finish the current comment first.".into());
            return;
        }
        self.open_history(page_id);
        if !self.history.open || self.history.page_id.as_deref() != Some(page_id) {
            return;
        }
        self.history.entity_id = Some(entity_id.to_owned());
        self.history.property_id = property_id.map(str::to_owned);
        self.history.entries.clear();
        self.load_more_history();
    }

    pub fn load_more_history(&mut self) {
        let Some(page_id) = self.history.page_id.as_deref() else {
            return;
        };
        let before = self.history.entries.last().map(|entry| entry.sequence);
        let result = if let Some(entity_id) = self.history.entity_id.as_deref() {
            self.store.item_history(
                &self.workspace.id,
                page_id,
                entity_id,
                self.history.property_id.as_deref(),
                before,
                50,
            )
        } else {
            self.store
                .page_history(&self.workspace.id, page_id, None, before, 50)
        };
        match result {
            Ok(entries) => {
                self.history.has_more = entries.len() == 50;
                self.history.entries.extend(entries);
                self.history.error = None;
            }
            Err(error) => self.history.error = Some(format!("Could not load history: {error}")),
        }
    }

    pub fn set_history_live(&mut self, live: bool) {
        self.history.live = live;
        self.history.next_refresh = 0.;
        self.history.entries.clear();
        self.load_more_history();
    }

    pub fn select_history(&mut self, sequence: i64) {
        let Some(entry) = self
            .history
            .entries
            .iter()
            .find(|entry| entry.sequence == sequence)
            .cloned()
        else {
            return;
        };
        self.history.expected_block = None;
        self.history.expected_value = None;
        let result = match entry.entity_kind.as_str() {
            "block" => self.store.get_blocks(&entry.page_id).map(|blocks| {
                self.history.expected_block =
                    blocks.into_iter().find(|block| block.id == entry.entity_id);
            }),
            "record" => self.store.get_record(&entry.entity_id).map(|record| {
                self.history.expected_value = record.and_then(|record| {
                    entry
                        .property_id
                        .as_ref()
                        .map(|id| record.values.get(id).cloned().unwrap_or(Value::Null))
                });
            }),
            _ => Ok(()),
        };
        match result {
            Ok(()) => {
                self.history.selected = Some(entry);
                self.history.error = None;
            }
            Err(error) => self.history.error = Some(format!("Could not load this change: {error}")),
        }
    }

    pub fn restore_history(&mut self) {
        let Some(entry) = self.history.selected.clone() else {
            return;
        };
        let busy = self.tabs.iter().any(|tab| {
            tab.page.id == entry.page_id
                && (tab.dirty_blocks
                    || tab.dirty_title
                    || tab.title.preedit.is_some()
                    || tab.blocks.iter().any(|block| block.edit.preedit.is_some()))
        }) || self.record_peek.as_ref().is_some_and(|peek| {
            peek.page.id == entry.page_id
                && (peek.dirty_blocks
                    || peek.dirty_title
                    || peek.title.preedit.is_some()
                    || peek.blocks.iter().any(|block| block.edit.preedit.is_some()))
        }) || (entry.entity_kind == "record"
            && (self.tabs.iter().any(|tab| {
                tab.record_context
                    .as_ref()
                    .is_some_and(|(_, record)| record.id == entry.entity_id)
                    && (tab.dirty_title || tab.title.preedit.is_some())
            }) || self.record_peek.as_ref().is_some_and(|peek| {
                peek.record.id == entry.entity_id
                    && (peek.dirty_title || peek.title.preedit.is_some())
            })))
            || self.editing_cell.is_some();
        if busy {
            self.history.error = Some("Finish the current edit before restoring.".into());
            return;
        }
        let result = match entry.entity_kind.as_str() {
            "block" => self
                .store
                .restore_history_block(
                    &self.workspace.id,
                    &entry.page_id,
                    entry.sequence,
                    self.history.expected_block.as_ref(),
                )
                .map(|blocks| {
                    for tab in self
                        .tabs
                        .iter_mut()
                        .filter(|tab| tab.page.id == entry.page_id)
                    {
                        tab.saved_blocks = blocks.clone();
                        let mut old = std::mem::take(&mut tab.blocks);
                        tab.blocks = blocks
                            .iter()
                            .cloned()
                            .map(|block| {
                                if let Some(index) = old.iter().position(|item| item.block == block)
                                {
                                    old.remove(index)
                                } else {
                                    let edit = crate::rich_text::load(&block);
                                    EditableBlock { block, edit }
                                }
                            })
                            .collect();
                        if tab.blocks.is_empty() {
                            let block = Block::paragraph(&entry.page_id, "", 0);
                            let edit = crate::rich_text::load(&block);
                            tab.blocks.push(EditableBlock { block, edit });
                        }
                    }
                    if let Some(peek) = self
                        .record_peek
                        .as_mut()
                        .filter(|peek| peek.page.id == entry.page_id)
                    {
                        peek.saved_blocks = blocks.clone();
                        let mut old = std::mem::take(&mut peek.blocks);
                        peek.blocks = blocks
                            .iter()
                            .cloned()
                            .map(|block| {
                                if let Some(index) = old.iter().position(|item| item.block == block)
                                {
                                    old.remove(index)
                                } else {
                                    let edit = crate::rich_text::load(&block);
                                    EditableBlock { block, edit }
                                }
                            })
                            .collect();
                    }
                    self.sync_gallery_preview(&entry.page_id, &blocks);
                    self.sync_record_content(&entry.page_id, &blocks);
                    self.load_comments(&entry.page_id);
                }),
            "record" => match self.history.expected_value.as_ref() {
                Some(value) => self
                    .store
                    .restore_history_record_value(
                        &self.workspace.id,
                        &entry.page_id,
                        entry.sequence,
                        value,
                    )
                    .map(|record| self.sync_record(&record)),
                None => {
                    self.history.error = Some("The record is no longer available.".into());
                    return;
                }
            },
            _ => {
                self.history.error = Some("This change cannot be restored here.".into());
                return;
            }
        };
        match result {
            Ok(()) => {
                let page_id = entry.page_id;
                if let Some(entity_id) = self.history.entity_id.clone() {
                    let property_id = self.history.property_id.clone();
                    self.open_item_history(&page_id, &entity_id, property_id.as_deref());
                } else {
                    self.open_history(&page_id);
                }
            }
            Err(error) => {
                self.history.error = Some(format!("Could not restore this change: {error}"))
            }
        }
    }
}

pub fn poll(s: &mut State, time: f64, tx: &mut rune::Tx) -> bool {
    if !s.history.open || !s.history.live {
        return false;
    }
    tx.wake_after(std::time::Duration::from_secs(1));
    if time < s.history.next_refresh {
        return false;
    }
    s.history.next_refresh = time + 1.;
    let Some(page_id) = s.history.page_id.as_deref() else {
        return false;
    };
    let result = if let Some(entity_id) = s.history.entity_id.as_deref() {
        s.store.item_history(
            &s.workspace.id,
            page_id,
            entity_id,
            s.history.property_id.as_deref(),
            None,
            50,
        )
    } else {
        s.store
            .page_history(&s.workspace.id, page_id, None, None, 50)
    };
    match result {
        Ok(entries) if entries != s.history.entries => {
            s.history.entries = entries;
            true
        }
        Ok(_) => false,
        Err(error) => {
            let message = format!("Could not refresh history: {error}");
            let changed = s.history.error.as_deref() != Some(&message);
            s.history.error = Some(message);
            changed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::{Store, new_id};

    #[test]
    fn history_panel_opens_change_details_on_demand() {
        let path = std::env::temp_dir().join(format!("moss-history-panel-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page_id = state.tab().unwrap().page.id.clone();
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("first");
        state.save_blocks();
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("second");
        state.save_blocks();
        state.open_history(&page_id);
        let id = state.history.entries.first().unwrap().sequence;
        let mut rt =
            rune::Runtime::new(state, crate::ui::view, (1100., 800.), rune::Env::default());
        rt.frame(0.);
        rt.frame(0.02);
        assert!(rt.find("history-panel").is_some());
        let (x, y) = rt.find(&format!("history-entry-{id}")).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.find("history-restore").is_some());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn restore_updates_open_page_and_rejects_a_stale_preview() {
        let path = std::env::temp_dir().join(format!("moss-history-ui-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let page_id = state.tab().unwrap().page.id.clone();
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("first");
        state.save_blocks();
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("second");
        state.save_blocks();
        state.open_history(&page_id);
        let entry = state
            .history
            .entries
            .iter()
            .find(|entry| {
                entry
                    .before
                    .as_ref()
                    .and_then(|value| value.get("text"))
                    .and_then(Value::as_str)
                    == Some("first")
            })
            .unwrap()
            .sequence;
        state.select_history(entry);
        state.restore_history();
        assert!(state.history.error.is_none(), "{:?}", state.history.error);
        assert_eq!(state.tab().unwrap().blocks[0].edit.text, "first");
        assert_eq!(state.store.get_blocks(&page_id).unwrap()[0].text, "first");

        state.select_history(entry);
        let mut other = Store::open(path.join("workspace.sqlite3")).unwrap();
        let mut blocks = other.get_blocks(&page_id).unwrap();
        blocks[0].text = "concurrent".into();
        other.save_blocks(&page_id, &blocks).unwrap();
        state.restore_history();
        assert!(state.history.error.is_some());
        assert_eq!(other.get_blocks(&page_id).unwrap()[0].text, "concurrent");
        drop(other);
        drop(state);
        std::fs::remove_dir_all(path).unwrap();
    }
}
