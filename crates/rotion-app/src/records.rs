use crate::state::{CellTarget, EditableBlock, PeekMode, RecordPeek, State};
use rotion_core::*;
use rune::TextEdit;
use serde_json::{Value, json};
use std::collections::BTreeMap;

impl State {
    pub fn query_records(&mut self) {
        if !self.finish_cell(false) {
            return;
        }
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(db) = &tab.database else { return };
        let mut q = db
            .views
            .get(tab.view)
            .map(|v| v.query.clone())
            .unwrap_or_default();
        q.offset = tab.offset;
        q.limit = 100;
        if !self.filter.text.is_empty() {
            q.filter = Some(FilterExpr::Rule(FilterRule {
                property_id: db
                    .properties
                    .iter()
                    .find(|p| p.kind == PropertyKind::Title)
                    .map(|p| p.id.clone())
                    .unwrap_or_default(),
                op: FilterOp::Contains,
                value: json!(self.filter.text),
            }));
        }
        match self.store.query_records(&db.id, &q) {
            Ok(r) => tab.records = r,
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn add_record(&mut self) {
        self.create_peek(BTreeMap::new());
    }

    fn create_values(&self, context: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
        let mut values = BTreeMap::new();
        if let Some(db) = self.tab().and_then(|t| t.database.as_ref()) {
            for prop in &db.properties {
                if prop.kind == PropertyKind::Status {
                    if let Some(option) = prop
                        .config
                        .get("options")
                        .and_then(Value::as_array)
                        .and_then(|a| a.first())
                    {
                        if let Some(name) = option
                            .as_str()
                            .or_else(|| option.get("name").and_then(Value::as_str))
                        {
                            values.insert(prop.id.clone(), json!(name));
                        }
                    }
                }
            }
        }
        values.extend(context);
        values
    }
    fn create_item(&mut self, context: BTreeMap<String, Value>) -> Option<Record> {
        let db_id = self.tab()?.database.as_ref()?.id.clone();
        let values = self.create_values(context);
        match self.store.create_record(&db_id, "", &values) {
            Ok(record) => {
                self.tabs[self.active].records.push(record.clone());
                self.error = None;
                Some(record)
            }
            Err(e) => {
                self.error = Some(format!("Could not create the page: {e}"));
                None
            }
        }
    }
    pub fn create_inline(&mut self, values: BTreeMap<String, Value>, repeat: bool) {
        if !self.finish_cell(false) || !self.close_peek() {
            return;
        }
        if let Some(record) = self.create_item(values.clone()) {
            self.cell = TextEdit::new("");
            self.cell.focused = true;
            self.editing_cell = Some(CellTarget {
                record_id: record.id,
                property_id: None,
                repeat_values: repeat.then_some(values),
            });
            self.modal = None;
        }
    }
    pub fn create_peek(&mut self, values: BTreeMap<String, Value>) {
        if !self.finish_cell(false) || !self.close_peek() {
            return;
        }
        if let Some(record) = self.create_item(values) {
            self.open_record_id(&record.id);
        }
    }
    pub fn open_record(&mut self, index: usize) {
        if let Some(record) = self.tab().and_then(|t| t.records.get(index)) {
            let id = record.id.clone();
            self.open_record_id(&id);
        }
    }
    fn open_record_id(&mut self, id: &str) {
        if !self.finish_cell(false) || !self.close_peek() {
            return;
        }
        let mode = self
            .tab()
            .and_then(|t| t.database.as_ref().and_then(|d| d.views.get(t.view)))
            .map(
                |v| match v.config.get("open_mode").and_then(Value::as_str) {
                    Some("center") => PeekMode::Center,
                    Some("side") => PeekMode::Side,
                    _ if matches!(v.kind, ViewKind::Gallery | ViewKind::Calendar) => {
                        PeekMode::Center
                    }
                    _ => PeekMode::Side,
                },
            )
            .unwrap_or(PeekMode::Side);
        let result = (|| -> Result<RecordPeek, Box<dyn std::error::Error>> {
            let page = self.store.ensure_record_page(id)?;
            let record = self.store.get_record(id)?.ok_or("Record was not found")?;
            let mut blocks = self.store.get_blocks(&page.id)?;
            if blocks.is_empty() {
                blocks.push(Block::paragraph(&page.id, "", 0));
            }
            let mut title = TextEdit::new(&record.title);
            title.focused = true;
            Ok(RecordPeek {
                record,
                page,
                title,
                mode,
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
                self.sync_record(&peek.record);
                self.record_peek = Some(peek);
                self.modal = None;
            }
            Err(e) => self.error = Some(format!("Could not open the page: {e}")),
        }
    }
    pub(crate) fn sync_record(&mut self, record: &Record) {
        for tab in &mut self.tabs {
            if let Some(r) = tab.records.iter_mut().find(|r| r.id == record.id) {
                *r = record.clone();
            }
            if record.page_id.as_deref() == Some(&tab.page.id) {
                tab.page.title = record.title.clone();
                if tab.title.text != record.title {
                    tab.title.replace_text(&record.title);
                }
            }
        }
        if let Some(peek) = &mut self.record_peek {
            if peek.record.id == record.id {
                peek.record = record.clone();
                peek.page.title = record.title.clone();
                if peek.title.text != record.title {
                    peek.title.replace_text(&record.title);
                }
            }
        }
    }
    pub fn edit_record(&mut self, index: usize, prop: Option<String>) {
        if let Some(record) = self.tab().and_then(|t| t.records.get(index)) {
            let id = record.id.clone();
            self.edit_record_property(&id, prop);
        }
    }
    pub fn edit_record_property(&mut self, record_id: &str, prop: Option<String>) {
        if !self.finish_cell(false) {
            return;
        }
        let record = match self.store.get_record(record_id) {
            Ok(Some(r)) => r,
            Ok(None) => return,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        if let Some(id) = &prop {
            let kind = self
                .tab()
                .and_then(|t| t.database.as_ref())
                .and_then(|d| d.properties.iter().find(|p| &p.id == id))
                .map(|p| p.kind);
            if !matches!(
                kind,
                Some(
                    PropertyKind::Text
                        | PropertyKind::Number
                        | PropertyKind::Select
                        | PropertyKind::Status
                        | PropertyKind::Date
                        | PropertyKind::Checkbox
                        | PropertyKind::Url
                        | PropertyKind::Email
                        | PropertyKind::Phone
                )
            ) {
                self.error =
                    Some("This property is read-only until its editor is available.".into());
                return;
            }
            if record
                .values
                .get(id)
                .is_some_and(|v| v.is_array() || v.is_object())
            {
                self.error = Some("This value needs a structured property editor.".into());
                return;
            }
        }
        self.cell = TextEdit::new(
            prop.as_ref()
                .and_then(|p| record.values.get(p))
                .map(crate::state::display_value)
                .unwrap_or_else(|| {
                    if prop.is_none() {
                        record.title.clone()
                    } else {
                        String::new()
                    }
                }),
        );
        self.cell.focused = true;
        self.editing_cell = Some(CellTarget {
            record_id: record.id,
            property_id: prop,
            repeat_values: None,
        });
        self.error = None;
    }
    pub fn save_cell(&mut self) -> bool {
        let Some(target) = &self.editing_cell else {
            return true;
        };
        let id = target.record_id.clone();
        let property = target.property_id.clone();
        let result = (|| -> Result<Record, String> {
            if let Some(p) = property {
                let mut record = self
                    .store
                    .get_record(&id)
                    .map_err(|e| e.to_string())?
                    .ok_or("Record was not found")?;
                let kind = self
                    .tab()
                    .and_then(|t| t.database.as_ref())
                    .and_then(|d| d.properties.iter().find(|x| x.id == p))
                    .map(|p| p.kind);
                let text = self.cell.text.trim();
                let value = if text.is_empty() {
                    Value::Null
                } else {
                    match kind {
                        Some(PropertyKind::Number) => {
                            json!(text.parse::<f64>().map_err(|_| "Enter a number.")?)
                        }
                        Some(PropertyKind::Checkbox) => match text {
                            "true" => json!(true),
                            "false" => json!(false),
                            _ => return Err("Use true or false.".into()),
                        },
                        Some(PropertyKind::Date) => {
                            chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                                .map_err(|_| "Use a date in YYYY-MM-DD format.")?;
                            json!(text)
                        }
                        _ => json!(self.cell.text),
                    }
                };
                record.values.insert(p, value);
                record.updated_at = now();
                self.store.save_record(&record).map_err(|e| e.to_string())?;
                Ok(record)
            } else {
                self.store
                    .rename_record(&id, &self.cell.text)
                    .map_err(|e| e.to_string())
            }
        })();
        match result {
            Ok(record) => {
                self.sync_record(&record);
                self.error = None;
                true
            }
            Err(e) => {
                self.error = Some(format!("Changes are not saved: {e}"));
                false
            }
        }
    }
    pub fn finish_cell(&mut self, repeat: bool) -> bool {
        if self.editing_cell.is_none() {
            return true;
        }
        if !self.save_cell() {
            self.cell.focused = true;
            return false;
        }
        let values = self.editing_cell.take().and_then(|t| t.repeat_values);
        self.cell.focused = false;
        if repeat {
            if let Some(values) = values {
                self.create_inline(values, true);
            }
        }
        true
    }
    pub fn save_peek_title(&mut self) {
        let Some(peek) = &mut self.record_peek else {
            return;
        };
        peek.dirty_title = true;
        match self.store.rename_record(&peek.record.id, &peek.title.text) {
            Ok(record) => {
                peek.dirty_title = false;
                self.sync_record(&record);
            }
            Err(e) => self.error = Some(format!("Title is not saved: {e}")),
        }
    }
    pub fn save_peek_blocks(&mut self) {
        let Some(peek) = &mut self.record_peek else {
            return;
        };
        peek.dirty_blocks = true;
        let blocks: Vec<Block> = peek
            .blocks
            .iter_mut()
            .enumerate()
            .map(|(i, b)| {
                crate::rich_text::save(&mut b.block, &b.edit);
                b.block.sort_order = i as i64;
                b.block.clone()
            })
            .collect();
        match self.store.save_blocks(&peek.page.id, &blocks) {
            Ok(()) => {
                peek.dirty_blocks = false;
                for tab in &mut self.tabs {
                    if tab.page.id == peek.page.id {
                        tab.blocks = blocks
                            .iter()
                            .cloned()
                            .map(|block| {
                                let edit = crate::rich_text::load(&block);
                                EditableBlock { block, edit }
                            })
                            .collect();
                    }
                }
            }
            Err(e) => self.error = Some(format!("Page contents are not saved: {e}")),
        }
    }
    pub fn add_peek_block(&mut self, after: usize) {
        let Some(peek) = &mut self.record_peek else {
            return;
        };
        if after >= peek.blocks.len() {
            return;
        }
        let edit = &mut peek.blocks[after].edit;
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
        let block = Block::paragraph(&peek.page.id, "", after as i64 + 1);
        peek.blocks.insert(
            after + 1,
            EditableBlock {
                block,
                edit: suffix,
            },
        );
        self.save_peek_blocks();
    }
    pub fn close_peek(&mut self) -> bool {
        if !self.finish_cell(false) {
            return false;
        }
        if self.record_peek.as_ref().is_some_and(|p| p.dirty_title) {
            self.save_peek_title();
        }
        if self.record_peek.as_ref().is_some_and(|p| p.dirty_blocks) {
            self.save_peek_blocks();
        }
        if self
            .record_peek
            .as_ref()
            .is_some_and(|p| p.dirty_title || p.dirty_blocks)
        {
            return false;
        }
        self.record_peek = None;
        true
    }
    pub fn expand_peek(&mut self) {
        let Some(id) = self.record_peek.as_ref().map(|p| p.page.id.clone()) else {
            return;
        };
        if self.close_peek() {
            self.open_page(&id, true);
        }
    }
}

pub fn poll(s: &mut State, _: f64, tx: &mut rune::Tx) -> bool {
    if s.editing_cell.is_some() && !s.cell.focused {
        s.finish_cell(false);
        tx.instant();
        return true;
    }
    false
}
