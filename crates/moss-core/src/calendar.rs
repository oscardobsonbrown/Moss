use crate::{
    Database, FilterExpr, FilterOp, FilterRule, PropertyKind, Record, RecordPageAppearance,
    RecordQuery, Store, ViewKind, search,
    store::{decode, filter_sql, invalid, property_expr},
};
use chrono::NaiveDate;
use rusqlite::{Result, params_from_iter, types::Value as SqlValue};
use std::collections::BTreeMap;

const START: &str = "date(substr(CASE WHEN calendar_date.type='text' THEN calendar_date.value WHEN calendar_date.type='object' THEN json_extract(calendar_date.value,'$.start') END,1,10))";
const END: &str = "COALESCE(date(substr(CASE WHEN calendar_date.type='object' THEN json_extract(calendar_date.value,'$.end') END,1,10)), date(substr(CASE WHEN calendar_date.type='text' THEN calendar_date.value WHEN calendar_date.type='object' THEN json_extract(calendar_date.value,'$.start') END,1,10)))";

#[derive(Debug)]
pub struct CalendarProjection {
    pub records: Vec<Record>,
    pub undated: Vec<Record>,
    pub undated_count: u64,
    pub dated_count: u64,
    pub offset: u64,
    pub appearance: BTreeMap<String, RecordPageAppearance>,
}

fn ordering(db: &Database, query: &RecordQuery, args: &mut Vec<SqlValue>) -> Result<String> {
    if query.sorts.len() > 8 {
        return Err(invalid("too many sort rules"));
    }
    let mut order = String::from(" ORDER BY ");
    for sort in &query.sorts {
        order.push_str(&property_expr(&sort.property_id, db, args)?);
        order.push_str(if sort.descending {
            " DESC NULLS LAST, "
        } else {
            " ASC NULLS LAST, "
        });
    }
    order.push_str("id ASC");
    Ok(order)
}

impl Store {
    pub fn set_calendar_date_if_unchanged(
        &mut self,
        record_id: &str,
        property_id: &str,
        expected: &serde_json::Value,
        value: serde_json::Value,
    ) -> Result<Record> {
        let start = value
            .as_str()
            .or_else(|| value.get("start").and_then(|v| v.as_str()))
            .and_then(|v| v.get(..10))
            .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
            .ok_or_else(|| invalid("invalid calendar date"))?;
        if let Some(end) = value.get("end").filter(|v| !v.is_null()) {
            let end = end
                .as_str()
                .and_then(|v| v.get(..10))
                .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
                .ok_or_else(|| invalid("invalid calendar end date"))?;
            if end < start {
                return Err(invalid("calendar end precedes start"));
            }
        }
        self.set_record_value_if_unchanged_kind(
            record_id,
            property_id,
            expected,
            value,
            Some(PropertyKind::Date),
        )
    }

    pub fn query_calendar(
        &self,
        database_id: &str,
        view_id: &str,
        search_text: &str,
        undated_search: &str,
        start: NaiveDate,
        end: NaiveDate,
        offset: u64,
    ) -> Result<CalendarProjection> {
        if end < start || (end - start).num_days() > 62 {
            return Err(invalid("invalid calendar interval"));
        }
        let db: Database = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() {
            return Err(invalid("database in trash"));
        }
        let view = db
            .views
            .iter()
            .find(|view| view.id == view_id && view.kind == ViewKind::Calendar)
            .ok_or_else(|| invalid("calendar view missing"))?;
        let property = match view
            .config
            .get("calendar_date_property")
            .and_then(|v| v.as_str())
        {
            Some(id) => db
                .properties
                .iter()
                .find(|p| p.id == id && p.kind == PropertyKind::Date),
            None => db.properties.iter().find(|p| p.kind == PropertyKind::Date),
        }
        .ok_or_else(|| invalid("calendar date property missing"))?;
        let mut args = vec![SqlValue::Text(database_id.into())];
        let mut base = String::from(" FROM records WHERE database_id=? AND deleted_at IS NULL");
        if let Some(filter) = &view.query.filter {
            base.push_str(" AND ");
            base.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !search_text.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("title property missing"))?;
            let filter = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: search_text.into(),
            });
            base.push_str(" AND ");
            base.push_str(&filter_sql(&filter, &db, &mut args, 0)?);
        }
        let dated = format!(
            " EXISTS(SELECT 1 FROM json_each(records.values_json) calendar_date WHERE calendar_date.key=? AND {START} IS NOT NULL AND {START}<=? AND {END}>=?)"
        );
        let mut dated_args = args.clone();
        dated_args.extend([
            SqlValue::Text(property.id.clone()),
            SqlValue::Text(end.to_string()),
            SqlValue::Text(start.to_string()),
        ]);
        let dated_count: i64 = self.conn.query_row(
            &format!("SELECT count(*){base} AND {dated}"),
            params_from_iter(dated_args.iter()),
            |row| row.get(0),
        )?;
        let dated_count = dated_count as u64;
        let offset = offset.min(dated_count.saturating_sub(1) / 1000 * 1000);
        let order = ordering(&db, &view.query, &mut dated_args)?;
        dated_args.push(SqlValue::Integer(
            i64::try_from(offset).map_err(|_| invalid("calendar offset too large"))?,
        ));
        let mut statement = self.conn.prepare(&format!(
            "SELECT data{base} AND {dated}{order} LIMIT 1000 OFFSET ?"
        ))?;
        let records: Vec<Record> = statement
            .query_map(params_from_iter(dated_args), |row| decode(row.get(0)?))?
            .collect::<Result<_>>()?;
        let undated_clause = format!(
            " NOT EXISTS(SELECT 1 FROM json_each(records.values_json) calendar_date WHERE calendar_date.key=? AND {START} IS NOT NULL)"
        );
        let mut undated_args = args;
        undated_args.push(SqlValue::Text(property.id.clone()));
        let undated_count: i64 = self.conn.query_row(
            &format!("SELECT count(*){base} AND {undated_clause}"),
            params_from_iter(undated_args.iter()),
            |row| row.get(0),
        )?;
        let mut undated_sql = format!("SELECT data{base} AND {undated_clause}");
        if !undated_search.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("title property missing"))?;
            let filter = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: undated_search.into(),
            });
            undated_sql.push_str(" AND ");
            undated_sql.push_str(&filter_sql(&filter, &db, &mut undated_args, 0)?);
        }
        undated_sql.push_str(&ordering(&db, &view.query, &mut undated_args)?);
        // shortcut: cap undated results at 100; search narrows the full set.
        undated_sql.push_str(" LIMIT 100");
        let mut statement = self.conn.prepare(&undated_sql)?;
        let undated: Vec<Record> = statement
            .query_map(params_from_iter(undated_args), |row| decode(row.get(0)?))?
            .collect::<Result<_>>()?;
        let ids: Vec<_> = records
            .iter()
            .chain(&undated)
            .map(|r| r.id.clone())
            .collect();
        let appearance = self.record_page_appearance(&owner.workspace_id, &ids)?;
        Ok(CalendarProjection {
            records,
            undated,
            undated_count: undated_count as u64,
            dated_count,
            offset,
            appearance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseView, Property, RecordQuery, new_id};
    use serde_json::json;
    #[test]
    fn calendar_projects_matching_ranges_and_full_undated_count() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Calendar", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Calendar".into(),
            properties: vec![
                Property {
                    id: "title".into(),
                    name: "Name".into(),
                    kind: PropertyKind::Title,
                    config: json!({}),
                },
                Property {
                    id: "date".into(),
                    name: "Date".into(),
                    kind: PropertyKind::Date,
                    config: json!({}),
                },
                Property {
                    id: "other".into(),
                    name: "Other".into(),
                    kind: PropertyKind::Date,
                    config: json!({}),
                },
            ],
            views: vec![DatabaseView {
                id: new_id(),
                name: "Calendar".into(),
                kind: ViewKind::Calendar,
                query: RecordQuery::default(),
                config: json!({"calendar_date_property":"other"}),
            }],
        };
        store
            .create_database_page(&workspace.id, None, "Calendar", "", db.clone())
            .unwrap();
        for i in 0..125 {
            store
                .create_record(&db.id, &format!("Undated {i}"), &BTreeMap::new())
                .unwrap();
        }
        let range = store
            .create_record(
                &db.id,
                "Range",
                &BTreeMap::from([(
                    "other".into(),
                    json!({"start":"2026-10-29T09:00", "end":"2026-11-03T17:00"}),
                )]),
            )
            .unwrap();
        store
            .create_record(
                &db.id,
                "Other date",
                &BTreeMap::from([("date".into(), json!("2026-11-01"))]),
            )
            .unwrap();
        let projected = store
            .query_calendar(
                &db.id,
                &db.views[0].id,
                "",
                "",
                NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
                0,
            )
            .unwrap();
        assert_eq!(projected.records.len(), 1);
        assert_eq!(projected.records[0].id, range.id);
        assert_eq!(projected.undated_count, 126);
        assert_eq!(projected.undated.len(), 100);
        let searched = store
            .query_calendar(
                &db.id,
                &db.views[0].id,
                "Range",
                "",
                NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
                0,
            )
            .unwrap();
        assert_eq!(searched.records.len(), 1);
        assert_eq!(searched.undated_count, 0);
        let filtered = store
            .query_calendar(
                &db.id,
                &db.views[0].id,
                "",
                "Undated 124",
                NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
                0,
            )
            .unwrap();
        assert_eq!(filtered.undated_count, 126);
        assert_eq!(filtered.undated.len(), 1);
        let expected = range.values["other"].clone();
        assert!(
            store
                .set_calendar_date_if_unchanged(
                    &range.id,
                    "other",
                    &expected,
                    json!({"start":"2026-11-10", "end":"2026-11-09"})
                )
                .is_err()
        );
        for i in 0..1001 {
            store
                .create_record(
                    &db.id,
                    &format!("Dated {i}"),
                    &BTreeMap::from([("other".into(), json!("2026-11-12"))]),
                )
                .unwrap();
        }
        let start = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap();
        let first = store
            .query_calendar(&db.id, &db.views[0].id, "", "", start, end, 0)
            .unwrap();
        let second = store
            .query_calendar(&db.id, &db.views[0].id, "", "", start, end, 1000)
            .unwrap();
        assert_eq!(first.dated_count, 1002);
        assert_eq!((first.records.len(), second.records.len()), (1000, 2));
        assert!(
            second
                .records
                .iter()
                .all(|r| first.records.iter().all(|other| other.id != r.id))
        );
        assert!(
            store
                .set_calendar_date_if_unchanged(
                    &range.id,
                    "title",
                    &serde_json::Value::Null,
                    json!("2026-11-10")
                )
                .is_err()
        );
    }
}
