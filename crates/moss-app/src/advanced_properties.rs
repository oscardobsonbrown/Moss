use crate::state::State;
use moss_core::{Database, Property, PropertyKind, Record};
use rune::TextEdit;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub struct AdvancedProperty {
    pub expression: TextEdit,
    pub database_search: TextEdit,
    pub target_database_id: Option<String>,
    pub target_database_name: Option<String>,
    pub relation_property_id: Option<String>,
    pub target_property_id: Option<String>,
    pub operation: String,
    pub databases: Vec<Database>,
    pub target_properties: Vec<Property>,
    pub error: Option<String>,
    pub relation_picker: Option<RelationPicker>,
    pub relation_search: TextEdit,
}

pub struct RelationPicker {
    pub record_id: String,
    pub property_id: String,
    pub expected_property: Property,
    pub target_database_id: String,
    pub selected: Vec<String>,
    pub selected_names: BTreeMap<String, String>,
    pub expected: Value,
    pub candidates: Vec<Record>,
    pub error: Option<String>,
    pub save_failed: bool,
}

impl Default for AdvancedProperty {
    fn default() -> Self {
        Self {
            expression: TextEdit::new(""),
            database_search: TextEdit::new(""),
            target_database_id: None,
            target_database_name: None,
            relation_property_id: None,
            target_property_id: None,
            operation: "count".into(),
            databases: Vec::new(),
            target_properties: Vec::new(),
            error: None,
            relation_picker: None,
            relation_search: TextEdit::new(""),
        }
    }
}

impl State {
    pub fn reset_advanced_property(&mut self) {
        let config = self
            .database_property_editor
            .as_ref()
            .and_then(|e| e.property.as_ref())
            .map(|p| p.config.clone())
            .unwrap_or(Value::Null);
        let value = |key: &str| config.get(key).and_then(Value::as_str).map(str::to_owned);
        self.advanced_property = AdvancedProperty {
            expression: TextEdit::new(value("expression").as_deref().unwrap_or("")),
            target_database_id: value("target_database_id"),
            relation_property_id: value("relation_property_id"),
            target_property_id: value("target_property_id"),
            operation: value("operation").unwrap_or_else(|| "count".into()),
            ..Default::default()
        };
        let kind = self
            .database_property_editor
            .as_ref()
            .map(|editor| editor.kind);
        if kind == Some(PropertyKind::Relation) {
            self.search_advanced_databases();
            if let Some(id) = &self.advanced_property.target_database_id {
                match self.store.get_database(id) {
                    Ok(Some(db)) => self.advanced_property.target_database_name = Some(db.name),
                    Ok(None) => {
                        self.advanced_property.error =
                            Some("Related database is not available.".into())
                    }
                    Err(error) => {
                        self.advanced_property.error =
                            Some(format!("Related database is not loaded: {error}"))
                    }
                }
            }
        }
        if kind == Some(PropertyKind::Rollup) {
            self.refresh_advanced_target_properties();
        }
    }

    pub fn search_advanced_databases(&mut self) {
        match self.store.search_relation_databases(
            &self.workspace.id,
            &self.advanced_property.database_search.text,
            50,
        ) {
            Ok(databases) => {
                self.advanced_property.databases = databases;
                self.advanced_property.error = None;
            }
            Err(error) => {
                self.advanced_property.error = Some(format!("Databases are not loaded: {error}"))
            }
        }
    }

    pub fn refresh_advanced_target_properties(&mut self) {
        let target = self
            .advanced_property
            .relation_property_id
            .as_deref()
            .and_then(|id| {
                self.tab()?
                    .database
                    .as_ref()?
                    .properties
                    .iter()
                    .find(|p| p.id == id)
            })
            .and_then(|p| p.config.get("target_database_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.advanced_property.target_properties.clear();
        if let Some(id) = target {
            match self.store.get_database(&id) {
                Ok(Some(db)) => self.advanced_property.target_properties = db.properties,
                Ok(None) => {
                    self.advanced_property.error = Some("Related database is not available.".into())
                }
                Err(error) => {
                    self.advanced_property.error =
                        Some(format!("Properties are not loaded: {error}"))
                }
            }
        }
    }

    pub fn save_advanced_property(&mut self) -> bool {
        let Some(editor) = self.database_property_editor.as_ref() else {
            return false;
        };
        let name = self.database_property_input.text.trim();
        if name.is_empty() {
            self.database_property_editor.as_mut().unwrap().error =
                Some("Enter a property name.".into());
            return false;
        }
        let draft = &self.advanced_property;
        let config = match editor.kind {
            PropertyKind::Formula => json!({"expression": draft.expression.text}),
            PropertyKind::Relation => json!({"target_database_id": draft.target_database_id}),
            PropertyKind::Rollup => {
                json!({"relation_property_id": draft.relation_property_id, "target_property_id": draft.target_property_id, "operation": draft.operation})
            }
            _ => return false,
        };
        let relative = editor
            .insert_relative
            .as_ref()
            .map(|(id, after)| (id.as_str(), *after));
        match self.store.save_advanced_property(
            &editor.database_id,
            editor.property.as_ref(),
            name,
            editor.kind,
            config,
            relative,
        ) {
            Ok(db) => {
                self.apply_database_property_change(db, None);
                true
            }
            Err(error) => {
                self.database_property_editor.as_mut().unwrap().error =
                    Some(format!("Property is not saved: {error}"));
                false
            }
        }
    }
}

impl State {
    pub fn open_relation_picker(&mut self, record_id: &str, property_id: &str) {
        self.close_relation_picker();
        if self.advanced_property.relation_picker.is_some() {
            return;
        }
        if !self.finish_cell(false) {
            return;
        }
        self.dismiss_property_picker();
        if self.property_picker.is_some() {
            return;
        }
        let Some(property) = self
            .record_database(record_id)
            .and_then(|db| {
                db.properties
                    .iter()
                    .find(|p| p.id == property_id && p.kind == PropertyKind::Relation)
            })
            .cloned()
        else {
            return;
        };
        let Some(target) = property
            .config
            .get("target_database_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            self.error = Some("The relation has no target database.".into());
            return;
        };
        let record = match self.store.get_record(record_id) {
            Ok(Some(record)) if record.deleted_at.is_none() => record,
            Ok(_) => {
                self.error = Some("Record is not available.".into());
                return;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let expected = record
            .values
            .get(property_id)
            .cloned()
            .unwrap_or(Value::Null);
        let selected: Vec<String> = match &expected {
            Value::Null => Vec::new(),
            Value::Array(items) if items.iter().all(Value::is_string) => items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => {
                self.error = Some("This relation value needs a different editor.".into());
                return;
            }
        };
        let selected_names = match self.store.relation_records_by_ids(
            &target,
            &selected.iter().take(50).cloned().collect::<Vec<_>>(),
        ) {
            Ok(records) => records
                .into_iter()
                .map(|record| (record.id, record.title))
                .collect(),
            Err(error) => {
                self.error = Some(format!("Related records are not loaded: {error}"));
                return;
            }
        };
        self.advanced_property.relation_search = TextEdit::new("");
        self.advanced_property.relation_search.focused = true;
        self.advanced_property.relation_picker = Some(RelationPicker {
            record_id: record_id.into(),
            property_id: property_id.into(),
            expected_property: property,
            target_database_id: target,
            selected,
            selected_names,
            expected,
            candidates: Vec::new(),
            error: None,
            save_failed: false,
        });
        self.search_relation_candidates();
    }

    pub fn search_relation_candidates(&mut self) {
        let Some(picker) = &self.advanced_property.relation_picker else {
            return;
        };
        let result = self.store.search_relation_records(
            &picker.target_database_id,
            &self.advanced_property.relation_search.text,
            50,
        );
        let picker = self.advanced_property.relation_picker.as_mut().unwrap();
        match result {
            Ok(records) => {
                for record in &records {
                    if picker.selected.contains(&record.id) {
                        picker
                            .selected_names
                            .insert(record.id.clone(), record.title.clone());
                    }
                }
                picker.candidates = records;
                if !picker.save_failed {
                    picker.error = None;
                }
            }
            Err(error) => picker.error = Some(format!("Records are not loaded: {error}")),
        }
    }

    pub fn toggle_relation_candidate(&mut self, id: &str) {
        let Some(picker) = &mut self.advanced_property.relation_picker else {
            return;
        };
        if let Some(index) = picker.selected.iter().position(|selected| selected == id) {
            picker.selected.remove(index);
        } else if let Some(record) = picker.candidates.iter().find(|record| record.id == id) {
            picker
                .selected_names
                .insert(id.into(), record.title.clone());
            picker.selected.push(id.into());
        }
    }

    pub fn save_relation_picker(&mut self) -> bool {
        let Some(picker) = &self.advanced_property.relation_picker else {
            return false;
        };
        let result = self.store.set_relation_value_if_unchanged(
            &picker.record_id,
            &picker.expected_property,
            &picker.expected,
            &picker.selected,
        );
        match result {
            Ok(record) => {
                self.sync_record(&record);
                self.advanced_property.relation_picker = None;
                self.advanced_property.relation_search.focused = false;
                true
            }
            Err(error) => {
                let picker = self.advanced_property.relation_picker.as_mut().unwrap();
                picker.error = Some(format!("Relation is not saved: {error}"));
                picker.save_failed = true;
                false
            }
        }
    }
    pub fn discard_relation_picker(&mut self) {
        self.advanced_property.relation_picker = None;
        self.advanced_property.relation_search.focused = false;
    }
    pub fn close_relation_picker(&mut self) {
        if self
            .advanced_property
            .relation_picker
            .as_ref()
            .is_some_and(|p| p.save_failed)
        {
            return;
        }
        self.discard_relation_picker();
    }
}
