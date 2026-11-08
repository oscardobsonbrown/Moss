use crate::{Database, FilterExpr, SortRule, Store, properties::with_database, store::invalid};
use rusqlite::Result;

fn validate_filter(filter: &FilterExpr, db: &Database, depth: usize) -> Result<()> {
    if depth > 12 {
        return Err(invalid("filter too deep"));
    }
    match filter {
        FilterExpr::Rule(rule) => {
            if !db
                .properties
                .iter()
                .any(|property| property.id == rule.property_id)
            {
                return Err(invalid("unknown filter property"));
            }
        }
        FilterExpr::All(items) | FilterExpr::Any(items) => {
            for item in items {
                validate_filter(item, db, depth + 1)?;
            }
        }
    }
    Ok(())
}
impl Store {
    pub fn set_view_filter(
        &mut self,
        database_id: &str,
        view_id: &str,
        expected: Option<FilterExpr>,
        next: Option<FilterExpr>,
    ) -> Result<Database> {
        with_database(self, database_id, |_, db, _| {
            if let Some(filter) = &next {
                validate_filter(filter, db, 0)?;
            }
            let view = db
                .views
                .iter_mut()
                .find(|v| v.id == view_id)
                .ok_or_else(|| invalid("view missing"))?;
            if view.query.filter != expected {
                return Err(invalid("filter changed in another editor"));
            }
            view.query.filter = next;
            Ok(())
        })
    }
    pub fn set_view_sorts(
        &mut self,
        database_id: &str,
        view_id: &str,
        expected: Vec<SortRule>,
        next: Vec<SortRule>,
    ) -> Result<Database> {
        with_database(self, database_id, |_, db, _| {
            if next.len() > 8
                || next
                    .iter()
                    .any(|sort| !db.properties.iter().any(|p| p.id == sort.property_id))
            {
                return Err(invalid("invalid sort properties"));
            }
            let view = db
                .views
                .iter_mut()
                .find(|v| v.id == view_id)
                .ok_or_else(|| invalid("view missing"))?;
            if view.query.sorts != expected {
                return Err(invalid("sort changed in another editor"));
            }
            view.query.sorts = next;
            Ok(())
        })
    }
    pub fn set_view_group(
        &mut self,
        database_id: &str,
        view_id: &str,
        expected: Option<String>,
        next: Option<String>,
    ) -> Result<Database> {
        with_database(self, database_id, |_, db, _| {
            if next
                .as_ref()
                .is_some_and(|id| !db.properties.iter().any(|p| &p.id == id))
            {
                return Err(invalid("unknown group property"));
            }
            let view = db
                .views
                .iter_mut()
                .find(|v| v.id == view_id)
                .ok_or_else(|| invalid("view missing"))?;
            if view.query.group_by != expected {
                return Err(invalid("group changed in another editor"));
            }
            view.query.group_by = next;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseView, FilterOp, FilterRule, PropertyKind, RecordQuery, ViewKind, new_id};
    use serde_json::json;

    #[test]
    fn updates_compare_only_the_touched_field_and_validate_properties() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Test", "").unwrap();
        let definition = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Test".into(),
            properties: vec![],
            views: vec![DatabaseView {
                id: "view".into(),
                name: "Table".into(),
                kind: ViewKind::Table,
                query: RecordQuery::default(),
                config: json!({"other": 7}),
            }],
        };
        let page = store
            .create_database_page(&workspace.id, None, "Test", "", definition)
            .unwrap();
        let db = store.get_database_by_page(&page.id).unwrap().unwrap();
        let db = store
            .add_property(&db.id, "Number", PropertyKind::Number)
            .unwrap();
        let property = db.properties[0].id.clone();
        let sort = SortRule {
            property_id: property.clone(),
            descending: true,
        };
        let sorted = store
            .set_view_sorts(&db.id, "view", vec![], vec![sort.clone()])
            .unwrap();
        let filter = FilterExpr::Rule(FilterRule {
            property_id: property.clone(),
            op: FilterOp::Gt,
            value: json!(2),
        });
        let filtered = store
            .set_view_filter(&db.id, "view", None, Some(filter.clone()))
            .unwrap();
        assert_eq!(filtered.views[0].query.sorts, sorted.views[0].query.sorts);
        assert_eq!(filtered.views[0].config["other"], 7);
        assert!(store.set_view_filter(&db.id, "view", None, None).is_err());
        assert!(
            store
                .set_view_group(&db.id, "view", None, Some("missing".into()))
                .is_err()
        );
        let grouped = store
            .set_view_group(&db.id, "view", None, Some(property))
            .unwrap();
        assert_eq!(grouped.views[0].query.filter, Some(filter));
        assert_eq!(grouped.views[0].query.sorts, vec![sort]);
    }
    #[test]
    fn group_order_stays_contiguous_across_page_boundaries() {
        use std::collections::BTreeMap;
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Test", "").unwrap();
        let definition = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Test".into(),
            properties: vec![],
            views: vec![],
        };
        let page = store
            .create_database_page(&workspace.id, None, "Test", "", definition)
            .unwrap();
        let db = store.get_database_by_page(&page.id).unwrap().unwrap();
        let db = store
            .add_property(&db.id, "Group", PropertyKind::Number)
            .unwrap();
        let property = db.properties[0].id.clone();
        for value in [2, 1, 2, 1, 2] {
            store
                .create_record(
                    &db.id,
                    "row",
                    &BTreeMap::from([(property.clone(), json!(value))]),
                )
                .unwrap();
        }
        let mut values = Vec::new();
        for offset in [0, 2, 4] {
            let query = RecordQuery {
                group_by: Some(property.clone()),
                limit: 2,
                offset,
                ..Default::default()
            };
            values.extend(
                store
                    .query_records(&db.id, &query)
                    .unwrap()
                    .into_iter()
                    .map(|record| record.values[&property].clone()),
            );
        }
        assert_eq!(
            values,
            vec![json!(1), json!(1), json!(2), json!(2), json!(2)]
        );
    }
}
