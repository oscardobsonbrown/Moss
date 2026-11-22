use crate::{
    Database, FilterExpr, Page, PageKind, Property, PropertyKind, Record, Store, new_id, search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};

pub const OPTION_COLORS: [&str; 10] = [
    "default", "gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red",
];

pub const AUTO_OPTION_COLORS: [&str; 8] = [
    "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red",
];

pub const DATE_FORMATS: [(&str, &str); 6] = [
    ("full", "Full date"),
    ("month_day_year_full", "Month Day, Year"),
    ("day_month_year", "Day/Month/Year"),
    ("month_day_year", "Month/Day/Year"),
    ("year_month_day", "Year/Month/Day"),
    ("relative", "Relative"),
];

pub fn is_valid_date_format(format: &str) -> bool {
    DATE_FORMATS.iter().any(|(id, _)| *id == format)
}

pub fn default_status_option_color(name: &str) -> Option<&'static str> {
    match name {
        "Not started" => Some("gray"),
        "In progress" => Some("blue"),
        "Done" => Some("green"),
        _ => None,
    }
}

pub fn is_valid_option_color(color: &str) -> bool {
    OPTION_COLORS.contains(&color)
}

pub fn option_name(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("name").and_then(Value::as_str))
}

fn fnv1a_hash(parts: &[&str]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn auto_option_color(property_id: &str, option_index: usize) -> &'static str {
    let base = (fnv1a_hash(&[property_id]) as usize) % AUTO_OPTION_COLORS.len();
    AUTO_OPTION_COLORS[(base + option_index) % AUTO_OPTION_COLORS.len()]
}

pub fn resolved_option_color<'a>(property: &'a Property, name: &str) -> &'a str {
    if let Some(options) = property.config.get("options").and_then(Value::as_array) {
        if let Some((index, entry)) = options
            .iter()
            .enumerate()
            .find(|(_, v)| option_name(v) == Some(name))
        {
            if let Some(color) = entry
                .get("color")
                .and_then(Value::as_str)
                .filter(|c| is_valid_option_color(c))
            {
                return color;
            }
            if property.kind == PropertyKind::Status {
                if let Some(color) = default_status_option_color(name) {
                    return color;
                }
            }
            return auto_option_color(&property.id, index);
        }
    }
    if property.kind == PropertyKind::Status {
        if let Some(color) = default_status_option_color(name) {
            return color;
        }
    }
    let idx = (fnv1a_hash(&[&property.id, name]) as usize) % AUTO_OPTION_COLORS.len();
    AUTO_OPTION_COLORS[idx]
}

pub(crate) fn pick_unused_option_color(
    property_id: &str,
    existing: &[Value],
) -> &'static str {
    let used: std::collections::HashSet<&str> = existing
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            entry
                .get("color")
                .and_then(Value::as_str)
                .filter(|c| is_valid_option_color(c))
                .unwrap_or_else(|| auto_option_color(property_id, index))
        })
        .collect();
    let base = (fnv1a_hash(&[property_id]) as usize) % AUTO_OPTION_COLORS.len();
    for offset in 0..AUTO_OPTION_COLORS.len() {
        let candidate = AUTO_OPTION_COLORS[(base + existing.len() + offset) % AUTO_OPTION_COLORS.len()];
        if !used.contains(candidate) {
            return candidate;
        }
    }
    auto_option_color(property_id, existing.len())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyOption {
    pub name: String,
    pub color: String,
}

pub fn property_options(property: &Property) -> Vec<PropertyOption> {
    property
        .config
        .get("options")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    let name = option_name(entry)?.to_owned();
                    let color = entry
                        .get("color")
                        .and_then(Value::as_str)
                        .filter(|c| is_valid_option_color(c))
                        .or_else(|| {
                            (property.kind == PropertyKind::Status)
                                .then(|| default_status_option_color(&name))
                                .flatten()
                        })
                        .unwrap_or_else(|| auto_option_color(&property.id, index))
                        .to_owned();
                    Some(PropertyOption { name, color })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn normalize_options(options: &[String]) -> Result<Vec<String>> {
    if options.is_empty() {
        return Err(invalid("select requires at least one option"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut values = Vec::with_capacity(options.len());
    for option in options {
        let option = option.trim();
        if option.is_empty() {
            return Err(invalid("select option cannot be empty"));
        }
        if !seen.insert(option.to_owned()) {
            return Err(invalid("select options must be unique"));
        }
        values.push(option.to_owned());
    }
    Ok(values)
}

pub(crate) fn snapshot_formula_bindings(db: &mut Database) {
    let schema = db.properties.clone();
    for property in &mut db.properties {
        if property.kind == PropertyKind::Formula && property.config.get("_bindings").is_none() {
            if let Some(source) = property.config.get("expression").and_then(Value::as_str) {
                if let Ok(bindings) = crate::formula::bindings(source, &schema) {
                    if let Some(config) = property.config.as_object_mut() {
                        config.insert("_bindings".into(), json!(bindings));
                    }
                }
            }
        }
    }
}

pub(crate) fn validate_relation_target(
    tx: &Transaction<'_>,
    workspace_id: &str,
    config: &Value,
) -> Result<()> {
    let target = config
        .get("target_database_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("relation target missing"))?;
    let raw: String = tx
        .query_row("SELECT data FROM databases WHERE id=?1", [target], |row| {
            row.get(0)
        })
        .optional()?
        .ok_or_else(|| invalid("relation target missing"))?;
    let target_db: Database = decode(raw)?;
    let target_page =
        search::page(tx, &target_db.page_id)?.ok_or_else(|| invalid("relation target missing"))?;
    if target_page.deleted_at.is_some()
        || target_page.kind != PageKind::Database
        || target_page.workspace_id != workspace_id
    {
        return Err(invalid("invalid relation target"));
    }
    Ok(())
}

pub(crate) fn validate_rollup_operation(config: &Value) -> Result<()> {
    let op = config
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !matches!(
        op,
        "original"
            | "unique"
            | "count"
            | "count_values"
            | "count_empty"
            | "percent_empty"
            | "sum"
            | "average"
            | "min"
            | "max"
    ) {
        return Err(invalid("unsupported rollup operation"));
    }
    Ok(())
}

pub(crate) fn update_record_values(
    tx: &Transaction<'_>,
    owner: &Page,
    record: &Record,
) -> Result<()> {
    let values = encode(&record.values)?;
    tx.execute(
        "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
        params![values, encode(record)?, record.id],
    )?;
    if record.deleted_at.is_none() {
        let target = match &record.page_id {
            Some(id) => search::page(tx, id)?.ok_or_else(|| invalid("record page missing"))?,
            None => owner.clone(),
        };
        search::index(
            tx,
            &format!("record:{}", record.id),
            &target,
            &format!("{} {}", record.title, values),
        )?;
    }
    Ok(())
}

pub(crate) fn refresh_all_records(tx: &Transaction<'_>, database_id: &str) -> Result<()> {
    let mut after = String::new();
    loop {
        let ids: Vec<String> = {
            let mut q = tx.prepare(
                "SELECT id FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256",
            )?;
            let rows = q.query_map(params![database_id, after], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>>>()?
        };
        if ids.is_empty() {
            break;
        }
        after = ids.last().unwrap().clone();
        crate::calculation::refresh(tx, &ids)?;
    }
    Ok(())
}

pub(crate) fn colored_options(property_id: &str, options: &[String]) -> Vec<Value> {
    options
        .iter()
        .enumerate()
        .map(|(index, name)| {
            json!({
                "name": name,
                "color": auto_option_color(property_id, index),
            })
        })
        .collect()
}

pub(crate) fn with_database(
    store: &mut Store,
    id: &str,
    edit: impl FnOnce(&Transaction<'_>, &mut Database, &Page) -> Result<()>,
) -> Result<Database> {
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let raw: String = tx
        .query_row("SELECT data FROM databases WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?
        .ok_or_else(|| invalid("database missing"))?;
    let mut db: Database = decode(raw)?;
    let owner = search::page(&tx, &db.page_id)?.ok_or_else(|| invalid("database page missing"))?;
    if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
        return Err(invalid("invalid database page"));
    }
    edit(&tx, &mut db, &owner)?;
    tx.execute(
        "UPDATE databases SET data=?1 WHERE id=?2",
        params![encode(&db)?, db.id],
    )?;
    tx.commit()?;
    Ok(db)
}

pub(crate) fn name(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid("property name cannot be empty"))
    } else {
        Ok(value)
    }
}
pub(crate) fn existing(db: &Database, expected: &Property) -> Result<usize> {
    let index = db
        .properties
        .iter()
        .position(|p| p.id == expected.id)
        .ok_or_else(|| invalid("property missing"))?;
    if db.properties[index] != *expected {
        return Err(invalid("property changed in another editor"));
    }
    Ok(index)
}
fn prune_filter(filter: FilterExpr, id: &str) -> Option<FilterExpr> {
    match filter {
        FilterExpr::Rule(rule) => (rule.property_id != id).then_some(FilterExpr::Rule(rule)),
        FilterExpr::All(items) => {
            let children: Vec<_> = items
                .into_iter()
                .filter_map(|item| prune_filter(item, id))
                .collect();
            (!children.is_empty()).then_some(FilterExpr::All(children))
        }
        FilterExpr::Any(items) => {
            let children: Vec<_> = items
                .into_iter()
                .filter_map(|item| prune_filter(item, id))
                .collect();
            (!children.is_empty()).then_some(FilterExpr::Any(children))
        }
    }
}
pub(crate) fn prune_view(view: &mut crate::DatabaseView, id: &str) {
    view.query.filter = view
        .query
        .filter
        .take()
        .and_then(|expr| prune_filter(expr, id));
    view.query.sorts.retain(|sort| sort.property_id != id);
    if view.query.group_by.as_deref() == Some(id) {
        view.query.group_by = None;
    }
    if let Some(config) = view.config.as_object_mut() {
        for key in [
            "date_property_id",
            "map_place_property",
            "group_property_id",
            "group_by",
            "freeze_through",
        ] {
            if config.get(key).and_then(Value::as_str) == Some(id) {
                config.remove(key);
            }
        }
        if let Some(Value::Array(ids)) = config.get_mut("hidden_property_ids") {
            ids.retain(|value| value.as_str() != Some(id));
        }
        if let Some(Value::Object(calculations)) = config.get_mut("calculations") {
            calculations.remove(id);
        }
        if let Some(Value::Object(widths)) = config.get_mut("column_widths") {
            widths.remove(id);
        }
    }
}

impl Store {
    pub fn add_property(
        &mut self,
        database_id: &str,
        property_name: &str,
        kind: PropertyKind,
    ) -> Result<Database> {
        let property_name = name(property_name)?.to_owned();
        let config = match kind {
            PropertyKind::Status => json!({"options": ["Not started", "In progress", "Done"]}),
            PropertyKind::Select | PropertyKind::MultiSelect => json!({"options": []}),
            PropertyKind::Text
            | PropertyKind::Number
            | PropertyKind::Date
            | PropertyKind::Checkbox
            | PropertyKind::Url
            | PropertyKind::Email
            | PropertyKind::Phone
            | PropertyKind::Place => json!({}),
            _ => return Err(invalid("unsupported property type")),
        };
        with_database(self, database_id, |_, db, _| {
            db.properties.push(Property {
                id: new_id(),
                name: property_name,
                kind,
                config,
            });
            Ok(())
        })
    }
    pub fn add_select_property(
        &mut self,
        database_id: &str,
        property_name: &str,
        options: &[String],
    ) -> Result<Database> {
        self.add_options_property(database_id, property_name, PropertyKind::Select, options)
    }
    pub fn add_options_property(
        &mut self,
        database_id: &str,
        property_name: &str,
        kind: PropertyKind,
        options: &[String],
    ) -> Result<Database> {
        if !matches!(kind, PropertyKind::Select | PropertyKind::MultiSelect) {
            return Err(invalid("unsupported options property type"));
        }
        let property_name = name(property_name)?.to_owned();
        let values = normalize_options(options)?;
        with_database(self, database_id, |_, db, _| {
            let id = new_id();
            let colored = colored_options(&id, &values);
            db.properties.push(Property {
                id,
                name: property_name,
                kind,
                config: json!({"options": colored}),
            });
            Ok(())
        })
    }
    pub fn insert_property_relative(
        &mut self,
        database_id: &str,
        property_name: &str,
        kind: PropertyKind,
        options: &[String],
        anchor_id: &str,
        after: bool,
    ) -> Result<Database> {
        let property_name = name(property_name)?.to_owned();
        let id = new_id();
        let config = match kind {
            PropertyKind::Select | PropertyKind::MultiSelect => {
                let values = normalize_options(options)?;
                json!({"options": colored_options(&id, &values)})
            }
            PropertyKind::Status => json!({"options": ["Not started", "In progress", "Done"]}),
            PropertyKind::Text
            | PropertyKind::Number
            | PropertyKind::Date
            | PropertyKind::Checkbox
            | PropertyKind::Url
            | PropertyKind::Email
            | PropertyKind::Phone => json!({}),
            _ => return Err(invalid("unsupported property type")),
        };
        with_database(self, database_id, |_, db, _| {
            let index = db
                .properties
                .iter()
                .position(|property| property.id == anchor_id)
                .ok_or_else(|| invalid("anchor property missing"))?;
            let position = index + usize::from(after);
            db.properties.insert(
                position,
                Property {
                    id,
                    name: property_name,
                    kind,
                    config,
                },
            );
            Ok(())
        })
    }
    pub fn set_property_option_color(
        &mut self,
        database_id: &str,
        expected: &Property,
        target_name: &str,
        color: &str,
    ) -> Result<Database> {
        if !matches!(
            expected.kind,
            PropertyKind::Select | PropertyKind::MultiSelect | PropertyKind::Status
        ) {
            return Err(invalid("this property does not have colored options"));
        }
        if !is_valid_option_color(color) {
            return Err(invalid("invalid option color"));
        }
        let target_name = target_name.trim();
        if target_name.is_empty() {
            return Err(invalid("option name cannot be empty"));
        }
        with_database(self, database_id, |_, db, _| {
            let index = existing(db, expected)?;
            let property = &mut db.properties[index];
            if !property.config.is_object() {
                property.config = json!({});
            }
            let options = property
                .config
                .as_object_mut()
                .unwrap()
                .entry("options")
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or_else(|| invalid("property options are invalid"))?;
            if let Some(entry) = options
                .iter_mut()
                .find(|entry| option_name(entry) == Some(target_name))
            {
                *entry = json!({"name": target_name, "color": color});
            } else {
                options.push(json!({"name": target_name, "color": color}));
            }
            Ok(())
        })
    }
    pub fn add_property_option(
        &mut self,
        database_id: &str,
        expected: &Property,
        target_name: &str,
        color: Option<&str>,
    ) -> Result<Database> {
        if !matches!(
            expected.kind,
            PropertyKind::Select | PropertyKind::MultiSelect | PropertyKind::Status
        ) {
            return Err(invalid("this property does not accept new options"));
        }
        let target_name = target_name.trim();
        if target_name.is_empty() {
            return Err(invalid("option name cannot be empty"));
        }
        if let Some(color) = color {
            if !is_valid_option_color(color) {
                return Err(invalid("invalid option color"));
            }
        }
        with_database(self, database_id, |_, db, _| {
            let index = existing(db, expected)?;
            let property = &mut db.properties[index];
            let property_id = property.id.clone();
            let kind = property.kind;
            if !property.config.is_object() {
                property.config = json!({});
            }
            let options = property
                .config
                .as_object_mut()
                .unwrap()
                .entry("options")
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or_else(|| invalid("property options are invalid"))?;
            if kind == PropertyKind::Status {
                for entry in options.iter_mut() {
                    if let Some(raw) = entry.as_str().map(str::to_owned) {
                        let preserved = default_status_option_color(&raw).unwrap_or("gray");
                        *entry = json!({"name": raw, "color": preserved});
                    }
                }
            }
            if options
                .iter()
                .any(|entry| option_name(entry) == Some(target_name))
            {
                return Ok(());
            }
            let chosen = color.unwrap_or_else(|| pick_unused_option_color(&property_id, options));
            options.push(json!({"name": target_name, "color": chosen}));
            Ok(())
        })
    }
    pub fn update_property_option(
        &mut self,
        database_id: &str,
        expected: &Property,
        old_name: &str,
        new_name: &str,
        color: &str,
    ) -> Result<Database> {
        if !matches!(
            expected.kind,
            PropertyKind::Select | PropertyKind::MultiSelect | PropertyKind::Status
        ) {
            return Err(invalid("this property does not have editable options"));
        }
        if !is_valid_option_color(color) {
            return Err(invalid("invalid option color"));
        }
        let old_name = old_name.trim();
        let new_name = new_name.trim();
        if old_name.is_empty() || new_name.is_empty() {
            return Err(invalid("option name cannot be empty"));
        }
        let renamed = old_name != new_name;
        with_database(self, database_id, |tx, db, owner| {
            let index = existing(db, expected)?;
            let property = &mut db.properties[index];
            let kind = property.kind;
            if !property.config.is_object() {
                property.config = json!({});
            }
            let options = property
                .config
                .as_object_mut()
                .unwrap()
                .entry("options")
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or_else(|| invalid("property options are invalid"))?;
            if kind == PropertyKind::Status {
                for entry in options.iter_mut() {
                    if let Some(raw) = entry.as_str().map(str::to_owned) {
                        let preserved = default_status_option_color(&raw).unwrap_or("gray");
                        *entry = json!({"name": raw, "color": preserved});
                    }
                }
            }
            let pos = options
                .iter()
                .position(|entry| option_name(entry) == Some(old_name))
                .ok_or_else(|| invalid("option was not found"))?;
            if renamed
                && options
                    .iter()
                    .enumerate()
                    .any(|(i, entry)| i != pos && option_name(entry) == Some(new_name))
            {
                return Err(invalid("an option with that name already exists"));
            }
            options[pos] = json!({"name": new_name, "color": color});
            if !renamed {
                return Ok(());
            }
            let mut after = String::new();
            loop {
                let batch: Vec<Record> = {
                    let mut query = tx.prepare("SELECT data FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256")?;
                    let rows =
                        query.query_map(params![database_id, after], |row| decode(row.get(0)?))?;
                    rows.collect::<Result<Vec<_>>>()?
                };
                if batch.is_empty() {
                    break;
                }
                for mut record in batch {
                    after = record.id.clone();
                    let changed = match record.values.get_mut(&expected.id) {
                        Some(Value::String(val)) if val == old_name => {
                            *val = new_name.to_owned();
                            true
                        }
                        Some(Value::Array(items)) => {
                            let mut hit = false;
                            for item in items.iter_mut() {
                                if item.as_str() == Some(old_name) {
                                    *item = Value::String(new_name.to_owned());
                                    hit = true;
                                }
                            }
                            hit
                        }
                        _ => false,
                    };
                    if changed {
                        update_record_values(tx, owner, &record)?;
                    }
                }
            }
            tx.execute(
                "UPDATE databases SET data=?1 WHERE id=?2",
                params![encode(db)?, db.id],
            )?;
            refresh_all_records(tx, database_id)
        })
    }
    pub fn set_date_property_format(
        &mut self,
        database_id: &str,
        expected: &Property,
        format: &str,
    ) -> Result<Database> {
        if expected.kind != PropertyKind::Date {
            return Err(invalid("only date properties have a date format"));
        }
        if !is_valid_date_format(format) {
            return Err(invalid("invalid date format"));
        }
        with_database(self, database_id, |_, db, _| {
            let index = existing(db, expected)?;
            let property = &mut db.properties[index];
            if !property.config.is_object() {
                property.config = json!({});
            }
            property
                .config
                .as_object_mut()
                .unwrap()
                .insert("date_format".into(), json!(format));
            Ok(())
        })
    }
    pub fn rename_property(
        &mut self,
        database_id: &str,
        expected: &Property,
        property_name: &str,
    ) -> Result<Database> {
        let property_name = name(property_name)?.to_owned();
        with_database(self, database_id, |_, db, _| {
            let index = existing(db, expected)?;
            // Older formulas may not yet have stable ID bindings.
            snapshot_formula_bindings(db);
            db.properties[index].name = property_name;
            Ok(())
        })
    }
    pub fn delete_property(&mut self, database_id: &str, expected: &Property) -> Result<Database> {
        with_database(self, database_id, |tx, db, owner| {
            let index = existing(db, expected)?;
            if expected.kind == PropertyKind::Title {
                return Err(invalid("title property cannot be deleted"));
            }
            db.properties.remove(index);
            for view in &mut db.views {
                prune_view(view, &expected.id);
            }
            let mut after = String::new();
            loop {
                let batch: Vec<Record> = {
                    let mut query = tx.prepare("SELECT data FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256")?;
                    let rows =
                        query.query_map(params![database_id, after], |row| decode(row.get(0)?))?;
                    rows.collect::<Result<Vec<_>>>()?
                };
                if batch.is_empty() {
                    break;
                }
                for mut record in batch {
                    after = record.id.clone();
                    if record.values.remove(&expected.id).is_none() {
                        continue;
                    }
                    update_record_values(tx, owner, &record)?;
                }
            }
            // Recalculate local formulas and downstream rollups against the new schema.
            tx.execute(
                "UPDATE databases SET data=?1 WHERE id=?2",
                params![encode(db)?, db.id],
            )?;
            refresh_all_records(tx, database_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseView, FilterOp, FilterRule, RecordQuery, SortRule, ViewKind};
    use std::collections::BTreeMap;

    fn setup(s: &mut Store) -> (crate::Workspace, Database) {
        let workspace = s.create_workspace("Property tests", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Database".into(),
            properties: vec![Property {
                id: "title".into(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            }],
            views: vec![DatabaseView {
                id: "view".into(),
                name: "Table".into(),
                kind: ViewKind::Table,
                query: RecordQuery::default(),
                config: json!({}),
            }],
        };
        let page = s
            .create_database_page(&workspace.id, None, "Database", "", db)
            .unwrap();
        (
            workspace,
            s.get_database_by_page(&page.id).unwrap().unwrap(),
        )
    }

    #[test]
    fn property_edits_persist_and_reject_stale_or_required_title() {
        let path = std::env::temp_dir().join(format!("moss-properties-{}.sqlite", new_id()));
        let mut s = Store::open(&path).unwrap();
        let (_, db) = setup(&mut s);
        assert!(s.add_property(&db.id, "", PropertyKind::Text).is_err());
        assert!(
            s.add_property(&db.id, "Other title", PropertyKind::Title)
                .is_err()
        );
        let db = s
            .add_property(&db.id, "  Notes  ", PropertyKind::Text)
            .unwrap();
        let note = db.properties.last().unwrap().clone();
        assert_eq!(note.name, "Notes");
        let renamed = s.rename_property(&db.id, &note, "Details").unwrap();
        assert!(s.rename_property(&db.id, &note, "Stale").is_err());
        assert!(s.delete_property(&db.id, &db.properties[0]).is_err());
        assert_eq!(renamed.properties[1].name, "Details");
        drop(s);
        let s = Store::open(&path).unwrap();
        assert_eq!(s.get_database(&db.id).unwrap().unwrap(), renamed);
        drop(s);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn delete_cleans_values_search_and_view_references_without_losing_other_data() {
        let mut s = Store::in_memory().unwrap();
        let (workspace, db) = setup(&mut s);
        let db = s
            .add_property(&db.id, "Remove", PropertyKind::Text)
            .unwrap();
        let remove = db.properties.last().unwrap().clone();
        let mut db = s.add_property(&db.id, "Keep", PropertyKind::Text).unwrap();
        let keep = db.properties.last().unwrap().clone();
        let rule = |id: &str| {
            FilterExpr::Rule(FilterRule {
                property_id: id.into(),
                op: FilterOp::Eq,
                value: json!("x"),
            })
        };
        db.views[0].query.filter = Some(FilterExpr::All(vec![
            rule(&remove.id),
            FilterExpr::Any(vec![rule(&keep.id), rule(&remove.id)]),
        ]));
        db.views[0].query.sorts = vec![
            SortRule {
                property_id: remove.id.clone(),
                descending: false,
            },
            SortRule {
                property_id: keep.id.clone(),
                descending: true,
            },
        ];
        db.views[0].query.group_by = Some(remove.id.clone());
        db.views[0].config = json!({"date_property_id":remove.id, "group_property_id":remove.id, "hidden_property_ids":[remove.id, keep.id, "unknown"], "freeze_through":remove.id, "calculations": {remove.id.clone(): "sum", keep.id.clone(): "count_all"}, "other":"keep"});
        s.save_database(&db).unwrap();
        let values = BTreeMap::from([
            (remove.id.clone(), json!("removeuniquemarker")),
            (keep.id.clone(), json!("keepuniquemarker")),
        ]);
        let record = s.create_record(&db.id, "Record", &values).unwrap();
        let result = s.delete_property(&db.id, &remove).unwrap();
        let saved = s.get_record(&record.id).unwrap().unwrap();
        assert!(!saved.values.contains_key(&remove.id));
        assert_eq!(saved.values[&keep.id], "keepuniquemarker");
        assert!(
            s.search("removeuniquemarker", Some(&workspace.id), 10)
                .unwrap()
                .is_empty()
        );
        assert!(
            !s.search("keepuniquemarker", Some(&workspace.id), 10)
                .unwrap()
                .is_empty()
        );
        let view = &result.views[0];
        assert_eq!(view.query.sorts.len(), 1);
        assert_eq!(view.query.sorts[0].property_id, keep.id);
        assert!(view.query.group_by.is_none());
        assert_eq!(
            view.query.filter,
            Some(FilterExpr::All(vec![FilterExpr::Any(vec![rule(&keep.id)])]))
        );
        assert!(view.config.get("date_property_id").is_none());
        assert!(view.config.get("group_property_id").is_none());
        assert_eq!(
            view.config["hidden_property_ids"],
            json!([keep.id, "unknown"])
        );
        assert!(view.config.get("freeze_through").is_none());
        assert!(view.config["calculations"].get(&remove.id).is_none());
        assert_eq!(view.config["calculations"][&keep.id], "count_all");
        assert_eq!(view.config["other"], "keep");
    }

    #[test]
    fn delete_rolls_back_all_records_on_write_failure_and_rejects_trashed_database() {
        let mut s = Store::in_memory().unwrap();
        let (_, db) = setup(&mut s);
        let db = s.add_property(&db.id, "Notes", PropertyKind::Text).unwrap();
        let prop = db.properties.last().unwrap().clone();
        let record = s
            .create_record(
                &db.id,
                "Row",
                &BTreeMap::from([(prop.id.clone(), json!("keep"))]),
            )
            .unwrap();
        s.conn.execute_batch("CREATE TRIGGER fail_db_update BEFORE UPDATE ON databases BEGIN SELECT RAISE(ABORT, 'forced failure'); END;").unwrap();
        assert!(s.delete_property(&db.id, &prop).is_err());
        assert_eq!(s.get_database(&db.id).unwrap().unwrap(), db);
        assert_eq!(s.get_record(&record.id).unwrap().unwrap(), record);
        s.conn.execute_batch("DROP TRIGGER fail_db_update").unwrap();
        s.delete_page(&db.page_id).unwrap();
        assert!(s.add_property(&db.id, "No", PropertyKind::Text).is_err());
        assert!(s.rename_property(&db.id, &prop, "No").is_err());
        assert!(s.delete_property(&db.id, &prop).is_err());
    }
    #[test]
    fn select_options_are_validated_and_title_can_be_renamed() {
        let path = std::env::temp_dir().join(format!("moss-select-{}.sqlite", new_id()));
        let mut s = Store::open(&path).unwrap();
        let (_, db) = setup(&mut s);
        assert!(s.add_select_property(&db.id, "", &["One".into()]).is_err());
        assert!(s.add_select_property(&db.id, "Choice", &[]).is_err());
        assert!(
            s.add_select_property(&db.id, "Choice", &[" ".into()])
                .is_err()
        );
        assert!(
            s.add_select_property(&db.id, "Choice", &["One".into(), " One ".into()])
                .is_err()
        );
        let db = s
            .add_select_property(&db.id, " Choice ", &[" One ".into(), "Two".into()])
            .unwrap();
        let choice = db.properties.last().unwrap().clone();
        let option_names: Vec<_> = choice.config["options"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(option_name)
            .collect();
        assert_eq!(option_names, vec!["One", "Two"]);
        for entry in choice.config["options"].as_array().unwrap() {
            let color = entry.get("color").and_then(Value::as_str).unwrap();
            assert!(AUTO_OPTION_COLORS.contains(&color));
        }
        assert_ne!(
            choice.config["options"][0]["color"],
            choice.config["options"][1]["color"]
        );
        let db = s
            .set_property_option_color(&db.id, &choice, "One", "purple")
            .unwrap();
        assert_eq!(resolved_option_color(&db.properties[1], "One"), "purple");
        let db = s
            .rename_property(&db.id, &db.properties[0], "Task name")
            .unwrap();
        assert_eq!(db.properties[0].kind, PropertyKind::Title);
        assert_eq!(db.properties[0].name, "Task name");
        drop(s);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.get_database(&db.id).unwrap().unwrap(), db);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn relative_insertion_before_title_preserves_its_identity_and_protection() {
        let mut s = Store::in_memory().unwrap();
        let (_, db) = setup(&mut s);
        let title = db.properties[0].clone();
        let db = s
            .insert_property_relative(&db.id, "Before", PropertyKind::Text, &[], &title.id, false)
            .unwrap();
        assert_eq!(db.properties[0].name, "Before");
        assert_eq!(db.properties[1], title);
        let db = s
            .save_advanced_property(
                &db.id,
                None,
                "Formula",
                PropertyKind::Formula,
                json!({"expression":"1"}),
                Some((&title.id, false)),
            )
            .unwrap();
        assert_eq!(db.properties[1].name, "Formula");
        assert_eq!(db.properties[2], title);
        assert!(s.delete_property(&db.id, &title).is_err());
        assert_eq!(s.get_database(&db.id).unwrap().unwrap(), db);
    }
}
