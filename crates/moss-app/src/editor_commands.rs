use crate::state::{EditableBlock, State};
use moss_core::{Block, BlockKind, Page};
use rune::{Key, TextEdit, Tx, edit::MarkRange};
use std::collections::BTreeMap;

pub fn is_text_block(kind: BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::Paragraph
            | BlockKind::Heading1
            | BlockKind::Heading2
            | BlockKind::Heading3
            | BlockKind::BulletedList
            | BlockKind::NumberedList
            | BlockKind::Todo
            | BlockKind::Toggle
            | BlockKind::Quote
            | BlockKind::Callout
            | BlockKind::Code
    )
}

pub(crate) fn input_key(block_id: &str, peek: bool) -> String {
    if peek {
        format!("peek-block-{block_id}")
    } else {
        format!("block-input-{block_id}")
    }
}

fn char_column_on_line(text: &str, byte_offset: usize) -> usize {
    let offset = byte_offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    text[line_start..offset].chars().count()
}

fn byte_for_column(line: &str, col: usize) -> usize {
    line.char_indices()
        .nth(col)
        .map_or(line.len(), |(idx, _)| idx)
}

fn merge_blocks(target: &mut EditableBlock, source: &EditableBlock) -> usize {
    crate::text_colors::sync(&mut target.block, &target.edit);
    let mut source_block = source.block.clone();
    crate::text_colors::sync(&mut source_block, &source.edit);
    let join_offset = target.edit.text.len();
    if source.edit.text.is_empty() {
        target.edit.focused = true;
        target.edit.set_caret(join_offset);
        return join_offset;
    }
    let combined_text = format!("{}{}", target.edit.text, source.edit.text);
    let mut marks = target.edit.marks().to_vec();
    marks.extend(
        source
            .edit
            .marks()
            .iter()
            .map(|m| MarkRange::new(m.start + join_offset..m.end + join_offset, m.mark.clone())),
    );
    let mut paragraphs = target.edit.blocks().to_vec();
    let target_lines = target.edit.text.bytes().filter(|&b| b == b'\n').count() + 1;
    let source_blocks = source.edit.blocks();
    if source_blocks.len() > 1 {
        paragraphs.extend_from_slice(&source_blocks[1..]);
    }
    let mut soft_breaks = target.edit.soft_breaks().to_vec();
    for &sb in source.edit.soft_breaks() {
        if sb > 0 {
            soft_breaks.push(sb + (target_lines - 1) as u32);
        }
    }
    let mut merged =
        TextEdit::from_parts(&combined_text, marks, paragraphs).with_soft_breaks(soft_breaks);
    merged.set_caret(join_offset);
    merged.focused = true;
    target.edit = merged;
    target.block.text = combined_text;
    crate::text_colors::append_runs(&mut target.block, &source_block, join_offset);
    join_offset
}

impl State {
    pub(crate) fn editor_surface(
        &self,
        peek: bool,
    ) -> Option<(&str, &TextEdit, &[EditableBlock], &BTreeMap<String, Page>)> {
        if peek {
            let p = self.record_peek.as_ref()?;
            Some((&p.page.id, &p.title, &p.blocks, &p.page_links))
        } else {
            let t = self.tab()?;
            Some((&t.page.id, &t.title, &t.blocks, &t.page_links))
        }
    }

    pub(crate) fn editor_surface_mut(
        &mut self,
        peek: bool,
    ) -> Option<(
        String,
        &mut TextEdit,
        &mut Vec<EditableBlock>,
        &mut BTreeMap<String, Page>,
    )> {
        if peek {
            let p = self.record_peek.as_mut()?;
            Some((p.page.id.clone(), &mut p.title, &mut p.blocks, &mut p.page_links))
        } else {
            let t = self.tabs.get_mut(self.active)?;
            Some((t.page.id.clone(), &mut t.title, &mut t.blocks, &mut t.page_links))
        }
    }

    pub(crate) fn editor_page_id(&self, peek: bool) -> Option<String> {
        self.editor_surface(peek).map(|(id, _, _, _)| id.to_owned())
    }

    pub(crate) fn editor_blocks_mut(&mut self, peek: bool) -> Option<&mut Vec<EditableBlock>> {
        self.editor_surface_mut(peek).map(|(_, _, blocks, _)| blocks)
    }

    pub(crate) fn editor_blocks_mut_for_page(
        &mut self,
        page_id: &str,
        peek: bool,
    ) -> Option<&mut Vec<EditableBlock>> {
        self.editor_surface_mut(peek)
            .filter(|(id, _, _, _)| id == page_id)
            .map(|(_, _, blocks, _)| blocks)
    }

    pub(crate) fn editor_blocks_dirty(&self, peek: bool) -> bool {
        if peek {
            self.record_peek.as_ref().is_some_and(|p| p.dirty_blocks)
        } else {
            self.tab().is_some_and(|t| t.dirty_blocks)
        }
    }

    pub(crate) fn save_editor_blocks(&mut self, peek: bool) {
        if peek {
            self.save_peek_blocks();
        } else {
            self.save_blocks();
        }
    }

    fn select_single_block(&mut self, page_id: String, peek: bool, id: String, tx: Option<&mut Tx>) {
        self.editor_selection = Some(crate::editor_selection::Selection {
            page_id,
            peek,
            ids: vec![id.clone()],
            start: [0., 0.],
            end: [0., 0.],
            dragging: false,
        });
        if let Some(tx) = tx {
            tx.blur();
            tx.scroll_into_view(if peek {
                format!("peek-drag-block-{id}")
            } else {
                format!("block-{id}")
            });
        }
    }

    pub(crate) fn focus_editor_title(&mut self, peek: bool, at_end: bool) {
        self.editor_selection = None;
        let Some((_, title, blocks, _)) = self.editor_surface_mut(peek) else {
            return;
        };
        for block in blocks.iter_mut() {
            block.edit.focused = false;
        }
        title.set_caret(if at_end { title.text.len() } else { 0 });
        title.focused = true;
    }

    pub(crate) fn focus_block_index(
        &mut self,
        index: usize,
        caret: Option<usize>,
        peek: bool,
        tx: Option<&mut Tx>,
    ) -> Option<String> {
        let (page_id, id, is_text) = {
            let (page_id, title, blocks, _) = self.editor_surface_mut(peek)?;
            if index >= blocks.len() {
                return None;
            }
            title.focused = false;
            for block in blocks.iter_mut() {
                block.edit.focused = false;
            }
            let block = &mut blocks[index];
            let id = block.block.id.clone();
            let is_text = is_text_block(block.block.kind);
            if is_text {
                let pos = caret.unwrap_or(block.edit.text.len()).min(block.edit.text.len());
                block.edit.set_caret(pos);
                block.edit.focused = true;
            }
            (page_id, id, is_text)
        };
        if is_text {
            self.editor_selection = None;
            if let Some(tx) = tx {
                tx.scroll_into_view(input_key(&id, peek));
            }
        } else {
            self.select_single_block(page_id, peek, id.clone(), tx);
        }
        Some(id)
    }

    pub(crate) fn focus_after_removal(
        &mut self,
        removed_index: usize,
        peek: bool,
        tx: Option<&mut Tx>,
    ) -> Option<String> {
        let (target_index, caret_end) = {
            let (_, _, blocks, _) = self.editor_surface(peek)?;
            let clamped = removed_index.min(blocks.len());
            if let Some(prev) = (0..clamped)
                .rev()
                .find(|&i| is_text_block(blocks[i].block.kind))
            {
                (Some(prev), true)
            } else {
                (
                    (clamped..blocks.len()).find(|&i| is_text_block(blocks[i].block.kind)),
                    false,
                )
            }
        };
        let index = match target_index {
            Some(index) => index,
            None => {
                let (page_id, _, blocks, _) = self.editor_surface_mut(peek)?;
                blocks.push(EditableBlock {
                    block: Block::paragraph(&page_id, "", blocks.len() as i64),
                    edit: TextEdit::new("").rich(),
                });
                blocks.len() - 1
            }
        };
        self.focus_block_index(index, (!caret_end).then_some(0), peek, tx)
    }

    pub fn add_block_in(&mut self, after: usize, peek: bool) {
        let Some((page_id, _, blocks, _)) = self.editor_surface_mut(peek) else {
            return;
        };
        if blocks.is_empty() {
            blocks.push(EditableBlock {
                block: Block::paragraph(&page_id, "", 0),
                edit: TextEdit::new("").rich(),
            });
        }
        let after = after.min(blocks.len() - 1);
        let edit = &mut blocks[after].edit;
        let (start, end) = edit.range();
        let mut suffix = edit.clone();
        suffix.set_selection(0, end);
        suffix.insert("");
        suffix.clear_history();
        edit.set_selection(start, edit.text.len());
        edit.insert("");
        edit.focused = false;
        suffix.set_caret(0);
        suffix.focused = true;
        let mut block = Block::paragraph(&page_id, &suffix.text, after as i64 + 1);
        crate::text_colors::copy_suffix(&blocks[after].block, &mut block, end);
        crate::editor_drag::copy_column(&blocks[after].block, &mut block);
        if matches!(
            blocks[after].block.kind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo
        ) {
            block.kind = blocks[after].block.kind;
        }
        blocks.insert(after + 1, EditableBlock { block, edit: suffix });
        self.save_editor_blocks(peek);
    }

    pub fn toggle_todo(&mut self, block_id: &str, peek: bool) {
        let was_dirty = self.editor_blocks_dirty(peek);
        let Some((page_id, checked)) = self.editor_surface_mut(peek).and_then(|(page_id, _, blocks, _)| {
            let block = blocks
                .iter_mut()
                .find(|b| b.block.id == block_id && b.block.kind == BlockKind::Todo)?;
            block.block.checked = !block.block.checked;
            Some((page_id, block.block.checked))
        }) else {
            return;
        };
        if was_dirty {
            self.save_editor_blocks(peek);
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

    #[cfg(test)]
    pub fn editor_backspace_empty(&mut self, block_id: &str) -> bool {
        self.editor_surface(false)
            .and_then(|(_, _, blocks, _)| blocks.iter().find(|b| b.block.id == block_id))
            .is_some_and(|b| b.edit.text.is_empty())
            && self.editor_backspace_in(block_id, false, None)
    }

    pub fn editor_backspace_in(
        &mut self,
        block_id: &str,
        peek: bool,
        tx: Option<&mut Tx>,
    ) -> bool {
        let Some((page_id, _, blocks, _)) = self.editor_surface_mut(peek) else {
            return false;
        };
        let Some(index) = blocks.iter().position(|b| b.block.id == block_id) else {
            return false;
        };
        if blocks[index].edit.preedit.is_some() || blocks[index].edit.range() != (0, 0) {
            return false;
        }
        if blocks[index].block.kind != BlockKind::Paragraph {
            if blocks[index].block.kind == BlockKind::Divider
                || (blocks[index].block.kind == BlockKind::Code
                    && !blocks[index].edit.text.is_empty())
            {
                return false;
            }
            blocks[index].block.kind = BlockKind::Paragraph;
            blocks[index].block.checked = false;
            self.save_editor_blocks(peek);
            return true;
        }
        if index == 0 {
            return false;
        }
        let prev_kind = blocks[index - 1].block.kind;
        if prev_kind == BlockKind::Divider {
            blocks.remove(index - 1);
            let edit = &mut blocks[index - 1].edit;
            edit.focused = true;
            edit.set_caret(0);
            crate::editor_drag::normalize(blocks);
            self.save_editor_blocks(peek);
            return true;
        }
        if is_text_block(prev_kind) {
            let current = blocks.remove(index);
            merge_blocks(&mut blocks[index - 1], &current);
            let target_id = blocks[index - 1].block.id.clone();
            crate::editor_drag::normalize(blocks);
            self.save_editor_blocks(peek);
            if let Some(tx) = tx {
                tx.scroll_into_view(input_key(&target_id, peek));
            }
            return true;
        }
        let prev_id = blocks[index - 1].block.id.clone();
        if blocks[index].edit.text.is_empty() {
            blocks.remove(index);
            crate::editor_drag::normalize(blocks);
            self.save_editor_blocks(peek);
        } else {
            blocks[index].edit.focused = false;
        }
        self.select_single_block(page_id, peek, prev_id, tx);
        true
    }

    pub fn editor_delete_in(&mut self, block_id: &str, peek: bool, tx: Option<&mut Tx>) -> bool {
        let Some((page_id, _, blocks, _)) = self.editor_surface_mut(peek) else {
            return false;
        };
        let Some(index) = blocks.iter().position(|b| b.block.id == block_id) else {
            return false;
        };
        if blocks[index].edit.preedit.is_some()
            || blocks[index].edit.anchor != blocks[index].edit.focus
        {
            return false;
        }
        if blocks[index].edit.text.is_empty() {
            if blocks.len() == 1 {
                if !matches!(
                    blocks[index].block.kind,
                    BlockKind::Paragraph | BlockKind::Divider
                ) {
                    blocks[index].block.kind = BlockKind::Paragraph;
                    blocks[index].block.checked = false;
                    self.save_editor_blocks(peek);
                    return true;
                }
                return false;
            }
            blocks.remove(index);
            crate::editor_drag::normalize(blocks);
            self.focus_after_removal(index, peek, tx);
            self.save_editor_blocks(peek);
            return true;
        }
        if blocks[index].edit.focus != blocks[index].edit.text.len() || index + 1 >= blocks.len() {
            return false;
        }
        let next_kind = blocks[index + 1].block.kind;
        if next_kind == BlockKind::Divider {
            blocks.remove(index + 1);
            crate::editor_drag::normalize(blocks);
            self.save_editor_blocks(peek);
            return true;
        }
        if is_text_block(next_kind) {
            let next = blocks.remove(index + 1);
            merge_blocks(&mut blocks[index], &next);
            crate::editor_drag::normalize(blocks);
            self.save_editor_blocks(peek);
            return true;
        }
        let next_id = blocks[index + 1].block.id.clone();
        blocks[index].edit.focused = false;
        self.select_single_block(page_id, peek, next_id, tx);
        true
    }

    pub fn editor_escape_select_block(&mut self, block_id: &str, peek: bool, tx: &mut Tx) -> bool {
        if self.block_commands.is_some() || self.modal.is_some() {
            return false;
        }
        let Some((page_id, _, blocks, _)) = self.editor_surface_mut(peek) else {
            return false;
        };
        let Some(block) = blocks.iter_mut().find(|b| b.block.id == block_id) else {
            return false;
        };
        if block.edit.preedit.is_some() {
            return false;
        }
        block.edit.focused = false;
        self.select_single_block(page_id, peek, block_id.to_owned(), Some(tx));
        tx.instant();
        true
    }

    pub fn editor_arrow_key(
        &mut self,
        block_id: &str,
        peek: bool,
        key: Key,
        at_first_line: bool,
        at_last_line: bool,
        tx: &mut Tx,
    ) -> bool {
        let Some((_, _, blocks, _)) = self.editor_surface(peek) else {
            return false;
        };
        let Some(index) = blocks.iter().position(|b| b.block.id == block_id) else {
            return false;
        };
        let edit = &blocks[index].edit;
        if edit.preedit.is_some() || edit.anchor != edit.focus {
            return false;
        }
        let (focus, len, col, count) = (
            edit.focus,
            edit.text.len(),
            char_column_on_line(&edit.text, edit.focus),
            blocks.len(),
        );
        match key {
            Key::Left if focus == 0 => {
                if index == 0 {
                    self.focus_editor_title(peek, true);
                } else {
                    self.focus_block_index(index - 1, None, peek, Some(tx));
                }
            }
            Key::Right if focus == len && index + 1 < count => {
                self.focus_block_index(index + 1, Some(0), peek, Some(tx));
            }
            Key::Up if at_first_line => {
                if index == 0 {
                    self.focus_editor_title(peek, true);
                } else {
                    let prev = &blocks[index - 1];
                    let caret = is_text_block(prev.block.kind).then(|| {
                        let last = prev.edit.text.rfind('\n').map_or(0, |i| i + 1);
                        last + byte_for_column(&prev.edit.text[last..], col)
                    });
                    self.focus_block_index(index - 1, caret, peek, Some(tx));
                }
            }
            Key::Down if at_last_line && index + 1 < count => {
                let next = &blocks[index + 1];
                let caret = if is_text_block(next.block.kind) {
                    let end = next.edit.text.find('\n').unwrap_or(next.edit.text.len());
                    Some(byte_for_column(&next.edit.text[..end], col))
                } else {
                    Some(0)
                };
                self.focus_block_index(index + 1, caret, peek, Some(tx));
            }
            _ => return false,
        }
        tx.instant();
        true
    }

    pub fn editor_title_arrow_key(
        &mut self,
        peek: bool,
        key: Key,
        at_last_line: bool,
        tx: &mut Tx,
    ) -> bool {
        let Some((_, title, blocks, _)) = self.editor_surface(peek) else {
            return false;
        };
        if blocks.is_empty() || title.preedit.is_some() || title.anchor != title.focus {
            return false;
        }
        if !matches!(
            key,
            Key::Down if at_last_line
        ) && !(key == Key::Right && title.focus == title.text.len())
        {
            return false;
        }
        self.focus_block_index(0, Some(0), peek, Some(tx));
        tx.instant();
        true
    }

    pub fn append_editor_block(&mut self) {
        self.append_editor_block_in(false);
    }

    pub fn append_editor_block_in(&mut self, peek: bool) {
        self.editor_selection = None;
        let Some((page_id, title, blocks, _)) = self.editor_surface_mut(peek) else {
            return;
        };
        title.focused = false;
        for block in blocks.iter_mut() {
            block.edit.focused = false;
        }
        if let Some(last) = blocks
            .last_mut()
            .filter(|block| block.block.kind == BlockKind::Paragraph && block.edit.text.is_empty())
        {
            last.edit.focused = true;
            last.edit.set_caret(0);
            return;
        }
        let mut edit = TextEdit::new("").rich();
        edit.focused = true;
        let block = Block::paragraph(&page_id, "", blocks.len() as i64);
        blocks.push(EditableBlock { block, edit });
        self.save_editor_blocks(peek);
    }

    /// Handle Enter in the named editor block. Code blocks keep their own Enter behavior.
    #[cfg(test)]
    pub fn editor_enter(&mut self, block_id: &str) -> bool {
        self.editor_enter_in(block_id, false)
    }

    pub fn editor_enter_in(&mut self, block_id: &str, peek: bool) -> bool {
        let Some(blocks) = self.editor_blocks_mut(peek) else {
            return false;
        };
        let Some(index) = blocks.iter().position(|b| b.block.id == block_id) else {
            return false;
        };
        let block = &mut blocks[index];
        if matches!(block.block.kind, BlockKind::Code | BlockKind::Divider) {
            return false;
        }
        if matches!(
            block.block.kind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo
        ) && block.edit.text.is_empty()
        {
            block.block.kind = BlockKind::Paragraph;
            block.block.checked = false;
            self.save_editor_blocks(peek);
        } else {
            self.add_block_in(index, peek);
        }
        true
    }

    /// Call before a single typed space, so pastes cannot trigger a shortcut.
    #[cfg(test)]
    pub fn editor_markdown_space(&mut self, block_id: &str) -> bool {
        self.editor_markdown_space_in(block_id, false)
    }

    pub fn editor_markdown_space_in(&mut self, block_id: &str, peek: bool) -> bool {
        let Some(blocks) = self.editor_blocks_mut(peek) else {
            return false;
        };
        let Some(index) = blocks.iter().position(|b| b.block.id == block_id) else {
            return false;
        };
        let block = &mut blocks[index];
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
            self.add_block_in(index, peek);
        } else {
            self.save_editor_blocks(peek);
        }
        true
    }

    pub fn editor_markdown_divider(&mut self, block_id: &str, peek: bool) -> bool {
        let Some(blocks) = self.editor_blocks_mut(peek) else {
            return false;
        };
        let Some(index) = blocks.iter().position(|b| b.block.id == block_id) else {
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
        self.add_block_in(index, peek);
        true
    }
}

#[cfg(test)]
#[path = "editor_commands_tests.rs"]
mod tests;
