use crate::state::{EditableBlock, State};
use moss_core::{Block, BlockKind};
use rune::TextEdit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockCommandMode {
    Change,
    InsertAfter,
    Slash,
    TextType,
    TextLink { selection: (usize, usize) },
    Color { selection: Option<(usize, usize)> },
}

pub struct BlockCommands {
    pub page_id: String,
    pub block_id: String,
    pub anchor: String,
    pub mode: BlockCommandMode,
    pub selected: usize,
    pub error: Option<String>,
    pub peek: bool,
    pub link_picker: bool,
    pub link_results: Vec<moss_core::SearchHit>,
}

pub const COMMANDS: [(BlockKind, &str, &str); 15] = [
    (BlockKind::Paragraph, "Text", "paragraph"),
    (BlockKind::Heading1, "Heading 1", "h1"),
    (BlockKind::Heading2, "Heading 2", "h2"),
    (BlockKind::Heading3, "Heading 3", "h3"),
    (BlockKind::Todo, "To-do list", "todo checkbox"),
    (BlockKind::BulletedList, "Bulleted list", "bullet"),
    (BlockKind::NumberedList, "Numbered list", "number"),
    (BlockKind::Toggle, "Toggle", "fold"),
    (BlockKind::Quote, "Quote", "quotation"),
    (BlockKind::Callout, "Callout", "note"),
    (BlockKind::Code, "Code", "code block"),
    (BlockKind::ChildPage, "Page", "child subpage"),
    (BlockKind::SimpleTable, "Table", "simple table"),
    (BlockKind::Image, "Image", "photo picture"),
    (
        BlockKind::Bookmark,
        "Link to page or website",
        "link bookmark",
    ),
];
pub const DIVIDER: (BlockKind, &str, &str) = (BlockKind::Divider, "Divider", "line separator");

fn is_text_type(kind: BlockKind) -> bool {
    matches!(kind, BlockKind::Paragraph | BlockKind::Heading1 | BlockKind::Heading2
        | BlockKind::Heading3 | BlockKind::Todo | BlockKind::BulletedList
        | BlockKind::NumberedList | BlockKind::Quote
        | BlockKind::Callout | BlockKind::Code)
}

fn set_kind(block: &mut EditableBlock, kind: BlockKind) {
    block.block.checked &= block.block.kind == BlockKind::Todo && kind == BlockKind::Todo;
    block.block.kind = kind;
    block.edit.focused = true;
}

pub fn matches(query: &str) -> Vec<(BlockKind, &'static str)> {
    let query = query.trim().to_lowercase();
    COMMANDS
        .iter()
        .chain(std::iter::once(&DIVIDER))
        .filter(|(_, name, aliases)| {
            name.to_lowercase().contains(&query) || aliases.contains(&query)
        })
        .map(|(kind, name, _)| (*kind, *name))
        .collect()
}

impl State {
    pub fn apply_editor_color(
        &mut self,
        channel: crate::text_colors::Channel,
        color: Option<crate::text_colors::Palette>,
    ) {
        let Some(menu) = &self.block_commands else { return };
        let BlockCommandMode::Color { selection } = menu.mode else { return };
        if menu.error.is_some() { return; }
        let blocks = if menu.peek {
            self.record_peek.as_mut().filter(|p| p.page.id == menu.page_id).map(|p| &mut p.blocks)
        } else {
            self.tabs.get_mut(self.active).filter(|p| p.page.id == menu.page_id).map(|p| &mut p.blocks)
        };
        let Some(block) = blocks.and_then(|blocks| blocks.iter_mut().find(|b| b.block.id == menu.block_id)) else {
            return;
        };
        if let Some((start, end)) = selection {
            if block.edit.range() != (start, end) || start >= end {
                self.dismiss_block_commands();
                return;
            }
            crate::text_colors::sync(&mut block.block, &block.edit);
            block.block.text.clone_from(&block.edit.text);
            crate::text_colors::apply_inline(&mut block.block, start..end, channel, color);
        } else {
            crate::text_colors::set_block_color(&mut block.block, channel, color);
        }
        block.edit.focused = true;
        self.retry_block_command();
        if let Some(menu) = &mut self.block_commands {
            menu.error = Some("The color change is not saved. Retry to keep it.".into());
        }
    }

    pub fn open_block_commands(&mut self, block_id: &str, anchor: &str, mode: BlockCommandMode) {
        self.open_block_commands_in(block_id, anchor, mode, false);
    }
    pub fn open_peek_block_commands(
        &mut self,
        block_id: &str,
        anchor: &str,
        mode: BlockCommandMode,
    ) {
        self.open_block_commands_in(block_id, anchor, mode, true);
    }
    pub fn open_block_commands_in(
        &mut self,
        block_id: &str,
        anchor: &str,
        mode: BlockCommandMode,
        peek: bool,
    ) {
        if self.block_commands.as_ref().is_some_and(|m| m.error.is_some()) {
            return;
        }
        let Some((page_id, _, blocks, _)) = self.editor_surface(peek) else {
            return;
        };
        let Some(block) = blocks.iter().find(|b| b.block.id == block_id) else {
            return;
        };
        if mode == BlockCommandMode::Slash && (!block.edit.text.is_empty() || !block.edit.focused) {
            return;
        }
        let page_id = page_id.to_owned();
        if !peek {
            self.modal = None;
        }
        self.block_command_search = TextEdit::new("");
        self.block_command_search.focused = true;
        self.block_commands = Some(BlockCommands {
            page_id,
            block_id: block_id.into(),
            anchor: anchor.into(),
            mode,
            selected: 0,
            error: None,
            peek,
            link_picker: false,
            link_results: Vec::new(),
        });
    }
    pub fn block_command_matches(&self) -> Vec<(BlockKind, &'static str)> {
        let mut results = matches(&self.block_command_search.text);
        if let Some(menu) = &self.block_commands
            && menu.mode == BlockCommandMode::TextType
        {
            results.retain(|(kind, _)| is_text_type(*kind));
        }
        results
    }
    pub fn dismiss_block_commands(&mut self) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        if menu.error.is_some() {
            return;
        }
        if menu.link_picker && !matches!(menu.mode, BlockCommandMode::TextLink { .. }) {
            self.block_commands.as_mut().unwrap().link_picker = false;
            self.block_command_search.replace_text("link");
            return;
        }
        let (page, block, mode, peek) = (
            menu.page_id.clone(),
            menu.block_id.clone(),
            menu.mode,
            menu.peek,
        );
        if mode == BlockCommandMode::Slash {
            let literal = format!("/{}", self.block_command_search.text);
            let Some(target) = self
                .editor_blocks_mut_for_page(&page, peek)
                .and_then(|blocks| blocks.iter_mut().find(|b| b.block.id == block))
            else {
                if !peek
                    && let Some(menu) = &mut self.block_commands
                {
                    menu.error = Some("The block is no longer available.".into());
                }
                return;
            };
            target.edit.insert(&literal);
            target.edit.focused = true;
            if let Some(menu) = &mut self.block_commands {
                menu.mode = BlockCommandMode::Change;
            }
            self.save_editor_blocks(peek);
            if self.editor_blocks_dirty(peek) {
                if let Some(menu) = &mut self.block_commands {
                    menu.error = Some("Block text is not saved. Retry to keep it.".into());
                }
                return;
            }
        }
        if let Some(target) = self
            .editor_blocks_mut_for_page(&page, peek)
            .and_then(|blocks| blocks.iter_mut().find(|b| b.block.id == block))
        {
            target.edit.focused = true;
        }
        self.block_commands = None;
        self.block_command_search.focused = false;
    }
    pub fn apply_block_command(&mut self, kind: BlockKind) {
        if let Some(menu) = &self.block_commands
            && menu.mode == BlockCommandMode::TextType
        {
            if menu.error.is_some() || !is_text_type(kind) {
                return;
            }
            if menu.peek {
                let Some(peek) = self.record_peek.as_mut().filter(|p| p.page.id == menu.page_id) else {
                    return;
                };
                let Some(block) = peek.blocks.iter_mut().find(|b| b.block.id == menu.block_id) else {
                    return;
                };
                set_kind(block, kind);
                self.save_peek_blocks();
                if self.record_peek.as_ref().is_some_and(|p| p.dirty_blocks) {
                    self.block_commands.as_mut().unwrap().error = Some("Block change is not saved. Retry to keep it.".into());
                } else {
                    self.block_commands = None;
                    self.block_command_search.focused = false;
                }
                return;
            }
        }
        if kind == BlockKind::Bookmark {
            if let Some(menu) = &mut self.block_commands {
                menu.link_picker = true;
                menu.selected = 0;
            }
            self.block_command_search.replace_text("");
            self.refresh_block_link_results();
            return;
        }
        if kind == BlockKind::ChildPage {
            self.insert_child_page_block();
            return;
        }
        let Some(menu) = &self.block_commands else {
            return;
        };
        if menu.error.is_some() {
            return;
        }
        let (page_id, block_id, mode, peek) = (
            menu.page_id.clone(),
            menu.block_id.clone(),
            menu.mode,
            menu.peek,
        );
        let Some(blocks) = self.editor_blocks_mut_for_page(&page_id, peek) else {
            if let Some(menu) = &mut self.block_commands {
                menu.error = Some("The page is no longer open.".into());
            }
            return;
        };
        let Some(index) = blocks.iter().position(|block| block.block.id == block_id) else {
            if let Some(menu) = &mut self.block_commands {
                menu.error = Some("The block is no longer available.".into());
            }
            return;
        };
        if mode == BlockCommandMode::Change
            && blocks[index].block.kind == BlockKind::SimpleTable
            && kind != BlockKind::SimpleTable
        {
            if let Some(menu) = &mut self.block_commands {
                menu.error = Some(
                    "A table cannot be changed to another block type. Copy its contents first."
                        .into(),
                );
            }
            return;
        }
        let reuse_empty = blocks[index].block.kind == BlockKind::Paragraph
            && blocks[index].edit.text.is_empty();
        // A Divider, SimpleTable, or Image has no text input. Keep nonempty content visible
        // by inserting after it, even when the user chose Change.
        let insert = (mode == BlockCommandMode::InsertAfter && !reuse_empty)
            || (mode == BlockCommandMode::Change
                && matches!(
                    kind,
                    BlockKind::Divider | BlockKind::SimpleTable | BlockKind::Image
                )
                && !blocks[index].edit.text.is_empty());
        let target_index = if insert {
            blocks[index].edit.focused = false;
            let mut block = Block::paragraph(&page_id, "", (index + 1) as i64);
            crate::editor_drag::copy_column(&blocks[index].block, &mut block);
            block.kind = kind;
            let mut edit = TextEdit::new("").rich();
            edit.focused = crate::editor_commands::is_text_block(kind);
            blocks.insert(index + 1, EditableBlock { block, edit });
            index + 1
        } else {
            set_kind(&mut blocks[index], kind);
            if !crate::editor_commands::is_text_block(kind) {
                blocks[index].edit.focused = false;
            }
            index
        };
        let target_id = blocks[target_index].block.id.clone();
        if kind == BlockKind::Divider {
            blocks[target_index].edit.focused = false;
            let next = target_index + 1;
            let reuse = blocks.get(next).is_some_and(|block| {
                block.block.kind == BlockKind::Paragraph && block.edit.text.is_empty()
            });
            if !reuse {
                blocks.insert(
                    next,
                    EditableBlock {
                        block: Block::paragraph(&page_id, "", next as i64),
                        edit: TextEdit::new("").rich(),
                    },
                );
            }
            blocks[next].edit.set_caret(0);
            blocks[next].edit.focused = true;
        }
        if let Some(menu) = &mut self.block_commands {
            menu.block_id = target_id.clone();
            menu.mode = BlockCommandMode::Change;
        }
        self.save_editor_blocks(peek);
        if self.editor_blocks_dirty(peek) {
            if let Some(menu) = &mut self.block_commands {
                menu.error = Some("Block change is not saved. Retry to keep it.".into());
            }
        } else {
            self.block_commands = None;
            self.block_command_search.focused = false;
            if kind == BlockKind::SimpleTable {
                if let Some(blocks) = self.editor_blocks_mut_for_page(&page_id, peek)
                    && let Some(block) = blocks.iter_mut().find(|b| b.block.id == target_id)
                {
                    block.edit.focused = false;
                }
                self.start_simple_table_cell(&page_id, &target_id, 0, 0, peek);
            }
        }
    }
    pub fn submit_block_command(&mut self) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        if menu.error.is_some() {
            self.retry_block_command();
            return;
        }
        if menu.link_picker {
            self.submit_block_link();
            return;
        }
        if let Some((kind, _)) = self.block_command_matches().get(menu.selected).copied() {
            self.apply_block_command(kind);
        }
    }
    pub fn retry_block_command(&mut self) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        let (page_id, peek) = (menu.page_id.clone(), menu.peek);
        if self.editor_page_id(peek).as_deref() != Some(page_id.as_str()) {
            return;
        }
        self.save_editor_blocks(peek);
        if !self.editor_blocks_dirty(peek) {
            self.block_commands = None;
            self.block_command_search.focused = false;
        }
    }
    /// Close the failed command without discarding the unsaved block draft.
    pub fn keep_block_command_draft(&mut self) {
        let Some(menu) = self
            .block_commands
            .as_ref()
            .filter(|menu| menu.error.is_some())
        else {
            return;
        };
        let (page_id, block_id, peek) = (menu.page_id.clone(), menu.block_id.clone(), menu.peek);
        if let Some(blocks) = self.editor_blocks_mut_for_page(&page_id, peek) {
            if !blocks
                .iter()
                .any(|block| block.edit.focused && crate::editor_commands::is_text_block(block.block.kind))
            {
                let index = blocks
                    .iter()
                    .position(|block| {
                        block.block.id == block_id
                            && crate::editor_commands::is_text_block(block.block.kind)
                    })
                    .or_else(|| {
                        blocks
                            .iter()
                            .position(|block| crate::editor_commands::is_text_block(block.block.kind))
                    });
                if let Some(index) = index {
                    blocks[index].edit.focused = true;
                }
            }
        }
        self.block_commands = None;
        self.block_command_search.focused = false;
    }
    pub fn block_command_action(&mut self, duplicate: bool) {
        let Some(menu) = &self.block_commands else {
            return;
        };
        if menu.error.is_some() || menu.mode != BlockCommandMode::Change {
            return;
        }
        let (page_id, block_id, peek) = (menu.page_id.clone(), menu.block_id.clone(), menu.peek);
        let Some(blocks) = self.editor_blocks_mut_for_page(&page_id, peek) else {
            return;
        };
        let Some(index) = blocks.iter().position(|b| b.block.id == block_id) else {
            return;
        };
        if duplicate {
            let mut block = blocks[index].block.clone();
            block.id = moss_core::new_id();
            let mut edit = blocks[index].edit.clone();
            edit.clear_history();
            edit.focused = crate::editor_commands::is_text_block(block.kind);
            blocks.insert(index + 1, EditableBlock { block, edit });
        } else if blocks.len() > 1 {
            blocks.remove(index);
            crate::editor_drag::normalize(blocks);
            if let Some(menu) = &mut self.block_commands {
                menu.anchor = if peek {
                    "peek-actions".into()
                } else {
                    "page-toolbar".into()
                };
            }
        } else {
            blocks[index].edit.replace_text("");
            blocks[index].block.kind = BlockKind::Paragraph;
            blocks[index].block.checked = false;
            blocks[index].block.config = serde_json::Value::Null;
        }
        let focus_id = if duplicate {
            let preferred = (index + 1).min(
                self.editor_blocks_mut_for_page(&page_id, peek)
                    .map_or(0, |b| b.len().saturating_sub(1)),
            );
            self.focus_block_index(preferred, None, peek, None)
        } else {
            self.focus_after_removal(index, peek, None)
        };
        if let (Some(menu), Some(focus_id)) = (&mut self.block_commands, focus_id) {
            menu.block_id = focus_id;
        }
        self.save_editor_blocks(peek);
        if self.editor_blocks_dirty(peek) {
            if let Some(menu) = &mut self.block_commands {
                menu.error = Some("Block change is not saved. Retry to keep it.".into());
            }
        } else {
            self.block_commands = None;
            self.block_command_search.focused = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::new_id;

    fn fixture() -> (State, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("moss-block-commands-{}", new_id()));
        (State::open(path.clone(), None).unwrap(), path)
    }

    #[test]
    fn duplicate_and_delete_leave_an_editable_block_focused() {
        let (mut s, path) = fixture();
        s.new_page(false, None);
        let active = s.active;
        let id = s.tabs[active].blocks[0].block.id.clone();
        s.tabs[active].blocks[0].edit.replace_text("Keep this text");
        s.open_block_commands(&id, "test-anchor", BlockCommandMode::Change);
        s.block_command_action(true);
        assert_eq!(s.tab().unwrap().blocks.len(), 2);
        assert!(s.tab().unwrap().blocks[1].edit.focused);
        let copy_id = s.tab().unwrap().blocks[1].block.id.clone();
        s.open_block_commands(&copy_id, "test-anchor", BlockCommandMode::Change);
        s.block_command_action(false);
        assert_eq!(s.tab().unwrap().blocks.len(), 1);
        assert!(s.tab().unwrap().blocks[0].edit.focused);
        assert_eq!(s.tab().unwrap().blocks[0].edit.text, "Keep this text");
        s.open_block_commands(&id, "test-anchor", BlockCommandMode::Change);
        s.block_command_action(false);
        assert_eq!(s.tab().unwrap().blocks[0].block.kind, BlockKind::Paragraph);
        assert!(s.tab().unwrap().blocks[0].edit.focused);
        assert!(s.tab().unwrap().blocks[0].edit.text.is_empty());
        assert!(s.block_commands.is_none());
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn slash_cancel_restores_literal_and_filter_selects_real_kind() {
        let (mut s, path) = fixture();
        let tab = s.active;
        let id = s.tabs[tab].blocks[0].block.id.clone();
        s.tabs[tab].blocks[0].edit.replace_text("");
        s.tabs[tab].blocks[0].edit.focused = true;
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::Slash);
        s.block_command_search.replace_text("hello");
        s.dismiss_block_commands();
        assert_eq!(s.tab().unwrap().blocks[0].edit.text, "/hello");
        assert_eq!(
            s.store.get_blocks(&s.tab().unwrap().page.id).unwrap()[0].text,
            "/hello"
        );
        s.tabs[tab].blocks[0].edit.replace_text("");
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::Slash);
        s.block_command_search.replace_text("heading 2");
        s.submit_block_command();
        assert_eq!(s.tab().unwrap().blocks[0].block.kind, BlockKind::Heading2);
        assert_eq!(s.tab().unwrap().blocks[0].edit.text, "");
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn insert_does_not_split_source_and_failed_save_keeps_draft() {
        let (mut s, path) = fixture();
        let tab = s.active;
        let id = s.tabs[tab].blocks[0].block.id.clone();
        s.tabs[tab].blocks[0].edit.replace_text("Keep this text");
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::InsertAfter);
        s.apply_block_command(BlockKind::Todo);
        assert_eq!(s.tab().unwrap().blocks[0].edit.text, "Keep this text");
        assert_eq!(s.tab().unwrap().blocks[1].block.kind, BlockKind::Todo);
        let page = s.tab().unwrap().page.id.clone();
        s.store.delete_page(&page).unwrap();
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::InsertAfter);
        s.apply_block_command(BlockKind::Heading1);
        assert!(s.block_commands.as_ref().unwrap().error.is_some());
        assert!(s.tab().unwrap().dirty_blocks);
        let count = s.tab().unwrap().blocks.len();
        s.retry_block_command();
        assert_eq!(s.tab().unwrap().blocks.len(), count);
        assert!(!s.flush_current());
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn plus_on_empty_paragraph_reuses_the_writing_line() {
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        s.tabs[i].blocks[0].edit.replace_text("");
        let count = s.tabs[i].blocks.len();
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::InsertAfter);
        s.apply_block_command(BlockKind::Heading1);
        assert_eq!(s.tab().unwrap().blocks.len(), count);
        assert_eq!(s.tab().unwrap().blocks[0].block.id, id);
        assert_eq!(s.tab().unwrap().blocks[0].block.kind, BlockKind::Heading1);
        assert_eq!(
            s.store.get_blocks(&s.tab().unwrap().page.id).unwrap()[0].kind,
            BlockKind::Heading1
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn changing_checked_todo_clears_check_except_when_remaining_todo() {
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        s.tabs[i].blocks[0].block.kind = BlockKind::Todo;
        s.tabs[i].blocks[0].block.checked = true;
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::Change);
        s.apply_block_command(BlockKind::Todo);
        assert!(s.tab().unwrap().blocks[0].block.checked);
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::Change);
        s.apply_block_command(BlockKind::Paragraph);
        assert!(!s.tab().unwrap().blocks[0].block.checked);
        assert!(!s.store.get_blocks(&s.tab().unwrap().page.id).unwrap()[0].checked);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn changing_nonempty_block_to_divider_keeps_source_visible() {
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        s.tabs[i].blocks[0].edit.replace_text("Keep this visible");
        s.tabs[i].blocks[0].edit.set_selection(5, 9);
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::Change);
        s.apply_block_command(BlockKind::Divider);
        let tab = s.tab().unwrap();
        assert_eq!(tab.blocks[0].block.id, id);
        assert_ne!(tab.blocks[0].block.kind, BlockKind::Divider);
        assert_eq!(tab.blocks[0].edit.text, "Keep this visible");
        assert_eq!(tab.blocks[0].edit.range(), (5, 9));
        assert_eq!(tab.blocks[1].block.kind, BlockKind::Divider);
        assert_eq!(tab.blocks[2].block.kind, BlockKind::Paragraph);
        assert!(tab.blocks[2].edit.focused);
        let saved = s.store.get_blocks(&tab.page.id).unwrap();
        assert_eq!(saved[0].text, "Keep this visible");
        assert_eq!(saved[1].kind, BlockKind::Divider);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn keep_failed_draft_closes_menu_without_discarding_content() {
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        let page = s.tabs[i].page.id.clone();
        s.tabs[i].blocks[0]
            .edit
            .replace_text("Keep draft for recovery");
        s.store.delete_page(&page).unwrap();
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::Change);
        s.apply_block_command(BlockKind::Heading1);
        assert!(s.block_commands.as_ref().unwrap().error.is_some());
        assert!(s.tab().unwrap().dirty_blocks);
        assert!(s.error.is_some());
        s.keep_block_command_draft();
        assert!(s.block_commands.is_none());
        assert!(!s.block_command_search.focused);
        assert!(s.tab().unwrap().dirty_blocks);
        assert!(s.error.is_some());
        assert_eq!(
            s.tab().unwrap().blocks[0].edit.text,
            "Keep draft for recovery"
        );
        assert_eq!(s.tab().unwrap().blocks[0].block.kind, BlockKind::Heading1);
        assert!(s.tab().unwrap().blocks[0].edit.focused);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_command_offers_a_reachable_keep_draft_button() {
        use rune::{Env, Runtime};
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        let page = s.tabs[i].page.id.clone();
        s.tabs[i].blocks[0].edit.replace_text("Recover this");
        s.store.delete_page(&page).unwrap();
        s.open_block_commands(&id, "page-toolbar", BlockCommandMode::Change);
        s.apply_block_command(BlockKind::Heading1);
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (850., 600.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        for _ in 0..20 {
            rt.frame(1. / 120.);
        }
        let (x, y) = rt
            .find("block-command-keep-draft")
            .expect("keep draft control must be visible");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
        assert!(rt.state.block_commands.is_none());
        assert!(rt.state.tab().unwrap().dirty_blocks);
        assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "Recover this");
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn nonempty_insert_keeps_source_text_and_selection() {
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        s.tabs[i].blocks[0].edit.replace_text("Keep selected text");
        s.tabs[i].blocks[0].edit.set_selection(5, 13);
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::InsertAfter);
        s.apply_block_command(BlockKind::Heading2);
        assert_eq!(s.tab().unwrap().blocks[0].edit.text, "Keep selected text");
        assert_eq!(s.tab().unwrap().blocks[0].edit.range(), (5, 13));
        assert_eq!(s.tab().unwrap().blocks[1].block.kind, BlockKind::Heading2);
        assert!(s.tab().unwrap().blocks[1].edit.focused);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn divider_reuses_empty_following_paragraph_and_saves_it() {
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        s.tabs[i].blocks[0].edit.replace_text("Source");
        let mut following = Block::paragraph(&s.tabs[i].page.id, "", 1);
        following.id = new_id();
        let following_id = following.id.clone();
        s.tabs[i].blocks.insert(
            1,
            EditableBlock {
                block: following,
                edit: TextEdit::new("").rich(),
            },
        );
        let count = s.tabs[i].blocks.len();
        s.open_block_commands(&id, "block-anchor", BlockCommandMode::InsertAfter);
        s.apply_block_command(BlockKind::Divider);
        let tab = s.tab().unwrap();
        assert_eq!(tab.blocks.len(), count + 1);
        assert_eq!(tab.blocks[1].block.kind, BlockKind::Divider);
        assert_eq!(tab.blocks[2].block.id, following_id);
        assert!(tab.blocks[2].edit.focused);
        assert!(!tab.blocks[1].edit.focused);
        let saved = s.store.get_blocks(&tab.page.id).unwrap();
        assert_eq!(saved[1].kind, BlockKind::Divider);
        assert_eq!(saved[2].id, following_id);
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn search_keyboard_applies_filtered_command_and_escape_restores_text() {
        use rune::{Env, Key, KeyEvent, Mods, Runtime};
        let (mut s, path) = fixture();
        let id = s.tab().unwrap().blocks[0].block.id.clone();
        s.tabs[s.active].blocks[0].edit.replace_text("");
        s.tabs[s.active].blocks[0].edit.focused = true;
        s.open_block_commands(&id, &format!("block-input-{id}"), BlockCommandMode::Slash);
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (850., 600.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        rt.key(KeyEvent {
            key: Key::Text("heading".into()),
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        assert_eq!(rt.state.block_command_search.text, "heading");
        assert!(rt.find("block-kind-Heading 2").is_some());
        rt.key(KeyEvent {
            key: Key::Down,
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        rt.key(KeyEvent {
            key: Key::Enter,
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        assert_eq!(
            rt.state.tab().unwrap().blocks[0].block.kind,
            BlockKind::Heading2
        );
        assert!(rt.state.block_commands.is_none());
        rt.key(KeyEvent {
            key: Key::Text("Title".into()),
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "Title");
        rt.state.tabs[rt.state.active].blocks[0]
            .edit
            .replace_text("");
        rt.state
            .open_block_commands(&id, &format!("block-input-{id}"), BlockCommandMode::Slash);
        rt.invalidate();
        rt.frame(0.);
        rt.key(KeyEvent {
            key: Key::Text("words".into()),
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "/words");
        assert!(rt.state.block_commands.is_none());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn change_from_grip_returns_keyboard_to_the_block() {
        use rune::{Env, Key, KeyEvent, Mods, Runtime};
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        s.tabs[i].blocks[0].edit.replace_text("");
        s.tabs[i].blocks[0].edit.focused = true;
        s.open_block_commands(&id, "block-menu-0", BlockCommandMode::Change);
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (850., 600.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        rt.key(KeyEvent {
            key: Key::Text("heading 1".into()),
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        rt.key(KeyEvent {
            key: Key::Enter,
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        rt.key(KeyEvent {
            key: Key::Text("Title".into()),
            mods: Mods::default(),
        });
        rt.frame(1. / 120.);
        assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "Title");
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn divider_command_leaves_a_writable_paragraph_after_it() {
        use rune::{Env, Key, KeyEvent, Mods, Runtime};
        let (mut s, path) = fixture();
        let i = s.active;
        let id = s.tabs[i].blocks[0].block.id.clone();
        s.tabs[i].blocks[0].edit.replace_text("Source text");
        s.tabs[i].blocks[0].edit.focused = true;
        s.open_block_commands(
            &id,
            &format!("block-input-{id}"),
            BlockCommandMode::InsertAfter,
        );
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (850., 600.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        for key in [
            Key::Text("divider".into()),
            Key::Enter,
            Key::Text("After divider".into()),
        ] {
            rt.key(KeyEvent {
                key,
                mods: Mods::default(),
            });
            rt.frame(1. / 120.);
        }
        let tab = rt.state.tab().unwrap();
        assert_eq!(tab.blocks[1].block.kind, BlockKind::Divider);
        assert_eq!(tab.blocks[2].block.kind, BlockKind::Paragraph);
        assert_eq!(tab.blocks[2].edit.text, "After divider");
        let saved = rt.state.store.get_blocks(&tab.page.id).unwrap();
        assert_eq!(saved[1].kind, BlockKind::Divider);
        assert_eq!(saved[2].text, "After divider");
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
