use crate::{
    Database, FilterExpr, Page, PageKind, Property, PropertyKind, Record, Store, new_id, search,
    store::{decode, encode, invalid},
};
use rusqlite::{OptionalExtension, Result, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};

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

fn name(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid("property name cannot be empty"))
    } else {
        Ok(value)
    }
}
fn existing(db: &Database, expected: &Property) -> Result<usize> {
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
            PropertyKind::Select => json!({"options": []}),
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
        let property_name = name(property_name)?.to_owned();
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
        with_database(self, database_id, |_, db, _| {
            db.properties.push(Property {
                id: new_id(),
                name: property_name,
                kind: PropertyKind::Select,
                config: json!({"options": values}),
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
        let config = match kind {
            PropertyKind::Select => {
                if options.is_empty() {
                    return Err(invalid("select requires at least one option"));
                }
                let mut seen = std::collections::HashSet::new();
                let mut values = Vec::new();
                for option in options {
                    let option = option.trim();
                    if option.is_empty() || !seen.insert(option.to_owned()) {
                        return Err(invalid("select options must be nonempty and unique"));
                    }
                    values.push(option.to_owned());
                }
                json!({"options": values})
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
                    id: new_id(),
                    name: property_name,
                    kind,
                    config,
                },
            );
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
            let schema = db.properties.clone();
            for property in &mut db.properties {
                if property.kind == PropertyKind::Formula
                    && property.config.get("_bindings").is_none()
                {
                    if let Some(source) = property.config.get("expression").and_then(Value::as_str)
                    {
                        if let Ok(bindings) = crate::formula::bindings(source, &schema) {
                            if let Some(config) = property.config.as_object_mut() {
                                config.insert("_bindings".into(), json!(bindings));
                            }
                        }
                    }
                }
            }
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
                    let values = encode(&record.values)?;
                    tx.execute(
                        "UPDATE records SET values_json=?1,data=?2 WHERE id=?3",
                        params![values, encode(&record)?, record.id],
                    )?;
                    if record.deleted_at.is_none() {
                        let target = match &record.page_id {
                            Some(id) => search::page(tx, id)?
                                .ok_or_else(|| invalid("record page missing"))?,
                            None => owner.clone(),
                        };
                        search::index(
                            tx,
                            &format!("record:{}", record.id),
                            &target,
                            &format!("{} {}", record.title, values),
                        )?;
                    }
                }
            }
            // Recalculate local formulas and downstream rollups against the new schema.
            tx.execute(
                "UPDATE databases SET data=?1 WHERE id=?2",
                params![encode(db)?, db.id],
            )?;
            let mut after = String::new();
            loop {
                let ids: Vec<String> = {
                    let mut q = tx.prepare("SELECT id FROM records WHERE database_id=?1 AND id>?2 ORDER BY id LIMIT 256")?;
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
        assert_eq!(
            db.properties.last().unwrap().config["options"],
            json!(["One", "Two"])
        );
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
