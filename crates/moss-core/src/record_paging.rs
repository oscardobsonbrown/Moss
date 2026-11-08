use crate::{
    Database, PropertyKind, Record, RecordQuery, Store, search,
    store::{decode, encode, filter_sql, invalid},
};
use rusqlite::{Result, params_from_iter, types::Value as SqlValue};

#[derive(Clone, Debug)]
pub struct RecordCursor {
    database_id: String,
    query_key: String,
    record_id: String,
    title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageDirection {
    Next,
    Previous,
}

#[derive(Clone, Copy)]
enum CursorOrder {
    Id,
    TitleAscending,
}

fn order(db: &Database, query: &RecordQuery) -> Option<CursorOrder> {
    if query.group_by.is_some() {
        return None;
    }
    match query.sorts.as_slice() {
        [] => Some(CursorOrder::Id),
        [sort]
            if !sort.descending
                && db
                    .properties
                    .iter()
                    .any(|p| p.id == sort.property_id && p.kind == PropertyKind::Title) =>
        {
            Some(CursorOrder::TitleAscending)
        }
        _ => None,
    }
}

fn query_key(db: &Database, query: &RecordQuery) -> Result<String> {
    let schema: Vec<_> = db.properties.iter().map(|p| (&p.id, p.kind)).collect();
    encode(&(&query.filter, &query.sorts, &query.group_by, schema))
}

impl Store {
    /// Return an anchor only for orders backed by a matching index.
    pub fn record_cursor(
        &self,
        database_id: &str,
        query: &RecordQuery,
        record: &Record,
    ) -> Result<Option<RecordCursor>> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        if record.database_id != database_id {
            return Err(invalid("record belongs to another database"));
        }
        if order(&db, query).is_none() {
            return Ok(None);
        }
        Ok(Some(RecordCursor {
            database_id: database_id.into(),
            query_key: query_key(&db, query)?,
            record_id: record.id.clone(),
            title: record.title.clone(),
        }))
    }

    /// Seek from an anchor. Returned rows follow the view order for both directions.
    pub fn query_records_cursor(
        &self,
        database_id: &str,
        query: &RecordQuery,
        cursor: &RecordCursor,
        direction: PageDirection,
    ) -> Result<Vec<Record>> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() {
            return Err(invalid("database in trash"));
        }
        let order = order(&db, query).ok_or_else(|| invalid("cursor order is not indexed"))?;
        if cursor.database_id != database_id || cursor.query_key != query_key(&db, query)? {
            return Err(invalid("cursor no longer matches this query"));
        }
        let anchor = self
            .get_record(&cursor.record_id)?
            .ok_or_else(|| invalid("cursor record is no longer available"))?;
        if anchor.database_id != database_id
            || anchor.deleted_at.is_some()
            || (matches!(order, CursorOrder::TitleAscending) && anchor.title != cursor.title)
        {
            return Err(invalid("cursor record changed; reload this page"));
        }
        let mut args = vec![SqlValue::Text(database_id.into())];
        let mut sql =
            String::from("SELECT data FROM records WHERE database_id=? AND deleted_at IS NULL");
        match order {
            CursorOrder::Id => {
                sql.push_str(if direction == PageDirection::Next {
                    " AND id>?"
                } else {
                    " AND id<?"
                });
                args.push(SqlValue::Text(cursor.record_id.clone()));
            }
            CursorOrder::TitleAscending => {
                sql.push_str(if direction == PageDirection::Next {
                    " AND (title,id)>(?,?)"
                } else {
                    " AND (title,id)<(?,?)"
                });
                args.push(SqlValue::Text(cursor.title.clone()));
                args.push(SqlValue::Text(cursor.record_id.clone()));
            }
        }
        if let Some(filter) = &query.filter {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        sql.push_str(match (order, direction) {
            (CursorOrder::Id, PageDirection::Next) => " ORDER BY id ASC LIMIT ?",
            (CursorOrder::Id, PageDirection::Previous) => " ORDER BY id DESC LIMIT ?",
            (CursorOrder::TitleAscending, PageDirection::Next) => {
                " ORDER BY title ASC,id ASC LIMIT ?"
            }
            (CursorOrder::TitleAscending, PageDirection::Previous) => {
                " ORDER BY title DESC,id DESC LIMIT ?"
            }
        });
        args.push(SqlValue::Integer(i64::from(query.limit.min(1000))));
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows: Vec<Record> = statement
            .query_map(params_from_iter(args), |row| row.get::<_, String>(0))?
            .map(|row| decode(row?))
            .collect::<Result<_>>()?;
        if direction == PageDirection::Previous {
            rows.reverse();
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DatabaseView, FilterExpr, FilterOp, FilterRule, Property, RecordQuery, SortRule, ViewKind,
        new_id,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn fixture() -> (Store, String) {
        let mut store = Store::in_memory().unwrap();
        let ws = store.create_workspace("Paging", "").unwrap();
        let page = store
            .create_database_page(
                &ws.id,
                None,
                "Pages",
                "",
                Database {
                    id: new_id(),
                    page_id: String::new(),
                    name: "Pages".into(),
                    properties: vec![
                        Property {
                            id: "title".into(),
                            name: "Name".into(),
                            kind: PropertyKind::Title,
                            config: json!({}),
                        },
                        Property {
                            id: "value".into(),
                            name: "Value".into(),
                            kind: PropertyKind::Number,
                            config: json!({}),
                        },
                    ],
                    views: vec![DatabaseView {
                        id: new_id(),
                        name: "Table".into(),
                        kind: ViewKind::Table,
                        query: RecordQuery::default(),
                        config: json!({}),
                    }],
                },
            )
            .unwrap();
        let db = store.get_database_by_page(&page.id).unwrap().unwrap();
        for n in 0..24 {
            store
                .create_record(
                    &db.id,
                    &format!("Title {}", n / 4),
                    &BTreeMap::from([("value".into(), json!(n % 3))]),
                )
                .unwrap();
        }
        (store, db.id)
    }

    #[test]
    fn indexed_cursor_matches_offset_in_both_directions_with_filter_and_ties() {
        let (store, db) = fixture();
        for sorts in [
            vec![],
            vec![SortRule {
                property_id: "title".into(),
                descending: false,
            }],
        ] {
            let query = RecordQuery {
                filter: Some(FilterExpr::Rule(FilterRule {
                    property_id: "value".into(),
                    op: FilterOp::Eq,
                    value: json!(1),
                })),
                sorts,
                limit: 3,
                ..Default::default()
            };
            let first = store.query_records(&db, &query).unwrap();
            let next_anchor = store
                .record_cursor(&db, &query, first.last().unwrap())
                .unwrap()
                .unwrap();
            let next = store
                .query_records_cursor(&db, &query, &next_anchor, PageDirection::Next)
                .unwrap();
            let offset = store
                .query_records(
                    &db,
                    &RecordQuery {
                        offset: 3,
                        ..query.clone()
                    },
                )
                .unwrap();
            assert_eq!(next, offset);
            let previous_anchor = store
                .record_cursor(&db, &query, next.first().unwrap())
                .unwrap()
                .unwrap();
            assert_eq!(
                store
                    .query_records_cursor(&db, &query, &previous_anchor, PageDirection::Previous)
                    .unwrap(),
                first
            );
        }
    }

    #[test]
    fn cursor_rejects_changed_query_and_nonindexed_orders() {
        let (store, db) = fixture();
        let query = RecordQuery {
            limit: 3,
            ..Default::default()
        };
        let first = store.query_records(&db, &query).unwrap();
        let cursor = store
            .record_cursor(&db, &query, &first[0])
            .unwrap()
            .unwrap();
        let changed = RecordQuery {
            filter: Some(FilterExpr::Rule(FilterRule {
                property_id: "title".into(),
                op: FilterOp::Contains,
                value: json!("Title"),
            })),
            ..query.clone()
        };
        assert!(
            store
                .query_records_cursor(&db, &changed, &cursor, PageDirection::Next)
                .is_err()
        );
        let sorted = RecordQuery {
            sorts: vec![SortRule {
                property_id: "value".into(),
                descending: false,
            }],
            ..query
        };
        assert!(
            store
                .record_cursor(&db, &sorted, &first[0])
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .query_records_cursor(&db, &sorted, &cursor, PageDirection::Next)
                .is_err()
        );
    }

    #[test]
    fn renamed_or_deleted_anchor_is_rejected() {
        let (mut store, db) = fixture();
        let query = RecordQuery {
            sorts: vec![SortRule {
                property_id: "title".into(),
                descending: false,
            }],
            limit: 3,
            ..Default::default()
        };
        let record = store.query_records(&db, &query).unwrap().remove(0);
        let cursor = store.record_cursor(&db, &query, &record).unwrap().unwrap();
        store.rename_record(&record.id, "Changed").unwrap();
        assert!(
            store
                .query_records_cursor(&db, &query, &cursor, PageDirection::Next)
                .is_err()
        );
        let current = store.get_record(&record.id).unwrap().unwrap();
        let cursor = store.record_cursor(&db, &query, &current).unwrap().unwrap();
        store.delete_record(&record.id).unwrap();
        assert!(
            store
                .query_records_cursor(&db, &query, &cursor, PageDirection::Previous)
                .is_err()
        );
    }
}
