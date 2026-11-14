use crate::state::State;
use moss_core::{Database, DatabaseView, ViewKind};
use rune::TextEdit;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewPane {
    Main,
    Layout,
    Create,
    Filter,
    Sort,
    Group,
    OpenMode,
    BoardLayout,
    BoardCardSize,
    BoardCardPreview,
    BoardGroup,
    BoardSubgroup,
    BoardGroupProperty,
    BoardSubgroupProperty,
    BoardProperties,
    CalendarLayout,
}

pub struct ViewEditor {
    pub database_id: String,
    pub expected: Option<DatabaseView>,
    pub order: Vec<String>,
    pub pane: ViewPane,
    pub kind: ViewKind,
    pub error: Option<String>,
    pub confirm_delete: bool,
    pub from_settings: bool,
}

pub fn ordered_board_groups(
    groups: &[serde_json::Value],
    config: &serde_json::Value,
    subgroup: bool,
) -> Vec<serde_json::Value> {
    let field = if subgroup {
        "board_subgroup_order"
    } else {
        "board_group_order"
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut output = Vec::with_capacity(groups.len());
    for key in config
        .get(field)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
    {
        if let Some(value) = groups
            .iter()
            .find(|v| moss_core::board::group_key(v) == key)
        {
            if seen.insert(key.to_owned()) {
                output.push(value.clone());
            }
        }
    }
    for value in groups {
        if seen.insert(moss_core::board::group_key(value)) {
            output.push(value.clone());
        }
    }
    output
}

impl State {
    pub fn create_calendar_date_property(&mut self) {
        let Some(db) = self.tab().and_then(|tab| tab.database.as_ref()) else {
            return;
        };
        let id = db.id.clone();
        match self
            .store
            .add_property(&id, "Date", moss_core::PropertyKind::Date)
        {
            Ok(db) => {
                let Some(property) = db.properties.last().map(|p| p.id.clone()) else {
                    return;
                };
                self.view_result(Ok(db), None, false);
                self.set_calendar_config("calendar_date_property", serde_json::json!(property));
            }
            Err(error) => {
                if let Some(editor) = &mut self.view_management {
                    editor.error = Some(error.to_string());
                } else {
                    self.error = Some(error.to_string());
                }
            }
        }
    }
    pub fn set_calendar_config(&mut self, key: &'static str, next: serde_json::Value) -> bool {
        let Some(db) = self.tab().and_then(|tab| tab.database.clone()) else {
            return false;
        };
        let Some(view) = self
            .tab()
            .and_then(|tab| db.views.get(tab.view))
            .cloned()
            .filter(|view| view.kind == ViewKind::Calendar)
        else {
            return false;
        };
        let valid = match key {
            "calendar_mode" => matches!(next.as_str(), Some("month" | "week")),
            "calendar_date_property" => next.as_str().is_some_and(|id| {
                db.properties
                    .iter()
                    .any(|p| p.id == id && p.kind == moss_core::PropertyKind::Date)
            }),
            _ => false,
        };
        if !valid {
            return false;
        }
        let expected = self
            .view_management
            .as_ref()
            .and_then(|e| e.expected.as_ref())
            .filter(|e| e.id == view.id)
            .unwrap_or(&view)
            .config
            .get(key)
            .cloned();
        let result = self
            .store
            .edit_view_config(&db.id, &view.id, ViewKind::Calendar, |config| {
                if config.get(key).cloned() != expected {
                    return Err(
                        "This Calendar setting changed in another window. Reopen settings.".into(),
                    );
                }
                if config.is_null() {
                    *config = serde_json::json!({});
                }
                config
                    .as_object_mut()
                    .ok_or("Invalid Calendar settings")?
                    .insert(key.into(), next);
                Ok(())
            })
            .map_err(|e| e.to_string());
        match result {
            Ok(db) => {
                self.view_result(Ok(db), None, false);
                self.database_views.calendar.invalidate();
                true
            }
            Err(error) => {
                if let Some(editor) = &mut self.view_management {
                    editor.error = Some(error);
                } else {
                    self.error = Some(error);
                }
                false
            }
        }
    }
    pub fn set_board_config(&mut self, key: &'static str, next: serde_json::Value) -> bool {
        let Some(db) = self.tab().and_then(|t| t.database.clone()) else {
            return false;
        };
        let Some(view) = self
            .tab()
            .and_then(|t| db.views.get(t.view))
            .cloned()
            .filter(|v| v.kind == ViewKind::Board)
        else {
            return false;
        };
        let expected = self
            .view_management
            .as_ref()
            .and_then(|e| e.expected.as_ref())
            .filter(|e| e.id == view.id)
            .unwrap_or(&view)
            .config
            .get(key)
            .cloned();
        let valid = match key {
            "card_size" => matches!(next.as_str(), Some("small" | "medium" | "large")),
            "card_preview" => matches!(next.as_str(), Some("none" | "content" | "cover")),
            "show_page_icon" | "wrap_cards" | "color_columns" | "board_hide_empty" => {
                next.is_boolean()
            }
            "board_group_order"
            | "board_hidden_groups"
            | "board_collapsed_groups"
            | "board_subgroup_order"
            | "board_hidden_subgroups"
            | "board_collapsed_subgroups" => next.as_array().is_some_and(|a| {
                a.len() <= 512
                    && a.iter()
                        .all(|v| v.as_str().is_some_and(|s| s.len() <= 8192))
            }),
            _ => false,
        };
        if !valid {
            self.board_settings_error("Invalid Board setting".into());
            return false;
        }
        let result = self
            .store
            .edit_view_config(&db.id, &view.id, ViewKind::Board, |config| {
                if config.get(key).cloned() != expected {
                    return Err(
                        "This Board setting changed in another window. Reopen settings.".into(),
                    );
                }
                config
                    .as_object_mut()
                    .ok_or("Invalid Board settings")?
                    .insert(key.into(), next);
                Ok(())
            })
            .map_err(|e| e.to_string());
        match result {
            Ok(database) => {
                self.view_result(Ok(database), None, false);
                true
            }
            Err(error) => {
                self.board_settings_error(error);
                false
            }
        }
    }
    fn board_settings_error(&mut self, error: String) {
        if let Some(editor) = &mut self.view_management {
            editor.error = Some(error);
        } else {
            self.error = Some(error);
        }
    }
    pub fn reload_board_settings(&mut self) {
        let Some((id, Some(expected))) = self.view_context() else {
            return;
        };
        match self.store.get_database(&id) {
            Ok(Some(database)) => {
                let Some(view) = database
                    .views
                    .iter()
                    .find(|v| v.id == expected.id && v.kind == ViewKind::Board)
                    .cloned()
                else {
                    self.board_settings_error("This Board view is no longer available.".into());
                    return;
                };
                self.apply_view_database(database, None);
                self.view_name_input = TextEdit::new(&view.name);
                if let Some(editor) = &mut self.view_management {
                    editor.expected = Some(view);
                    editor.error = None;
                }
            }
            Ok(None) => self.board_settings_error("The database is no longer available.".into()),
            Err(error) => self.board_settings_error(error.to_string()),
        }
    }
    pub fn choose_board_grouping(&mut self, property: Option<&str>, subgroup: bool) {
        let Some(db) = self.tab().and_then(|t| t.database.clone()) else {
            return;
        };
        let Some(view) = self
            .tab()
            .and_then(|t| db.views.get(t.view))
            .cloned()
            .filter(|v| v.kind == ViewKind::Board)
        else {
            return;
        };
        let expected = self
            .view_management
            .as_ref()
            .and_then(|e| e.expected.clone())
            .filter(|e| e.id == view.id)
            .unwrap_or(view);
        let old_primary = expected.query.group_by.as_deref().or_else(|| {
            db.properties
                .iter()
                .find(|p| {
                    matches!(
                        p.kind,
                        moss_core::PropertyKind::Status | moss_core::PropertyKind::Select
                    )
                })
                .map(|p| p.id.as_str())
        });
        let primary = if subgroup { old_primary } else { property };
        let Some(primary) = primary else {
            self.board_settings_error("Choose a grouping property first.".into());
            return;
        };
        let secondary = if subgroup {
            property
        } else {
            expected
                .config
                .get("board_subgroup_by")
                .and_then(serde_json::Value::as_str)
                .filter(|id| *id != primary)
        };
        match self
            .store
            .set_board_grouping(&db.id, &expected, primary, secondary)
        {
            Ok(database) => {
                self.view_result(Ok(database), None, false);
                if let Some(editor) = &mut self.view_management {
                    editor.pane = if subgroup {
                        ViewPane::BoardSubgroup
                    } else {
                        ViewPane::BoardGroup
                    };
                }
            }
            Err(error) => self.board_settings_error(error.to_string()),
        }
    }
    pub fn toggle_board_group_setting(&mut self, key: &'static str, group: &str) {
        let Some(config) = self
            .tab()
            .and_then(|t| t.database.as_ref().and_then(|db| db.views.get(t.view)))
            .map(|v| v.config.clone())
        else {
            return;
        };
        let mut values: Vec<serde_json::Value> = config
            .get(key)
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if values.iter().any(|v| v.as_str() == Some(group)) {
            values.retain(|v| v.as_str() != Some(group));
        } else {
            values.push(group.into());
        }
        self.set_board_config(key, values.into());
    }
    pub fn move_board_group_order(&mut self, subgroup: bool, group: &str, down: bool) {
        let Some(projection) = self.database_views.board.projection.as_ref() else {
            return;
        };
        let groups = if subgroup {
            &projection.subgroups
        } else {
            &projection.groups
        };
        let Some(config) = self
            .tab()
            .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
            .map(|view| &view.config)
        else {
            return;
        };
        let mut order: Vec<String> = ordered_board_groups(groups, config, subgroup)
            .iter()
            .map(moss_core::board::group_key)
            .collect();
        let Some(index) = order.iter().position(|key| key == group) else {
            return;
        };
        let target = if down {
            index + 1
        } else {
            index.saturating_sub(1)
        };
        if target < order.len() && target != index {
            order.swap(index, target);
            self.set_board_config(
                if subgroup {
                    "board_subgroup_order"
                } else {
                    "board_group_order"
                },
                serde_json::json!(order),
            );
        }
    }
    pub fn move_board_property_order(&mut self, property: &str, down: bool) {
        let Some(tab) = self.tab() else { return };
        let Some(db) = tab.database.as_ref() else {
            return;
        };
        let database_id = db.id.clone();
        let order: Vec<String> = moss_core::record_layout::properties(db)
            .iter()
            .map(|p| p.id.clone())
            .collect();
        let Some(index) = order.iter().position(|id| id == property) else {
            return;
        };
        let target = if down {
            index + 1
        } else {
            index.saturating_sub(1)
        };
        if target < order.len() && target != index {
            let before = if down {
                order.get(index + 2)
            } else {
                order.get(target)
            };
            self.move_metadata_property(&database_id, property, before.map(String::as_str));
        }
    }
    pub fn open_view_settings(&mut self) {
        if self
            .view_management
            .as_ref()
            .is_some_and(|editor| editor.error.is_some())
        {
            return;
        }
        if !self.flush_current() {
            return;
        }
        let Some(tab) = self.tab() else { return };
        let Some(db) = &tab.database else { return };
        let Some(view) = db.views.get(tab.view).cloned() else {
            return;
        };
        let database_id = db.id.clone();
        let order = db.views.iter().map(|v| v.id.clone()).collect();
        self.view_name_input = TextEdit::new(&view.name);
        self.view_management = Some(ViewEditor {
            database_id,
            order,
            kind: view.kind,
            expected: Some(view),
            pane: ViewPane::Main,
            error: None,
            confirm_delete: false,
            from_settings: false,
        });
    }
    pub fn open_new_view(&mut self) {
        if self
            .view_management
            .as_ref()
            .is_some_and(|editor| editor.error.is_some())
        {
            return;
        }
        if !self.flush_current() {
            return;
        }
        let Some(db) = self.tab().and_then(|t| t.database.as_ref()) else {
            return;
        };
        let database_id = db.id.clone();
        let order = db.views.iter().map(|v| v.id.clone()).collect();
        self.view_name_input = TextEdit::new("Table");
        self.view_management = Some(ViewEditor {
            database_id,
            order,
            expected: None,
            pane: ViewPane::Create,
            kind: ViewKind::Table,
            error: None,
            confirm_delete: false,
            from_settings: false,
        });
    }
    pub fn new_view_from_settings(&mut self) {
        if !self.autosave_view_name(true) {
            return;
        }
        self.open_new_view();
        if let Some(editor) = &mut self.view_management {
            editor.from_settings = true;
        }
    }
    fn view_context(&self) -> Option<(String, Option<DatabaseView>)> {
        let editor = self.view_management.as_ref()?;
        let tab = self.tab()?;
        let database = tab.database.as_ref()?;
        (database.id == editor.database_id
            && editor.expected.as_ref().is_none_or(|expected| {
                database
                    .views
                    .get(tab.view)
                    .is_some_and(|view| view.id == expected.id)
            }))
        .then(|| (editor.database_id.clone(), editor.expected.clone()))
    }
    fn apply_view_database(&mut self, database: Database, activate: Option<&str>) {
        let active = self.active;
        let original_filter = self.filter.clone();
        let mut affected = Vec::new();
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if tab.database.as_ref().is_some_and(|db| db.id == database.id) {
                let current = tab
                    .database
                    .as_ref()
                    .and_then(|db| db.views.get(tab.view))
                    .map(|v| v.id.clone());
                let target = if index == active {
                    activate.map(str::to_owned).or(current)
                } else {
                    current
                };
                tab.view = target
                    .as_ref()
                    .and_then(|id| database.views.iter().position(|v| &v.id == id))
                    .unwrap_or(0);
                tab.database = Some(database.clone());
                affected.push(index);
            }
        }
        for index in affected {
            self.active = index;
            self.filter = TextEdit::new(&self.tabs[index].title_search);
            self.query_records();
        }
        self.active = active;
        self.filter = original_filter;
        self.save_preferences();
    }
    fn view_result(
        &mut self,
        result: Result<Database, String>,
        activate: Option<String>,
        close: bool,
    ) -> bool {
        match result {
            Ok(db) => {
                self.apply_view_database(db.clone(), activate.as_deref());
                if close {
                    self.view_management = None;
                } else if let Some(editor) = self.view_management.as_mut() {
                    editor.order = db.views.iter().map(|v| v.id.clone()).collect();
                    if let Some(old) = &editor.expected {
                        editor.expected = db.views.iter().find(|v| v.id == old.id).cloned();
                    }
                    editor.error = None;
                }
                true
            }
            Err(error) => {
                if let Some(editor) = self.view_management.as_mut() {
                    editor.error = Some(error);
                }
                false
            }
        }
    }
    pub fn save_view_name(&mut self) -> bool {
        let Some((id, expected)) = self.view_context() else {
            return false;
        };
        let name = self.view_name_input.text.trim().to_owned();
        if name.is_empty() {
            self.view_management.as_mut().unwrap().error = Some("Enter a view name.".into());
            return false;
        }
        let result = match expected {
            Some(view) => self.store.rename_view(&id, &view, &name),
            None => self
                .store
                .create_view(&id, &name, self.view_management.as_ref().unwrap().kind),
        };
        match result {
            Ok(db) => {
                let activate = if self.view_management.as_ref().unwrap().expected.is_none() {
                    let old = &self.view_management.as_ref().unwrap().order;
                    db.views
                        .iter()
                        .find(|v| !old.contains(&v.id))
                        .map(|v| v.id.clone())
                } else {
                    None
                };
                self.view_result(Ok(db), activate, true)
            }
            Err(e) => self.view_result(Err(e.to_string()), None, false),
        }
    }
    pub fn autosave_view_name(&mut self, require_name: bool) -> bool {
        let Some((id, Some(expected))) = self.view_context() else {
            return false;
        };
        let name = self.view_name_input.text.trim().to_owned();
        if name.is_empty() {
            if require_name {
                self.view_management.as_mut().unwrap().error = Some("Enter a view name.".into());
            }
            return !require_name;
        }
        if name == expected.name {
            return self
                .view_management
                .as_ref()
                .is_some_and(|editor| editor.error.is_none());
        }
        match self.store.rename_view(&id, &expected, &name) {
            Ok(db) => {
                for tab in &mut self.tabs {
                    if tab.database.as_ref().is_some_and(|old| old.id == db.id) {
                        tab.database = Some(db.clone());
                    }
                }
                let editor = self.view_management.as_mut().unwrap();
                editor.expected = db.views.iter().find(|v| v.id == expected.id).cloned();
                editor.error = None;
                true
            }
            Err(error) => {
                self.view_management.as_mut().unwrap().error = Some(error.to_string());
                false
            }
        }
    }
    pub fn change_view_layout(&mut self, kind: ViewKind) {
        let Some((id, Some(view))) = self.view_context() else {
            return;
        };
        let result = self
            .store
            .change_view_kind(&id, &view, kind)
            .map_err(|e| e.to_string());
        if self.view_result(result, None, false) {
            let editor = self.view_management.as_mut().unwrap();
            editor.kind = kind;
            editor.pane = ViewPane::Main;
        }
    }
    pub fn duplicate_current_view(&mut self) {
        let Some((id, Some(view))) = self.view_context() else {
            return;
        };
        let result = self
            .store
            .duplicate_view(&id, &view, &format!("{} copy", view.name))
            .map_err(|e| e.to_string());
        match result {
            Ok(db) => {
                let old = &self.view_management.as_ref().unwrap().order;
                let added = db
                    .views
                    .iter()
                    .find(|v| !old.contains(&v.id))
                    .map(|v| v.id.clone());
                self.view_result(Ok(db), added, true);
            }
            Err(e) => {
                self.view_result(Err(e), None, false);
            }
        }
    }
    pub fn delete_current_view(&mut self) {
        let Some((id, Some(view))) = self.view_context() else {
            return;
        };
        let result = self
            .store
            .delete_view(&id, &view)
            .map_err(|e| e.to_string());
        self.view_result(result, None, true);
    }
    pub fn move_current_view(&mut self, down: bool) {
        let Some((id, Some(view))) = self.view_context() else {
            return;
        };
        let order = self.view_management.as_ref().unwrap().order.clone();
        let Some(index) = order.iter().position(|v| v == &view.id) else {
            return;
        };
        let before = if down {
            if index + 1 >= order.len() {
                return;
            }
            order.get(index + 2)
        } else {
            if index == 0 {
                return;
            }
            order.get(index - 1)
        };
        let result = self
            .store
            .move_view(&id, &view, &order, before.map(String::as_str))
            .map_err(|e| e.to_string());
        self.view_result(result, None, false);
    }
    pub fn set_view_open_mode(&mut self, mode: &'static str) {
        let Some((id, Some(view))) = self.view_context() else {
            return;
        };
        let expected = view.config.get("open_mode").cloned();
        let result = self
            .store
            .edit_view_config(&id, &view.id, view.kind, |config| {
                if config.get("open_mode").cloned() != expected {
                    return Err("Opening mode changed elsewhere".into());
                }
                if config.is_null() {
                    *config = serde_json::json!({});
                }
                config
                    .as_object_mut()
                    .ok_or("Invalid view settings")?
                    .insert("open_mode".into(), serde_json::json!(mode));
                Ok(())
            })
            .map_err(|e| e.to_string());
        if self.view_result(result, None, false) {
            self.view_management.as_mut().unwrap().pane = ViewPane::Main;
        }
    }
}
