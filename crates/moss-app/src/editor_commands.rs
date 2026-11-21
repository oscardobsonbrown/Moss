use crate::state::{EditableBlock, State};
use moss_core::{Block, BlockKind};
use rune::TextEdit;

impl State {
    pub fn toggle_todo(&mut self, block_id: &str, peek: bool) {
        let (page_id, checked, was_dirty) = if peek {
            let Some(page) = self.record_peek.as_mut() else {
                return;
            };
            let Some(block) = page
                .blocks
                .iter_mut()
                .find(|b| b.block.id == block_id && b.block.kind == BlockKind::Todo)
            else {
                return;
            };
            block.block.checked = !block.block.checked;
            (page.page.id.clone(), block.block.checked, page.dirty_blocks)
        } else {
            let Some(page) = self.tabs.get_mut(self.active) else {
                return;
            };
            let Some(block) = page
                .blocks
                .iter_mut()
                .find(|b| b.block.id == block_id && b.block.kind == BlockKind::Todo)
            else {
                return;
            };
            block.block.checked = !block.block.checked;
            (page.page.id.clone(), block.block.checked, page.dirty_blocks)
        };
        if was_dirty {
            if peek {
                self.save_peek_blocks()
            } else {
                self.save_blocks()
            }
            return;
        }
        if let Err(error) = self.store.set_todo_checked(&page_id, block_id, checked) {
            if peek {
                self.record_peek.as_mut().unwrap().dirty_blocks = true;
            } else {
                self.tabs[self.active].dirty_blocks = true;
            }
            self.error = Some(format!(
                "Changes are not saved. Keep this page open and try again: {error}"
            ));
            return;
        }
        for page in &mut self.tabs {
            if page.page.id == page_id && !page.dirty_blocks {
                if let Some(block) = page.blocks.iter_mut().find(|b| b.block.id == block_id) {
                    block.block.checked = checked;
                }
                if let Some(block) = page.saved_blocks.iter_mut().find(|b| b.id == block_id) {
                    block.checked = checked;
                }
            }
        }
        if let Some(page) = self
            .record_peek
            .as_mut()
            .filter(|p| p.page.id == page_id && !p.dirty_blocks)
        {
            if let Some(block) = page.blocks.iter_mut().find(|b| b.block.id == block_id) {
                block.block.checked = checked;
            }
            if let Some(block) = page.saved_blocks.iter_mut().find(|b| b.id == block_id) {
                block.checked = checked;
            }
        }
        self.keep_draft_for_page(&page_id);
    }

    pub fn editor_backspace_empty(&mut self, block_id: &str) -> bool {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let Some(index) = tab
            .blocks
            .iter()
            .position(|block| block.block.id == block_id)
        else {
            return false;
        };
        if !tab.blocks[index].edit.text.is_empty() {
            return false;
        }
        if tab.blocks[index].block.kind != BlockKind::Paragraph {
            if tab.blocks[index].block.kind == BlockKind::Divider {
                return false;
            }
            tab.blocks[index].block.kind = BlockKind::Paragraph;
            tab.blocks[index].block.checked = false;
            self.save_blocks();
            return true;
        }
        if index == 0 {
            return false;
        }
        if tab.blocks[index - 1].block.kind == BlockKind::Divider {
            tab.blocks.remove(index - 1);
            let edit = &mut tab.blocks[index - 1].edit;
            edit.focused = true;
            edit.set_caret(0);
        } else {
            tab.blocks.remove(index);
            let edit = &mut tab.blocks[index - 1].edit;
            edit.focused = true;
            edit.set_caret(edit.text.len());
        }
        crate::editor_drag::normalize(&mut tab.blocks);
        self.save_blocks();
        true
    }

    pub fn append_editor_block(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.title.focused = false;
        for block in &mut tab.blocks {
            block.edit.focused = false;
        }
        if let Some(last) = tab
            .blocks
            .last_mut()
            .filter(|block| block.block.kind == BlockKind::Paragraph && block.edit.text.is_empty())
        {
            last.edit.focused = true;
            last.edit.set_caret(0);
            return;
        }
        let mut edit = TextEdit::new("").rich();
        edit.focused = true;
        let block = Block::paragraph(&tab.page.id, "", tab.blocks.len() as i64);
        tab.blocks.push(EditableBlock { block, edit });
        self.save_blocks();
    }
    /// Handle Enter in the named editor block. Code blocks keep their own Enter behavior.
    pub fn editor_enter(&mut self, block_id: &str) -> bool {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let Some(index) = tab
            .blocks
            .iter()
            .position(|block| block.block.id == block_id)
        else {
            return false;
        };
        let block = &mut tab.blocks[index];
        if block.block.kind == BlockKind::Code || block.block.kind == BlockKind::Divider {
            return false;
        }
        if matches!(
            block.block.kind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo
        ) && block.edit.text.is_empty()
        {
            block.block.kind = BlockKind::Paragraph;
            block.block.checked = false;
            self.save_blocks();
        } else {
            self.add_block(index);
        }
        true
    }

    /// Call before a single typed space, so pastes cannot trigger a shortcut.
    pub fn editor_markdown_space(&mut self, block_id: &str) -> bool {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let Some(index) = tab
            .blocks
            .iter()
            .position(|block| block.block.id == block_id)
        else {
            return false;
        };
        let block = &mut tab.blocks[index];
        if block.block.kind != BlockKind::Paragraph {
            return false;
        }
        let (start, end) = block.edit.range();
        if start != end || end != block.edit.text.len() {
            return false;
        }
        let kind = match block.edit.text.as_str() {
            "#" => BlockKind::Heading1,
            "##" => BlockKind::Heading2,
            "###" => BlockKind::Heading3,
            "-" | "*" | "+" => BlockKind::BulletedList,
            "1." | "a." | "i." => BlockKind::NumberedList,
            "[]" | "[ ]" => BlockKind::Todo,
            ">" => BlockKind::Toggle,
            "\"" => BlockKind::Quote,
            "---" => BlockKind::Divider,
            _ => return false,
        };
        block.edit.set_selection(0, end);
        block.edit.insert("");
        block.block.kind = kind;
        block.block.checked = false;
        if kind == BlockKind::Divider {
            self.add_block(index);
        } else {
            self.save_blocks();
        }
        true
    }
    pub fn editor_markdown_divider(&mut self, block_id: &str, peek: bool) -> bool {
        let blocks = if peek {
            let Some(page) = self.record_peek.as_mut() else {
                return false;
            };
            &mut page.blocks
        } else {
            let Some(page) = self.tabs.get_mut(self.active) else {
                return false;
            };
            &mut page.blocks
        };
        let Some(index) = blocks.iter().position(|block| block.block.id == block_id) else {
            return false;
        };
        let block = &mut blocks[index];
        if block.block.kind != BlockKind::Paragraph
            || block.edit.text != "--"
            || block.edit.range() != (2, 2)
            || block.edit.preedit.is_some()
        {
            return false;
        }
        block.edit.set_selection(0, 2);
        block.edit.insert("");
        block.block.kind = BlockKind::Divider;
        if peek {
            self.add_peek_block(index)
        } else {
            self.add_block(index)
        }
        true
    }
}

#[cfg(test)]
#[path = "editor_commands_tests.rs"]
mod tests;
