use crate::state::{CellTarget, EditableBlock, State};
use moss_core::*;
use rune::TextEdit;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) fn split_record_appearance(
    appearance: BTreeMap<String, RecordPageAppearance>,
) -> (BTreeMap<String, String>, BTreeMap<String, bool>) {
    let mut icons = BTreeMap::new();
    let mut content = BTreeMap::new();
    for (id, value) in appearance {
        content.insert(id.clone(), value.has_body_content);
        icons.insert(id, value.icon);
    }
    (icons, content)
}

pub fn view_load_limit(view: &DatabaseView) -> u32 {
    view.config
        .get("load_limit")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0 && *n < 1000)
        .unwrap_or(if view.kind == ViewKind::Gallery {
            50
        } else {
            100
        })
}

impl State {
    pub fn query_records(&mut self) -> bool {
        if !self.finish_cell(false) {
            return false;
        }
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        tab.title_search = self.filter.text.clone();
        let Some(db) = &tab.database else {
            return false;
        };
        let mut q = db
            .views
            .get(tab.view)
            .map(|v| v.query.clone())
            .unwrap_or_default();
        q.offset = tab.offset;
        let limit = db
            .views
            .get(tab.view)
            .map(|view| crate::table_window::initial_limit(view, &tab.title_search, tab.offset))
            .unwrap_or(100);
        q.limit = limit + 1;
        if !self.filter.text.is_empty() {
            let search = FilterExpr::Rule(FilterRule {
                property_id: db
                    .properties
                    .iter()
                    .find(|p| p.kind == PropertyKind::Title)
                    .map(|p| p.id.clone())
                    .unwrap_or_default(),
                op: FilterOp::Contains,
                value: json!(self.filter.text),
            });
            q.filter = Some(match q.filter.take() {
                Some(saved) => FilterExpr::All(vec![saved, search]),
                None => search,
            });
        }
        let request = tab.paging_request.take();
        if request
            .as_ref()
            .is_some_and(|(_, direction)| *direction == PageDirection::Previous)
        {
            q.limit = limit;
        }
        let result = match &request {
            Some((cursor, direction)) => self
                .store
                .query_records_cursor(&db.id, &q, cursor, *direction),
            None => self.store.query_records(&db.id, &q),
        };
        match result {
            Ok(mut records) => {
                tab.has_more_records = if request
                    .as_ref()
                    .is_some_and(|(_, direction)| *direction == PageDirection::Previous)
                {
                    !records.is_empty()
                } else {
                    records.len() > limit as usize
                };
                records.truncate(limit as usize);
                tab.paging_previous = records
                    .first()
                    .and_then(|record| self.store.record_cursor(&db.id, &q, record).ok().flatten());
                tab.paging_next = records
                    .last()
                    .and_then(|record| self.store.record_cursor(&db.id, &q, record).ok().flatten());
                {
                    let ids: Vec<_> = records.iter().map(|record| record.id.clone()).collect();
                    match self.store.record_page_appearance(&self.workspace.id, &ids) {
                        Ok(appearance) => {
                            (tab.record_icons, tab.record_has_content) =
                                split_record_appearance(appearance);
                        }
                        Err(e) => {
                            tab.record_icons.clear();
                            tab.record_has_content.clear();
                            self.error = Some(format!("Page icons are not loaded: {e}"));
                        }
                    }
                }
                if db
                    .views
                    .get(tab.view)
                    .is_some_and(|v| v.kind == ViewKind::Gallery)
                {
                    let ids: Vec<_> = records.iter().map(|record| record.id.clone()).collect();
                    match self.store.record_page_previews(&self.workspace.id, &ids) {
                        Ok(previews) => tab.gallery_previews = previews,
                        Err(e) => {
                            tab.gallery_previews.clear();
                            self.error = Some(format!("Page previews are not loaded: {e}"));
                        }
                    }
                    match self.store.record_page_covers(&self.workspace.id, &ids) {
                        Ok(covers) => tab.gallery_covers = covers,
                        Err(e) => {
                            tab.gallery_covers.clear();
                            self.error = Some(format!("Page covers are not loaded: {e}"));
                        }
                    }
                } else {
                    tab.gallery_previews.clear();
                    tab.gallery_covers.clear();
                }
                match crate::relation_titles::load(&self.store, db, &records) {
                    Ok(titles) => tab.relation_titles = titles,
                    Err(e) => {
                        self.error = Some(format!("Related titles are not loaded: {e}"));
                    }
                }
                tab.records = records;
                tab.created_record_ids.clear();
                tab.table_window = crate::table_window::supported(tab)
                    .then(|| crate::table_window::TableWindow::new(tab.records.len()));
                self.refresh_table_summaries();
                true
            }
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }
    pub fn select_database_view(&mut self, view: usize) -> bool {
        self.dismiss_property_picker();
        if self.property_picker.is_some() || !self.finish_cell(false) || !self.close_peek() {
            return false;
        }
        if self
            .tab()
            .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
            .is_some_and(|v| v.kind == ViewKind::Calendar)
        {
            self.save_preferences();
        }
        let calendar_position = self
            .tab()
            .and_then(|tab| tab.database.as_ref()?.views.get(view))
            .filter(|v| v.kind == ViewKind::Calendar)
            .and_then(|v| {
                self.prefs
                    .calendar_positions
                    .get(&self.workspace.id)?
                    .get(&v.id)
            })
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        if tab
            .database
            .as_ref()
            .is_none_or(|db| view >= db.views.len())
        {
            return false;
        }
        let (old_view, old_offset, old_calendar) = (tab.view, tab.offset, tab.calendar_month);
        tab.view = view;
        if let Some(date) = calendar_position {
            tab.calendar_month = date;
        }
        tab.offset = 0;
        if !self.query_records() {
            let tab = &mut self.tabs[self.active];
            tab.view = old_view;
            tab.offset = old_offset;
            tab.calendar_month = old_calendar;
            return false;
        }
        self.gallery_settings_open = false;
        self.database_property_editor = None;
        self.database_new_open = false;
        self.property_visibility_open = false;
        self.undated_open = false;
        self.undated_search = TextEdit::new("");
        self.save_preferences();
        true
    }
    pub fn move_record_page(&mut self, offset: u64) {
        let old = self.tabs[self.active].offset;
        let tab = &mut self.tabs[self.active];
        let limit = tab
            .database
            .as_ref()
            .and_then(|db| db.views.get(tab.view))
            .map(view_load_limit)
            .unwrap_or(100);
        tab.paging_request = if !tab.created_record_ids.is_empty() {
            None
        } else if offset == old.saturating_add(u64::from(limit)) {
            tab.paging_next
                .clone()
                .map(|cursor| (cursor, PageDirection::Next))
        } else if offset == old.saturating_sub(u64::from(limit)) && offset < old {
            tab.paging_previous
                .clone()
                .map(|cursor| (cursor, PageDirection::Previous))
        } else {
            None
        };
        self.tabs[self.active].offset = offset;
        if !self.query_records() {
            self.tabs[self.active].offset = old;
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
        let context = if let Some((db, view)) = self
            .tab()
            .and_then(|tab| {
                Some((
                    tab.database.as_ref()?,
                    tab.database.as_ref()?.views.get(tab.view)?,
                ))
            })
            .filter(|(_, view)| view.kind == moss_core::ViewKind::Board)
        {
            match moss_core::board::creation_values(db, view, context) {
                Ok(values) => values,
                Err(error) => {
                    let message = format!("Could not create the page: {error}");
                    self.database_views.board_ui.error = Some(message.clone());
                    self.error = Some(message);
                    return None;
                }
            }
        } else {
            context
        };
        let board_order = self.tab().and_then(|tab| {
            let db = tab.database.as_ref()?;
            let view = db.views.get(tab.view)?;
            if view.kind != moss_core::ViewKind::Board || !view.query.sorts.is_empty() {
                return None;
            }
            let primary = moss_core::board::group_property(db, view).ok()?.clone();
            let secondary = moss_core::board::subgroup_property(db, view).ok()?.cloned();
            Some((view.clone(), primary, secondary))
        });
        let values = self.create_values(context);
        match self.store.create_record(&db_id, "", &values) {
            Ok(mut record) => {
                self.database_views.board_ui.error = None;
                if let Some((view, primary, secondary)) = board_order {
                    let group = moss_core::board::group_value(&record, &primary);
                    let sub = secondary
                        .as_ref()
                        .map(|p| moss_core::board::group_value(&record, p))
                        .unwrap_or(Value::Null);
                    match self.store.move_board_record(
                        &db_id,
                        &view,
                        &record,
                        &primary,
                        secondary.as_ref(),
                        group,
                        sub,
                        None,
                        record.id.as_bytes(),
                    ) {
                        Ok(placed) => record = placed,
                        Err(error) => {
                            self.database_views.board_ui.error = Some(format!(
                                "Page created, but its position was not saved: {error}"
                            ))
                        }
                    }
                }
                // Keep a new draft visible while its title is being edited.
                let tab = &mut self.tabs[self.active];
                tab.records.push(record.clone());
                // Keep recent creations visible until the next explicit query.
                if tab.created_record_ids.len() == 100 {
                    tab.created_record_ids.remove(0);
                }
                tab.created_record_ids.push(record.id.clone());
                crate::table_window::note_created(self, &db_id);
                self.error = None;
                self.track_new_record(&record);
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
                expected_value: Value::Null,
                save_failed: false,
                repeat_values: repeat.then_some(values),
            });
            self.modal = None;
        }
    }
    pub fn create_peek(&mut self, values: BTreeMap<String, Value>) {
        if !self.finish_cell(false) || !self.close_peek() {
            return;
        }
        let full_page = self
            .tab()
            .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
            .and_then(|view| view.config.get("open_mode").and_then(Value::as_str))
            == Some("full");
        if full_page && !self.flush_current() {
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
        let Some(tab) = self.tab() else { return };
        let Some(db) = &tab.database else { return };
        let database_id = db.id.clone();
        let mode = db
            .views
            .get(tab.view)
            .map(|v| {
                v.config
                    .get("open_mode")
                    .and_then(Value::as_str)
                    .unwrap_or(
                        if matches!(v.kind, ViewKind::Gallery | ViewKind::Calendar) {
                            "center"
                        } else {
                            "side"
                        },
                    )
                    .to_owned()
            })
            .unwrap_or_else(|| "side".into());
        self.open_database_record(&database_id, id, &mode);
    }
    pub(crate) fn sync_record(&mut self, record: &Record) {
        if self
            .new_item_drafts
            .get(&record.id)
            .and_then(|draft| draft.record.as_ref())
            .is_some_and(|original| original != record)
        {
            self.new_item_drafts.remove(&record.id);
            self.pending_empty_items.retain(|id| id != &record.id);
        }
        self.sidebar_sections.update_record_title(record);
        let calendar = &mut self.database_views.calendar;
        for projection in calendar
            .projection
            .iter_mut()
            .chain(calendar.adjacent.values_mut())
        {
            for current in projection
                .records
                .iter_mut()
                .chain(projection.undated.iter_mut())
            {
                if current.id == record.id {
                    *current = record.clone();
                }
            }
        }
        for tab in &mut self.tabs {
            if let Some((_, current)) = &mut tab.record_context {
                if current.id == record.id {
                    *current = record.clone();
                }
            }
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
                .record_database(record_id)
                .and_then(|d| d.properties.iter().find(|p| &p.id == id))
                .map(|p| p.kind);
            if kind == Some(PropertyKind::Place) {
                self.open_place_picker(record_id, id);
                return;
            }
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
        let expected_value = prop
            .as_ref()
            .and_then(|id| record.values.get(id))
            .cloned()
            .unwrap_or(Value::Null);
        self.editing_cell = Some(CellTarget {
            record_id: record.id,
            property_id: prop,
            expected_value,
            save_failed: false,
            repeat_values: None,
        });
        self.error = None;
    }
    pub fn table_edit_tab(&mut self, backwards: bool) {
        let Some(target) = self.editing_cell.as_ref() else {
            return;
        };
        let record_id = target.record_id.clone();
        let property = target.property_id.clone();
        let Some(next) = self.adjacent_table_property(property.as_deref(), backwards) else {
            return;
        };
        if self.finish_cell(false) {
            self.selected_cell = Some((record_id, next));
        }
    }

    fn adjacent_table_property(
        &self,
        current: Option<&str>,
        backwards: bool,
    ) -> Option<Option<String>> {
        let tab = self.tab()?;
        let db = tab.database.as_ref()?;
        let columns: Vec<Option<String>> = moss_core::record_layout::columns_for_view(db, tab.view)
            .into_iter()
            .filter(|p| crate::ui::property_visibility::visible(db, tab.view, &p.id))
            .map(|p| (p.kind != PropertyKind::Title).then(|| p.id.clone()))
            .collect();
        let index = columns.iter().position(|id| id.as_deref() == current)?;
        // shortcut: row-edge traversal needs a reference capture before it can move to another row.
        let next = if backwards {
            index.checked_sub(1)?
        } else {
            index + 1
        };
        columns.get(next).cloned()
    }

    pub fn select_next_table_row(&mut self, record_id: &str, property_id: &str) {
        let Some(tab) = self.tab() else { return };
        let Some(index) = tab.records.iter().position(|r| r.id == record_id) else {
            return;
        };
        if let Some(next) = tab.records.get(index + 1) {
            self.selected_cell = Some((next.id.clone(), Some(property_id.to_owned())));
        }
    }

    pub fn table_selection_key(&mut self, event: &rune::KeyEvent) -> bool {
        if self
            .tab()
            .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
            .map(|view| view.kind)
            != Some(ViewKind::Table)
            || self.modal.is_some()
            || self.record_peek.is_some()
            || self.property_picker.is_some()
            || self.place_picker.is_some()
            || self.advanced_property.relation_picker.is_some()
            || self.editing_cell.is_some()
        {
            return false;
        }
        let Some((record_id, property)) = self.selected_cell.clone() else {
            return false;
        };
        match event.key {
            rune::Key::Tab => {
                if let Some(next) =
                    self.adjacent_table_property(property.as_deref(), event.mods.shift)
                {
                    self.selected_cell = Some((record_id, next));
                }
                true
            }
            rune::Key::Enter => {
                self.selected_cell = None;
                let kind = property
                    .as_deref()
                    .and_then(|id| {
                        self.tab()?
                            .database
                            .as_ref()?
                            .properties
                            .iter()
                            .find(|p| p.id == id)
                    })
                    .map(|p| p.kind);
                if kind == Some(PropertyKind::Checkbox) {
                    self.toggle_table_checkbox(&record_id, property.as_deref().unwrap());
                } else if kind == Some(PropertyKind::Relation) {
                    self.open_relation_picker(&record_id, property.as_deref().unwrap());
                } else if kind == Some(PropertyKind::Place) {
                    self.open_place_picker(&record_id, property.as_deref().unwrap());
                } else if matches!(kind, Some(PropertyKind::Formula | PropertyKind::Rollup)) {
                    self.selected_cell = Some((record_id, property));
                    return true;
                } else if matches!(
                    kind,
                    Some(PropertyKind::Date | PropertyKind::Status | PropertyKind::Select)
                ) {
                    self.open_property_picker(&record_id, property.as_deref().unwrap());
                } else {
                    self.edit_record_property(&record_id, property.clone());
                }
                if self.editing_cell.is_none()
                    && self.property_picker.is_none()
                    && self.place_picker.is_none()
                {
                    self.selected_cell = Some((record_id, property));
                }
                true
            }
            rune::Key::Escape => {
                self.selected_cell = None;
                true
            }
            _ => false,
        }
    }
    pub fn toggle_table_checkbox(&mut self, record_id: &str, property_id: &str) {
        self.dismiss_property_picker();
        if self.property_picker.is_some() || !self.finish_cell(false) {
            return;
        }
        let kind = self
            .tab()
            .and_then(|tab| tab.database.as_ref())
            .and_then(|db| db.properties.iter().find(|p| p.id == property_id))
            .map(|p| p.kind);
        if kind != Some(PropertyKind::Checkbox) {
            return;
        }
        let record = self
            .record_peek
            .as_ref()
            .filter(|p| p.record.id == record_id)
            .map(|p| &p.record)
            .or_else(|| {
                self.tab()
                    .and_then(|t| t.records.iter().find(|r| r.id == record_id))
            });
        let Some(record) = record else {
            self.error = Some("Record was not found.".into());
            return;
        };
        let expected = record
            .values
            .get(property_id)
            .cloned()
            .unwrap_or(Value::Null);
        let checked = match expected {
            Value::Null | Value::Bool(false) => false,
            Value::Bool(true) => true,
            _ => {
                self.error = Some("This checkbox value is not valid.".into());
                return;
            }
        };
        match self.store.set_record_value_if_unchanged(
            record_id,
            property_id,
            &expected,
            json!(!checked),
        ) {
            Ok(record) => {
                self.sync_record(&record);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Changes are not saved: {e}")),
        }
    }
    pub fn save_cell(&mut self) -> bool {
        let Some(target) = &self.editing_cell else {
            return true;
        };
        let id = target.record_id.clone();
        let property = target.property_id.clone();
        let expected_value = target.expected_value.clone();
        if property.is_none()
            && self
                .new_item_drafts
                .get(&id)
                .and_then(|draft| draft.record.as_ref())
                .is_some_and(|record| record.title == self.cell.text)
        {
            return true;
        }
        let result = (|| -> Result<Record, String> {
            if let Some(p) = property.as_ref() {
                let kind = self
                    .tab()
                    .and_then(|t| t.database.as_ref())
                    .and_then(|d| d.properties.iter().find(|x| &x.id == p))
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
                self.store
                    .set_record_value_if_unchanged(&id, p, &expected_value, value)
                    .map_err(|e| e.to_string())
            } else {
                self.store
                    .rename_record(&id, &self.cell.text)
                    .map_err(|e| e.to_string())
            }
        })();
        match result {
            Ok(record) => {
                if let (Some(property), Some(target)) =
                    (property.as_ref(), self.editing_cell.as_mut())
                {
                    target.expected_value =
                        record.values.get(property).cloned().unwrap_or(Value::Null);
                }
                if let Some(target) = &mut self.editing_cell {
                    target.save_failed = false;
                }
                self.sync_record(&record);
                self.error = None;
                true
            }
            Err(e) => {
                if let Some(target) = &mut self.editing_cell {
                    target.save_failed = true;
                }
                self.error = Some(format!("Changes are not saved: {e}"));
                false
            }
        }
    }
    pub(crate) fn reload_record_for_discard(&mut self, record_id: &str) -> bool {
        let record = match self.store.get_record(record_id) {
            Ok(Some(record)) if record.deleted_at.is_none() => record,
            Ok(_) => {
                self.error =
                    Some("Could not reload the saved record. Keep this draft open.".into());
                return false;
            }
            Err(e) => {
                self.error = Some(format!(
                    "Could not reload the saved record. Keep this draft open: {e}"
                ));
                return false;
            }
        };
        self.sync_record(&record);
        self.error = None;
        true
    }
    pub fn discard_cell_and_reload(&mut self) {
        let Some(record_id) = self
            .editing_cell
            .as_ref()
            .map(|target| target.record_id.clone())
        else {
            return;
        };
        if self.reload_record_for_discard(&record_id) {
            self.editing_cell = None;
            self.cell.focused = false;
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
        let target = self.editing_cell.take();
        if let Some(target) = &target {
            if self.new_item_drafts.contains_key(&target.record_id)
                && !self.pending_empty_items.contains(&target.record_id)
            {
                self.pending_empty_items.push(target.record_id.clone());
            }
        }
        let values = target.and_then(|t| t.repeat_values);
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
    pub(crate) fn sync_gallery_preview(&mut self, page_id: &str, blocks: &[Block]) {
        let preview: String = blocks
            .first()
            .map(|block| block.text.chars().take(512).collect())
            .unwrap_or_default();
        for tab in &mut self.tabs {
            for record in &tab.records {
                if record.page_id.as_deref() == Some(page_id) {
                    tab.gallery_previews
                        .insert(record.id.clone(), preview.clone());
                }
            }
        }
    }
    pub(crate) fn sync_record_content(&mut self, page_id: &str, blocks: &[Block]) {
        let has_content = blocks_have_content(blocks);
        for tab in &mut self.tabs {
            for record in &tab.records {
                if record.page_id.as_deref() == Some(page_id) {
                    tab.record_has_content
                        .insert(record.id.clone(), has_content);
                }
            }
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
        match self.store.save_editor_blocks(&peek.page.id, &blocks) {
            Ok(blocks) => {
                crate::page_covers::sync_saved_image_assets(&mut peek.blocks, &blocks);
                peek.dirty_blocks = false;
                let page_id = peek.page.id.clone();
                for tab in &mut self.tabs {
                    if tab.page.id == page_id && !tab.dirty_blocks {
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
                self.sync_gallery_preview(&page_id, &blocks);
                self.sync_record_content(&page_id, &blocks);
                self.keep_draft_for_page(&page_id);
                if self.comments.threads.get(&page_id).is_some_and(|threads| !threads.is_empty()) {
                    self.load_comments(&page_id);
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
        let mut block = Block::paragraph(&peek.page.id, &suffix.text, after as i64 + 1);
        crate::text_colors::copy_suffix(&peek.blocks[after].block, &mut block, end);
        crate::editor_drag::copy_column(&peek.blocks[after].block, &mut block);
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
        for _ in 0..2 {
            let Some(menu) = self.block_commands.as_ref().filter(|menu| menu.peek) else {
                break;
            };
            if menu.error.is_some() {
                return false;
            }
            self.dismiss_block_commands();
        }
        if self.block_commands.as_ref().is_some_and(|menu| menu.peek) {
            return false;
        }
        self.dismiss_place_picker();
        if !self.finish_map_interaction() {
            return false;
        }
        if !self.finish_page_cover() {
            return false;
        }
        if !self.finish_simple_table() {
            return false;
        }
        if self
            .property_picker
            .as_ref()
            .is_some_and(|picker| picker.save_failed)
        {
            return false;
        }
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
        if let Some(id) = self.record_peek.as_ref().map(|peek| peek.record.id.clone()) {
            if !self.discard_draft(&id) {
                return false;
            }
        }
        self.record_peek = None;
        true
    }
    pub fn expand_peek(&mut self) {
        let Some(id) = self.record_peek.as_ref().map(|p| p.page.id.clone()) else {
            return;
        };
        let record_id = self.record_peek.as_ref().unwrap().record.id.clone();
        let draft = self.new_item_drafts.remove(&record_id);
        let closed = self.close_peek();
        if let Some(draft) = draft {
            self.new_item_drafts.insert(record_id, draft);
        }
        if closed {
            self.open_page(&id, true);
        }
    }
}

pub fn poll(s: &mut State, time: f64, tx: &mut rune::Tx) -> bool {
    let draft_changed = s.discard_closed_items();
    let caret_changed = crate::caret::poll(s, time, tx);
    let computed_changed = crate::computed_refresh::poll(s);
    let window_changed = crate::table_window::poll(s, tx);
    let summaries_changed = crate::table_summaries::poll(s, tx);
    if s.property_picker.is_none() && s.editing_cell.is_some() && !s.cell.focused {
        s.finish_cell(false);
        tx.instant();
        return true;
    }
    draft_changed || caret_changed || computed_changed || summaries_changed || window_changed
}

#[cfg(test)]
#[path = "records_atomic_tests.rs"]
mod atomic_tests;
