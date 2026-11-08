use crate::{
    Database, FilterExpr, PageKind, RecordQuery, Store, ViewKind,
    properties::with_database,
    search,
    store::{decode, invalid},
};
use rusqlite::{OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DashboardConfig {
    pub widgets: Vec<DashboardWidget>,
    pub global_filters: Vec<DashboardFilter>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DashboardWidget {
    pub id: String,
    pub database_id: String,
    pub view_id: String,
    pub width: u8,
    pub height: u16,
    #[serde(default)]
    pub query: Option<RecordQuery>,
    #[serde(default)]
    pub layout: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DashboardFilter {
    pub property_name: String,
    pub property_kind: crate::PropertyKind,
    pub op: crate::FilterOp,
    pub value: Value,
}
impl DashboardConfig {
    pub fn read(config: &Value) -> std::result::Result<Self, String> {
        match config.get("dashboard") {
            None | Some(Value::Null) => Ok(Self::default()),
            Some(value) => serde_json::from_value(value.clone()).map_err(|e| e.to_string()),
        }
    }
    pub fn filter_for(&self, db: &Database) -> Option<FilterExpr> {
        let rules: Vec<_> =
            self.global_filters
                .iter()
                .filter_map(|filter| {
                    let mut matches = db.properties.iter().filter(|p| {
                        p.name == filter.property_name && p.kind == filter.property_kind
                    });
                    let p = matches.next()?;
                    if matches.next().is_some() {
                        return None;
                    }
                    Some(FilterExpr::Rule(crate::FilterRule {
                        property_id: p.id.clone(),
                        op: filter.op,
                        value: filter.value.clone(),
                    }))
                })
                .collect();
        if rules.is_empty() {
            None
        } else {
            Some(FilterExpr::All(rules))
        }
    }
}
impl Store {
    pub fn save_dashboard(
        &mut self,
        database_id: &str,
        view_id: &str,
        expected: &DashboardConfig,
        next: &DashboardConfig,
    ) -> Result<Database> {
        if next.widgets.len() > 12 {
            return Err(invalid("Dashboard supports at most 12 widgets"));
        }
        let mut ids = HashSet::new();
        for widget in &next.widgets {
            if widget.id.is_empty()
                || !ids.insert(&widget.id)
                || !(1..=4).contains(&widget.width)
                || !(120..=1200).contains(&widget.height)
            {
                return Err(invalid("Invalid dashboard widget"));
            }
        }
        with_database(self, database_id, |tx, db, owner| {
            let index = db
                .views
                .iter()
                .position(|v| v.id == view_id && v.kind == ViewKind::Dashboard)
                .ok_or_else(|| invalid("Dashboard view missing or changed"))?;
            let current =
                DashboardConfig::read(&db.views[index].config).map_err(|e| invalid(&e))?;
            if &current != expected {
                return Err(invalid("Dashboard changed in another editor"));
            }
            for widget in &next.widgets {
                let source = if widget.database_id == db.id {
                    db.clone()
                } else {
                    let raw: String = tx
                        .query_row(
                            "SELECT data FROM databases WHERE id=?1",
                            [&widget.database_id],
                            |row| row.get(0),
                        )
                        .optional()?
                        .ok_or_else(|| invalid("Widget database missing"))?;
                    decode(raw)?
                };
                let page = search::page(tx, &source.page_id)?
                    .ok_or_else(|| invalid("Widget page missing"))?;
                if page.workspace_id != owner.workspace_id
                    || page.deleted_at.is_some()
                    || page.kind != PageKind::Database
                {
                    return Err(invalid("Widget source is not available in this workspace"));
                }
                if !source
                    .views
                    .iter()
                    .any(|v| v.id == widget.view_id && v.kind != ViewKind::Dashboard)
                {
                    return Err(invalid("Widget view missing or recursive"));
                }
            }
            let config = db.views[index]
                .config
                .as_object_mut()
                .ok_or_else(|| invalid("Invalid view config"))?;
            config.insert(
                "dashboard".into(),
                serde_json::to_value(next).map_err(|e| invalid(&e.to_string()))?,
            );
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseView, Property, PropertyKind, RecordQuery, Workspace, new_id};
    use serde_json::json;
    fn database(s: &mut Store, w: &Workspace) -> Database {
        let db = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Tasks".into(),
            properties: vec![Property {
                id: new_id(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            }],
            views: vec![
                DatabaseView {
                    id: new_id(),
                    name: "Table".into(),
                    kind: ViewKind::Table,
                    query: RecordQuery::default(),
                    config: json!({}),
                },
                DatabaseView {
                    id: new_id(),
                    name: "Dashboard".into(),
                    kind: ViewKind::Dashboard,
                    query: RecordQuery::default(),
                    config: json!({"unrelated": 42}),
                },
            ],
        };
        let page = s
            .create_database_page(&w.id, None, "Tasks", "", db)
            .unwrap();
        s.get_database_by_page(&page.id).unwrap().unwrap()
    }
    fn widget(db: &Database) -> DashboardWidget {
        DashboardWidget {
            id: new_id(),
            database_id: db.id.clone(),
            view_id: db.views[0].id.clone(),
            width: 2,
            height: 300,
            query: None,
            layout: Value::Null,
            extra: Default::default(),
        }
    }
    #[test]
    fn save_checks_stale_config_and_source_without_losing_other_fields() {
        let mut s = Store::in_memory().unwrap();
        let w = s.create_workspace("A", "").unwrap();
        let other = s.create_workspace("B", "").unwrap();
        let db = database(&mut s, &w);
        let foreign = database(&mut s, &other);
        let empty = DashboardConfig::default();
        let mut next = empty.clone();
        next.widgets.push(widget(&db));
        let saved = s
            .save_dashboard(&db.id, &db.views[1].id, &empty, &next)
            .unwrap();
        assert_eq!(saved.views[1].config["unrelated"], 42);
        assert_eq!(DashboardConfig::read(&saved.views[1].config).unwrap(), next);
        assert!(
            s.save_dashboard(&db.id, &db.views[1].id, &empty, &empty)
                .is_err()
        );
        let mut invalid = next.clone();
        invalid.widgets.push(widget(&foreign));
        assert!(
            s.save_dashboard(&db.id, &db.views[1].id, &next, &invalid)
                .is_err()
        );
        let mut recursive = next.clone();
        recursive.widgets[0].view_id = db.views[1].id.clone();
        assert!(
            s.save_dashboard(&db.id, &db.views[1].id, &next, &recursive)
                .is_err()
        );
        assert_eq!(
            DashboardConfig::read(&s.get_database(&db.id).unwrap().unwrap().views[1].config)
                .unwrap(),
            next
        );
    }
}
