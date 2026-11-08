use moss_core::{FilterExpr, FilterOp, Record, RecordQuery};
use serde_json::Value;
use std::cmp::Ordering;

fn value<'a>(r: &'a Record, key: &str) -> &'a Value {
    r.values.get(key).unwrap_or(&Value::Null)
}
fn empty(v: &Value) -> bool {
    v.is_null() || v.as_str().is_some_and(str::is_empty) || v.as_array().is_some_and(Vec::is_empty)
}
fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    if let (Some(a), Some(b)) = (a.as_f64(), b.as_f64()) {
        return a.partial_cmp(&b);
    }
    if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
        return Some(a.cmp(b));
    }
    if let (Some(a), Some(b)) = (a.as_bool(), b.as_bool()) {
        return Some(a.cmp(&b));
    }
    None
}
pub fn matches_filter(record: &Record, filter: &FilterExpr) -> bool {
    match filter {
        FilterExpr::All(items) => items.iter().all(|f| matches_filter(record, f)),
        FilterExpr::Any(items) => items.iter().any(|f| matches_filter(record, f)),
        FilterExpr::Rule(rule) => {
            let actual = value(record, &rule.property_id);
            let cmp = compare(actual, &rule.value);
            let contains = || {
                actual
                    .as_str()
                    .zip(rule.value.as_str())
                    .is_some_and(|(a, b)| a.contains(b))
                    || actual.as_array().is_some_and(|a| a.contains(&rule.value))
            };
            let equal = if actual.is_number() && rule.value.is_number() {
                cmp == Some(Ordering::Equal)
            } else {
                actual == &rule.value
            };
            match rule.op {
                FilterOp::Eq => equal,
                FilterOp::Ne => !equal,
                FilterOp::Contains => contains(),
                FilterOp::NotContains => !contains(),
                FilterOp::Gt => cmp == Some(Ordering::Greater),
                FilterOp::Ge => matches!(cmp, Some(Ordering::Greater | Ordering::Equal)),
                FilterOp::Lt => cmp == Some(Ordering::Less),
                FilterOp::Le => matches!(cmp, Some(Ordering::Less | Ordering::Equal)),
                FilterOp::IsEmpty => empty(actual),
                FilterOp::IsNotEmpty => !empty(actual),
            }
        }
    }
}
/// Intended for small, already-loaded projections. Storage owns paged queries.
/// Missing and null values sort last, regardless of direction.
pub fn apply_query(records: &[Record], query: &RecordQuery) -> Vec<Record> {
    let mut result: Vec<_> = records
        .iter()
        .filter(|r| {
            r.deleted_at.is_none() && query.filter.as_ref().is_none_or(|f| matches_filter(r, f))
        })
        .cloned()
        .collect();
    result.sort_by(|a, b| {
        for sort in &query.sorts {
            let (a, b) = (value(a, &sort.property_id), value(b, &sort.property_id));
            let order = if a.is_null() && b.is_null() {
                Ordering::Equal
            } else if a.is_null() {
                Ordering::Greater
            } else if b.is_null() {
                Ordering::Less
            } else {
                compare(a, b).unwrap_or_else(|| a.to_string().cmp(&b.to_string()))
            };
            if order != Ordering::Equal {
                // Missing and null values sort last in either direction.
                if a.is_null() || b.is_null() {
                    return order;
                }
                return if sort.descending {
                    order.reverse()
                } else {
                    order
                };
            }
        }
        a.id.cmp(&b.id)
    });
    result
        .into_iter()
        .skip(usize::try_from(query.offset).unwrap_or(usize::MAX))
        .take(query.limit as usize)
        .collect()
}
pub fn group_records(records: &[Record], property_id: &str) -> Vec<(Value, Vec<Record>)> {
    let mut groups: Vec<(Value, Vec<Record>)> = Vec::new();
    for record in records {
        let v = value(record, property_id);
        if let Some((_, items)) = groups.iter_mut().find(|(key, _)| key == v) {
            items.push(record.clone());
        } else {
            groups.push((v.clone(), vec![record.clone()]));
        }
    }
    groups
}
