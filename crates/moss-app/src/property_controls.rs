use crate::state::{CellTarget, State};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use moss_core::PropertyKind;
use rune::TextEdit;
use serde_json::{Value, json};

pub struct PropertyPicker {
    pub record_id: String,
    pub property_id: String,
    pub kind: PropertyKind,
    pub month: NaiveDate,
    pub selected: Option<usize>,
    pub error: Option<String>,
    pub save_failed: bool,
    pub date: Option<DateDraft>,
}

pub struct DateDraft {
    original: Value,
    start_suffix: String,
    end_suffix: String,
    pub has_end: bool,
    pub has_time: bool,
    pending_clear: bool,
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
    pub fn open_property_picker(&mut self, record_id: &str, property_id: &str) {
        self.dismiss_property_picker();
        if self.property_picker.is_some() {
            return;
        }
        let kind = self
            .record_database(record_id)
            .and_then(|d| d.properties.iter().find(|p| p.id == property_id))
            .map(|p| p.kind);
        let Some(kind @ (PropertyKind::Date | PropertyKind::Status | PropertyKind::Select)) = kind
        else {
            return;
        };
        let mut date_draft = None;
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
            self.date_end_input = TextEdit::new(
                &end.as_ref()
                    .map(|(d, _, _)| d.format("%d/%m/%Y").to_string())
                    .unwrap_or_default(),
            );
            self.date_start_time_input = TextEdit::new(
                start
                    .as_ref()
                    .and_then(|(_, t, _)| t.as_deref())
                    .unwrap_or(""),
            );
            self.date_end_time_input = TextEdit::new(
                end.as_ref()
                    .and_then(|(_, t, _)| t.as_deref())
                    .unwrap_or(""),
            );
            date_draft = Some(DateDraft {
                original,
                start_suffix: start.map(|(_, _, s)| s).unwrap_or_default(),
                end_suffix: end.as_ref().map(|(_, _, s)| s.clone()).unwrap_or_default(),
                has_end: end.is_some(),
                has_time,
                pending_clear: false,
            });
            self.error = None;
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
        self.property_picker_input = input;
        self.property_picker = Some(PropertyPicker {
            record_id: record_id.into(),
            property_id: property_id.into(),
            kind,
            month,
            selected: None,
            error: None,
            save_failed: false,
            date: date_draft,
        });
    }
    pub fn dismiss_property_picker(&mut self) {
        self.dismiss_place_picker();
        if self.property_picker.as_ref().is_some_and(|p| p.save_failed) {
            self.property_picker_input.focused = true;
            return;
        }
        if self.property_picker.take().is_some() {
            self.property_picker_input.focused = false;
            self.editing_cell = None;
            self.cell.focused = false;
        }
    }
    pub fn discard_failed_property_change(&mut self) {
        let Some(record_id) = self
            .property_picker
            .as_ref()
            .map(|picker| picker.record_id.clone())
        else {
            return;
        };
        if !self.reload_record_for_discard(&record_id) {
            if let Some(picker) = &mut self.property_picker {
                picker.error = self.error.clone();
            }
            self.property_picker_input.focused = true;
            return;
        }
        if let Some(picker) = &mut self.property_picker {
            picker.save_failed = false;
        }
        self.dismiss_property_picker();
    }
    pub fn retry_property_change(&mut self) {
        if let Some(date) = self.property_picker.as_ref().and_then(|p| p.date.as_ref()) {
            if date.pending_clear {
                self.clear_date_picker();
            } else {
                self.submit_date_picker();
            }
        } else {
            let draft = self.cell.text.clone();
            self.choose_property_value(&draft);
        }
    }
    pub fn date_draft_changed(&mut self) {
        if let Some(draft) = self.property_picker.as_mut().and_then(|p| p.date.as_mut()) {
            draft.pending_clear = false;
        }
    }
    pub fn property_options(&self) -> Vec<String> {
        let Some(picker) = &self.property_picker else {
            return vec![];
        };
        let query = self.property_picker_input.text.to_lowercase();
        self.record_database(&picker.record_id)
            .and_then(|d| d.properties.iter().find(|p| p.id == picker.property_id))
            .and_then(|p| p.config.get("options"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        v.as_str()
                            .or_else(|| v.get("name").and_then(|n| n.as_str()))
                    })
                    .filter(|s| s.to_lowercase().contains(&query))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn choose_property_value(&mut self, value: &str) -> bool {
        let Some(picker) = &self.property_picker else {
            return false;
        };
        if !self.editing_cell.as_ref().is_some_and(|target| {
            target.record_id == picker.record_id
                && target.property_id.as_deref() == Some(picker.property_id.as_str())
        }) {
            let message =
                "The property editor lost its target. Discard this change and open it again.";
            self.error = Some(message.into());
            let picker = self.property_picker.as_mut().unwrap();
            picker.error = Some(message.into());
            picker.save_failed = true;
            return false;
        }
        self.cell.replace_text(value);
        if self.save_cell() {
            if let Some(picker) = &mut self.property_picker {
                picker.save_failed = false;
            }
            self.dismiss_property_picker();
            true
        } else {
            if let Some(picker) = &mut self.property_picker {
                picker.save_failed = true;
                picker.error = self.error.clone();
                self.property_picker_input.focused = true;
            }
            false
        }
    }
    pub fn toggle_date_end(&mut self) {
        if let Some(draft) = self.property_picker.as_mut().and_then(|p| p.date.as_mut()) {
            draft.has_end = !draft.has_end;
            draft.pending_clear = false;
        }
    }
    pub fn toggle_date_time(&mut self) {
        let Some(draft) = self.property_picker.as_mut().and_then(|p| p.date.as_mut()) else {
            return;
        };
        if draft.has_time
            && [draft.start_suffix.as_str(), draft.end_suffix.as_str()]
                .iter()
                .any(|suffix| suffix.contains(['Z', 'z', '+', '-']))
        {
            self.property_picker.as_mut().unwrap().error = Some(
                "The existing time zone must be preserved. Clear the date to remove it.".into(),
            );
            return;
        }
        draft.has_time = !draft.has_time;
        draft.pending_clear = false;
    }
    pub fn choose_calendar_date(&mut self, date: NaiveDate) {
        self.date_draft_changed();
        self.property_picker_input
            .replace_text(&date.format("%d/%m/%Y").to_string());
        self.cell.replace_text(&date.to_string());
        let extended = self
            .property_picker
            .as_ref()
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
        let Some(draft) = self.property_picker.as_ref().and_then(|p| p.date.as_ref()) else {
            return;
        };
        let start_text = self.property_picker_input.text.trim();
        if start_text.is_empty() && !draft.has_end && !draft.has_time {
            self.save_date_value(Value::Null, false);
            return;
        }
        let Some(start) = parse_date(start_text) else {
            self.date_error("Enter a valid start date (DD/MM/YYYY).");
            return;
        };
        let end = if draft.has_end {
            let Some(end) = parse_date(&self.date_end_input.text) else {
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
            let Ok(start_time) =
                NaiveTime::parse_from_str(self.date_start_time_input.text.trim(), "%H:%M")
            else {
                self.date_error("Enter a valid start time (HH:MM).");
                return;
            };
            let end_time = if end.is_some() {
                let Ok(end_time) =
                    NaiveTime::parse_from_str(self.date_end_time_input.text.trim(), "%H:%M")
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
        if let Some(picker) = &mut self.property_picker {
            picker.error = Some(message.into());
        }
    }
    fn save_date_value(&mut self, value: Value, clear: bool) -> bool {
        let Some(picker) = &self.property_picker else {
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
            self.property_picker.as_mut().unwrap().save_failed = true;
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
                self.property_picker.as_mut().unwrap().save_failed = false;
                self.dismiss_property_picker();
                true
            }
            Err(error) => {
                self.error = Some(format!("Changes are not saved: {error}"));
                let picker = self.property_picker.as_mut().unwrap();
                picker.error = self.error.clone();
                picker.save_failed = true;
                picker.date.as_mut().unwrap().pending_clear = clear;
                self.property_picker_input.focused = true;
                false
            }
        }
    }
    pub fn submit_property_picker(&mut self) {
        let Some(picker) = &self.property_picker else {
            return;
        };
        if picker.kind == PropertyKind::Date {
            self.submit_date_picker();
        } else {
            let selected = picker.selected.unwrap_or(0);
            let next = (picker.kind == PropertyKind::Status && self.record_peek.is_none())
                .then(|| (picker.record_id.clone(), picker.property_id.clone()));
            if let Some(value) = self.property_options().get(selected).cloned() {
                if self.choose_property_value(&value) {
                    if let Some((record_id, property_id)) = next {
                        self.select_next_table_row(&record_id, &property_id);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "property_controls_tests.rs"]
mod tests;
