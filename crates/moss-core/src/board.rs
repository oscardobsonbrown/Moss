use crate::{
    Database, DatabaseView, FilterExpr, FilterOp, FilterRule, Page, PageCover, PageKind, Property,
    PropertyKind, Record, RecordPageAppearance, Store, ViewKind, search,
    store::{decode, encode, filter_sql, invalid, property_expr},
};
use rusqlite::{Connection, Result, params_from_iter, types::Value as SqlValue};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[path = "board_mutations.rs"]
mod mutations;

pub const BOARD_PAGE_SIZE: usize = 50;
const MAX_GROUPS: usize = 128;
const MAX_SUBGROUPS: usize = 32;
const MAX_BUCKETS: usize = 256;

pub(crate) fn install_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS board_record_order(view_id TEXT NOT NULL, record_id TEXT NOT NULL REFERENCES records(id) ON DELETE CASCADE, rank BLOB NOT NULL, PRIMARY KEY(view_id,record_id)); CREATE INDEX IF NOT EXISTS board_order_rank ON board_record_order(view_id,rank);")
}

pub fn group_supported(kind: PropertyKind) -> bool {
    matches!(
        kind,
        PropertyKind::Status
            | PropertyKind::Select
            | PropertyKind::Text
            | PropertyKind::Number
            | PropertyKind::Checkbox
            | PropertyKind::Date
            | PropertyKind::Url
            | PropertyKind::Email
            | PropertyKind::Phone
    )
}
pub fn group_key(value: &Value) -> String {
    encode(&normalize(value)).unwrap_or_else(|_| "null".into())
}
pub fn bucket_key(group: &Value, subgroup: &Value) -> String {
    json!([normalize(group), normalize(subgroup)]).to_string()
}
pub fn normalize(value: &Value) -> Value {
    if value.is_null() || value.as_str() == Some("") {
        Value::Null
    } else if value.as_number().is_some_and(|number| number.is_f64()) {
        let number = value.as_f64().unwrap();
        if number.fract() == 0. && number.abs() <= 9_007_199_254_740_992. {
            json!(number as i64)
        } else {
            value.clone()
        }
    } else {
        value.clone()
    }
}
pub fn group_value(record: &Record, property: &Property) -> Value {
    normalize(record.values.get(&property.id).unwrap_or(&Value::Null))
}
pub fn group_property<'a>(db: &'a Database, view: &DatabaseView) -> Result<&'a Property> {
    let property = if let Some(id) = &view.query.group_by {
        db.properties.iter().find(|p| &p.id == id)
    } else {
        db.properties
            .iter()
            .find(|p| matches!(p.kind, PropertyKind::Status | PropertyKind::Select))
    }
    .ok_or_else(|| invalid("Choose a Board grouping property in View settings."))?;
    if !group_supported(property.kind) {
        return Err(invalid("This property type cannot group a Board."));
    }
    Ok(property)
}
pub fn subgroup_property<'a>(
    db: &'a Database,
    view: &DatabaseView,
) -> Result<Option<&'a Property>> {
    let Some(id) = view
        .config
        .get("board_subgroup_by")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let primary = group_property(db, view)?;
    let property = db
        .properties
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| invalid("The Board subgroup property is unavailable."))?;
    if property.id == primary.id || !group_supported(property.kind) {
        return Err(invalid("Choose a different supported subgroup property."));
    }
    Ok(Some(property))
}
fn options(property: &Property) -> Vec<Value> {
    let mut values = Vec::new();
    if matches!(property.kind, PropertyKind::Select | PropertyKind::Status) {
        for option in property
            .config
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(MAX_GROUPS + 1)
        {
            if let Some(name) = option
                .as_str()
                .or_else(|| option.get("name").and_then(Value::as_str))
                .filter(|name| !name.is_empty())
            {
                let value = json!(name);
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
    } else if property.kind == PropertyKind::Checkbox {
        values.extend([json!(false), json!(true)]);
    }
    values
}
fn option_count(property: &Property) -> usize {
    property
        .config
        .get("options")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn admit_group(
    groups: &mut Vec<Value>,
    seen: &mut BTreeSet<String>,
    totals: &BTreeMap<String, u64>,
    value: &Value,
    limit: usize,
) -> bool {
    let key = group_key(value);
    if seen.contains(&key) {
        return true;
    }
    while groups.len() >= limit {
        let Some(index) = groups.iter().rposition(|group| {
            !group.is_null() && totals.get(&group_key(group)).copied().unwrap_or(0) == 0
        }) else {
            return false;
        };
        let removed = groups.remove(index);
        seen.remove(&group_key(&removed));
    }
    seen.insert(key);
    groups.push(value.clone());
    true
}

fn editable_eq(property: &Property, value: &Value) -> bool {
    if !group_supported(property.kind) {
        return false;
    }
    if value.is_null() {
        return true;
    }
    match property.kind {
        PropertyKind::Number => value.is_number(),
        PropertyKind::Checkbox => value.is_boolean(),
        PropertyKind::Date => value
            .as_str()
            .and_then(|s| s.get(..10))
            .is_some_and(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()),
        _ => value.is_string(),
    }
}
fn eq_value(value: &Value) -> Value {
    if value.is_number() {
        normalize(value)
    } else {
        value.clone()
    }
}
fn compatible_eq(property: &Property, actual: &Value, expected: &Value) -> bool {
    if property.kind == PropertyKind::Date {
        if let Some(expected) = expected.as_str() {
            let actual = actual
                .as_str()
                .or_else(|| actual.get("start").and_then(Value::as_str));
            return if expected.len() == 10 {
                actual.and_then(|s| s.get(..10)) == Some(expected)
            } else {
                actual == Some(expected)
            };
        }
    }
    eq_value(actual) == eq_value(expected)
}
fn required_eq(
    db: &Database,
    filter: &FilterExpr,
    depth: usize,
) -> Result<Option<BTreeMap<String, Value>>> {
    if depth > 12 {
        return Err(invalid("The Board filter is too deep."));
    }
    match filter {
        FilterExpr::Rule(rule) => {
            let property = db
                .properties
                .iter()
                .find(|p| p.id == rule.property_id)
                .ok_or_else(|| invalid("The Board filter property is unavailable."))?;
            Ok(Some(
                if matches!(rule.op, FilterOp::Eq) && editable_eq(property, &rule.value) {
                    [(property.id.clone(), eq_value(&rule.value))].into()
                } else {
                    BTreeMap::new()
                },
            ))
        }
        FilterExpr::All(children) => {
            if children.len() > 100 {
                return Err(invalid("The Board has too many filter rules."));
            }
            let mut required = BTreeMap::new();
            for child in children {
                let Some(values) = required_eq(db, child, depth + 1)? else {
                    return Ok(None);
                };
                for (id, value) in values {
                    if let Some(old) = required.get(&id) {
                        let property = db.properties.iter().find(|p| p.id == id).unwrap();
                        if !compatible_eq(property, old, &value) {
                            if compatible_eq(property, &value, old) {
                                required.insert(id, value);
                            } else {
                                return Ok(None);
                            }
                        }
                    } else {
                        required.insert(id, value);
                    }
                }
            }
            Ok(Some(required))
        }
        FilterExpr::Any(children) => {
            if children.len() > 100 {
                return Err(invalid("The Board has too many filter rules."));
            }
            let mut common: Option<BTreeMap<String, Value>> = None;
            for child in children {
                let Some(values) = required_eq(db, child, depth + 1)? else {
                    continue;
                };
                if let Some(common) = &mut common {
                    common.retain(|id, value| values.get(id) == Some(value));
                } else {
                    common = Some(values);
                }
            }
            Ok(common)
        }
    }
}
fn context_possible(db: &Database, filter: &FilterExpr, context: &BTreeMap<String, Value>) -> bool {
    match filter {
        FilterExpr::Rule(rule) if matches!(rule.op, FilterOp::Eq) => {
            let Some(value) = context.get(&rule.property_id) else {
                return true;
            };
            let Some(property) = db.properties.iter().find(|p| p.id == rule.property_id) else {
                return false;
            };
            !editable_eq(property, &rule.value)
                || compatible_eq(property, value, &rule.value)
                || (normalize(value).is_null() && normalize(&rule.value).is_null())
        }
        FilterExpr::Rule(_) => true,
        FilterExpr::All(children) => children
            .iter()
            .all(|child| context_possible(db, child, context)),
        FilterExpr::Any(children) => children
            .iter()
            .any(|child| context_possible(db, child, context)),
    }
}
/// Inherit only equality values required in every matching branch; never choose an OR branch.
pub fn creation_values(
    db: &Database,
    view: &DatabaseView,
    mut context: BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>> {
    for (id, value) in &context {
        let property = db
            .properties
            .iter()
            .find(|property| property.id == *id)
            .ok_or_else(|| invalid("The Board creation property is unavailable."))?;
        if !group_supported(property.kind) || !mutations::valid_group(property, value) {
            return Err(invalid(
                "This Board creation context is not editable or has an invalid value.",
            ));
        }
    }
    let Some(filter) = &view.query.filter else {
        return Ok(context);
    };
    let required=required_eq(db,filter,0)?.ok_or_else(||invalid("The Board filter has conflicting equality rules. Edit the filter before adding a card."))?;
    if !context_possible(db, filter, &context) {
        return Err(invalid(
            "This column conflicts with the Board filter. Choose another column or edit the filter.",
        ));
    }
    for (id, value) in required {
        let property = db.properties.iter().find(|p| p.id == id).unwrap();
        if let Some(actual) = context.get(&id) {
            if compatible_eq(property, actual, &value) {
                continue;
            }
            if !(normalize(actual).is_null() && normalize(&value).is_null()) {
                return Err(invalid(
                    "This column conflicts with the Board filter. Choose another column or edit the filter.",
                ));
            }
        }
        context.insert(id, value);
    }
    Ok(context)
}

#[derive(Clone, Debug)]
pub struct BoardBucket {
    pub group: Value,
    pub subgroup: Value,
    pub total: u64,
    pub offset: u64,
    pub records: Vec<Record>,
}
#[derive(Clone, Debug)]
pub struct BoardProjection {
    pub group_property: Property,
    pub subgroup_property: Option<Property>,
    pub groups: Vec<Value>,
    pub subgroups: Vec<Value>,
    pub buckets: Vec<BoardBucket>,
    pub group_totals: BTreeMap<String, u64>,
    pub subgroup_totals: BTreeMap<String, u64>,
    pub total: u64,
    pub groups_truncated: bool,
    pub appearance: BTreeMap<String, RecordPageAppearance>,
    pub previews: BTreeMap<String, String>,
    pub covers: BTreeMap<String, PageCover>,
    pub order: BTreeMap<String, Vec<u8>>,
}

/// Immediate in-memory view while the full query runs in the reader.
pub fn project_loaded(
    db: &Database,
    view: &DatabaseView,
    records: &[Record],
) -> Result<BoardProjection> {
    let property = group_property(db, view)?;
    let sub = subgroup_property(db, view)?;
    let mut output = BoardProjection {
        group_property: property.clone(),
        subgroup_property: sub.cloned(),
        groups: options(property),
        subgroups: sub.map(options).unwrap_or_else(|| vec![Value::Null]),
        buckets: Vec::new(),
        group_totals: BTreeMap::new(),
        subgroup_totals: BTreeMap::new(),
        total: records.len() as u64,
        groups_truncated: option_count(property) > MAX_GROUPS
            || sub.is_some_and(|p| option_count(p) > MAX_SUBGROUPS),
        appearance: BTreeMap::new(),
        previews: BTreeMap::new(),
        covers: BTreeMap::new(),
        order: BTreeMap::new(),
    };
    output.groups.truncate(MAX_GROUPS);
    output.subgroups.truncate(MAX_SUBGROUPS);
    if !output.groups.contains(&Value::Null) {
        output.groups.push(Value::Null);
    }
    if !output.subgroups.contains(&Value::Null) {
        output.subgroups.push(Value::Null);
    }
    let mut seen_groups = output.groups.iter().map(group_key).collect();
    let mut seen_subgroups = output.subgroups.iter().map(group_key).collect();
    for record in records {
        let group = group_value(record, property);
        let subgroup = sub.map(|p| group_value(record, p)).unwrap_or(Value::Null);
        let group_kept = admit_group(
            &mut output.groups,
            &mut seen_groups,
            &output.group_totals,
            &group,
            MAX_GROUPS + 1,
        );
        let subgroup_kept = admit_group(
            &mut output.subgroups,
            &mut seen_subgroups,
            &output.subgroup_totals,
            &subgroup,
            MAX_SUBGROUPS + 1,
        );
        if !group_kept || !subgroup_kept {
            output.groups_truncated = true;
            continue;
        }
        *output.group_totals.entry(group_key(&group)).or_default() += 1;
        *output
            .subgroup_totals
            .entry(group_key(&subgroup))
            .or_default() += 1;
        if !output.groups.contains(&group) {
            output.groups.push(group.clone());
        }
        if !output.subgroups.contains(&subgroup) {
            output.subgroups.push(subgroup.clone());
        }
        if !output
            .buckets
            .iter()
            .any(|bucket| bucket.group == group && bucket.subgroup == subgroup)
            && output.buckets.len() >= MAX_BUCKETS
        {
            output.groups_truncated = true;
            continue;
        }
        let index = output
            .buckets
            .iter()
            .position(|bucket| bucket.group == group && bucket.subgroup == subgroup)
            .unwrap_or_else(|| {
                output.buckets.push(BoardBucket {
                    group,
                    subgroup,
                    total: 0,
                    offset: 0,
                    records: Vec::new(),
                });
                output.buckets.len() - 1
            });
        let bucket = &mut output.buckets[index];
        bucket.total += 1;
        if bucket.records.len() < BOARD_PAGE_SIZE {
            bucket.records.push(record.clone());
            output
                .order
                .insert(record.id.clone(), record.id.as_bytes().to_vec());
        }
    }
    if !output.groups.contains(&Value::Null) {
        output.groups.push(Value::Null);
    }
    if !output.subgroups.contains(&Value::Null) {
        output.subgroups.push(Value::Null);
    }
    Ok(output)
}

impl Store {
    /// Count all matches and read one bounded window from each group/subgroup bucket.
    pub fn query_board(
        &self,
        database_id: &str,
        view_id: &str,
        title_search: &str,
        offsets: &BTreeMap<String, u64>,
    ) -> Result<BoardProjection> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("Database is unavailable."))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("Database page is unavailable."))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("Database is in Trash."));
        }
        let view = db
            .views
            .iter()
            .find(|v| v.id == view_id && v.kind == ViewKind::Board)
            .ok_or_else(|| invalid("Board view is unavailable."))?;
        let property = group_property(&db, view)?;
        let subgroup = subgroup_property(&db, view)?;
        let mut output = BoardProjection {
            group_property: property.clone(),
            subgroup_property: subgroup.cloned(),
            groups: options(property),
            subgroups: subgroup.map(options).unwrap_or_else(|| vec![Value::Null]),
            buckets: Vec::new(),
            group_totals: BTreeMap::new(),
            subgroup_totals: BTreeMap::new(),
            total: 0,
            groups_truncated: false,
            appearance: BTreeMap::new(),
            previews: BTreeMap::new(),
            covers: BTreeMap::new(),
            order: BTreeMap::new(),
        };
        output.groups_truncated = option_count(property) > MAX_GROUPS
            || subgroup.is_some_and(|p| option_count(p) > MAX_SUBGROUPS);
        output.groups.truncate(MAX_GROUPS);
        output.subgroups.truncate(MAX_SUBGROUPS);
        if !output.groups.contains(&Value::Null) {
            output.groups.push(Value::Null);
        }
        if !output.subgroups.contains(&Value::Null) {
            output.subgroups.push(Value::Null);
        }
        let mut seen_groups: BTreeSet<_> = output.groups.iter().map(group_key).collect();
        let mut seen_subgroups: BTreeSet<_> = output.subgroups.iter().map(group_key).collect();
        let mut indices: BTreeMap<String, usize> = BTreeMap::new();
        let mut args = vec![
            SqlValue::Text(view_id.into()),
            SqlValue::Text(database_id.into()),
        ];
        let mut sql = String::from(
            "SELECT records.data, COALESCE(bo.rank,CAST(records.id AS BLOB)) FROM records LEFT JOIN board_record_order bo ON bo.record_id=records.id AND bo.view_id=? WHERE records.database_id=? AND records.deleted_at IS NULL",
        );
        if let Some(filter) = &view.query.filter {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !title_search.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("Title property is unavailable."))?;
            let filter = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: json!(title_search),
            });
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(&filter, &db, &mut args, 0)?);
        }
        if view.query.sorts.len() > 8 {
            return Err(invalid("Too many sort rules."));
        }
        sql.push_str(" ORDER BY ");
        for sort in &view.query.sorts {
            sql.push_str(&property_expr(&sort.property_id, &db, &mut args)?);
            sql.push_str(if sort.descending {
                " DESC NULLS LAST, "
            } else {
                " ASC NULLS LAST, "
            });
        }
        sql.push_str("COALESCE(bo.rank,CAST(records.id AS BLOB)) ASC, records.id ASC");
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(args))?;
        while let Some(row) = rows.next()? {
            let record: Record = decode(row.get(0)?)?;
            output.total += 1;
            let group = group_value(&record, property);
            let sub = subgroup
                .map(|p| group_value(&record, p))
                .unwrap_or(Value::Null);
            let key = group_key(&group);
            let subkey = group_key(&sub);
            let group_kept = admit_group(
                &mut output.groups,
                &mut seen_groups,
                &output.group_totals,
                &group,
                MAX_GROUPS + 1,
            );
            let subgroup_kept = admit_group(
                &mut output.subgroups,
                &mut seen_subgroups,
                &output.subgroup_totals,
                &sub,
                MAX_SUBGROUPS + 1,
            );
            if !group_kept || !subgroup_kept {
                output.groups_truncated = true;
            }
            if group_kept {
                *output.group_totals.entry(key).or_default() += 1;
            }
            if subgroup_kept {
                *output.subgroup_totals.entry(subkey).or_default() += 1;
            }
            if !group_kept || !subgroup_kept {
                continue;
            }
            let bucket = bucket_key(&group, &sub);
            let index = if let Some(index) = indices.get(&bucket) {
                *index
            } else {
                if indices.len() >= MAX_BUCKETS {
                    output.groups_truncated = true;
                    continue;
                }
                let index = output.buckets.len();
                indices.insert(bucket.clone(), index);
                output.buckets.push(BoardBucket {
                    group,
                    subgroup: sub,
                    total: 0,
                    offset: offsets.get(&bucket).copied().unwrap_or(0),
                    records: Vec::new(),
                });
                index
            };
            let bucket = &mut output.buckets[index];
            let ordinal = bucket.total;
            bucket.total += 1;
            if ordinal >= bucket.offset && bucket.records.len() < BOARD_PAGE_SIZE {
                output.order.insert(record.id.clone(), row.get(1)?);
                bucket.records.push(record);
            }
        }
        drop(rows);
        drop(statement);
        let ids: Vec<_> = output
            .buckets
            .iter()
            .flat_map(|bucket| bucket.records.iter().map(|r| r.id.clone()))
            .collect();
        output.appearance = self.record_page_appearance(&owner.workspace_id, &ids)?;
        output.previews = self.record_page_previews(&owner.workspace_id, &ids)?;
        let mut statement=self.conn.prepare("SELECT r.id,p.data FROM json_each(?1) requested JOIN records r ON r.id=requested.value JOIN pages p ON p.id=r.page_id WHERE p.deleted_at IS NULL")?;
        let mut rows = statement.query([encode(&ids)?])?;
        while let Some(row) = rows.next()? {
            let page: Page = decode(row.get(1)?)?;
            if let Some(cover) = page.cover {
                output.covers.insert(row.get(0)?, cover);
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
