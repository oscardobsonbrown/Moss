use crate::{
    FilterExpr, FilterOp, FilterRule, PageKind, PropertyKind, RecordQuery, Store, search,
    store::{filter_sql, invalid},
};
use rusqlite::{Result, params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaceValue {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}
impl PlaceValue {
    pub fn valid(&self) -> bool {
        !self.name.trim().is_empty()
            && self.name.len() <= 512
            && !self.name.chars().any(char::is_control)
            && self.latitude.is_finite()
            && (-90. ..=90.).contains(&self.latitude)
            && self.longitude.is_finite()
            && (-180. ..=180.).contains(&self.longitude)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapBounds {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}
impl MapBounds {
    pub fn valid(self) -> bool {
        [self.south, self.west, self.north, self.east]
            .iter()
            .all(|n| n.is_finite())
            && (-90. ..=90.).contains(&self.south)
            && (-90. ..=90.).contains(&self.north)
            && (-180. ..=180.).contains(&self.west)
            && (-180. ..=180.).contains(&self.east)
            && self.south <= self.north
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct MapPoint {
    pub record_id: String,
    pub title: String,
    pub place: PlaceValue,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MapPoints {
    pub points: Vec<MapPoint>,
    pub truncated: bool,
}

impl Store {
    /// Project matching records in the requested bounds, independent of UI paging.
    pub fn map_points(
        &self,
        database_id: &str,
        query: &RecordQuery,
        title_search: &str,
        place_id: &str,
        bounds: MapBounds,
        limit: u32,
    ) -> Result<MapPoints> {
        if !bounds.valid() || limit == 0 || limit > 500 {
            return Err(invalid("invalid map bounds or marker limit"));
        }
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        if !db
            .properties
            .iter()
            .any(|p| p.id == place_id && p.kind == PropertyKind::Place)
        {
            return Err(invalid("place property missing"));
        }
        let mut args = vec![
            SqlValue::Text(place_id.into()),
            SqlValue::Text(database_id.into()),
        ];
        // Do not add a `place.type='object'` join predicate: SQLite can use it to
        // fold away the CASE guards below before scanning scalar sibling values.
        let mut sql = String::from(
            "SELECT records.id, records.title, place.value FROM records JOIN json_each(records.values_json) AS place ON place.key=? WHERE records.database_id=? AND records.deleted_at IS NULL",
        );
        if let Some(filter) = &query.filter {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !title_search.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("title property missing"))?;
            let filter = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: json!(title_search),
            });
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(&filter, &db, &mut args, 0)?);
        }
        sql.push_str(" AND json_type(CASE WHEN place.type='object' THEN place.value ELSE '{}' END,'$.latitude') IN ('integer','real') AND json_type(CASE WHEN place.type='object' THEN place.value ELSE '{}' END,'$.longitude') IN ('integer','real') AND json_extract(CASE WHEN place.type='object' THEN place.value ELSE '{}' END,'$.latitude') BETWEEN ? AND ? AND ");
        args.extend([SqlValue::Real(bounds.south), SqlValue::Real(bounds.north)]);
        if bounds.west <= bounds.east {
            sql.push_str("json_extract(CASE WHEN place.type='object' THEN place.value ELSE '{}' END,'$.longitude') BETWEEN ? AND ?");
        } else {
            sql.push_str("(json_extract(CASE WHEN place.type='object' THEN place.value ELSE '{}' END,'$.longitude') >= ? OR json_extract(CASE WHEN place.type='object' THEN place.value ELSE '{}' END,'$.longitude') <= ?)");
        }
        args.extend([SqlValue::Real(bounds.west), SqlValue::Real(bounds.east)]);
        sql.push_str(" ORDER BY records.id ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(args))?;
        let mut points = Vec::new();
        while let Some(row) = rows.next()? {
            let raw: String = row.get(2)?;
            let Ok(place) = serde_json::from_str::<PlaceValue>(&raw) else {
                continue;
            };
            if !place.valid() {
                continue;
            }
            points.push(MapPoint {
                record_id: row.get(0)?,
                title: row.get(1)?,
                place,
            });
            if points.len() > limit as usize {
                break;
            }
        }
        let truncated = points.len() > limit as usize;
        points.truncate(limit as usize);
        Ok(MapPoints { points, truncated })
    }

    /// Return the extent of every matching valid place without paging or collecting rows.
    pub fn map_extent(
        &self,
        database_id: &str,
        query: &RecordQuery,
        title_search: &str,
        place_id: &str,
    ) -> Result<Option<MapBounds>> {
        let db = self
            .get_database(database_id)?
            .ok_or_else(|| invalid("database missing"))?;
        let owner = search::page(&self.conn, &db.page_id)?
            .ok_or_else(|| invalid("database page missing"))?;
        if owner.deleted_at.is_some() || owner.kind != PageKind::Database {
            return Err(invalid("invalid database page"));
        }
        if !db
            .properties
            .iter()
            .any(|p| p.id == place_id && p.kind == PropertyKind::Place)
        {
            return Err(invalid("place property missing"));
        }
        let mut args = vec![
            SqlValue::Text(place_id.into()),
            SqlValue::Text(database_id.into()),
        ];
        let mut sql = String::from(
            "SELECT CASE WHEN place.type='object' THEN place.value ELSE '{}' END FROM records JOIN json_each(records.values_json) AS place ON place.key=? WHERE records.database_id=? AND records.deleted_at IS NULL",
        );
        if let Some(filter) = &query.filter {
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(filter, &db, &mut args, 0)?);
        }
        if !title_search.is_empty() {
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == PropertyKind::Title)
                .ok_or_else(|| invalid("title property missing"))?;
            let filter = FilterExpr::Rule(FilterRule {
                property_id: title.id.clone(),
                op: FilterOp::Contains,
                value: json!(title_search),
            });
            sql.push_str(" AND ");
            sql.push_str(&filter_sql(&filter, &db, &mut args, 0)?);
        }
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(args))?;
        let mut extent: Option<MapBounds> = None;
        while let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            let Ok(place) = serde_json::from_str::<PlaceValue>(&raw) else {
                continue;
            };
            if !place.valid() {
                continue;
            }
            match &mut extent {
                None => {
                    extent = Some(MapBounds {
                        south: place.latitude,
                        north: place.latitude,
                        west: place.longitude,
                        east: place.longitude,
                    })
                }
                Some(bounds) => {
                    bounds.south = bounds.south.min(place.latitude);
                    bounds.north = bounds.north.max(place.latitude);
                    bounds.west = bounds.west.min(place.longitude);
                    bounds.east = bounds.east.max(place.longitude);
                }
            }
        }
        Ok(extent)
    }

    pub fn set_record_place_if_unchanged(
        &mut self,
        record_id: &str,
        property_id: &str,
        expected: &Value,
        place: Option<PlaceValue>,
    ) -> Result<crate::Record> {
        let value = match place {
            Some(place) if place.valid() => json!(place),
            Some(_) => return Err(invalid("invalid place")),
            None => Value::Null,
        };
        self.set_record_value_if_unchanged_kind(
            record_id,
            property_id,
            expected,
            value,
            Some(PropertyKind::Place),
        )
    }

    pub fn set_record_place_text_if_unchanged(
        &mut self,
        record_id: &str,
        property_id: &str,
        expected: &Value,
        text: &str,
    ) -> Result<crate::Record> {
        let text = text.trim();
        if text.is_empty() || text.len() > 512 || text.chars().any(char::is_control) {
            return Err(invalid("invalid place name"));
        }
        self.set_record_value_if_unchanged_kind(
            record_id,
            property_id,
            expected,
            json!(text),
            Some(PropertyKind::Place),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, DatabaseView, Property, ViewKind, new_id};
    use std::collections::BTreeMap;

    #[test]
    fn full_query_ignores_ui_paging_and_reports_truncation() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Map", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Map".into(),
            properties: vec![
                Property {
                    id: "title".into(),
                    name: "Name".into(),
                    kind: PropertyKind::Title,
                    config: json!({}),
                },
                Property {
                    id: "place".into(),
                    name: "Place".into(),
                    kind: PropertyKind::Place,
                    config: json!({}),
                },
                Property {
                    id: "status".into(),
                    name: "Status".into(),
                    kind: PropertyKind::Status,
                    config: json!({}),
                },
                Property {
                    id: "notes".into(),
                    name: "Notes".into(),
                    kind: PropertyKind::Text,
                    config: json!({}),
                },
            ],
            views: vec![DatabaseView {
                id: new_id(),
                name: "Map".into(),
                kind: ViewKind::Map,
                query: Default::default(),
                config: json!({}),
            }],
        };
        store
            .create_database_page(&workspace.id, None, "Map", "", db.clone())
            .unwrap();
        for i in 0..125 {
            store
                .create_record(
                    &db.id,
                    &format!("Marker {i}"),
                    &BTreeMap::from([
                        (
                            "place".into(),
                            json!({"name":"Perth","latitude":-31.95,"longitude":115.86}),
                        ),
                        ("status".into(), json!("In progress")),
                        (
                            "notes".into(),
                            json!("Text containing quotes \" and symbols"),
                        ),
                    ]),
                )
                .unwrap();
        }
        let query = RecordQuery {
            limit: 1,
            offset: 100,
            ..Default::default()
        };
        let bounds = MapBounds {
            south: -90.,
            west: -180.,
            north: 90.,
            east: 180.,
        };
        let empty_extent = store
            .map_extent(&db.id, &query, "not present", "place")
            .unwrap();
        assert!(empty_extent.is_none());
        let extent = store
            .map_extent(&db.id, &query, "", "place")
            .unwrap()
            .unwrap();
        assert_eq!((extent.south, extent.west), (-31.95, 115.86));
        let all = store
            .map_points(&db.id, &query, "", "place", bounds, 200)
            .unwrap();
        assert_eq!(all.points.len(), 125);
        assert!(!all.truncated);
        let few = store
            .map_points(&db.id, &query, "", "place", bounds, 10)
            .unwrap();
        assert_eq!(few.points.len(), 10);
        assert!(few.truncated);
        let found = store
            .map_points(&db.id, &query, "Marker 124", "place", bounds, 10)
            .unwrap();
        assert_eq!(found.points.len(), 1);
        for i in 0..401 {
            store
                .create_record(
                    &db.id,
                    &format!("More {i}"),
                    &BTreeMap::from([
                        (
                            "place".into(),
                            json!({"name":"East","latitude":40.,"longitude":-70.}),
                        ),
                        ("status".into(), json!("Done")),
                        ("notes".into(), json!("scalar text")),
                    ]),
                )
                .unwrap();
        }
        let extent = store
            .map_extent(&db.id, &query, "", "place")
            .unwrap()
            .unwrap();
        assert_eq!(
            (extent.south, extent.north, extent.west, extent.east),
            (-31.95, 40., -70., 115.86)
        );
        assert!(
            store
                .map_points(
                    &db.id,
                    &query,
                    "",
                    "place",
                    MapBounds {
                        south: 10.,
                        north: 0.,
                        ..bounds
                    },
                    10
                )
                .is_err()
        );
    }

    #[test]
    fn place_update_is_validated_and_detects_conflict() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Map", "").unwrap();
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Map".into(),
            properties: vec![
                Property {
                    id: "title".into(),
                    name: "Name".into(),
                    kind: PropertyKind::Title,
                    config: json!({}),
                },
                Property {
                    id: "place".into(),
                    name: "Place".into(),
                    kind: PropertyKind::Place,
                    config: json!({}),
                },
            ],
            views: vec![DatabaseView {
                id: new_id(),
                name: "Map".into(),
                kind: ViewKind::Map,
                query: Default::default(),
                config: json!({}),
            }],
        };
        store
            .create_database_page(&workspace.id, None, "Map", "", db.clone())
            .unwrap();
        let row = store
            .create_record(&db.id, "Site", &BTreeMap::new())
            .unwrap();
        let place = PlaceValue {
            name: "Site".into(),
            latitude: -31.95,
            longitude: 115.86,
        };
        let saved = store
            .set_record_place_if_unchanged(&row.id, "place", &Value::Null, Some(place.clone()))
            .unwrap();
        assert_eq!(saved.values["place"], json!(place));
        assert!(
            store
                .set_record_place_if_unchanged(&row.id, "place", &Value::Null, None)
                .is_err()
        );
        assert!(
            store
                .set_record_place_if_unchanged(
                    &row.id,
                    "place",
                    &saved.values["place"],
                    Some(PlaceValue {
                        latitude: 91.,
                        ..place
                    })
                )
                .is_err()
        );
        assert!(
            store
                .set_record_place_text_if_unchanged(&row.id, "place", &saved.values["place"], " ")
                .is_err()
        );
        assert!(
            store
                .set_record_place_text_if_unchanged(&row.id, "title", &Value::Null, "Place")
                .is_err()
        );
        let text = store
            .set_record_place_text_if_unchanged(
                &row.id,
                "place",
                &saved.values["place"],
                "  123 Example Street  ",
            )
            .unwrap();
        assert_eq!(text.values["place"], "123 Example Street");
        let points = store
            .map_points(
                &db.id,
                &RecordQuery::default(),
                "",
                "place",
                MapBounds {
                    south: -90.,
                    north: 90.,
                    west: -180.,
                    east: 180.,
                },
                10,
            )
            .unwrap();
        assert!(points.points.is_empty());
    }
}
