use crate::state::State;
use moss_core::simple_table::SimpleTable;
use rune::TextEdit;

#[derive(Clone)]
pub struct TableCell {
    pub page_id: String,
    pub block_id: String,
    pub row: usize,
    pub column: usize,
    pub peek: bool,
}
#[derive(Clone)]
pub enum DeleteTablePart {
    Row(usize),
    Column(usize),
}
#[derive(Default)]
pub struct SimpleTableState {
    pub editing: Option<TableCell>,
    pub input: TextEdit,
    pub error: Option<String>,
    pub deleting: Option<(TableCell, DeleteTablePart)>,
}

impl State {
    fn table_block_mut(&mut self, target: &TableCell) -> Option<&mut moss_core::Block> {
        if target.peek {
            self.record_peek
                .as_mut()
                .filter(|p| p.page.id == target.page_id)?
                .blocks
                .iter_mut()
                .find(|b| b.block.id == target.block_id)
                .map(|b| &mut b.block)
        } else {
            self.tabs
                .get_mut(self.active)
                .filter(|p| p.page.id == target.page_id)?
                .blocks
                .iter_mut()
                .find(|b| b.block.id == target.block_id)
                .map(|b| &mut b.block)
        }
    }
    pub fn start_simple_table_cell(
        &mut self,
        page: &str,
        block: &str,
        row: usize,
        column: usize,
        peek: bool,
    ) {
        if !self.finish_simple_table() {
            return;
        }
        let target = TableCell {
            page_id: page.into(),
            block_id: block.into(),
            row,
            column,
            peek,
        };
        let value = self
            .table_block_mut(&target)
            .and_then(|b| SimpleTable::read(b).ok())
            .and_then(|t| t.rows.get(row)?.get(column).cloned());
        if let Some(value) = value {
            self.simple_table.input = TextEdit::new(&value);
            self.simple_table.input.focused = true;
            self.simple_table.editing = Some(target);
            self.simple_table.error = None;
        }
    }
    pub fn save_simple_table_draft(&mut self) -> bool {
        let Some(target) = self.simple_table.editing.clone() else {
            return true;
        };
        let value = self.simple_table.input.text.clone();
        let result = self
            .table_block_mut(&target)
            .ok_or("Table is no longer available.".to_owned())
            .and_then(|block| {
                let mut table = SimpleTable::read(block)?;
                let cell = table
                    .rows
                    .get_mut(target.row)
                    .and_then(|r| r.get_mut(target.column))
                    .ok_or("Table cell is no longer available.")?;
                *cell = value;
                table.write(block)
            });
        if let Err(error) = result {
            self.simple_table.error = Some(error);
            return false;
        }
        if target.peek {
            self.save_peek_blocks();
        } else {
            self.save_blocks();
        }
        let dirty = if target.peek {
            self.record_peek.as_ref().is_none_or(|p| p.dirty_blocks)
        } else {
            self.tab().is_none_or(|p| p.dirty_blocks)
        };
        if dirty {
            self.simple_table.error = Some("Table changes are not saved. Try again.".into());
            return false;
        }
        self.simple_table.error = None;
        true
    }
    pub fn finish_simple_table(&mut self) -> bool {
        if !self.save_simple_table_draft() {
            return false;
        }
        self.simple_table.editing = None;
        self.simple_table.input.focused = false;
        true
    }
    pub fn simple_table_step(&mut self, backwards: bool) {
        let Some(target) = self.simple_table.editing.clone() else {
            return;
        };
        let size = self
            .table_block_mut(&target)
            .and_then(|b| SimpleTable::read(b).ok())
            .map(|t| (t.rows.len(), t.rows[0].len()));
        let Some((rows, cols)) = size else {
            return;
        };
        let index = target.row * cols + target.column;
        let next = if backwards {
            index.checked_sub(1)
        } else {
            (index + 1 < rows * cols).then_some(index + 1)
        };
        if let Some(next) = next {
            self.start_simple_table_cell(
                &target.page_id,
                &target.block_id,
                next / cols,
                next % cols,
                target.peek,
            );
        } else {
            self.finish_simple_table();
        }
    }
    pub fn change_simple_table(
        &mut self,
        target: TableCell,
        f: impl FnOnce(&mut SimpleTable) -> Result<(), String>,
    ) {
        if !self.finish_simple_table() {
            return;
        }
        let result = self
            .table_block_mut(&target)
            .ok_or("Table is no longer available.".to_owned())
            .and_then(|block| {
                let mut table = SimpleTable::read(block)?;
                f(&mut table)?;
                table.write(block)
            });
        if let Err(error) = result {
            self.simple_table.error = Some(error);
            return;
        }
        if target.peek {
            self.save_peek_blocks();
        } else {
            self.save_blocks();
        }
        let dirty = if target.peek {
            self.record_peek.as_ref().is_none_or(|p| p.dirty_blocks)
        } else {
            self.tab().is_none_or(|p| p.dirty_blocks)
        };
        self.simple_table.error = dirty.then(|| "Table changes are not saved. Try again.".into());
    }
    pub fn paste_simple_table_into_paragraph(
        &mut self,
        block_id: &str,
        peek: bool,
        text: &str,
    ) -> bool {
        let Ok(table) = SimpleTable::from_markdown(text) else {
            return false;
        };
        let target = if peek {
            self.record_peek.as_ref().and_then(|p| {
                p.blocks
                    .iter()
                    .find(|b| b.block.id == block_id)
                    .map(|b| (p.page.id.clone(), b.block.kind, b.edit.text.is_empty()))
            })
        } else {
            self.tab().and_then(|t| {
                t.blocks
                    .iter()
                    .find(|b| b.block.id == block_id)
                    .map(|b| (t.page.id.clone(), b.block.kind, b.edit.text.is_empty()))
            })
        };
        let Some((page_id, moss_core::BlockKind::Paragraph, true)) = target else {
            return false;
        };
        if !self.finish_simple_table() {
            return false;
        }
        let target = TableCell {
            page_id,
            block_id: block_id.into(),
            row: 0,
            column: 0,
            peek,
        };
        let Some(block) = self.table_block_mut(&target) else {
            return false;
        };
        let mut converted = block.clone();
        converted.kind = moss_core::BlockKind::SimpleTable;
        if let Err(error) = table.write(&mut converted) {
            self.simple_table.error = Some(error);
            return false;
        }
        *block = converted;
        if peek {
            self.save_peek_blocks();
        } else {
            self.save_blocks();
        }
        let dirty = if peek {
            self.record_peek.as_ref().is_none_or(|p| p.dirty_blocks)
        } else {
            self.tab().is_none_or(|p| p.dirty_blocks)
        };
        if dirty {
            self.simple_table.error = Some("Table changes are not saved. Try again.".into());
        }
        true
    }
    pub fn paste_simple_table(&mut self, text: &str) -> bool {
        let Ok(table) = SimpleTable::from_markdown(text) else {
            return false;
        };
        let Some(target) = self.simple_table.editing.clone() else {
            return false;
        };
        if !self.finish_simple_table() {
            return true;
        }
        self.change_simple_table(target, |current| {
            *current = table;
            Ok(())
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moss_core::{BlockKind, new_id};

    #[test]
    fn cells_persist_tab_and_markdown_paste() {
        let path = std::env::temp_dir().join(format!("moss-simple-table-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        let page = s.tab().unwrap().page.id.clone();
        let block = &mut s.tabs[s.active].blocks[0].block;
        block.kind = BlockKind::SimpleTable;
        let id = block.id.clone();
        s.save_blocks();
        s.start_simple_table_cell(&page, &id, 0, 0, false);
        s.simple_table.input.replace_text("first");
        assert!(s.save_simple_table_draft());
        s.simple_table_step(false);
        assert_eq!(s.simple_table.editing.as_ref().unwrap().column, 1);
        s.simple_table.input.replace_text("second");
        assert!(s.finish_simple_table());
        let table = SimpleTable::read(&s.store.get_blocks(&page).unwrap()[0]).unwrap();
        assert_eq!(
            (&table.rows[0][0], &table.rows[0][1]),
            (&"first".to_string(), &"second".to_string())
        );
        s.start_simple_table_cell(&page, &id, 0, 0, false);
        assert!(s.paste_simple_table("| A | B |\n| --- | --- |\n| C | D |"));
        let table = SimpleTable::read(&s.store.get_blocks(&page).unwrap()[0]).unwrap();
        assert_eq!(table.rows[1][1], "D");
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn paste_into_empty_paragraph_converts_and_preserves_other_text() {
        let path = std::env::temp_dir().join(format!("moss-simple-table-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        let page = s.tab().unwrap().page.id.clone();
        let id = s.tab().unwrap().blocks[0].block.id.clone();
        let text = "| A | B |\n| --- | --- |\n| C | D |";
        assert!(!s.paste_simple_table_into_paragraph(&id, false, "plain text"));
        s.tabs[s.active].blocks[0].edit.replace_text("existing");
        assert!(!s.paste_simple_table_into_paragraph(&id, false, text));
        s.tabs[s.active].blocks[0].edit.replace_text("");
        s.tabs[s.active].blocks[0].block.kind = BlockKind::Code;
        assert!(!s.paste_simple_table_into_paragraph(&id, false, text));
        s.tabs[s.active].blocks[0].block.kind = BlockKind::Paragraph;
        assert!(s.paste_simple_table_into_paragraph(&id, false, text));
        let table = SimpleTable::read(&s.store.get_blocks(&page).unwrap()[0]).unwrap();
        assert_eq!(table.rows[1][1], "D");
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn missing_target_keeps_draft_and_blocks_navigation() {
        let path = std::env::temp_dir().join(format!("moss-simple-table-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(false, None);
        let page = s.tab().unwrap().page.id.clone();
        let block = &mut s.tabs[s.active].blocks[0].block;
        block.kind = BlockKind::SimpleTable;
        let id = block.id.clone();
        s.start_simple_table_cell(&page, &id, 0, 0, false);
        s.simple_table.input.replace_text("unsaved");
        s.tabs[s.active].blocks.clear();
        assert!(!s.finish_simple_table());
        assert_eq!(s.simple_table.input.text, "unsaved");
        assert!(s.simple_table.editing.is_some());
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
}
