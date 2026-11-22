use crate::state::{CellTarget, State};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use moss_core::PropertyKind;
use rune::TextEdit;
use serde_json::{Value, json};

pub fn is_editable_property(kind: PropertyKind) -> bool {
    matches!(
        kind,
        PropertyKind::Select
            | PropertyKind::MultiSelect
            | PropertyKind::Status
            | PropertyKind::Date
            | PropertyKind::Text
            | PropertyKind::Number
            | PropertyKind::Url
            | PropertyKind::Email
            | PropertyKind::Phone
            | PropertyKind::Checkbox
            | PropertyKind::Relation
            | PropertyKind::Place
    )
}

pub fn is_popup_picker_kind(kind: PropertyKind) -> bool {
    matches!(
        kind,
        PropertyKind::Date
            | PropertyKind::Status
            | PropertyKind::Select
            | PropertyKind::MultiSelect
    )
}

pub enum CellPicker {
    Property(PropertyPicker),
    Relation(crate::advanced_properties::RelationPicker),
    Place(crate::place_property::PlacePicker),
}

impl CellPicker {
    pub fn record_id(&self) -> &str {
        match self {
            Self::Property(p) => &p.record_id,
            Self::Relation(p) => &p.record_id,
            Self::Place(p) => &p.record_id,
        }
    }

    pub fn is_relation(&self) -> bool {
        matches!(self, Self::Relation(_))
    }

    pub fn is_place(&self) -> bool {
        matches!(self, Self::Place(_))
    }

    pub fn as_property(&self) -> Option<&PropertyPicker> {
        match self {
            Self::Property(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_property_mut(&mut self) -> Option<&mut PropertyPicker> {
        match self {
            Self::Property(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_relation(&self) -> Option<&crate::advanced_properties::RelationPicker> {
        match self {
            Self::Relation(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_relation_mut(&mut self) -> Option<&mut crate::advanced_properties::RelationPicker> {
        match self {
            Self::Relation(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_place(&self) -> Option<&crate::place_property::PlacePicker> {
        match self {
            Self::Place(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_place_mut(&mut self) -> Option<&mut crate::place_property::PlacePicker> {
        match self {
            Self::Place(p) => Some(p),
            _ => None,
        }
    }

    pub fn save_failed(&self) -> bool {
        match self {
            Self::Property(p) => p.save_failed,
            Self::Relation(p) => p.save_failed,
            Self::Place(_) => false,
        }
    }
}

pub struct OptionEditor {
    pub original_name: String,
    pub name_input: TextEdit,
    pub color: String,
}

pub struct PropertyPicker {
    pub record_id: String,
    pub property_id: String,
    pub kind: PropertyKind,
    pub month: NaiveDate,
    pub selected: Option<usize>,
    pub error: Option<String>,
    pub save_failed: bool,
    pub input: TextEdit,
    pub date: Option<DateDraft>,
    pub date_format_pane: bool,
    pub option_editor: Option<OptionEditor>,
    pub multi_selected: Vec<String>,
}

pub struct DateDraft {
    original: Value,
    start_suffix: String,
    end_suffix: String,
    pub has_end: bool,
    pub has_time: bool,
    pending_clear: bool,
    pub end_input: TextEdit,
    pub start_time_input: TextEdit,
    pub end_time_input: TextEdit,
}

fn endpoint(value: &str) -> Option<(NaiveDate, Option<String>, String)> {
    let date = NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()?;
    if value.len() == 10 {
        return Some((date, None, String::new()));
    }
    let valid = DateTime::parse_from_rfc3339(value).is_ok()
        || NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        || NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M").is_ok();
    if !valid {
        return None;
    }
    let time = value.get(11..16)?.to_string();
    NaiveTime::parse_from_str(&time, "%H:%M").ok()?;
    Some((date, Some(time), value.get(16..)?.to_owned()))
}
fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%d/%m/%Y")
        .or_else(|_| NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d"))
        .ok()
}
fn date_endpoint(date: NaiveDate, time: Option<&str>, suffix: &str) -> String {
    match time {
        Some(time) => format!("{date}T{time}{suffix}"),
        None => date.to_string(),
    }
}

fn timed_endpoint(value: &str) -> Option<(NaiveDateTime, Option<i32>)> {
    if let Ok(date) = DateTime::parse_from_rfc3339(value) {
        return Some((date.naive_local(), Some(date.offset().local_minus_utc())));
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))
        .ok()
        .map(|date| (date, None))
}

impl State {
    pub fn property_picker(&self) -> Option<&PropertyPicker> {
        self.cell_picker.as_ref().and_then(CellPicker::as_property)
    }

    pub fn property_picker_mut(&mut self) -> Option<&mut PropertyPicker> {
        self.cell_picker
            .as_mut()
            .and_then(CellPicker::as_property_mut)
    }

    pub fn relation_picker(&self) -> Option<&crate::advanced_properties::RelationPicker> {
        self.cell_picker.as_ref().and_then(CellPicker::as_relation)
    }

    pub fn relation_picker_mut(
        &mut self,
    ) -> Option<&mut crate::advanced_properties::RelationPicker> {
        self.cell_picker
            .as_mut()
            .and_then(CellPicker::as_relation_mut)
    }

    pub fn place_picker(&self) -> Option<&crate::place_property::PlacePicker> {
        self.cell_picker.as_ref().and_then(CellPicker::as_place)
    }

    pub fn place_picker_mut(&mut self) -> Option<&mut crate::place_property::PlacePicker> {
        self.cell_picker.as_mut().and_then(CellPicker::as_place_mut)
    }

    pub fn dismiss_cell_picker(&mut self) {
        if let Some(CellPicker::Property(picker)) = self.cell_picker.as_mut() {
            if picker.save_failed {
                picker.input.focused = true;
                return;
            }
        }
        if let Some(CellPicker::Relation(picker)) = self.cell_picker.as_ref() {
            if picker.save_failed {
                return;
            }
        }
        match self.cell_picker.take() {
            Some(CellPicker::Property(_)) => {
                self.editing_cell = None;
                self.cell.focused = false;
            }
            Some(CellPicker::Place(_)) => {
                let map = &mut self.database_views.map;
                map.search_changed();
                map.place_query.focused = false;
            }
            Some(CellPicker::Relation(_)) | None => {}
        }
    }

    pub fn property_anchor(
        &self,
        record_id: &str,
        property_id: &str,
        kind: PropertyKind,
    ) -> Option<String> {
        if self
            .record_peek
            .as_ref()
            .is_some_and(|p| p.record.id == record_id)
        {
            return Some(format!("peek-property-{property_id}"));
        }
        if self
            .tab()
            .and_then(|tab| tab.record_context.as_ref())
            .is_some_and(|(_, record)| record.id == record_id)
        {
            return Some(if kind == PropertyKind::Place {
                format!("page-place-{property_id}")
            } else {
                format!("page-property-{property_id}")
            });
        }
        let view_kind = self
            .tab()
            .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
            .map(|view| view.kind);
        match view_kind {
            Some(moss_core::ViewKind::Calendar) => {
                self.database_views.calendar.property_anchor.clone()
            }
            Some(moss_core::ViewKind::Board) => {
                if kind == PropertyKind::Place {
                    let empty = self
                        .store
                        .get_record(record_id)
                        .ok()
                        .flatten()
                        .and_then(|r| r.values.get(property_id).cloned())
                        .is_none_or(|v| v.is_null() || v.as_str().is_some_and(str::is_empty));
                    if empty {
                        return Some(format!("board-card-{record_id}"));
                    }
                }
                Some(format!("board-value-{record_id}-{property_id}"))
            }
            _ => {
                let index = self
                    .tab()?
                    .records
                    .iter()
                    .position(|r| r.id == record_id)?;
                Some(match view_kind {
                    Some(moss_core::ViewKind::Gallery) => {
                        format!("gallery-value-{index}-{property_id}")
                    }
                    Some(moss_core::ViewKind::List) => {
                        format!("list-value-{index}-{property_id}")
                    }
                    _ => format!("cell-{index}-{property_id}"),
                })
            }
        }
    }

    pub fn open_property_editor(&mut self, record_id: &str, property_id: &str) -> Option<PropertyKind> {
        let kind = self
            .record_database(record_id)
            .and_then(|db| db.properties.iter().find(|p| p.id == property_id))
            .map(|p| p.kind)?;
        match kind {
            PropertyKind::Checkbox => self.toggle_table_checkbox(record_id, property_id),
            PropertyKind::Place => self.open_place_picker(record_id, property_id),
            PropertyKind::Relation => self.open_relation_picker(record_id, property_id),
            PropertyKind::Formula | PropertyKind::Rollup => {}
            PropertyKind::Date
            | PropertyKind::Status
            | PropertyKind::Select
            | PropertyKind::MultiSelect => self.open_property_picker(record_id, property_id),
            _ => {
                self.dismiss_cell_picker();
                self.edit_record_property(record_id, Some(property_id.to_owned()));
            }
        }
        Some(kind)
    }

    pub fn open_property_picker(&mut self, record_id: &str, property_id: &str) {
        self.dismiss_cell_picker();
        if self.cell_picker.is_some() {
            return;
        }
        let kind = self
            .record_database(record_id)
            .and_then(|d| d.properties.iter().find(|p| p.id == property_id))
            .map(|p| p.kind);
        let Some(
            kind @ (PropertyKind::Date
            | PropertyKind::Status
            | PropertyKind::Select
            | PropertyKind::MultiSelect),
        ) = kind
        else {
            return;
        };
        let mut date_draft = None;
        let mut multi_selected = Vec::new();
        let display;
        let month;
        if kind == PropertyKind::Date {
            if !self.finish_cell(false) {
                return;
            }
            let record = match self.store.get_record(record_id) {
                Ok(Some(record)) => record,
                Ok(None) => {
                    self.error = Some("Record was not found.".into());
                    return;
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            };
            let original = record
                .values
                .get(property_id)
                .cloned()
                .unwrap_or(Value::Null);
            let (start, end) = match &original {
                Value::Null => (None, None),
                Value::String(start) => (Some(start.as_str()), None),
                Value::Object(map) if !map.contains_key("time") => {
                    let Some(start) = map.get("start").and_then(Value::as_str) else {
                        self.error = Some("This date value needs a different editor.".into());
                        return;
                    };
                    let end = match map.get("end") {
                        None | Some(Value::Null) => None,
                        Some(Value::String(end)) => Some(end.as_str()),
                        _ => {
                            self.error = Some("This date range needs a different editor.".into());
                            return;
                        }
                    };
                    (Some(start), end)
                }
                _ => {
                    self.error = Some("This date value needs a different editor.".into());
                    return;
                }
            };
            let start = match match start {
                None => Some(None),
                Some(value) => endpoint(value).map(Some),
            } {
                Some(value) => value,
                None => {
                    self.error = Some("This date value needs a different editor.".into());
                    return;
                }
            };
            let end = match match end {
                None => Some(None),
                Some(value) => endpoint(value).map(Some),
            } {
                Some(value) => value,
                None => {
                    self.error = Some("This date range needs a different editor.".into());
                    return;
                }
            };
            let has_time = start.as_ref().is_some_and(|(_, t, _)| t.is_some())
                || end.as_ref().is_some_and(|(_, t, _)| t.is_some());
            // Mixed timed/untimed ranges need a separate editor; never change their precision silently.
            if has_time
                && (start.as_ref().is_some_and(|(_, t, _)| t.is_none())
                    || end.as_ref().is_some_and(|(_, t, _)| t.is_none()))
            {
                self.error = Some(
                    "This date range has mixed time precision and needs a different editor.".into(),
                );
                return;
            }
            month = start
                .as_ref()
                .map(|(d, _, _)| *d)
                .unwrap_or_else(|| chrono::Local::now().date_naive())
                .with_day(1)
                .unwrap();
            display = start
                .as_ref()
                .map(|(d, _, _)| d.format("%d/%m/%Y").to_string())
                .unwrap_or_default();
            self.cell = TextEdit::new(
                &start
                    .as_ref()
                    .map(|(d, _, _)| d.to_string())
                    .unwrap_or_default(),
            );
            self.cell.focused = true;
            self.editing_cell = Some(CellTarget {
                record_id: record_id.into(),
                property_id: Some(property_id.into()),
                expected_value: original.clone(),
                save_failed: false,
                repeat_values: None,
            });
            date_draft = Some(DateDraft {
                original,
                start_suffix: start.as_ref().map(|(_, _, s)| s.clone()).unwrap_or_default(),
                end_suffix: end.as_ref().map(|(_, _, s)| s.clone()).unwrap_or_default(),
                has_end: end.is_some(),
                has_time,
                pending_clear: false,
                end_input: TextEdit::new(
                    &end.as_ref()
                        .map(|(d, _, _)| d.format("%d/%m/%Y").to_string())
                        .unwrap_or_default(),
                ),
                start_time_input: TextEdit::new(
                    start
                        .as_ref()
                        .and_then(|(_, t, _)| t.as_deref())
                        .unwrap_or(""),
                ),
                end_time_input: TextEdit::new(
                    end.as_ref()
                        .and_then(|(_, t, _)| t.as_deref())
                        .unwrap_or(""),
                ),
            });
            self.error = None;
        } else if kind == PropertyKind::MultiSelect {
            if !self.finish_cell(false) {
                return;
            }
            let record = match self.store.get_record(record_id) {
                Ok(Some(record)) => record,
                Ok(None) => {
                    self.error = Some("Record was not found.".into());
                    return;
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            };
            let original = record
                .values
                .get(property_id)
                .cloned()
                .unwrap_or(Value::Null);
            multi_selected = match &original {
                Value::Null => Vec::new(),
                Value::Array(items) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
                Value::String(s) if !s.trim().is_empty() => vec![s.trim().to_owned()],
                _ => {
                    self.error = Some("This value needs a different editor.".into());
                    return;
                }
            };
            self.editing_cell = Some(CellTarget {
                record_id: record_id.into(),
                property_id: Some(property_id.into()),
                expected_value: original,
                save_failed: false,
                repeat_values: None,
            });
            self.error = None;
            month = chrono::Local::now().date_naive().with_day(1).unwrap();
            display = String::new();
        } else {
            self.edit_record_property(record_id, Some(property_id.to_owned()));
            if !self.editing_cell.as_ref().is_some_and(|c| {
                c.record_id == record_id && c.property_id.as_deref() == Some(property_id)
            }) {
                return;
            }
            month = chrono::Local::now().date_naive().with_day(1).unwrap();
            display = String::new();
        }
        let mut input = TextEdit::new(&display);
        input.focused = true;
        if kind == PropertyKind::Date {
            input.set_selection(0, display.len());
        }
        self.cell_picker = Some(CellPicker::Property(PropertyPicker {
            record_id: record_id.into(),
            property_id: property_id.into(),
            kind,
            month,
            selected: None,
            error: None,
            save_failed: false,
            input,
            date: date_draft,
            date_format_pane: false,
            option_editor: None,
            multi_selected,
        }));
    }
    pub fn discard_failed_property_change(&mut self) {
        let Some(record_id) = self
            .property_picker()
            .map(|picker| picker.record_id.clone())
        else {
            return;
        };
        if !self.reload_record_for_discard(&record_id) {
            let error = self.error.clone();
            if let Some(picker) = self.property_picker_mut() {
                picker.error = error;
                picker.input.focused = true;
            }
            return;
        }
        if let Some(picker) = self.property_picker_mut() {
            picker.save_failed = false;
        }
        self.dismiss_cell_picker();
    }
    pub fn retry_property_change(&mut self) {
        if let Some(date) = self.property_picker().and_then(|p| p.date.as_ref()) {
            if date.pending_clear {
                self.clear_date_picker();
            } else {
                self.submit_date_picker();
            }
        } else if self
            .property_picker()
            .is_some_and(|p| p.kind == PropertyKind::MultiSelect)
        {
            let selected = self
                .property_picker()
                .map(|p| p.multi_selected.clone())
                .unwrap_or_default();
            self.save_multi_select_values(selected);
        } else {
            let draft = self.cell.text.clone();
            self.choose_property_value(&draft);
        }
    }
    pub fn date_draft_changed(&mut self) {
        if let Some(draft) = self.property_picker_mut().and_then(|p| p.date.as_mut()) {
            draft.pending_clear = false;
        }
    }
    pub fn property_options(&self) -> Vec<String> {
        let Some(picker) = self.property_picker() else {
            return vec![];
        };
        let query = picker.input.text.to_lowercase();
        self.record_database(&picker.record_id)
            .and_then(|d| d.properties.iter().find(|p| p.id == picker.property_id))
            .map(|p| {
                moss_core::property_options(p)
                    .into_iter()
                    .map(|opt| opt.name)
                    .filter(|name| name.to_lowercase().contains(&query))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn choose_property_value(&mut self, value: &str) -> bool {
        let Some(picker) = self.property_picker() else {
            return false;
        };
        if !self.editing_cell.as_ref().is_some_and(|target| {
            target.record_id == picker.record_id
                && target.property_id.as_deref() == Some(picker.property_id.as_str())
        }) {
            let message =
                "The property editor lost its target. Discard this change and open it again.";
            self.error = Some(message.into());
            let picker = self.property_picker_mut().unwrap();
            picker.error = Some(message.into());
            picker.save_failed = true;
            return false;
        }
        self.cell.replace_text(value);
        if self.save_cell() {
            if let Some(picker) = self.property_picker_mut() {
                picker.save_failed = false;
            }
            self.dismiss_cell_picker();
            true
        } else {
            let error = self.error.clone();
            if let Some(picker) = self.property_picker_mut() {
                picker.save_failed = true;
                picker.error = error;
                picker.input.focused = true;
            }
            false
        }
    }
    pub fn toggle_date_end(&mut self) {
        if let Some(draft) = self.property_picker_mut().and_then(|p| p.date.as_mut()) {
            draft.has_end = !draft.has_end;
            draft.pending_clear = false;
        }
    }
    pub fn toggle_date_time(&mut self) {
        let Some(draft) = self.property_picker_mut().and_then(|p| p.date.as_mut()) else {
            return;
        };
        if draft.has_time
            && [draft.start_suffix.as_str(), draft.end_suffix.as_str()]
                .iter()
                .any(|suffix| suffix.contains(['Z', 'z', '+', '-']))
        {
            self.property_picker_mut().unwrap().error = Some(
                "The existing time zone must be preserved. Clear the date to remove it.".into(),
            );
            return;
        }
        draft.has_time = !draft.has_time;
        draft.pending_clear = false;
    }
    pub fn choose_calendar_date(&mut self, date: NaiveDate) {
        self.date_draft_changed();
        if let Some(picker) = self.property_picker_mut() {
            picker
                .input
                .replace_text(&date.format("%d/%m/%Y").to_string());
        }
        self.cell.replace_text(&date.to_string());
        let extended = self
            .property_picker()
            .and_then(|p| p.date.as_ref())
            .is_some_and(|d| d.has_end || d.has_time);
        if !extended {
            self.submit_date_picker();
        }
    }
    pub fn clear_date_picker(&mut self) {
        self.save_date_value(Value::Null, true);
    }
    fn submit_date_picker(&mut self) {
        let Some(picker) = self.property_picker() else {
            return;
        };
        let Some(draft) = picker.date.as_ref() else {
            return;
        };
        let start_text = picker.input.text.trim();
        if start_text.is_empty() && !draft.has_end && !draft.has_time {
            self.save_date_value(Value::Null, false);
            return;
        }
        let Some(start) = parse_date(start_text) else {
            self.date_error("Enter a valid start date (DD/MM/YYYY).");
            return;
        };
        let end = if draft.has_end {
            let Some(end) = parse_date(&draft.end_input.text) else {
                self.date_error("Enter a valid end date (DD/MM/YYYY).");
                return;
            };
            if end < start {
                self.date_error("The end date cannot be before the start date.");
                return;
            }
            Some(end)
        } else {
            None
        };
        let (start_time, end_time) = if draft.has_time {
            let Ok(start_time) = NaiveTime::parse_from_str(draft.start_time_input.text.trim(), "%H:%M")
            else {
                self.date_error("Enter a valid start time (HH:MM).");
                return;
            };
            let end_time = if end.is_some() {
                let Ok(end_time) =
                    NaiveTime::parse_from_str(draft.end_time_input.text.trim(), "%H:%M")
                else {
                    self.date_error("Enter a valid end time (HH:MM).");
                    return;
                };
                Some(end_time)
            } else {
                None
            };
            (
                Some(start_time.format("%H:%M").to_string()),
                end_time.map(|t| t.format("%H:%M").to_string()),
            )
        } else {
            (None, None)
        };
        let start_value = date_endpoint(start, start_time.as_deref(), &draft.start_suffix);
        let end_value = end.map(|d| date_endpoint(d, end_time.as_deref(), &draft.end_suffix));
        if let Some(ref end_value) = end_value {
            if draft.has_time {
                let Some((start_full, start_offset)) = timed_endpoint(&start_value) else {
                    self.date_error("The start time is not valid.");
                    return;
                };
                let Some((end_full, end_offset)) = timed_endpoint(end_value) else {
                    self.date_error("The end time is not valid.");
                    return;
                };
                if start_offset != end_offset {
                    self.date_error("The start and end must use the same offset.");
                    return;
                }
                if end_full < start_full {
                    self.date_error("The end time cannot be before the start time.");
                    return;
                }
            }
        }
        let mut value = draft.original.clone();
        if value.is_object() || draft.has_end || draft.has_time {
            if !value.is_object() {
                value = json!({});
            }
            let object = value.as_object_mut().unwrap();
            object.insert("start".into(), Value::String(start_value));
            if let Some(end) = end_value {
                object.insert("end".into(), Value::String(end));
            } else if draft
                .original
                .get("end")
                .is_some_and(|value| !value.is_null())
            {
                object.remove("end");
            }
        } else {
            value = Value::String(start_value);
        }
        self.save_date_value(value, false);
    }
    fn date_error(&mut self, message: &str) {
        if let Some(picker) = self.property_picker_mut() {
            picker.error = Some(message.into());
        }
    }
    fn save_date_value(&mut self, value: Value, clear: bool) -> bool {
        let Some(picker) = self.property_picker() else {
            return false;
        };
        let Some(draft) = &picker.date else {
            return false;
        };
        let (record_id, property_id, original) = (
            picker.record_id.clone(),
            picker.property_id.clone(),
            draft.original.clone(),
        );
        if !self.editing_cell.as_ref().is_some_and(|c| {
            c.record_id == record_id && c.property_id.as_deref() == Some(property_id.as_str())
        }) {
            self.date_error(
                "The property editor lost its target. Discard this change and open it again.",
            );
            self.property_picker_mut().unwrap().save_failed = true;
            return false;
        }
        let result = self
            .store
            .set_record_value_if_unchanged(&record_id, &property_id, &original, value)
            .map_err(|e| e.to_string());
        match result {
            Ok(record) => {
                self.sync_record(&record);
                self.error = None;
                self.property_picker_mut().unwrap().save_failed = false;
                self.dismiss_cell_picker();
                true
            }
            Err(error) => {
                self.error = Some(format!("Changes are not saved: {error}"));
                let message = self.error.clone();
                let picker = self.property_picker_mut().unwrap();
                picker.error = message;
                picker.save_failed = true;
                picker.date.as_mut().unwrap().pending_clear = clear;
                picker.input.focused = true;
                false
            }
        }
    }
    pub fn toggle_multi_select_value(&mut self, value: &str) -> bool {
        let Some(picker) = self.property_picker() else {
            return false;
        };
        if picker.kind != PropertyKind::MultiSelect {
            return false;
        }
        let mut next = picker.multi_selected.clone();
        if let Some(pos) = next.iter().position(|item| item == value) {
            next.remove(pos);
        } else {
            next.push(value.to_owned());
        }
        self.save_multi_select_values(next)
    }
    fn save_multi_select_values(&mut self, next: Vec<String>) -> bool {
        let Some(picker) = self.property_picker() else {
            return false;
        };
        let (record_id, property_id) = (picker.record_id.clone(), picker.property_id.clone());
        let expected = match self.editing_cell.as_ref() {
            Some(target)
                if target.record_id == record_id
                    && target.property_id.as_deref() == Some(property_id.as_str()) =>
            {
                target.expected_value.clone()
            }
            _ => {
                let message =
                    "The property editor lost its target. Discard this change and open it again.";
                self.error = Some(message.into());
                if let Some(picker) = self.property_picker_mut() {
                    picker.error = Some(message.into());
                    picker.save_failed = true;
                }
                return false;
            }
        };
        let value = if next.is_empty() {
            Value::Null
        } else {
            Value::Array(next.iter().cloned().map(Value::String).collect())
        };
        match self
            .store
            .set_record_value_if_unchanged(&record_id, &property_id, &expected, value)
        {
            Ok(record) => {
                let saved = record
                    .values
                    .get(&property_id)
                    .cloned()
                    .unwrap_or(Value::Null);
                self.sync_record(&record);
                self.error = None;
                if let Some(target) = &mut self.editing_cell {
                    target.expected_value = saved;
                    target.save_failed = false;
                }
                if let Some(picker) = self.property_picker_mut() {
                    picker.multi_selected = next;
                    picker.save_failed = false;
                    picker.error = None;
                }
                true
            }
            Err(error) => {
                self.error = Some(format!("Changes are not saved: {error}"));
                let message = self.error.clone();
                if let Some(picker) = self.property_picker_mut() {
                    picker.multi_selected = next;
                    picker.save_failed = true;
                    picker.error = message;
                    picker.input.focused = true;
                }
                false
            }
        }
    }
    pub fn open_picker_option_editor(&mut self, original_name: &str) {
        let Some(picker) = self.property_picker() else {
            return;
        };
        let (record_id, property_id) = (picker.record_id.clone(), picker.property_id.clone());
        let Some(property) = self
            .record_database(&record_id)
            .and_then(|db| db.properties.iter().find(|p| p.id == property_id))
            .cloned()
        else {
            return;
        };
        let initial_color = moss_core::resolved_option_color(&property, original_name).to_owned();
        let mut name_input = TextEdit::new(original_name);
        name_input.focused = true;
        if let Some(picker) = self.property_picker_mut() {
            picker.input.focused = false;
            picker.option_editor = Some(OptionEditor {
                original_name: original_name.to_owned(),
                name_input,
                color: initial_color,
            });
            picker.error = None;
        }
    }
    pub fn save_picker_option_editor(&mut self, color_override: Option<&str>) -> bool {
        let Some(picker) = self.property_picker() else {
            return false;
        };
        let Some(editor) = &picker.option_editor else {
            return false;
        };
        let (record_id, property_id) = (picker.record_id.clone(), picker.property_id.clone());
        let old_name = editor.original_name.clone();
        let new_name = editor.name_input.text.trim().to_owned();
        let color = color_override
            .map(str::to_owned)
            .unwrap_or_else(|| editor.color.clone());
        if new_name.is_empty() {
            if let Some(picker) = self.property_picker_mut() {
                picker.error = Some("Enter an option name.".into());
            }
            return false;
        }
        let Some((database_id, property)) = self.record_database(&record_id).and_then(|db| {
            db.properties
                .iter()
                .find(|p| p.id == property_id)
                .cloned()
                .map(|p| (db.id.clone(), p))
        }) else {
            return false;
        };
        match self.store.update_property_option(
            &database_id,
            &property,
            &old_name,
            &new_name,
            &color,
        ) {
            Ok(database) => {
                self.sync_database_schema(&database);
                if old_name != new_name {
                    if self.cell.text == old_name {
                        self.cell.replace_text(&new_name);
                    }
                    if let Some(target) = &mut self.editing_cell {
                        if target.expected_value.as_str() == Some(old_name.as_str()) {
                            target.expected_value = Value::String(new_name.clone());
                        } else if let Some(items) = target.expected_value.as_array_mut() {
                            for item in items.iter_mut() {
                                if item.as_str() == Some(old_name.as_str()) {
                                    *item = Value::String(new_name.clone());
                                }
                            }
                        }
                    }
                    if let Some(picker) = self.property_picker_mut() {
                        for item in &mut picker.multi_selected {
                            if item == &old_name {
                                *item = new_name.clone();
                            }
                        }
                    }
                    if let Ok(Some(fresh)) = self.store.get_record(&record_id) {
                        self.sync_record(&fresh);
                    }
                }
                if let Some(picker) = self.property_picker_mut() {
                    picker.option_editor = None;
                    picker.input.focused = true;
                    picker.error = None;
                }
                true
            }
            Err(error) => {
                if let Some(picker) = self.property_picker_mut() {
                    picker.error = Some(format!("Option is not saved: {error}"));
                }
                false
            }
        }
    }
    pub fn set_picker_date_format(&mut self, format: &str) -> bool {
        let Some(picker) = self.property_picker() else {
            return false;
        };
        let (record_id, property_id) = (picker.record_id.clone(), picker.property_id.clone());
        let Some((database_id, property)) = self.record_database(&record_id).and_then(|db| {
            db.properties
                .iter()
                .find(|p| p.id == property_id)
                .cloned()
                .map(|p| (db.id.clone(), p))
        }) else {
            return false;
        };
        match self
            .store
            .set_date_property_format(&database_id, &property, format)
        {
            Ok(database) => {
                self.sync_database_schema(&database);
                if let Some(picker) = self.property_picker_mut() {
                    picker.date_format_pane = false;
                    picker.error = None;
                }
                true
            }
            Err(error) => {
                if let Some(picker) = self.property_picker_mut() {
                    picker.error = Some(format!("Date format is not saved: {error}"));
                }
                false
            }
        }
    }
    pub fn create_and_choose_option(&mut self, name: &str) -> bool {
        let trimmed = name.trim().to_owned();
        if trimmed.is_empty() {
            return false;
        }
        let Some(picker) = self.property_picker() else {
            return false;
        };
        let (record_id, property_id, kind) = (
            picker.record_id.clone(),
            picker.property_id.clone(),
            picker.kind,
        );
        let Some((database_id, property)) = self.record_database(&record_id).and_then(|db| {
            db.properties
                .iter()
                .find(|p| p.id == property_id)
                .cloned()
                .map(|p| (db.id.clone(), p))
        }) else {
            return false;
        };
        match self
            .store
            .add_property_option(&database_id, &property, &trimmed, None)
        {
            Ok(database) => {
                self.sync_database_schema(&database);
                if let Some(picker) = self.property_picker_mut() {
                    picker.input.replace_text("");
                }
                if kind == PropertyKind::MultiSelect {
                    self.toggle_multi_select_value(&trimmed)
                } else {
                    self.choose_property_value(&trimmed)
                }
            }
            Err(error) => {
                if let Some(picker) = self.property_picker_mut() {
                    picker.error = Some(format!("Option is not saved: {error}"));
                }
                false
            }
        }
    }
    pub(crate) fn sync_database_schema(&mut self, database: &moss_core::Database) {
        for tab in &mut self.tabs {
            if tab
                .database
                .as_ref()
                .is_some_and(|db| db.id == database.id)
            {
                tab.database = Some(database.clone());
            }
            if let Some((db, _)) = &mut tab.record_context {
                if db.id == database.id {
                    *db = database.clone();
                }
            }
        }
        if let Some(peek) = self
            .record_peek
            .as_mut()
            .filter(|p| p.database.id == database.id)
        {
            peek.database = database.clone();
        }
    }
    pub fn submit_property_picker(&mut self) {
        let Some(picker) = self.property_picker() else {
            return;
        };
        if picker.kind == PropertyKind::Date {
            self.submit_date_picker();
        } else {
            let kind = picker.kind;
            let selected = picker.selected.unwrap_or(0);
            let next = (kind == PropertyKind::Status && self.record_peek.is_none())
                .then(|| (picker.record_id.clone(), picker.property_id.clone()));
            let options = self.property_options();
            if let Some(value) = options.get(selected).cloned() {
                if kind == PropertyKind::MultiSelect {
                    self.toggle_multi_select_value(&value);
                } else if self.choose_property_value(&value) {
                    if let Some((record_id, property_id)) = next {
                        self.select_next_table_row(&record_id, &property_id);
                    }
                }
            } else if matches!(
                kind,
                PropertyKind::Select | PropertyKind::MultiSelect | PropertyKind::Status
            ) {
                let typed = self
                    .property_picker()
                    .map(|p| p.input.text.trim().to_owned())
                    .unwrap_or_default();
                if !typed.is_empty() {
                    self.create_and_choose_option(&typed);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "property_controls_tests.rs"]
mod tests;
