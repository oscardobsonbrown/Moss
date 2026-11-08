use crate::state::State;
use moss_core::{PlaceValue, PropertyKind, ViewKind};
use rune::{TextEdit, Tx};
use serde_json::Value;
use std::time::{Duration, Instant};

pub struct PlacePicker {
    pub record_id: String,
    pub property_id: String,
    pub anchor: String,
    pub expected: Value,
    pub error: Option<String>,
    pub search_at: Option<Instant>,
}

impl State {
    pub fn open_place_picker(&mut self, record_id: &str, property_id: &str) {
        self.dismiss_property_picker();
        if self.property_picker.is_some() || !self.finish_cell(false) {
            return;
        }
        if !self.record_database(record_id).is_some_and(|db| {
            db.properties
                .iter()
                .any(|p| p.id == property_id && p.kind == PropertyKind::Place)
        }) {
            return;
        }
        let record = match self.store.get_record(record_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                self.error = Some("Record was not found.".into());
                return;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let anchor = if self
            .record_peek
            .as_ref()
            .is_some_and(|p| p.record.id == record_id)
        {
            format!("peek-property-{property_id}")
        } else if self
            .tab()
            .and_then(|tab| tab.record_context.as_ref())
            .is_some_and(|(_, record)| record.id == record_id)
        {
            format!("page-place-{property_id}")
        } else if self
            .tab()
            .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
            .is_some_and(|v| v.kind == ViewKind::Board)
        {
            format!("board-value-{record_id}-{property_id}")
        } else if let Some(index) = self
            .tab()
            .and_then(|tab| tab.records.iter().position(|r| r.id == record_id))
        {
            match self
                .tab()
                .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
                .map(|view| view.kind)
            {
                Some(ViewKind::Gallery) => format!("gallery-value-{index}-{property_id}"),
                Some(ViewKind::List) => format!("list-value-{index}-{property_id}"),
                _ => format!("cell-{index}-{property_id}"),
            }
        } else {
            return;
        };
        let expected = record
            .values
            .get(property_id)
            .cloned()
            .unwrap_or(Value::Null);
        let map = &mut self.database_views.map;
        map.search_changed();
        map.place_query = TextEdit::new(&crate::state::display_value(&expected));
        map.place_query.focused = true;
        map.search_selected = 0;
        self.place_picker = Some(PlacePicker {
            record_id: record_id.into(),
            property_id: property_id.into(),
            anchor,
            expected,
            error: None,
            search_at: None,
        });
        self.error = None;
    }

    pub fn dismiss_place_picker(&mut self) {
        if self.place_picker.take().is_some() {
            let map = &mut self.database_views.map;
            map.search_changed();
            map.place_query.focused = false;
        }
    }

    pub fn save_place_choice(&mut self, place: Option<PlaceValue>, as_text: bool) {
        let Some(picker) = &self.place_picker else {
            return;
        };
        let (record, property, expected) = (
            picker.record_id.clone(),
            picker.property_id.clone(),
            picker.expected.clone(),
        );
        let result = if as_text {
            self.store.set_record_place_text_if_unchanged(
                &record,
                &property,
                &expected,
                &self.database_views.map.place_query.text,
            )
        } else {
            self.store
                .set_record_place_if_unchanged(&record, &property, &expected, place)
        };
        match result {
            Ok(record) => {
                self.sync_record(&record);
                self.database_views.map.invalidate();
                self.database_views.board.invalidate();
                self.dismiss_place_picker();
                self.error = None;
            }
            Err(error) => {
                if let Some(picker) = &mut self.place_picker {
                    picker.error = Some(format!("Place is not saved: {error}"));
                }
            }
        }
    }
}

pub fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let Some(picker) = &mut s.place_picker else {
        return false;
    };
    let Some(at) = picker.search_at else {
        return false;
    };
    if let Some(wait) = at.checked_duration_since(Instant::now()) {
        tx.wake_after(wait);
        return false;
    }
    picker.search_at = None;
    if !s.database_views.map.place_query.text.trim().is_empty() {
        s.database_views.map.search();
        tx.wake_after(Duration::from_millis(50));
        return true;
    }
    false
}
