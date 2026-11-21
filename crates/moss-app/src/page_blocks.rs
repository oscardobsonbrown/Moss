use crate::{
    block_commands::BlockCommandMode,
    state::{EditableBlock, State},
};
use moss_core::{Block, BlockKind, Page, Store};
use std::collections::BTreeMap;

pub fn load_links(
    store: &Store,
    workspace: &str,
    blocks: &[Block],
) -> Result<BTreeMap<String, Page>, Box<dyn std::error::Error>> {
    let mut links = BTreeMap::new();
    for block in blocks.iter().filter(|b| b.kind == BlockKind::ChildPage) {
        if let Some(id) = block.config["page_id"].as_str() {
            if let Some(page) = store
                .get_page(id)?
                .filter(|p| p.workspace_id == workspace && p.deleted_at.is_none())
            {
                links.insert(id.into(), page);
            }
        }
    }
    Ok(links)
}

impl State {
    pub fn refresh_block_link_results(&mut self) {
        let query = self.block_command_search.text.trim();
        let results = if query.is_empty() {
            self.pages
                .iter()
                .take(30)
                .map(|p| moss_core::SearchHit {
                    page_id: p.id.clone(),
                    workspace_id: p.workspace_id.clone(),
                    title: p.title.clone(),
                    snippet: String::new(),
                    kind: p.kind,
                })
                .collect()
        } else if query.starts_with("https://") || query.starts_with("http://") {
            Vec::new()
        } else {
            match self.store.search(query, Some(&self.workspace.id), 30) {
                Ok(results) => results,
                Err(error) => {
                    self.error = Some(error.to_string());
                    Vec::new()
                }
            }
        };
        if let Some(menu) = &mut self.block_commands {
            menu.link_results = results;
            menu.selected = 0;
        }
    }
    pub fn block_link_count(&self) -> usize {
        self.block_commands
            .as_ref()
            .map(|menu| {
                menu.link_results.len()
                    + usize::from(
                        self.block_command_search.text.starts_with("https://")
                            || self.block_command_search.text.starts_with("http://"),
                    )
            })
            .unwrap_or(0)
    }
    pub fn submit_block_link(&mut self) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        let selected = menu.selected;
        if let Some(hit) = menu.link_results.get(selected) {
            self.insert_block_link(Some(hit.page_id.clone()), None);
        } else if selected == menu.link_results.len() {
            let url = self.block_command_search.text.trim().to_owned();
            if url.starts_with("https://") || url.starts_with("http://") {
                self.insert_block_link(None, Some(url));
            }
        }
    }
    pub fn insert_peek_image(&mut self) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        let (page, id) = (menu.page_id.clone(), menu.block_id.clone());
        let Some(peek) = self
            .record_peek
            .as_mut()
            .filter(|peek| peek.page.id == page)
        else {
            return;
        };
        let Some(index) = peek.blocks.iter().position(|block| block.block.id == id) else {
            return;
        };
        let reuse = peek.blocks[index].block.kind == BlockKind::Paragraph
            && peek.blocks[index].edit.text.is_empty();
        let index = if reuse {
            index
        } else {
            let mut block = Block::paragraph(&page, "", (index + 1) as i64);
            crate::editor_drag::copy_column(&peek.blocks[index].block, &mut block);
            peek.blocks.insert(
                index + 1,
                EditableBlock {
                    edit: rune::TextEdit::new("").rich(),
                    block,
                },
            );
            index + 1
        };
        peek.blocks[index].block.kind = BlockKind::Image;
        peek.blocks[index].edit.focused = false;
        self.finish_peek_block_insert();
    }
    fn finish_peek_block_insert(&mut self) {
        self.save_peek_blocks();
        if self
            .record_peek
            .as_ref()
            .is_some_and(|peek| peek.dirty_blocks)
        {
            if let Some(menu) = &mut self.block_commands {
                menu.error = Some("Block is not saved. Retry to keep it.".into());
            }
        } else {
            self.block_commands = None;
            self.block_command_search.focused = false;
        }
    }
    fn insert_peek_link(&mut self, page: Option<String>, url: Option<String>) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        let (page_id, block_id) = (menu.page_id.clone(), menu.block_id.clone());
        let target = page
            .as_ref()
            .and_then(|id| self.store.get_page(id).ok().flatten())
            .filter(|p| p.workspace_id == self.workspace.id && p.deleted_at.is_none());
        if page.is_some() && target.is_none() {
            self.error = Some("Page is not available.".into());
            return;
        }
        let Some(peek) = self
            .record_peek
            .as_mut()
            .filter(|peek| peek.page.id == page_id)
        else {
            return;
        };
        let Some(index) = peek.blocks.iter().position(|b| b.block.id == block_id) else {
            return;
        };
        let reuse = peek.blocks[index].block.kind == BlockKind::Paragraph
            && peek.blocks[index].edit.text.is_empty();
        let index = if reuse {
            index
        } else {
            let mut block = Block::paragraph(&page_id, "", (index + 1) as i64);
            crate::editor_drag::copy_column(&peek.blocks[index].block, &mut block);
            peek.blocks.insert(
                index + 1,
                EditableBlock {
                    edit: rune::TextEdit::new("").rich(),
                    block,
                },
            );
            index + 1
        };
        let block = &mut peek.blocks[index];
        if block.block.config.is_null() {
            block.block.config = serde_json::json!({});
        }
        if !block.block.config.is_object() {
            self.error = Some("Link cannot replace this block's settings.".into());
            return;
        }
        block.edit.focused = false;
        if let Some(target) = target {
            block.block.kind = BlockKind::ChildPage;
            block.block.config["page_id"] = serde_json::json!(target.id);
            peek.page_links.insert(target.id.clone(), target);
        } else if let Some(url) = url {
            block.block.kind = BlockKind::Bookmark;
            block.block.config["url"] = serde_json::json!(url);
        } else {
            return;
        }
        self.finish_peek_block_insert();
    }
    pub fn insert_block_link(&mut self, page: Option<String>, url: Option<String>) {
        if self.block_commands.as_ref().is_some_and(|menu| menu.peek) {
            self.insert_peek_link(page, url);
            return;
        }
        let Some(menu) = &self.block_commands else {
            return;
        };
        let (page_id, block_id) = (menu.page_id.clone(), menu.block_id.clone());
        let target = page
            .as_ref()
            .and_then(|id| self.store.get_page(id).ok().flatten())
            .filter(|p| p.workspace_id == self.workspace.id && p.deleted_at.is_none());
        if page.is_some() && target.is_none() {
            self.error = Some("Page is not available.".into());
            return;
        }
        let Some(tab) = self
            .tabs
            .get_mut(self.active)
            .filter(|tab| tab.page.id == page_id)
        else {
            return;
        };
        let Some(index) = tab.blocks.iter().position(|b| b.block.id == block_id) else {
            return;
        };
        let reuse = tab.blocks[index].block.kind == BlockKind::Paragraph
            && tab.blocks[index].edit.text.is_empty();
        let index = if reuse {
            index
        } else {
            let mut block = Block::paragraph(&page_id, "", (index + 1) as i64);
            crate::editor_drag::copy_column(&tab.blocks[index].block, &mut block);
            tab.blocks.insert(
                index + 1,
                EditableBlock {
                    edit: rune::TextEdit::new("").rich(),
                    block,
                },
            );
            index + 1
        };
        let block = &mut tab.blocks[index];
        if block.block.config.is_null() {
            block.block.config = serde_json::json!({});
        }
        if !block.block.config.is_object() {
            self.error = Some("Link cannot replace this block's settings.".into());
            return;
        }
        block.edit.focused = false;
        if let Some(target) = target {
            block.block.kind = BlockKind::ChildPage;
            block.block.config["page_id"] = serde_json::json!(target.id);
            tab.page_links.insert(target.id.clone(), target);
        } else if let Some(url) = url {
            block.block.kind = BlockKind::Bookmark;
            block.block.config["url"] = serde_json::json!(url);
        } else {
            return;
        }
        self.save_blocks();
        if self.tab().is_some_and(|tab| tab.dirty_blocks) {
            if let Some(menu) = &mut self.block_commands {
                menu.error = Some("Link is not saved. Retry to keep it.".into());
            }
        } else {
            self.block_commands = None;
            self.block_command_search.focused = false;
        }
    }
    pub fn focus_first_page_block(&mut self, peek: bool) {
        let target = if peek {
            self.record_peek.as_ref().map(|p| (&p.page, &p.blocks))
        } else {
            self.tab().map(|t| (&t.page, &t.blocks))
        }
        .and_then(|(p, blocks)| {
            blocks
                .iter()
                .position(|b| {
                    !matches!(
                        b.block.kind,
                        BlockKind::Divider
                            | BlockKind::ChildPage
                            | BlockKind::Image
                            | BlockKind::Bookmark
                    )
                })
                .map(|i| {
                    (
                        p.id.clone(),
                        blocks[i].block.id.clone(),
                        blocks[i].block.kind,
                        i,
                    )
                })
        });
        if let Some((page, block, kind, index)) = target {
            if kind == BlockKind::SimpleTable {
                self.start_simple_table_cell(&page, &block, 0, 0, peek);
            } else {
                let block = if peek {
                    &mut self.record_peek.as_mut().unwrap().blocks[index]
                } else {
                    &mut self.tabs[self.active].blocks[index]
                };
                block.edit.focused = true;
                block.edit.set_caret(0);
            }
        }
    }
    pub fn insert_child_page_block(&mut self) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        if menu.error.is_some() {
            return;
        }
        let (parent, block, mode) = (menu.page_id.clone(), menu.block_id.clone(), menu.mode);
        if !self.tab().is_some_and(|t| t.page.id == parent) || !self.flush_current() {
            return;
        }
        let result = (|| -> Result<_, Box<dyn std::error::Error>> {
            let expected = self.store.get_blocks(&parent)?;
            Ok(self.store.create_child_page_block(
                &self.workspace.id,
                &parent,
                &expected,
                &block,
                mode == BlockCommandMode::InsertAfter,
            )?)
        })();
        match result {
            Ok((child, blocks)) => {
                self.track_new_child_page(&child, &blocks);
                let tab = &mut self.tabs[self.active];
                tab.saved_blocks = blocks.clone();
                tab.blocks = blocks
                    .into_iter()
                    .map(|block| {
                        let edit = crate::rich_text::load(&block);
                        EditableBlock { block, edit }
                    })
                    .collect();
                tab.page_links.insert(child.id.clone(), child.clone());
                tab.dirty_blocks = false;
                self.block_commands = None;
                self.block_command_search.focused = false;
                self.refresh_child_pages(&parent);
                self.refresh_pages();
                self.open_page(&child.id, false);
            }
            Err(error) => {
                if let Some(menu) = &mut self.block_commands {
                    menu.error = Some(format!("Page is not created: {error}"));
                }
            }
        }
    }
    pub fn refresh_page_links(&mut self) {
        for tab in &mut self.tabs {
            let blocks: Vec<_> = tab.blocks.iter().map(|b| b.block.clone()).collect();
            match load_links(&self.store, &self.workspace.id, &blocks) {
                Ok(links) => tab.page_links = links,
                Err(error) => self.error = Some(format!("Page links are not loaded: {error}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_commands::BlockCommandMode;
    use moss_core::new_id;
    use rune::{Env, Runtime};
    #[test]
    fn link_command_inserts_existing_page_and_website_without_replacing_text() {
        let path = std::env::temp_dir().join(format!("moss-link-block-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        let source = s.tab().unwrap().page.id.clone();
        let block = s.tab().unwrap().blocks[0].block.id.clone();
        let target = s
            .store
            .create_page(
                &s.workspace.id,
                None,
                "Unique destination",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        s.tabs[s.active].blocks[0].edit.focused = true;
        s.tabs[s.active].blocks[0].block.config = serde_json::json!({"import_source_path":"Notes/Example.md", "import_source_sha256":"keep"});
        s.open_block_commands(&block, "anchor", BlockCommandMode::Slash);
        s.block_command_search.replace_text("link");
        s.submit_block_command();
        assert!(s.block_commands.as_ref().unwrap().link_picker);
        s.block_command_search.replace_text("Unique destination");
        s.refresh_block_link_results();
        s.submit_block_link();
        let saved = s.store.get_blocks(&source).unwrap();
        assert_eq!(saved[0].kind, BlockKind::ChildPage);
        assert_eq!(saved[0].config["page_id"], target.id);
        assert_eq!(saved[0].config["import_source_path"], "Notes/Example.md");
        assert_eq!(saved[0].config["import_source_sha256"], "keep");
        let first = saved[0].id.clone();
        s.open_block_commands(&first, "anchor", BlockCommandMode::InsertAfter);
        s.block_command_search.replace_text("link");
        s.submit_block_command();
        s.block_command_search.replace_text("https://example.com");
        s.refresh_block_link_results();
        s.submit_block_link();
        let saved = s.store.get_blocks(&source).unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[1].kind, BlockKind::Bookmark);
        assert_eq!(saved[1].config["url"], "https://example.com");
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn slash_link_picker_opens_from_the_peek_editor() {
        use rune::{Key, KeyEvent, Mods};
        let path = std::env::temp_dir().join(format!("moss-peek-picker-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let db = s.tab().unwrap().database.clone().unwrap();
        let record = s
            .store
            .create_record(&db.id, "Record", &Default::default())
            .unwrap();
        let target = s
            .store
            .create_page(
                &s.workspace.id,
                None,
                "Destination note",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        s.open_database_record(&db.id, &record.id, "center");
        let id = s.record_peek.as_ref().unwrap().blocks[0].block.id.clone();
        let mut rt = Runtime::new(s, crate::ui::view, (1100., 800.), Env::default());
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        let (x, y) = rt.find(&format!("peek-block-{id}")).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 60.);
        let press = |rt: &mut Runtime<State>, key| {
            rt.key(KeyEvent {
                key,
                mods: Mods::default(),
            });
            rt.frame(1. / 60.);
        };
        press(&mut rt, Key::Text("/".into()));
        assert!(
            rt.state
                .block_commands
                .as_ref()
                .is_some_and(|menu| menu.peek)
        );
        press(&mut rt, Key::Text("link".into()));
        press(&mut rt, Key::Enter);
        assert!(
            rt.state
                .block_commands
                .as_ref()
                .is_some_and(|menu| menu.link_picker)
        );
        press(&mut rt, Key::Text("Destination note".into()));
        press(&mut rt, Key::Enter);
        assert!(rt.state.block_commands.is_none());
        assert_eq!(
            rt.state.record_peek.as_ref().unwrap().blocks[0]
                .block
                .config["page_id"],
            target.id
        );
        assert!(rt.find(&format!("peek-page-link-{id}")).is_some());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn peek_link_and_image_commands_preserve_and_reopen_record_content() {
        let path = std::env::temp_dir().join(format!("moss-peek-link-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let db = s.tab().unwrap().database.clone().unwrap();
        let record = s
            .store
            .create_record(&db.id, "Record", &Default::default())
            .unwrap();
        let target = s
            .store
            .create_page(
                &s.workspace.id,
                None,
                "Target note",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        s.open_database_record(&db.id, &record.id, "side");
        let peek = s.record_peek.as_mut().unwrap();
        let page = peek.page.id.clone();
        let id = peek.blocks[0].block.id.clone();
        peek.blocks[0].block.config = serde_json::json!({"import_source_path":"record.md"});
        peek.blocks[0].edit.focused = true;
        s.open_peek_block_commands(&id, "peek-anchor", BlockCommandMode::Slash);
        s.block_command_search.replace_text("link");
        s.submit_block_command();
        s.block_command_search.replace_text("Target note");
        s.refresh_block_link_results();
        s.submit_block_link();
        assert_eq!(
            s.store.get_blocks(&page).unwrap()[0].config["import_source_path"],
            "record.md"
        );
        assert_eq!(
            s.record_peek.as_ref().unwrap().blocks[0].block.config["page_id"],
            target.id
        );
        s.open_peek_block_commands(&id, "peek-anchor", BlockCommandMode::InsertAfter);
        s.block_command_search.replace_text("image");
        s.submit_block_command();
        assert_eq!(s.store.get_blocks(&page).unwrap()[1].kind, BlockKind::Image);
        assert!(s.close_peek());
        s.open_database_record(&db.id, &record.id, "side");
        let peek = s.record_peek.as_ref().unwrap();
        assert_eq!(peek.blocks.len(), 2);
        assert_eq!(peek.page_links[&target.id].title, "Target note");
        let mut rt = Runtime::new(s, crate::ui::view, (1100., 800.), Env::default());
        rt.frame(0.);
        assert!(rt.find(&format!("peek-page-link-{id}")).is_some());
        assert!(
            rt.find(&format!(
                "image-upload-{}",
                rt.state.record_peek.as_ref().unwrap().blocks[1].block.id
            ))
            .is_some()
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn slash_page_opens_child_and_renders_one_link_at_the_original_position() {
        let path = std::env::temp_dir().join(format!("moss-page-block-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        let parent = s.tab().unwrap().page.id.clone();
        let block = s.tab().unwrap().blocks[0].block.id.clone();
        s.tabs[s.active].blocks[0].edit.focused = true;
        s.open_block_commands(
            &block,
            &format!("block-input-{block}"),
            BlockCommandMode::Slash,
        );
        s.block_command_search.replace_text("PAGE");
        s.submit_block_command();
        let child = s.tab().unwrap().page.id.clone();
        assert_ne!(child, parent);
        assert_eq!(
            s.tab().unwrap().page.parent_id.as_deref(),
            Some(parent.as_str())
        );
        s.tabs[s.active].title.replace_text("Project notes");
        s.save_title();
        s.open_page(&parent, false);
        assert_eq!(
            s.tab()
                .unwrap()
                .blocks
                .iter()
                .filter(|b| b.block.kind == BlockKind::ChildPage)
                .count(),
            1
        );
        assert_eq!(s.tab().unwrap().page_links[&child].title, "Project notes");
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (1100., 800.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let links = rt
            .elements()
            .iter()
            .filter(|e| {
                e.key
                    .as_deref()
                    .is_some_and(|k| k.starts_with("page-link-"))
            })
            .count();
        assert_eq!(links, 1);
        assert!(
            !rt.elements()
                .iter()
                .any(|e| e.key.as_deref() == Some(&format!("child-page-{child}")))
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
