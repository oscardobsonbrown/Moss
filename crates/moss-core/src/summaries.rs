use crate::{
    FilterExpr, FilterOp, FilterRule, PageKind, PropertyKind, Record, Store, search,
    store::{decode, filter_sql, invalid},
};
use rusqlite::{Result, params_from_iter, types::Value as SqlValue};
use serde_json::{Value, json};
use std::collections::HashSet;

impl Store {
    /// Summarize every matching record, independent of the displayed page or sort order.
    pub fn summarize_records(
        &self,
        database_id: &str,
        filter: Option<&FilterExpr>,
        title_search: &str,
        property_id: &str,
        operation: &str,
    ) -> Result<Value> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        let property = db
            .properties
            .iter()
            .find(|p| p.id == property_id)
            .ok_or_else(|| invalid("unknown summary property"))?;
        let numeric = matches!(operation, "sum" | "average" | "min" | "max");
        if numeric
            && !matches!(
                property.kind,
                PropertyKind::Number | PropertyKind::Formula | PropertyKind::Rollup
            )
        {
            return Err(invalid("summary requires a numeric property"));
        }
        if !matches!(
            operation,
            "count_all"
                | "count_values"
                | "count_empty"
                | "count_unique"
                | "sum"
                | "average"
                | "min"
                | "max"
        ) {
            return Err(invalid("unsupported summary"));
        }
        let mut args = vec![SqlValue::Text(database_id.into())];
        let mut sql =
            String::from("SELECT data FROM records WHERE database_id=? AND deleted_at IS NULL");
        if let Some(filter) = filter {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !title_search.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("title property missing"))?;
            let search = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: json!(title_search),
            });
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(&search, &db, &mut args, 0)?);
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(args))?;
        let (mut count, mut filled, mut sum, mut min, mut max) =
            (0u64, 0u64, 0f64, f64::INFINITY, f64::NEG_INFINITY);
        let mut unique = HashSet::new();
        while let Some(row) = rows.next()? {
            let record: Record = decode(row.get(0)?)?;
            let title = Value::String(record.title.clone());
            let value = if property.kind == PropertyKind::Title {
                &title
            } else {
                record.values.get(property_id).unwrap_or(&Value::Null)
            };
            count += 1;
            let empty = value.is_null()
                || value.as_str().is_some_and(str::is_empty)
                || value.as_array().is_some_and(Vec::is_empty);
            if empty {
                continue;
            }
            filled += 1;
            if operation == "count_unique" {
                unique.insert(
                    serde_json::to_string(value).map_err(|_| invalid("invalid summary value"))?,
                );
            }
            if numeric {
                let number = value
                    .as_f64()
                    .ok_or_else(|| invalid("summary requires numeric values"))?;
                sum += number;
                min = min.min(number);
                max = max.max(number);
            }
        }
        Ok(match operation {
            "count_all" => json!(count),
            "count_values" => json!(filled),
            "count_empty" => json!(count - filled),
            "count_unique" => json!(unique.len()),
            _ if filled == 0 => Value::Null,
            "sum" => json!(sum),
            "average" => json!(sum / filled as f64),
            "min" => json!(min),
            "max" => json!(max),
            _ => unreachable!(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, DatabaseView, Property, RecordQuery, ViewKind, new_id};
    use std::collections::BTreeMap;
    #[test]
    fn summaries_cover_every_matching_record_not_just_the_first_page() {
        let mut s = Store::in_memory().unwrap();
        let w = s.create_workspace("test", "").unwrap();
        let definition = Database {
            id: new_id(),
            page_id: String::new(),
            name: "db".into(),
            properties: vec![
                Property {
                    id: "name".into(),
                    name: "Name".into(),
                    kind: PropertyKind::Title,
                    config: json!({}),
                },
                Property {
                    id: "number".into(),
                    name: "N".into(),
                    kind: PropertyKind::Number,
                    config: json!({}),
                },
            ],
            views: vec![DatabaseView {
                id: "view".into(),
                name: "Table".into(),
                kind: ViewKind::Table,
                query: RecordQuery::default(),
                config: json!({}),
            }],
        };
        s.create_database_page(&w.id, None, "db", "", definition.clone())
            .unwrap();
        for i in 0..150 {
            s.create_record(
                &definition.id,
                if i % 2 == 0 { "match" } else { "other" },
                &BTreeMap::from([("number".into(), json!(i))]),
            )
            .unwrap();
        }
        assert_eq!(
            s.summarize_records(&definition.id, None, "match", "number", "count_all")
                .unwrap(),
            json!(75)
        );
        assert_eq!(
            s.summarize_records(&definition.id, None, "match", "number", "sum")
                .unwrap(),
            json!(5550.)
        );
        assert!(
            s.summarize_records(&definition.id, None, "", "name", "sum")
                .is_err()
        );
    }
}

#[cfg(test)]
mod read_only_tests {
    use crate::{
        Database, DatabaseView, Property, PropertyKind, RecordQuery, Store, ViewKind, new_id,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn read_only_store_reads_summary_and_rejects_writes() {
        let path = std::env::temp_dir().join(format!("moss-readonly-{}.sqlite", new_id()));
        let mut writer = Store::open(&path).unwrap();
        let workspace = writer.create_workspace("Test", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Test".into(),
            properties: vec![
                Property {
                    id: "name".into(),
                    name: "Name".into(),
                    kind: PropertyKind::Title,
                    config: json!({}),
                },
                Property {
                    id: "number".into(),
                    name: "Number".into(),
                    kind: PropertyKind::Number,
                    config: json!({}),
                },
            ],
            views: vec![DatabaseView {
                id: "view".into(),
                name: "Table".into(),
                kind: ViewKind::Table,
                query: RecordQuery::default(),
                config: json!({}),
            }],
        };
        writer
            .create_database_page(&workspace.id, None, "Test", "", db.clone())
            .unwrap();
        writer
            .create_record(
                &db.id,
                "row",
                &BTreeMap::from([("number".into(), json!(4))]),
            )
            .unwrap();
        let mut reader = Store::open_read_only(&path).unwrap();
        assert!(reader.get_database(&db.id).unwrap().is_some());
        assert_eq!(
            reader
                .summarize_records(&db.id, None, "", "number", "sum")
                .unwrap(),
            json!(4.0)
        );
        assert!(reader.create_workspace("No", "").is_err());
        drop(reader);
        drop(writer);
        std::fs::remove_file(path).unwrap();
    }
}
