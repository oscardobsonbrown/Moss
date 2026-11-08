use crate::{
    Database, Page, PropertyKind, Record, formula,
    rollup::{RollupOp, aggregate},
    search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, Transaction, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn fail(message: impl ToString) -> Value {
    json!({"_moss_error": message.to_string()})
}
pub fn calculation_error(value: &Value) -> Option<&Value> {
    value.get("_moss_error")
}
fn rollup_op(name: &str) -> Option<RollupOp> {
    Some(match name {
        "original" => RollupOp::Original,
        "unique" => RollupOp::Unique,
        "count" => RollupOp::Count,
        "count_values" => RollupOp::CountValues,
        "count_empty" => RollupOp::CountEmpty,
        "percent_empty" => RollupOp::PercentEmpty,
        "sum" => RollupOp::Sum,
        "average" => RollupOp::Average,
        "min" => RollupOp::Min,
        "max" => RollupOp::Max,
        _ => return None,
    })
}
fn db_optional(tx: &Transaction<'_>, id: &str) -> Result<Option<Database>> {
    let raw: Option<String> = tx
        .query_row("SELECT data FROM databases WHERE id=?1", [id], |r| r.get(0))
        .optional()?;
    raw.map(decode).transpose()
}
fn db_for(tx: &Transaction<'_>, id: &str) -> Result<Database> {
    db_optional(tx, id)?.ok_or_else(|| invalid("database missing"))
}
fn row_for(tx: &Transaction<'_>, id: &str) -> Result<Option<Record>> {
    let raw: Option<String> = tx
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .optional()?;
    raw.map(decode).transpose()
}
fn recompute_property(
    tx: &Transaction<'_>,
    db: &Database,
    record: &mut Record,
    index: usize,
    visiting: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
) -> Result<()> {
    let property = &db.properties[index];
    if !matches!(property.kind, PropertyKind::Formula | PropertyKind::Rollup)
        || done.contains(&property.id)
    {
        return Ok(());
    }
    if !visiting.insert(property.id.clone()) {
        record
            .values
            .insert(property.id.clone(), fail("Circular calculation"));
        return Ok(());
    }
    let value = match property.kind {
        PropertyKind::Formula => {
            let source = property
                .config
                .get("expression")
                .and_then(Value::as_str)
                .unwrap_or("");
            let bindings: BTreeMap<String, String> = property
                .config
                .get("_bindings")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let deps = if bindings.is_empty() {
                formula::dependencies(source, &db.properties)
            } else if bindings
                .values()
                .all(|id| db.properties.iter().any(|p| &p.id == id))
            {
                Ok(bindings.values().cloned().collect())
            } else {
                Err(crate::error("A referenced property was deleted"))
            };
            match deps {
                Err(e) => fail(e),
                Ok(deps) => {
                    for id in &deps {
                        if let Some(i) = db.properties.iter().position(|p| &p.id == id) {
                            recompute_property(tx, db, record, i, visiting, done)?;
                        }
                    }
                    if deps.iter().any(|id| {
                        record
                            .values
                            .get(id)
                            .is_some_and(|v| calculation_error(v).is_some())
                    }) {
                        fail("A referenced calculation has an error")
                    } else {
                        let mut values = record.values.clone();
                        if let Some(title) =
                            db.properties.iter().find(|p| p.kind == PropertyKind::Title)
                        {
                            values.insert(title.id.clone(), json!(record.title));
                        }
                        formula::evaluate_bound(source, &values, &db.properties, &bindings)
                            .unwrap_or_else(fail)
                    }
                }
            }
        }
        PropertyKind::Rollup => {
            let relation_id = property
                .config
                .get("relation_property_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let target_id = property
                .config
                .get("target_property_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let op = property
                .config
                .get("operation")
                .and_then(Value::as_str)
                .and_then(rollup_op);
            let relation = db
                .properties
                .iter()
                .find(|p| p.id == relation_id && p.kind == PropertyKind::Relation);
            match (relation, op) {
                (Some(relation), Some(op)) => {
                    let target_db_id = relation
                        .config
                        .get("target_database_id")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let target_db = db_optional(tx, target_db_id)?;
                    let target_prop = target_db
                        .as_ref()
                        .and_then(|db| db.properties.iter().find(|p| p.id == target_id));
                    if let Some(target_prop) = target_prop {
                        let mut values = Vec::new();
                        let mut invalid_relation = false;
                        match record.values.get(relation_id) {
                            None | Some(Value::Null) => {}
                            Some(Value::Array(ids)) => {
                                if ids.iter().any(|id| !id.is_string()) {
                                    invalid_relation = true;
                                } else {
                                    let mut query = tx.prepare("SELECT r.data FROM json_each(?1) requested LEFT JOIN records r ON r.id=requested.value ORDER BY CAST(requested.key AS INTEGER)")?;
                                    let mut rows = query.query([encode(ids)?])?;
                                    while let Some(row) = rows.next()? {
                                        let raw: Option<String> = row.get(0)?;
                                        let Some(raw) = raw else {
                                            invalid_relation = true;
                                            break;
                                        };
                                        let target: Record = decode(raw)?;
                                        if target.database_id != target_db_id {
                                            invalid_relation = true;
                                            break;
                                        }
                                        if target.deleted_at.is_none() {
                                            values.push(
                                                if target_prop.kind == PropertyKind::Title {
                                                    json!(target.title)
                                                } else {
                                                    target
                                                        .values
                                                        .get(target_id)
                                                        .cloned()
                                                        .unwrap_or(Value::Null)
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            _ => invalid_relation = true,
                        }
                        if invalid_relation {
                            fail("Relation contains an invalid or missing target")
                        } else if values.iter().any(|v| calculation_error(v).is_some()) {
                            fail("A related calculation has an error")
                        } else {
                            aggregate(op, &values).unwrap_or_else(fail)
                        }
                    } else {
                        fail("Rollup target property is missing")
                    }
                }
                _ => fail("Invalid rollup configuration"),
            }
        }
        _ => unreachable!(),
    };
    visiting.remove(&property.id);
    done.insert(property.id.clone());
    record.values.insert(property.id.clone(), value);
    Ok(())
}

fn reindex_edges(tx: &Transaction<'_>, db: &Database, record: &Record) -> Result<()> {
    tx.execute(
        "DELETE FROM calculation_edges WHERE source_id=?1",
        [&record.id],
    )?;
    if record.deleted_at.is_none() {
        for prop in db
            .properties
            .iter()
            .filter(|p| p.kind == PropertyKind::Relation)
        {
            if let Some(ids) = record.values.get(&prop.id).and_then(Value::as_array) {
                for id in ids.iter().filter_map(Value::as_str) {
                    tx.execute("INSERT OR IGNORE INTO calculation_edges(source_id,property_id,target_id) SELECT ?1,?2,id FROM records WHERE id=?3", params![record.id, prop.id, id])?;
                }
            }
        }
    }
    Ok(())
}

/// Update changed records and only rows that point to them through indexed relations.
pub(crate) fn refresh(tx: &Transaction<'_>, initial: &[String]) -> Result<()> {
    let mut queue: BTreeSet<String> = initial.iter().cloned().collect();
    let mut forced: BTreeSet<String> = queue.clone();
    while let Some(id) = queue.pop_first() {
        let Some(mut record) = row_for(tx, &id)? else {
            continue;
        };
        let db = db_for(tx, &record.database_id)?;
        let before = record.values.clone();
        let was_forced = forced.remove(&id);
        if was_forced {
            reindex_edges(tx, &db, &record)?;
        }
        if record.deleted_at.is_none() {
            let mut visiting = BTreeSet::new();
            let mut done = BTreeSet::new();
            for i in 0..db.properties.len() {
                recompute_property(tx, &db, &mut record, i, &mut visiting, &mut done)?;
            }
        }
        let changed = before != record.values;
        if changed {
            let values = encode(&record.values)?;
            tx.execute(
                "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
                params![values, encode(&record)?, record.id],
            )?;
            if record.deleted_at.is_none() {
                let owner: Page =
                    search::page(tx, record.page_id.as_deref().unwrap_or(&db.page_id))?
                        .ok_or_else(|| invalid("record page missing"))?;
                search::index(
                    tx,
                    &format!("record:{}", record.id),
                    &owner,
                    &format!("{} {values}", record.title),
                )?;
            }
        }
        if changed || was_forced {
            let mut query =
                tx.prepare("SELECT DISTINCT source_id FROM calculation_edges WHERE target_id=?1")?;
            let sources = query
                .query_map([&id], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>>>()?;
            queue.extend(sources);
        }
    }
    Ok(())
}
