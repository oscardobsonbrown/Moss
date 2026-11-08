use crate::{
    Database, PageKind, Property, PropertyKind, Store, calculation, formula, new_id,
    properties::with_database,
    search,
    store::{decode, encode, invalid},
};
use rusqlite::{Result, Transaction, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn graph(
    tx: &Transaction<'_>,
    workspace: &str,
    current: &Database,
    strict_property: Option<&str>,
) -> Result<()> {
    let mut all = BTreeMap::<String, Database>::new();
    let mut stmt = tx.prepare("SELECT d.data FROM databases d JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 AND p.deleted_at IS NULL")?;
    for row in stmt.query_map([workspace], |r| decode::<Database>(r.get(0)?))? {
        let db = row?;
        all.insert(db.id.clone(), db);
    }
    all.insert(current.id.clone(), current.clone());
    let mut edges = BTreeMap::<(String, String), Vec<(String, String)>>::new();
    for db in all.values() {
        for property in &db.properties {
            let key = (db.id.clone(), property.id.clone());
            let result: Result<Vec<(String, String)>> = (|| {
                let mut deps = Vec::new();
                if property.kind == PropertyKind::Formula {
                    let source = property
                        .config
                        .get("expression")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("formula expression missing"))?;
                    let bindings: Option<BTreeMap<String, String>> = property
                        .config
                        .get("_bindings")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    let ids = if let Some(bindings) = bindings {
                        bindings.into_values().collect()
                    } else {
                        formula::dependencies(source, &db.properties)
                            .map_err(|e| invalid(&e.to_string()))?
                    };
                    for id in ids {
                        if !db.properties.iter().any(|p| p.id == id) {
                            return Err(invalid("formula references a missing property"));
                        }
                        deps.push((db.id.clone(), id));
                    }
                } else if property.kind == PropertyKind::Rollup {
                    let relation_id = property
                        .config
                        .get("relation_property_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("rollup relation missing"))?;
                    let target_id = property
                        .config
                        .get("target_property_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("rollup target property missing"))?;
                    let relation = db
                        .properties
                        .iter()
                        .find(|p| p.id == relation_id && p.kind == PropertyKind::Relation)
                        .ok_or_else(|| invalid("rollup relation missing"))?;
                    let target_db_id = relation
                        .config
                        .get("target_database_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("relation target missing"))?;
                    let target = all
                        .get(target_db_id)
                        .ok_or_else(|| invalid("relation target missing"))?;
                    if !target.properties.iter().any(|p| p.id == target_id) {
                        return Err(invalid("rollup target property missing"));
                    }
                    deps.push((target.id.clone(), target_id.into()));
                }
                Ok(deps)
            })();
            let deps = match result {
                Ok(deps) => deps,
                Err(error)
                    if db.id == current.id && strict_property == Some(property.id.as_str()) =>
                {
                    return Err(error);
                }
                Err(_) => Vec::new(),
            };
            edges.insert(key, deps);
        }
    }
    fn visit(
        key: &(String, String),
        edges: &BTreeMap<(String, String), Vec<(String, String)>>,
        active: &mut BTreeSet<(String, String)>,
        done: &mut BTreeSet<(String, String)>,
    ) -> Result<()> {
        if done.contains(key) {
            return Ok(());
        }
        if !active.insert(key.clone()) {
            return Err(invalid("circular calculation dependency"));
        }
        if let Some(children) = edges.get(key) {
            for child in children {
                visit(child, edges, active, done)?;
            }
        }
        active.remove(key);
        done.insert(key.clone());
        Ok(())
    }
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    for key in edges.keys() {
        visit(key, &edges, &mut active, &mut done)?;
    }
    Ok(())
}

impl Store {
    pub fn save_advanced_property(
        &mut self,
        database_id: &str,
        expected: Option<&Property>,
        name: &str,
        kind: PropertyKind,
        mut config: Value,
        insert_relative: Option<(&str, bool)>,
    ) -> Result<Database> {
        let name = name.trim();
        if name.is_empty() {
            return Err(invalid("property name cannot be empty"));
        }
        if !matches!(
            kind,
            PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
        ) {
            return Err(invalid("unsupported advanced property type"));
        }
        if !config.is_object() {
            return Err(invalid("invalid property configuration"));
        }
        with_database(self, database_id, |tx, db, owner| {
            let position = if let Some(expected) = expected {
                let i = db
                    .properties
                    .iter()
                    .position(|p| p.id == expected.id)
                    .ok_or_else(|| invalid("property missing"))?;
                if db.properties[i] != *expected || expected.kind != kind {
                    return Err(invalid("property changed in another editor"));
                }
                i
            } else if let Some((anchor, after)) = insert_relative {
                let i = db
                    .properties
                    .iter()
                    .position(|p| p.id == anchor)
                    .ok_or_else(|| invalid("anchor property missing"))?
                    + usize::from(after);
                i
            } else {
                db.properties.len()
            };
            let id = expected.map(|p| p.id.clone()).unwrap_or_else(new_id);
            if kind == PropertyKind::Relation {
                let target = config
                    .get("target_database_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("relation target missing"))?;
                let raw: String =
                    tx.query_row("SELECT data FROM databases WHERE id=?1", [target], |r| {
                        r.get(0)
                    })?;
                let target_db: Database = decode(raw)?;
                let target_page = search::page(tx, &target_db.page_id)?
                    .ok_or_else(|| invalid("relation target missing"))?;
                if target_page.deleted_at.is_some()
                    || target_page.kind != PageKind::Database
                    || target_page.workspace_id != owner.workspace_id
                {
                    return Err(invalid("invalid relation target"));
                }
                if expected.is_some_and(|old| {
                    old.config.get("target_database_id") != config.get("target_database_id")
                }) {
                    let has_values: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM records r, json_each(r.values_json) v WHERE r.database_id=?1 AND v.key=?2 AND v.value IS NOT NULL AND v.value <> '[]')", params![db.id, id], |r| r.get(0))?;
                    if has_values {
                        return Err(invalid("clear relation values before changing its target"));
                    }
                }
            }
            if kind == PropertyKind::Rollup {
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
            }
            let property = Property {
                id,
                name: name.into(),
                kind,
                config: config.clone(),
            };
            if expected.is_some() {
                db.properties[position] = property;
            } else {
                db.properties.insert(position, property);
            }
            if kind == PropertyKind::Formula {
                let source = config
                    .get("expression")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("formula expression missing"))?;
                let existing_bindings: Option<BTreeMap<String, String>> = expected
                    .filter(|old| {
                        old.config.get("expression").and_then(Value::as_str) == Some(source)
                    })
                    .and_then(|old| old.config.get("_bindings"))
                    .and_then(|value| serde_json::from_value(value.clone()).ok());
                let bindings = match existing_bindings {
                    Some(bindings)
                        if bindings
                            .values()
                            .all(|id| db.properties.iter().any(|p| &p.id == id)) =>
                    {
                        bindings
                    }
                    _ => formula::bindings(source, &db.properties)
                        .map_err(|e| invalid(&e.to_string()))?,
                };
                config
                    .as_object_mut()
                    .unwrap()
                    .insert("_bindings".into(), json!(bindings));
                db.properties[position].config = config;
            }
            graph(
                tx,
                &owner.workspace_id,
                db,
                Some(&db.properties[position].id),
            )?;
            tx.execute(
                "UPDATE databases SET data=?1 WHERE id=?2",
                params![encode(db)?, db.id],
            )?;
            let mut after = String::new();
            loop {
                let ids: Vec<String> = {
                    let mut q = tx.prepare("SELECT id FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256")?;
                    let rows = q.query_map(params![db.id, after], |r| r.get(0))?;
                    rows.collect::<Result<Vec<_>>>()?
                };
                if ids.is_empty() {
                    break;
                }
                after = ids.last().unwrap().clone();
                calculation::refresh(tx, &ids)?;
            }
            Ok(())
        })
    }
}
