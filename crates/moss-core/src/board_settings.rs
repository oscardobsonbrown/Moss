use crate::{Database, DatabaseView, Store, ViewKind, properties::with_database, store::invalid};
use rusqlite::Result;
use serde_json::{Value, json};

impl Store {
    /// Change both grouping axes together, retaining changes to other view fields.
    pub fn set_board_grouping(
        &mut self,
        database_id: &str,
        expected: &DatabaseView,
        primary: &str,
        subgroup: Option<&str>,
    ) -> Result<Database> {
        if expected.kind != ViewKind::Board {
            return Err(invalid("not a Board view"));
        }
        with_database(self, database_id, |_, db, _| {
            for id in std::iter::once(primary).chain(subgroup) {
                if !db
                    .properties
                    .iter()
                    .any(|p| p.id == id && crate::board::group_supported(p.kind))
                {
                    return Err(invalid(
                        "Board grouping property is unavailable or unsupported",
                    ));
                }
            }
            if subgroup == Some(primary) {
                return Err(invalid("Group and subgroup must use different properties"));
            }
            let previous_primary = db
                .views
                .iter()
                .find(|view| view.id == expected.id)
                .and_then(|view| {
                    view.query.group_by.clone().or_else(|| {
                        db.properties
                            .iter()
                            .find(|p| {
                                matches!(
                                    p.kind,
                                    crate::PropertyKind::Status | crate::PropertyKind::Select
                                )
                            })
                            .map(|p| p.id.clone())
                    })
                });
            let view = db
                .views
                .iter_mut()
                .find(|v| v.id == expected.id)
                .ok_or_else(|| invalid("view missing"))?;
            if view.kind != ViewKind::Board
                || view.query.group_by != expected.query.group_by
                || view.config.get("board_subgroup_by") != expected.config.get("board_subgroup_by")
            {
                return Err(invalid(
                    "Board grouping changed in another window; reopen settings",
                ));
            }
            let primary_changed = previous_primary.as_deref() != Some(primary);
            let subgroup_changed =
                view.config.get("board_subgroup_by").and_then(Value::as_str) != subgroup;
            let fields = [
                (
                    primary_changed,
                    [
                        "board_group_order",
                        "board_hidden_groups",
                        "board_collapsed_groups",
                    ],
                ),
                (
                    subgroup_changed,
                    [
                        "board_subgroup_order",
                        "board_hidden_subgroups",
                        "board_collapsed_subgroups",
                    ],
                ),
            ];
            for (changed, fields) in fields {
                if changed {
                    for field in fields {
                        if view.config.get(field) != expected.config.get(field) {
                            return Err(invalid(
                                "Board group settings changed in another window; reopen settings",
                            ));
                        }
                    }
                }
            }
            let config = view
                .config
                .as_object_mut()
                .ok_or_else(|| invalid("Invalid Board settings"))?;
            config.insert(
                "board_subgroup_by".into(),
                subgroup.map_or(Value::Null, |id| json!(id)),
            );
            for (changed, fields) in fields {
                if changed {
                    for field in fields {
                        config.remove(field);
                    }
                }
            }
            view.query.group_by = Some(primary.into());
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PropertyKind, RecordQuery, new_id};

    #[test]
    fn grouping_axes_change_atomically_and_guard_cleared_metadata() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Board test", "").unwrap();
        let definition = Database {
            id: new_id(),
            page_id: String::new(),
            name: "Tasks".into(),
            properties: vec![],
            views: vec![DatabaseView {
                id: "board".into(),
                name: "Board".into(),
                kind: ViewKind::Board,
                query: RecordQuery::default(),
                config: json!({"other":17}),
            }],
        };
        let page = store
            .create_database_page(&workspace.id, None, "Tasks", "", definition)
            .unwrap();
        let db = store.get_database_by_page(&page.id).unwrap().unwrap();
        let db = store
            .add_property(&db.id, "Status", PropertyKind::Status)
            .unwrap();
        let status = db.properties.last().unwrap().id.clone();
        let db = store
            .add_select_property(&db.id, "Priority", &["High".into(), "Low".into()])
            .unwrap();
        let priority = db.properties.last().unwrap().id.clone();
        let db = store
            .set_board_grouping(&db.id, &db.views[0], &status, Some(&priority))
            .unwrap();
        assert_eq!(db.views[0].query.group_by.as_deref(), Some(status.as_str()));
        assert_eq!(db.views[0].config["board_subgroup_by"], priority);
        assert!(
            store
                .set_board_grouping(&db.id, &db.views[0], &priority, Some(&priority))
                .is_err()
        );
        assert_eq!(store.get_database(&db.id).unwrap().unwrap().views, db.views);
        let db = store
            .edit_view_config(&db.id, "board", ViewKind::Board, |config| {
                config["board_group_order"] = json!(["Done"]);
                config["board_hidden_groups"] = json!(["Done"]);
                Ok(())
            })
            .unwrap();
        store
            .edit_view_config(&db.id, "board", ViewKind::Board, |config| {
                config["board_group_order"] = json!(["New"]);
                Ok(())
            })
            .unwrap();
        assert!(
            store
                .set_board_grouping(&db.id, &db.views[0], &priority, None)
                .is_err()
        );
        let current = store.get_database(&db.id).unwrap().unwrap();
        assert_eq!(
            current.views[0].query.group_by.as_deref(),
            Some(status.as_str())
        );
        assert_eq!(current.views[0].config["board_group_order"], json!(["New"]));
        let changed = store
            .set_board_grouping(&db.id, &current.views[0], &priority, None)
            .unwrap();
        assert!(changed.views[0].config.get("board_group_order").is_none());
        assert!(changed.views[0].config.get("board_hidden_groups").is_none());
        assert!(changed.views[0].config["board_subgroup_by"].is_null());
        assert_eq!(changed.views[0].config["other"], 17);
        assert!(
            store
                .set_board_grouping(&db.id, &current.views[0], &status, None)
                .is_err()
        );
        assert!(
            store
                .set_board_grouping(&db.id, &changed.views[0], "missing", None)
                .is_err()
        );
    }
}
