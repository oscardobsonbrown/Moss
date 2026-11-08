use crate::{
    Database, Property, PropertyKind, Record, Store, properties::with_database, store::invalid,
};
use rusqlite::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const KEY: &str = "record_page_layout";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordLayout {
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub visibility: BTreeMap<String, String>,
}

pub fn saved(db: &Database) -> Option<&Value> {
    db.properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)?
        .config
        .get(KEY)
}

pub fn layout(db: &Database) -> Option<RecordLayout> {
    let mut layout: RecordLayout = match saved(db) {
        Some(value) => serde_json::from_value(value.clone()).ok(),
        None => Some(RecordLayout::default()),
    }?;
    let ids: BTreeSet<_> = db
        .properties
        .iter()
        .filter(|p| p.kind != PropertyKind::Title)
        .map(|p| p.id.as_str())
        .collect();
    let mut seen = BTreeSet::new();
    layout
        .order
        .retain(|id| ids.contains(id.as_str()) && seen.insert(id.clone()));
    layout.visibility.retain(|id, _| ids.contains(id.as_str()));
    if layout
        .visibility
        .values()
        .any(|mode| !matches!(mode.as_str(), "always" | "hide_empty" | "hidden"))
    {
        return None;
    }
    Some(layout)
}

pub fn properties(db: &Database) -> Vec<&Property> {
    let mut result: Vec<_> = db
        .properties
        .iter()
        .filter(|p| p.kind != PropertyKind::Title)
        .collect();
    if let Some(layout) = layout(db) {
        let positions: BTreeMap<_, _> = layout
            .order
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect();
        result.sort_by_key(|p| positions.get(p.id.as_str()).copied().unwrap_or(usize::MAX));
    }
    result
}

pub fn visible(db: &Database, property: &Property, record: &Record) -> bool {
    layout(db).unwrap_or_default().visible(property, record)
}

impl RecordLayout {
    pub fn visible(&self, property: &Property, record: &Record) -> bool {
        match self.visibility.get(&property.id).map(String::as_str) {
            Some("hidden") => false,
            Some("hide_empty") => record.values.get(&property.id).is_some_and(|v| match v {
                Value::Null => false,
                Value::String(s) => !s.is_empty(),
                Value::Array(a) => !a.is_empty(),
                _ => true,
            }),
            _ => true,
        }
    }
}

impl Store {
    pub fn set_record_layout(
        &mut self,
        database_id: &str,
        expected: Option<&Value>,
        layout: &RecordLayout,
    ) -> Result<Database> {
        with_database(self, database_id, |_, db, _| {
            if saved(db) != expected {
                return Err(invalid("record layout changed in another window"));
            }
            let ids: BTreeSet<_> = db
                .properties
                .iter()
                .filter(|p| p.kind != PropertyKind::Title)
                .map(|p| p.id.as_str())
                .collect();
            let mut order = BTreeSet::new();
            if layout
                .order
                .iter()
                .any(|id| !ids.contains(id.as_str()) || !order.insert(id))
                || layout.visibility.iter().any(|(id, value)| {
                    !ids.contains(id.as_str())
                        || !matches!(value.as_str(), "always" | "hide_empty" | "hidden")
                })
            {
                return Err(invalid("invalid record layout"));
            }
            let title = db
                .properties
                .iter_mut()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("title property missing"))?;
            if title.config.is_null() {
                title.config = json!({});
            }
            let config = title
                .config
                .as_object_mut()
                .ok_or_else(|| invalid("invalid title property configuration"))?;
            config.insert(
                KEY.into(),
                serde_json::to_value(layout).map_err(|_| invalid("invalid record layout"))?,
            );
            Ok(())
        })
    }
}
