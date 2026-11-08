use crate::{
    Database, DatabaseView, RecordQuery, Store, ViewKind, new_id, properties::with_database,
    store::invalid,
};
use rusqlite::Result;
use serde_json::json;

fn view_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("view name cannot be empty"));
    }
    Ok(name.to_owned())
}

fn expected_view(db: &Database, expected: &DatabaseView) -> Result<usize> {
    let index = db
        .views
        .iter()
        .position(|view| view.id == expected.id)
        .ok_or_else(|| invalid("view missing"))?;
    if db.views[index] != *expected {
        return Err(invalid("view changed in another editor"));
    }
    Ok(index)
}

impl Store {
    pub fn create_view(
        &mut self,
        database_id: &str,
        name: &str,
        kind: ViewKind,
    ) -> Result<Database> {
        let name = view_name(name)?;
        with_database(self, database_id, |_, db, _| {
            db.views.push(DatabaseView {
                id: new_id(),
                name,
                kind,
                query: RecordQuery::default(),
                config: json!({}),
            });
            Ok(())
        })
    }

    pub fn rename_view(
        &mut self,
        database_id: &str,
        expected: &DatabaseView,
        name: &str,
    ) -> Result<Database> {
        let name = view_name(name)?;
        with_database(self, database_id, |_, db, _| {
            let index = expected_view(db, expected)?;
            db.views[index].name = name;
            Ok(())
        })
    }

    pub fn duplicate_view(
        &mut self,
        database_id: &str,
        expected: &DatabaseView,
        name: &str,
    ) -> Result<Database> {
        let name = view_name(name)?;
        with_database(self, database_id, |_, db, _| {
            let index = expected_view(db, expected)?;
            let mut duplicate = db.views[index].clone();
            duplicate.id = new_id();
            duplicate.name = name;
            db.views.insert(index + 1, duplicate);
            Ok(())
        })
    }

    pub fn delete_view(&mut self, database_id: &str, expected: &DatabaseView) -> Result<Database> {
        with_database(self, database_id, |_, db, _| {
            let index = expected_view(db, expected)?;
            if db.views.len() <= 1 {
                return Err(invalid("the last view cannot be deleted"));
            }
            db.views.remove(index);
            Ok(())
        })
    }

    /// Move a view before a stable ID, or to the end. The saved order must still match.
    pub fn move_view(
        &mut self,
        database_id: &str,
        expected: &DatabaseView,
        expected_order: &[String],
        before: Option<&str>,
    ) -> Result<Database> {
        with_database(self, database_id, |_, db, _| {
            let index = expected_view(db, expected)?;
            if !db
                .views
                .iter()
                .map(|view| view.id.as_str())
                .eq(expected_order.iter().map(String::as_str))
            {
                return Err(invalid("view order changed in another editor"));
            }
            if before == Some(expected.id.as_str()) {
                return Ok(());
            }
            if let Some(id) = before {
                if !db.views.iter().any(|view| view.id == id) {
                    return Err(invalid("destination view missing"));
                }
            }
            let moved = db.views.remove(index);
            let destination = match before {
                Some(id) => db
                    .views
                    .iter()
                    .position(|view| view.id == id)
                    .ok_or_else(|| invalid("destination view missing"))?,
                None => db.views.len(),
            };
            db.views.insert(destination, moved);
            Ok(())
        })
    }

    pub fn change_view_kind(
        &mut self,
        database_id: &str,
        expected: &DatabaseView,
        kind: ViewKind,
    ) -> Result<Database> {
        with_database(self, database_id, |_, db, _| {
            let index = expected_view(db, expected)?;
            db.views[index].kind = kind;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, FilterExpr, FilterOp, FilterRule, Property, PropertyKind, SortRule};

    fn fixture(store: &mut Store) -> Database {
        let ws = store.create_workspace("Views", "").unwrap();
        let page = store
            .create_database_page(
                &ws.id,
                None,
                "Items",
                "",
                Database {
                    id: new_id(),
                    page_id: String::new(),
                    name: "Items".into(),
                    properties: vec![Property {
                        id: "title".into(),
                        name: "Name".into(),
                        kind: PropertyKind::Title,
                        config: json!({}),
                    }],
                    views: vec![DatabaseView {
                        id: "original".into(),
                        name: "Table".into(),
                        kind: ViewKind::Table,
                        query: RecordQuery {
                            filter: Some(FilterExpr::Rule(FilterRule {
                                property_id: "title".into(),
                                op: FilterOp::Contains,
                                value: json!("task"),
                            })),
                            sorts: vec![SortRule {
                                property_id: "title".into(),
                                descending: false,
                            }],
                            ..Default::default()
                        },
                        config: json!({"unknown": {"keep": true}, "wrap_cells": true}),
                    }],
                },
            )
            .unwrap();
        store.get_database_by_page(&page.id).unwrap().unwrap()
    }

    #[test]
    fn create_duplicate_reorder_rename_and_layout_persist_without_losing_config() {
        let path = std::env::temp_dir().join(format!("moss-view-{}.sqlite3", new_id()));
        let mut store = Store::open(&path).unwrap();
        let db = fixture(&mut store);
        let original = db.views[0].clone();
        let db = store
            .create_view(&db.id, "Another Table", ViewKind::Table)
            .unwrap();
        assert_eq!(
            db.views
                .iter()
                .filter(|v| v.kind == ViewKind::Table)
                .count(),
            2
        );
        let second = db.views[1].clone();
        let db = store.duplicate_view(&db.id, &original, "Copy").unwrap();
        let duplicate = db.views[1].clone();
        assert_ne!(duplicate.id, original.id);
        assert_eq!(duplicate.query, original.query);
        assert_eq!(duplicate.config, original.config);
        let order: Vec<_> = db.views.iter().map(|v| v.id.clone()).collect();
        let db = store
            .move_view(&db.id, &second, &order, Some(&original.id))
            .unwrap();
        assert_eq!(
            db.views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            vec![
                second.id.as_str(),
                original.id.as_str(),
                duplicate.id.as_str()
            ]
        );
        let db = store.rename_view(&db.id, &duplicate, "Renamed").unwrap();
        let renamed = db
            .views
            .iter()
            .find(|v| v.id == duplicate.id)
            .unwrap()
            .clone();
        let db = store
            .change_view_kind(&db.id, &renamed, ViewKind::Gallery)
            .unwrap();
        let changed = db.views.iter().find(|v| v.id == duplicate.id).unwrap();
        assert_eq!(changed.kind, ViewKind::Gallery);
        assert_eq!(changed.query, original.query);
        assert_eq!(changed.config, original.config);
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.get_database(&db.id).unwrap().unwrap(), db);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn stale_view_and_order_reject_without_changing_other_data() {
        let mut store = Store::in_memory().unwrap();
        let db = fixture(&mut store);
        let original = db.views[0].clone();
        let db = store
            .create_view(&db.id, "Second", ViewKind::Board)
            .unwrap();
        let old_order: Vec<_> = db.views.iter().map(|v| v.id.clone()).collect();
        let db = store.rename_view(&db.id, &original, "Changed").unwrap();
        assert!(
            store
                .change_view_kind(&db.id, &original, ViewKind::List)
                .is_err()
        );
        let changed = db.views[0].clone();
        let db = store.move_view(&db.id, &changed, &old_order, None).unwrap();
        assert!(store.move_view(&db.id, &changed, &old_order, None).is_err());
        assert!(store.delete_view(&db.id, &original).is_err());
        assert!(store.create_view(&db.id, "  ", ViewKind::Table).is_err());
        assert_eq!(store.get_database(&db.id).unwrap().unwrap(), db);
    }

    #[test]
    fn last_view_trashed_owner_and_write_failure_leave_database_unchanged() {
        let mut store = Store::in_memory().unwrap();
        let db = fixture(&mut store);
        assert!(store.delete_view(&db.id, &db.views[0]).is_err());
        store.conn.execute_batch("CREATE TRIGGER fail_view_save BEFORE UPDATE ON databases BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
        assert!(store.create_view(&db.id, "Failed", ViewKind::List).is_err());
        assert_eq!(store.get_database(&db.id).unwrap().unwrap(), db);
        store
            .conn
            .execute_batch("DROP TRIGGER fail_view_save;")
            .unwrap();
        store.delete_page(&db.page_id).unwrap();
        assert!(
            store
                .create_view(&db.id, "Trashed", ViewKind::Table)
                .is_err()
        );
        assert!(store.rename_view(&db.id, &db.views[0], "Trashed").is_err());
        assert_eq!(store.get_database(&db.id).unwrap().unwrap(), db);
    }

    #[test]
    fn another_connection_can_change_an_unrelated_view_without_losing_it() {
        let path = std::env::temp_dir().join(format!("moss-view-concurrent-{}.sqlite3", new_id()));
        let mut first = Store::open(&path).unwrap();
        let db = fixture(&mut first);
        let db = first.create_view(&db.id, "Other", ViewKind::Table).unwrap();
        let target = db.views[0].clone();
        let other = db.views[1].clone();
        let mut second = Store::open(&path).unwrap();
        second
            .edit_view_config(&db.id, &other.id, ViewKind::Table, |config| {
                config["external"] = json!("kept");
                Ok(())
            })
            .unwrap();
        let saved = first.rename_view(&db.id, &target, "Renamed").unwrap();
        assert_eq!(saved.views[0].name, "Renamed");
        assert_eq!(saved.views[1].config["external"], "kept");
        assert!(second.rename_view(&db.id, &target, "Stale").is_err());
        assert_eq!(second.get_database(&db.id).unwrap().unwrap(), saved);
        drop(second);
        drop(first);
        std::fs::remove_file(path).unwrap();
    }
}
