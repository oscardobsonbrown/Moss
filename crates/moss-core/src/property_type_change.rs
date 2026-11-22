use crate::{
    Database, DatabaseView, Property, PropertyKind, Record, Store, ViewKind, calculation, formula,
    properties::{
        existing, name, refresh_all_records, snapshot_formula_bindings, update_record_values,
        validate_relation_target, validate_rollup_operation, with_database,
    },
    store::{decode, encode, invalid},
};
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use rusqlite::{OptionalExtension, Result, Transaction, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

pub fn property_type_convertible(kind: PropertyKind) -> bool {
    matches!(
        kind,
        PropertyKind::Text
            | PropertyKind::Number
            | PropertyKind::Select
            | PropertyKind::MultiSelect
            | PropertyKind::Status
            | PropertyKind::Date
            | PropertyKind::Checkbox
            | PropertyKind::Url
            | PropertyKind::Email
            | PropertyKind::Phone
            | PropertyKind::Formula
            | PropertyKind::Relation
            | PropertyKind::Rollup
    )
}

pub fn is_type_change_lossy(from: PropertyKind, to: PropertyKind) -> bool {
    if from == to {
        return false;
    }
    if matches!(
        from,
        PropertyKind::Formula | PropertyKind::Rollup | PropertyKind::Relation
    ) {
        return true;
    }
    if from == PropertyKind::MultiSelect
        && matches!(to, PropertyKind::Select | PropertyKind::Status)
    {
        return true;
    }
    matches!(
        to,
        PropertyKind::Date
            | PropertyKind::Number
            | PropertyKind::Checkbox
            | PropertyKind::Formula
            | PropertyKind::Relation
            | PropertyKind::Rollup
    )
}

fn validate_convertible(from: PropertyKind, to: PropertyKind) -> Result<()> {
    if from == PropertyKind::Title {
        return Err(invalid("title property type cannot be changed"));
    }
    if to == PropertyKind::Title {
        return Err(invalid("cannot change property type to title"));
    }
    if from == PropertyKind::Place || to == PropertyKind::Place {
        return Err(invalid("place property type cannot be changed"));
    }
    if !property_type_convertible(from) || !property_type_convertible(to) {
        return Err(invalid("unsupported property type"));
    }
    Ok(())
}

fn relation_titles(tx: &Transaction<'_>, ids: &[Value]) -> Result<Option<String>> {
    let mut names = Vec::new();
    for item in ids {
        let Some(id) = item
            .as_str()
            .or_else(|| item.get("id").and_then(Value::as_str))
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let raw: Option<String> = tx
            .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        if let Some(raw) = raw {
            let record: Record = decode(raw)?;
            if !record.title.trim().is_empty() {
                names.push(record.title);
                continue;
            }
        }
        names.push(id.to_owned());
    }
    if names.is_empty() {
        Ok(None)
    } else {
        Ok(Some(names.join(", ")))
    }
}

fn value_to_text(
    tx: &Transaction<'_>,
    old_kind: PropertyKind,
    value: &Value,
) -> Result<Option<String>> {
    if value.is_null() || calculation::calculation_error(value).is_some() {
        return Ok(None);
    }
    match value {
        Value::String(s) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(s.clone()))
            }
        }
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 && f.abs() <= 9_007_199_254_740_992.0 {
                    return Ok(Some((f as i64).to_string()));
                }
            }
            Ok(Some(n.to_string()))
        }
        Value::Bool(b) => Ok(Some(if *b { "true" } else { "false" }.into())),
        Value::Array(items) => {
            if old_kind == PropertyKind::Relation {
                return relation_titles(tx, items);
            }
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    Value::Bool(b) => Some(if *b { "true" } else { "false" }.into()),
                    _ => None,
                })
                .collect();
            Ok((!parts.is_empty()).then(|| parts.join(", ")))
        }
        Value::Object(map) if old_kind == PropertyKind::Date => {
            let start = map
                .get("start")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let end = map
                .get("end")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            Ok(match (start, end) {
                (Some(start), Some(end)) => Some(format!("{start} → {end}")),
                (Some(start), None) => Some(start.to_owned()),
                (None, Some(end)) => Some(end.to_owned()),
                (None, None) => None,
            })
        }
        Value::Object(_) | Value::Null => Ok(None),
    }
}

fn parse_number_text(raw: &str) -> Option<Value> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let parsed = text.parse::<f64>().ok().or_else(|| {
        if !text.contains(',') {
            return None;
        }
        let (sign, rest) = if let Some(rest) = text.strip_prefix('-') {
            ("-", rest)
        } else if let Some(rest) = text.strip_prefix('+') {
            ("", rest)
        } else {
            ("", text)
        };
        let (int_part, frac_part) = match rest.split_once('.') {
            Some((int_part, frac_part)) => (int_part, Some(frac_part)),
            None => (rest, None),
        };
        let groups: Vec<&str> = int_part.split(',').collect();
        if groups.len() < 2
            || groups[0].is_empty()
            || groups[0].len() > 3
            || !groups[0].bytes().all(|b| b.is_ascii_digit())
            || groups[1..]
                .iter()
                .any(|g| g.len() != 3 || !g.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        let mut clean = String::from(sign);
        for g in groups {
            clean.push_str(g);
        }
        if let Some(frac) = frac_part {
            if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            clean.push('.');
            clean.push_str(frac);
        }
        clean.parse::<f64>().ok()
    })?;
    if !parsed.is_finite() {
        return None;
    }
    if parsed.fract() == 0.0 && parsed.abs() <= 9_007_199_254_740_992.0 {
        Some(json!(parsed as i64))
    } else {
        serde_json::Number::from_f64(parsed).map(Value::Number)
    }
}

fn parse_checkbox_value(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(b) => Some(Value::Bool(*b)),
        Value::Number(n) => match n.as_f64() {
            Some(v) if v == 1.0 => Some(Value::Bool(true)),
            Some(v) if v == 0.0 => Some(Value::Bool(false)),
            _ => None,
        },
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" | "checked" => Some(Value::Bool(true)),
            "false" | "no" | "0" | "unchecked" => Some(Value::Bool(false)),
            _ => None,
        },
        _ => None,
    }
}

fn parse_date_endpoint(raw: &str) -> Option<(NaiveDate, String, bool)> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    if text.len() == 10 {
        if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
            return Some((date, date.to_string(), false));
        }
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some((dt.date_naive(), text.to_owned(), true));
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%B %d, %Y %H:%M",
        "%B %d, %Y %I:%M %p",
        "%d %B, %Y %H:%M",
        "%d %B %Y %H:%M",
        "%b %d, %Y %H:%M",
        "%b %d, %Y %I:%M %p",
        "%d %b, %Y %H:%M",
        "%d %b %Y %H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(text, fmt) {
            return Some((dt.date(), dt.format("%Y-%m-%dT%H:%M").to_string(), true));
        }
    }
    for fmt in [
        "%Y-%m-%d",
        "%B %d, %Y",
        "%B %d %Y",
        "%d %B, %Y",
        "%d %B %Y",
        "%b %d, %Y",
        "%b %d %Y",
        "%d %b, %Y",
        "%d %b %Y",
        "%Y/%m/%d",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%d-%m-%Y",
        "%m-%d-%Y",
    ] {
        if let Ok(date) = NaiveDate::parse_from_str(text, fmt) {
            return Some((date, date.to_string(), false));
        }
    }
    None
}

fn parse_date_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }
            for sep in [" → ", " -> "] {
                if let Some((left, right)) = trimmed.split_once(sep) {
                    let (start_date, start_str, start_time) = parse_date_endpoint(left)?;
                    let (end_date, end_str, end_time) = parse_date_endpoint(right)?;
                    if end_date < start_date {
                        return None;
                    }
                    let (start_val, end_val) = if start_time != end_time {
                        (start_date.to_string(), end_date.to_string())
                    } else {
                        (start_str, end_str)
                    };
                    return Some(json!({"start": start_val, "end": end_val}));
                }
            }
            let (_, formatted, _) = parse_date_endpoint(trimmed)?;
            Some(Value::String(formatted))
        }
        Value::Object(map) => {
            let start = map.get("start").and_then(Value::as_str)?;
            let (start_date, _, _) = parse_date_endpoint(start)?;
            if let Some(end) = map.get("end").filter(|v| !v.is_null()) {
                let end = end.as_str()?;
                let (end_date, _, _) = parse_date_endpoint(end)?;
                if end_date < start_date {
                    return None;
                }
            }
            Some(value.clone())
        }
        _ => None,
    }
}

fn convert_record_value(
    tx: &Transaction<'_>,
    old_kind: PropertyKind,
    new_kind: PropertyKind,
    old_value: Option<&Value>,
    collected_options: &mut Vec<String>,
    seen_options: &mut HashSet<String>,
) -> Result<Option<Value>> {
    let Some(old_value) = old_value.filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    if calculation::calculation_error(old_value).is_some() {
        return Ok(None);
    }
    match new_kind {
        PropertyKind::Formula | PropertyKind::Rollup | PropertyKind::Relation => Ok(None),
        PropertyKind::Text | PropertyKind::Url | PropertyKind::Email | PropertyKind::Phone => {
            Ok(value_to_text(tx, old_kind, old_value)?.map(Value::String))
        }
        PropertyKind::Number => match old_value {
            Value::Number(n) => Ok(n
                .as_f64()
                .filter(|v| v.is_finite())
                .map(|_| Value::Number(n.clone()))),
            Value::String(s) => Ok(parse_number_text(s)),
            _ => Ok(None),
        },
        PropertyKind::Checkbox => Ok(parse_checkbox_value(old_value)),
        PropertyKind::Date => Ok(parse_date_value(old_value)),
        PropertyKind::Select | PropertyKind::Status => {
            let option = if old_kind == PropertyKind::MultiSelect {
                old_value
                    .as_array()
                    .and_then(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::trim)
                            .find(|s| !s.is_empty())
                    })
                    .map(str::to_owned)
            } else {
                value_to_text(tx, old_kind, old_value)?
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
            };
            let Some(option) = option else {
                return Ok(None);
            };
            if seen_options.insert(option.clone()) {
                collected_options.push(option.clone());
            }
            Ok(Some(Value::String(option)))
        }
        PropertyKind::MultiSelect => {
            let mut tags = Vec::new();
            let mut seen_tag = HashSet::new();
            match old_value {
                Value::Array(items) if old_kind != PropertyKind::Relation => {
                    for item in items {
                        if let Some(text) = item
                            .as_str()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            let tag = text.to_owned();
                            if seen_tag.insert(tag.clone()) {
                                if seen_options.insert(tag.clone()) {
                                    collected_options.push(tag.clone());
                                }
                                tags.push(Value::String(tag));
                            }
                        }
                    }
                }
                _ => {
                    if let Some(text) = value_to_text(tx, old_kind, old_value)? {
                        let pieces: Vec<&str> = if matches!(
                            old_kind,
                            PropertyKind::Text
                                | PropertyKind::Url
                                | PropertyKind::Email
                                | PropertyKind::Phone
                                | PropertyKind::Relation
                        ) {
                            text.split(',').collect()
                        } else {
                            vec![text.as_str()]
                        };
                        for piece in pieces {
                            let trimmed = piece.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            let tag = trimmed.to_owned();
                            if seen_tag.insert(tag.clone()) {
                                if seen_options.insert(tag.clone()) {
                                    collected_options.push(tag.clone());
                                }
                                tags.push(Value::String(tag));
                            }
                        }
                    }
                }
            }
            Ok((!tags.is_empty()).then_some(Value::Array(tags)))
        }
        _ => Err(invalid("unsupported property type")),
    }
}

fn prune_view_for_type_change(
    view: &mut DatabaseView,
    id: &str,
    old_kind: PropertyKind,
    new_kind: PropertyKind,
) {
    if old_kind == new_kind {
        return;
    }
    let Some(config) = view.config.as_object_mut() else {
        return;
    };
    if !matches!(
        new_kind,
        PropertyKind::Number | PropertyKind::Formula | PropertyKind::Rollup
    ) {
        if let Some(Value::Object(calculations)) = config.get_mut("calculations") {
            if calculations
                .get(id)
                .and_then(Value::as_str)
                .is_some_and(|op| matches!(op, "sum" | "average" | "min" | "max"))
            {
                calculations.remove(id);
            }
        }
    }
    if old_kind == PropertyKind::Date && new_kind != PropertyKind::Date {
        for key in ["date_property_id", "calendar_date_property"] {
            if config.get(key).and_then(Value::as_str) == Some(id) {
                config.remove(key);
            }
        }
    }
    if old_kind == PropertyKind::Place && new_kind != PropertyKind::Place {
        if config.get("map_place_property").and_then(Value::as_str) == Some(id) {
            config.remove("map_place_property");
        }
    }
    if view.kind == ViewKind::Board {
        if view.query.group_by.as_deref() == Some(id) {
            if !crate::board::group_supported(new_kind) {
                view.query.group_by = None;
            }
            for key in [
                "board_group_order",
                "board_hidden_groups",
                "board_collapsed_groups",
            ] {
                config.remove(key);
            }
        }
        if config.get("board_subgroup_by").and_then(Value::as_str) == Some(id) {
            if !crate::board::group_supported(new_kind) {
                config.remove("board_subgroup_by");
            }
            for key in [
                "board_subgroup_order",
                "board_hidden_subgroups",
                "board_collapsed_subgroups",
            ] {
                config.remove(key);
            }
        }
    }
}

fn initial_options(new_kind: PropertyKind, config: &Value) -> Result<(Vec<String>, HashSet<String>)> {
    let mut options = Vec::new();
    let mut seen = HashSet::new();
    let provided = config.get("options").and_then(Value::as_array);
    if let Some(items) = provided {
        for item in items {
            let raw = item
                .as_str()
                .or_else(|| item.get("name").and_then(Value::as_str))
                .ok_or_else(|| invalid("invalid option value"))?;
            let option = raw.trim();
            if option.is_empty() {
                return Err(invalid("option cannot be empty"));
            }
            if !seen.insert(option.to_owned()) {
                return Err(invalid("options must be unique"));
            }
            options.push(option.to_owned());
        }
    }
    if new_kind == PropertyKind::Status && options.is_empty() {
        for default in ["Not started", "In progress", "Done"] {
            seen.insert(default.to_owned());
            options.push(default.to_owned());
        }
    }
    Ok((options, seen))
}

impl Store {
    pub fn property_has_values(&self, database_id: &str, property_id: &str) -> Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM records r, json_each(r.values_json) v WHERE r.database_id=?1 AND r.deleted_at IS NULL AND v.key=?2 AND v.value IS NOT NULL AND v.value <> '' AND v.value <> '[]')",
            params![database_id, property_id],
            |row| row.get(0),
        )
    }

    pub fn change_property_type(
        &mut self,
        database_id: &str,
        expected: &Property,
        property_name: Option<&str>,
        new_kind: PropertyKind,
        mut config: Value,
    ) -> Result<Database> {
        validate_convertible(expected.kind, new_kind)?;
        let next_name = match property_name {
            Some(raw) => name(raw)?.to_owned(),
            None => expected.name.clone(),
        };
        if !config.is_null() && !config.is_object() {
            return Err(invalid("invalid property configuration"));
        }
        with_database(self, database_id, |tx, db, owner| {
            let index = existing(db, expected)?;
            // Snapshot stable bindings for any legacy formulas before changing schema.
            snapshot_formula_bindings(db);

            if new_kind == PropertyKind::Relation {
                validate_relation_target(tx, &owner.workspace_id, &config)?;
            }
            if new_kind == PropertyKind::Rollup {
                validate_rollup_operation(&config)?;
            }
            if new_kind == PropertyKind::Formula
                && config
                    .get("expression")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .is_none_or(str::is_empty)
            {
                return Err(invalid("formula expression missing"));
            }

            let mut existing_colors = BTreeMap::<String, String>::new();
            if let Some(old_options) = expected.config.get("options").and_then(Value::as_array) {
                for entry in old_options {
                    if let (Some(name), Some(color)) = (
                        crate::option_name(entry),
                        entry
                            .get("color")
                            .and_then(Value::as_str)
                            .filter(|c| crate::is_valid_option_color(c)),
                    ) {
                        existing_colors.insert(name.to_owned(), color.to_owned());
                    }
                }
            }

            let (mut collected_options, mut seen_options) = if matches!(
                new_kind,
                PropertyKind::Select | PropertyKind::MultiSelect | PropertyKind::Status
            ) {
                initial_options(new_kind, &config)?
            } else {
                (Vec::new(), HashSet::new())
            };

            let convert_values = expected.kind != new_kind;
            if convert_values {
                let mut after = String::new();
                loop {
                    let batch: Vec<Record> = {
                        let mut query = tx.prepare(
                            "SELECT data FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256",
                        )?;
                        let rows = query.query_map(params![database_id, after], |row| {
                            decode(row.get(0)?)
                        })?;
                        rows.collect::<Result<Vec<_>>>()?
                    };
                    if batch.is_empty() {
                        break;
                    }
                    for mut record in batch {
                        after = record.id.clone();
                        let converted = convert_record_value(
                            tx,
                            expected.kind,
                            new_kind,
                            record.values.get(&expected.id),
                            &mut collected_options,
                            &mut seen_options,
                        )?;
                        let changed = match converted {
                            Some(next) => {
                                if record.values.get(&expected.id) == Some(&next) {
                                    false
                                } else {
                                    record.values.insert(expected.id.clone(), next);
                                    true
                                }
                            }
                            None => record.values.remove(&expected.id).is_some(),
                        };
                        if !changed {
                            continue;
                        }
                        update_record_values(tx, owner, &record)?;
                    }
                }
            }

            let next_config = match new_kind {
                PropertyKind::Select | PropertyKind::MultiSelect => {
                    let entries: Vec<Value> = collected_options
                        .iter()
                        .enumerate()
                        .map(|(idx, name)| {
                            let color = existing_colors
                                .get(name)
                                .map(String::as_str)
                                .unwrap_or_else(|| crate::auto_option_color(&expected.id, idx));
                            json!({"name": name, "color": color})
                        })
                        .collect();
                    json!({"options": entries})
                }
                PropertyKind::Status => {
                    json!({"options": collected_options})
                }
                PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup => {
                    if config.is_object() {
                        config.take()
                    } else {
                        json!({})
                    }
                }
                PropertyKind::Date if expected.kind == PropertyKind::Date => {
                    expected.config.clone()
                }
                _ => json!({}),
            };

            db.properties[index] = Property {
                id: expected.id.clone(),
                name: next_name,
                kind: new_kind,
                config: next_config,
            };

            if new_kind == PropertyKind::Formula {
                let source = db.properties[index]
                    .config
                    .get("expression")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("formula expression missing"))?
                    .to_owned();
                let bindings: BTreeMap<String, String> =
                    formula::bindings(&source, &db.properties)
                        .map_err(|e| invalid(&e.to_string()))?;
                db.properties[index]
                    .config
                    .as_object_mut()
                    .unwrap()
                    .insert("_bindings".into(), json!(bindings));
            }

            crate::advanced_properties::graph(
                tx,
                &owner.workspace_id,
                db,
                Some(&expected.id),
            )?;

            for view in &mut db.views {
                prune_view_for_type_change(view, &expected.id, expected.kind, new_kind);
            }

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
    use crate::{DatabaseView, RecordQuery, new_id};
    use std::collections::BTreeMap;

    fn setup(s: &mut Store) -> (crate::Workspace, Database) {
        let workspace = s.create_workspace("Type change tests", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Trips".into(),
            properties: vec![Property {
                id: "title".into(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            }],
            views: vec![DatabaseView {
                id: "table".into(),
                name: "Table".into(),
                kind: ViewKind::Table,
                query: RecordQuery::default(),
                config: json!({}),
            }],
        };
        let page = s
            .create_database_page(&workspace.id, None, "Trips", "", db)
            .unwrap();
        (
            workspace,
            s.get_database_by_page(&page.id).unwrap().unwrap(),
        )
    }

    #[test]
    fn text_to_date_converts_notion_and_iso_formats_and_clears_invalid_text() {
        let mut s = Store::in_memory().unwrap();
        let (ws, db) = setup(&mut s);
        let db = s
            .add_property(&db.id, "Departure Date", PropertyKind::Text)
            .unwrap();
        let prop = db.properties[1].clone();
        let r1 = s
            .create_record(
                &db.id,
                "Japan (Jan)",
                &BTreeMap::from([(prop.id.clone(), json!("January 5, 2026"))]),
            )
            .unwrap();
        let r2 = s
            .create_record(
                &db.id,
                "Japan (Feb)",
                &BTreeMap::from([(prop.id.clone(), json!("February 28, 2026"))]),
            )
            .unwrap();
        let r3 = s
            .create_record(
                &db.id,
                "Austria",
                &BTreeMap::from([(prop.id.clone(), json!("15 January, 2027"))]),
            )
            .unwrap();
        let r4 = s
            .create_record(
                &db.id,
                "Range",
                &BTreeMap::from([(prop.id.clone(), json!("March 7, 2026 → March 14, 2026"))]),
            )
            .unwrap();
        let r5 = s
            .create_record(
                &db.id,
                "Invalid",
                &BTreeMap::from([(prop.id.clone(), json!("TBD sometime next year"))]),
            )
            .unwrap();

        assert!(s.property_has_values(&db.id, &prop.id).unwrap());
        let updated = s
            .change_property_type(&db.id, &prop, None, PropertyKind::Date, json!({}))
            .unwrap();
        assert_eq!(updated.properties[1].kind, PropertyKind::Date);
        assert_eq!(
            s.get_record(&r1.id).unwrap().unwrap().values[&prop.id],
            json!("2026-01-05")
        );
        assert_eq!(
            s.get_record(&r2.id).unwrap().unwrap().values[&prop.id],
            json!("2026-02-28")
        );
        assert_eq!(
            s.get_record(&r3.id).unwrap().unwrap().values[&prop.id],
            json!("2027-01-15")
        );
        assert_eq!(
            s.get_record(&r4.id).unwrap().unwrap().values[&prop.id],
            json!({"start": "2026-03-07", "end": "2026-03-14"})
        );
        assert!(!s.get_record(&r5.id).unwrap().unwrap().values.contains_key(&prop.id));
        assert!(
            !s.search("2026-01-05", Some(&ws.id), 10)
                .unwrap()
                .is_empty()
        );

        // Round-trip Date -> Text -> Date preserves dates and ranges.
        let date_prop = updated.properties[1].clone();
        let back_to_text = s
            .change_property_type(&db.id, &date_prop, None, PropertyKind::Text, json!({}))
            .unwrap();
        assert_eq!(
            s.get_record(&r1.id).unwrap().unwrap().values[&prop.id],
            json!("2026-01-05")
        );
        assert_eq!(
            s.get_record(&r4.id).unwrap().unwrap().values[&prop.id],
            json!("2026-03-07 → 2026-03-14")
        );
        let text_prop = back_to_text.properties[1].clone();
        s.change_property_type(&db.id, &text_prop, None, PropertyKind::Date, json!({}))
            .unwrap();
        assert_eq!(
            s.get_record(&r4.id).unwrap().unwrap().values[&prop.id],
            json!({"start": "2026-03-07", "end": "2026-03-14"})
        );
    }

    #[test]
    fn text_converts_to_select_status_number_and_checkbox() {
        let mut s = Store::in_memory().unwrap();
        let (_, db) = setup(&mut s);
        let db = s.add_property(&db.id, "Stage", PropertyKind::Text).unwrap();
        let stage = db.properties[1].clone();
        let db = s.add_property(&db.id, "Budget", PropertyKind::Text).unwrap();
        let budget = db.properties[2].clone();
        let db = s.add_property(&db.id, "Paid", PropertyKind::Text).unwrap();
        let paid = db.properties[3].clone();

        let r1 = s
            .create_record(
                &db.id,
                "Trip 1",
                &BTreeMap::from([
                    (stage.id.clone(), json!("Booked")),
                    (budget.id.clone(), json!("1,250.50")),
                    (paid.id.clone(), json!("yes")),
                ]),
            )
            .unwrap();
        let r2 = s
            .create_record(
                &db.id,
                "Trip 2",
                &BTreeMap::from([
                    (stage.id.clone(), json!("Done")),
                    (budget.id.clone(), json!("not a number")),
                    (paid.id.clone(), json!("false")),
                ]),
            )
            .unwrap();

        let db = s
            .change_property_type(&db.id, &stage, None, PropertyKind::Select, json!({}))
            .unwrap();
        assert_eq!(db.properties[1].kind, PropertyKind::Select);
        let mut stage_names: Vec<_> = db.properties[1].config["options"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(crate::option_name)
            .collect();
        stage_names.sort();
        assert_eq!(stage_names, vec!["Booked", "Done"]);
        for entry in db.properties[1].config["options"].as_array().unwrap() {
            let color = entry.get("color").and_then(Value::as_str).unwrap();
            assert!(crate::AUTO_OPTION_COLORS.contains(&color));
        }

        let select_stage = db.properties[1].clone();
        let db = s
            .change_property_type(&db.id, &select_stage, None, PropertyKind::MultiSelect, json!({}))
            .unwrap();
        assert_eq!(db.properties[1].kind, PropertyKind::MultiSelect);
        assert_eq!(
            s.get_record(&r1.id).unwrap().unwrap().values[&stage.id],
            json!(["Booked"])
        );

        let multi_stage = db.properties[1].clone();
        let db = s
            .change_property_type(&db.id, &multi_stage, None, PropertyKind::Status, json!({}))
            .unwrap();
        assert_eq!(db.properties[1].kind, PropertyKind::Status);
        assert_eq!(
            db.properties[1].config["options"],
            json!(["Not started", "In progress", "Done", "Booked"])
        );

        let db = s
            .change_property_type(&db.id, &budget, None, PropertyKind::Number, json!({}))
            .unwrap();
        assert_eq!(db.properties[2].kind, PropertyKind::Number);
        assert_eq!(
            s.get_record(&r1.id).unwrap().unwrap().values[&budget.id],
            json!(1250.5)
        );
        assert!(!s.get_record(&r2.id).unwrap().unwrap().values.contains_key(&budget.id));

        let db = s
            .change_property_type(&db.id, &paid, None, PropertyKind::Checkbox, json!({}))
            .unwrap();
        assert_eq!(db.properties[3].kind, PropertyKind::Checkbox);
        assert_eq!(
            s.get_record(&r1.id).unwrap().unwrap().values[&paid.id],
            json!(true)
        );
        assert_eq!(
            s.get_record(&r2.id).unwrap().unwrap().values[&paid.id],
            json!(false)
        );
    }

    #[test]
    fn title_and_place_cannot_change_type_and_stale_edit_fails_atomically() {
        let mut s = Store::in_memory().unwrap();
        let (_, db) = setup(&mut s);
        let title = db.properties[0].clone();
        assert!(
            s.change_property_type(&db.id, &title, None, PropertyKind::Text, json!({}))
                .is_err()
        );
        let db = s.add_property(&db.id, "Place", PropertyKind::Place).unwrap();
        let place = db.properties[1].clone();
        assert!(
            s.change_property_type(&db.id, &place, None, PropertyKind::Text, json!({}))
                .is_err()
        );
        let db = s.add_property(&db.id, "Note", PropertyKind::Text).unwrap();
        let note = db.properties[2].clone();
        assert!(
            s.change_property_type(&db.id, &note, None, PropertyKind::Title, json!({}))
                .is_err()
        );
        assert!(
            s.change_property_type(&db.id, &note, None, PropertyKind::Place, json!({}))
                .is_err()
        );
        let renamed = s.rename_property(&db.id, &note, "Other").unwrap();
        assert!(
            s.change_property_type(&db.id, &note, None, PropertyKind::Number, json!({}))
                .is_err()
        );
        assert_eq!(s.get_database(&db.id).unwrap().unwrap(), renamed);
    }

    #[test]
    fn type_change_updates_formulas_and_prunes_incompatible_view_settings() {
        let mut s = Store::in_memory().unwrap();
        let (_, db) = setup(&mut s);
        let db = s
            .add_property(&db.id, "Amount", PropertyKind::Number)
            .unwrap();
        let amount = db.properties[1].clone();
        let mut db = s
            .add_property(&db.id, "When", PropertyKind::Date)
            .unwrap();
        let when = db.properties[2].clone();
        db.views[0].config = json!({
            "calculations": {
                amount.id.clone(): "sum",
                when.id.clone(): "count_values"
            }
        });
        db.views.push(DatabaseView {
            id: "cal".into(),
            name: "Calendar".into(),
            kind: ViewKind::Calendar,
            query: RecordQuery::default(),
            config: json!({"calendar_date_property": when.id}),
        });
        db.views.push(DatabaseView {
            id: "board".into(),
            name: "Board".into(),
            kind: ViewKind::Board,
            query: RecordQuery {
                group_by: Some(amount.id.clone()),
                ..RecordQuery::default()
            },
            config: json!({"board_group_order": ["10"]}),
        });
        s.save_database(&db).unwrap();
        let record = s
            .create_record(
                &db.id,
                "Row",
                &BTreeMap::from([
                    (amount.id.clone(), json!(10)),
                    (when.id.clone(), json!("2026-05-01")),
                ]),
            )
            .unwrap();

        // Change Amount from Number -> Text: numeric summary "sum" is pruned, board group order cleared.
        let db = s
            .change_property_type(&db.id, &amount, None, PropertyKind::Text, json!({}))
            .unwrap();
        assert!(db.views[0].config["calculations"].get(&amount.id).is_none());
        assert_eq!(db.views[0].config["calculations"][&when.id], "count_values");
        assert!(db.views[2].config.get("board_group_order").is_none());
        assert_eq!(
            s.get_record(&record.id).unwrap().unwrap().values[&amount.id],
            json!("10")
        );

        // Change When from Date -> Text: calendar_date_property is pruned.
        let db = s
            .change_property_type(&db.id, &when, None, PropertyKind::Text, json!({}))
            .unwrap();
        assert!(db.views[1].config.get("calendar_date_property").is_none());

        // Change Amount from Text -> Formula: recalculates records and clears unsupported Board group.
        let text_amount = db.properties[1].clone();
        let db = s
            .change_property_type(
                &db.id,
                &text_amount,
                Some("Computed"),
                PropertyKind::Formula,
                json!({"expression": "2 + 3"}),
            )
            .unwrap();
        assert_eq!(db.properties[1].name, "Computed");
        assert_eq!(db.properties[1].kind, PropertyKind::Formula);
        assert!(db.views[2].query.group_by.is_none());
        assert_eq!(
            s.get_record(&record.id).unwrap().unwrap().values[&amount.id].as_f64(),
            Some(5.0)
        );

        // Materialize Formula -> Text keeps the computed value as text.
        let formula_prop = db.properties[1].clone();
        s.change_property_type(&db.id, &formula_prop, None, PropertyKind::Text, json!({}))
            .unwrap();
        assert_eq!(
            s.get_record(&record.id).unwrap().unwrap().values[&amount.id],
            json!("5")
        );
    }
}
