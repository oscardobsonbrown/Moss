use crate::state::State;
use moss_core::{
    Database, FilterExpr, FilterOp, FilterRule, Property, PropertyKind, SortRule, ViewKind,
};
use rune::TextEdit;

pub struct PropertyVisibility {
    pub from_view_settings: bool,
    pub search: TextEdit,
}

impl PropertyVisibility {
    pub fn new(from_view_settings: bool) -> Self {
        let mut search = TextEdit::new("");
        search.focused = true;
        Self {
            from_view_settings,
            search,
        }
    }
}

pub struct PropertyEditor {
    pub database_id: String,
    pub property: Option<Property>,
    pub kind: PropertyKind,
    pub pending_kind: Option<PropertyKind>,
    pub color_option: Option<String>,
    pub confirm_delete: bool,
    pub error: Option<String>,
    pub pane: PropertyPane,
    pub insert_relative: Option<(String, bool)>,
    pub filter_op: FilterOp,
    pub anchor_override: Option<String>,
    pub rename_only: bool,
    pub name_input: TextEdit,
    pub options_input: TextEdit,
    pub filter_input: TextEdit,
    pub advanced: crate::advanced_properties::AdvancedDraft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyPane {
    Main,
    Type,
    ConfirmType,
    OptionColor,
    DateFormat,
    Filter,
    Sort,
    Calculate,
}

#[derive(Default)]
pub struct MetadataDrag {
    pub dragging: Option<String>,
    pub drop_before: Option<String>,
    pub suppress_press: bool,
}

pub struct MetadataMenu {
    pub database_id: String,
    pub property_id: String,
    pub anchor: String,
    pub pane: MetadataPane,
    pub error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MetadataPane {
    Main,
    Visibility,
    Layout,
}

impl State {
    pub fn metadata_database(&self, id: &str) -> Option<&Database> {
        self.record_peek
            .as_ref()
            .map(|p| &p.database)
            .filter(|db| db.id == id)
            .or_else(|| {
                self.tab()
                    .and_then(|t| t.record_context.as_ref().map(|(db, _)| db))
                    .filter(|db| db.id == id)
            })
            .or_else(|| {
                self.tab()
                    .and_then(|t| t.database.as_ref())
                    .filter(|db| db.id == id)
            })
    }

    pub fn open_metadata_menu(&mut self, database_id: &str, property_id: &str, anchor: &str) {
        if !self.finish_cell(false) {
            return;
        }
        if !self
            .metadata_database(database_id)
            .is_some_and(|db| db.properties.iter().any(|p| p.id == property_id))
        {
            return;
        }
        self.metadata_drag.suppress_press = false;
        self.metadata_menu = Some(MetadataMenu {
            database_id: database_id.into(),
            property_id: property_id.into(),
            anchor: anchor.into(),
            pane: MetadataPane::Main,
            error: None,
        });
    }

    pub fn open_metadata_add_property(&mut self, database_id: &str, anchor: &str) {
        let Some(db) = self.metadata_database(database_id).cloned() else {
            return;
        };
        self.metadata_menu = None;
        self.open_database_property_from(db, None);
        if let Some(editor) = &mut self.database_property_editor {
            editor.anchor_override = Some(anchor.into());
        }
    }

    pub fn edit_metadata_property(&mut self, delete: bool, rename: bool) {
        let Some(menu) = self.metadata_menu.take() else {
            return;
        };
        let Some(db) = self.metadata_database(&menu.database_id).cloned() else {
            return;
        };
        self.open_database_property_from(db, Some(&menu.property_id));
        if let Some(editor) = &mut self.database_property_editor {
            editor.anchor_override = Some(menu.anchor);
            editor.confirm_delete = delete;
            editor.rename_only = rename;
        }
    }

    pub fn set_metadata_layout(
        &mut self,
        database_id: &str,
        layout: moss_core::record_layout::RecordLayout,
    ) -> bool {
        if !self.finish_cell(false) {
            return false;
        }
        let Some(db) = self.metadata_database(database_id).cloned() else {
            return false;
        };
        let expected = moss_core::record_layout::saved(&db).cloned();
        match self
            .store
            .set_record_layout(database_id, expected.as_ref(), &layout)
        {
            Ok(db) => {
                self.apply_database_property_change(db, None);
                true
            }
            Err(error) => {
                if let Some(menu) = &mut self.metadata_menu {
                    menu.error = Some(format!("Layout is not saved: {error}"));
                } else {
                    self.error = Some(format!("Layout is not saved: {error}"));
                }
                false
            }
        }
    }

    pub fn set_metadata_visibility(&mut self, mode: &str) {
        let Some(menu) = &self.metadata_menu else {
            return;
        };
        let (database_id, property_id) = (menu.database_id.clone(), menu.property_id.clone());
        let Some(mut layout) = self
            .metadata_database(&database_id)
            .and_then(moss_core::record_layout::layout)
        else {
            if let Some(menu) = &mut self.metadata_menu {
                menu.error = Some("The saved layout is invalid.".into());
            }
            return;
        };
        if mode == "always" {
            layout.visibility.remove(&property_id);
        } else {
            layout.visibility.insert(property_id, mode.into());
        }
        if self.set_metadata_layout(&database_id, layout) {
            self.metadata_menu = None;
        }
    }

    pub fn move_metadata_property(
        &mut self,
        database_id: &str,
        property_id: &str,
        before: Option<&str>,
    ) {
        let Some(db) = self.metadata_database(database_id) else {
            return;
        };
        let Some(mut layout) = moss_core::record_layout::layout(db) else {
            self.error = Some("The saved layout is invalid.".into());
            return;
        };
        let mut order: Vec<_> = moss_core::record_layout::properties(db)
            .iter()
            .map(|p| p.id.clone())
            .collect();
        if !order.iter().any(|id| id == property_id) || before == Some(property_id) {
            return;
        }
        order.retain(|id| id != property_id);
        let index = before
            .and_then(|id| order.iter().position(|p| p == id))
            .unwrap_or(order.len());
        order.insert(index, property_id.into());
        layout.order = order;
        self.set_metadata_layout(database_id, layout);
    }

    pub fn open_database_property(&mut self, property_id: Option<&str>) {
        let Some(database) = self.tab().and_then(|tab| tab.database.clone()) else {
            return;
        };
        self.open_database_property_from(database, property_id);
    }

    pub fn open_database_property_from(&mut self, database: Database, property_id: Option<&str>) {
        if self
            .record_peek
            .as_ref()
            .is_some_and(|peek| peek.database.id == database.id)
        {
            if !self.finish_cell(false) {
                return;
            }
            if self
                .record_peek
                .as_ref()
                .is_some_and(|peek| peek.dirty_title)
            {
                self.save_peek_title();
            }
            if self
                .record_peek
                .as_ref()
                .is_some_and(|peek| peek.dirty_blocks)
            {
                self.save_peek_blocks();
            }
            if self
                .record_peek
                .as_ref()
                .is_some_and(|peek| peek.dirty_title || peek.dirty_blocks)
            {
                return;
            }
        } else if !self.flush_current() {
            return;
        }
        let property = match property_id {
            Some(id) => match database
                .properties
                .iter()
                .find(|property| property.id == id)
            {
                Some(property) => Some(property.clone()),
                None => {
                    self.error = Some("Property was not found.".into());
                    return;
                }
            },
            None => None,
        };
        let mut name_input = TextEdit::new(property.as_ref().map_or("", |p| p.name.as_str()));
        name_input.focused = true;
        let database_id = database.id.clone();
        self.database_property_editor = Some(PropertyEditor {
            database_id,
            kind: property.as_ref().map_or(PropertyKind::Text, |p| p.kind),
            pending_kind: None,
            color_option: None,
            property,
            confirm_delete: false,
            error: None,
            pane: PropertyPane::Main,
            insert_relative: None,
            filter_op: FilterOp::Eq,
            anchor_override: None,
            rename_only: false,
            name_input,
            options_input: TextEdit::new(""),
            filter_input: TextEdit::new(""),
            advanced: Default::default(),
        });
        if self.database_property_editor.as_ref().is_some_and(|e| {
            matches!(
                e.kind,
                PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
            )
        }) {
            self.reset_advanced_property();
        }
    }

    pub fn choose_property_type(&mut self, kind: PropertyKind) {
        let Some(editor) = self.database_property_editor.as_ref() else {
            return;
        };
        let Some(property) = editor.property.clone() else {
            if let Some(editor) = &mut self.database_property_editor {
                editor.kind = kind;
                editor.error = None;
            }
            if matches!(
                kind,
                PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
            ) {
                self.reset_advanced_property();
            }
            return;
        };
        if !moss_core::property_type_convertible(property.kind)
            || !moss_core::property_type_convertible(kind)
        {
            if let Some(editor) = &mut self.database_property_editor {
                editor.error = Some("This property type cannot be changed.".into());
            }
            return;
        }
        if kind == property.kind {
            if let Some(editor) = &mut self.database_property_editor {
                editor.kind = kind;
                editor.pending_kind = None;
                editor.pane = PropertyPane::Main;
                editor.error = None;
            }
            if matches!(
                kind,
                PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
            ) {
                self.reset_advanced_property();
            }
            return;
        }
        if matches!(
            kind,
            PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
        ) {
            if let Some(editor) = &mut self.database_property_editor {
                editor.kind = kind;
                editor.pending_kind = None;
                editor.pane = PropertyPane::Main;
                editor.error = None;
            }
            self.reset_advanced_property();
            return;
        }
        let database_id = editor.database_id.clone();
        let needs_confirm = moss_core::is_type_change_lossy(property.kind, kind)
            && (matches!(
                property.kind,
                PropertyKind::Formula | PropertyKind::Rollup | PropertyKind::Relation
            ) || self
                .store
                .property_has_values(&database_id, &property.id)
                .unwrap_or(true));
        if needs_confirm {
            if let Some(editor) = &mut self.database_property_editor {
                editor.pending_kind = Some(kind);
                editor.pane = PropertyPane::ConfirmType;
                editor.error = None;
            }
        } else {
            if let Some(editor) = &mut self.database_property_editor {
                editor.pending_kind = Some(kind);
                editor.error = None;
            }
            self.confirm_change_property_type();
        }
    }

    pub fn cancel_change_property_type(&mut self) {
        if let Some(editor) = &mut self.database_property_editor {
            let pending_advanced = editor.pending_kind.is_some_and(|k| {
                matches!(
                    k,
                    PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
                )
            });
            editor.pending_kind = None;
            editor.pane = if pending_advanced {
                PropertyPane::Main
            } else {
                PropertyPane::Type
            };
            editor.error = None;
        }
    }

    pub fn confirm_change_property_type(&mut self) -> bool {
        let Some(editor) = self.database_property_editor.as_ref() else {
            return false;
        };
        let Some(property) = editor.property.clone() else {
            return false;
        };
        let database_id = editor.database_id.clone();
        let target = editor.pending_kind.unwrap_or(editor.kind);
        let name = editor.name_input.text.trim().to_owned();
        if name.is_empty() {
            if let Some(editor) = &mut self.database_property_editor {
                editor.error = Some("Enter a property name.".into());
            }
            return false;
        }
        let draft = &editor.advanced;
        let config = match target {
            PropertyKind::Formula => serde_json::json!({"expression": draft.expression.text}),
            PropertyKind::Relation => {
                serde_json::json!({"target_database_id": draft.target_database_id})
            }
            PropertyKind::Rollup => serde_json::json!({
                "relation_property_id": draft.relation_property_id,
                "target_property_id": draft.target_property_id,
                "operation": draft.operation,
            }),
            _ => serde_json::json!({}),
        };
        match self
            .store
            .change_property_type(&database_id, &property, Some(&name), target, config)
        {
            Ok(database) => {
                self.apply_database_property_change(database, None);
                true
            }
            Err(error) => {
                if let Some(editor) = &mut self.database_property_editor {
                    editor.error = Some(format!("Property type is not changed: {error}"));
                    if matches!(
                        target,
                        PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
                    ) {
                        editor.pane = PropertyPane::Main;
                        editor.pending_kind = None;
                    }
                }
                false
            }
        }
    }

    pub fn set_editor_date_format(&mut self, format: &str) -> bool {
        let Some(editor) = self.database_property_editor.as_ref() else {
            return false;
        };
        let Some(property) = editor.property.clone() else {
            return false;
        };
        let database_id = editor.database_id.clone();
        match self
            .store
            .set_date_property_format(&database_id, &property, format)
        {
            Ok(database) => {
                let updated = database
                    .properties
                    .iter()
                    .find(|p| p.id == property.id)
                    .cloned();
                self.sync_database_schema(&database);
                if let Some(editor) = &mut self.database_property_editor {
                    editor.property = updated;
                    editor.pane = PropertyPane::Main;
                    editor.error = None;
                }
                true
            }
            Err(error) => {
                if let Some(editor) = &mut self.database_property_editor {
                    editor.error = Some(format!("Date format is not saved: {error}"));
                }
                false
            }
        }
    }

    pub fn set_editor_option_color(&mut self, option_name: &str, color: &str) -> bool {
        let Some(editor) = self.database_property_editor.as_ref() else {
            return false;
        };
        let Some(property) = editor.property.clone() else {
            return false;
        };
        let database_id = editor.database_id.clone();
        match self
            .store
            .set_property_option_color(&database_id, &property, option_name, color)
        {
            Ok(database) => {
                let updated = database
                    .properties
                    .iter()
                    .find(|p| p.id == property.id)
                    .cloned();
                self.sync_database_schema(&database);
                if let Some(editor) = &mut self.database_property_editor {
                    editor.property = updated;
                    editor.color_option = None;
                    editor.pane = PropertyPane::Main;
                    editor.error = None;
                }
                true
            }
            Err(error) => {
                if let Some(editor) = &mut self.database_property_editor {
                    editor.error = Some(format!("Color is not saved: {error}"));
                }
                false
            }
        }
    }

    pub fn save_database_property(&mut self) -> bool {
        let Some(editor) = self.database_property_editor.as_ref() else {
            return false;
        };
        if !editor.rename_only {
            if let Some(property) = editor.property.as_ref() {
                if editor.kind != property.kind {
                    let target = editor.kind;
                    let name = editor.name_input.text.trim();
                    if name.is_empty() {
                        self.database_property_editor.as_mut().unwrap().error =
                            Some("Enter a property name.".into());
                        return false;
                    }
                    let missing_config = match target {
                        PropertyKind::Formula
                            if editor.advanced.expression.text.trim().is_empty() =>
                        {
                            Some("Enter a formula expression.")
                        }
                        PropertyKind::Relation
                            if editor.advanced.target_database_id.is_none() =>
                        {
                            Some("Choose a related database.")
                        }
                        PropertyKind::Rollup
                            if editor.advanced.relation_property_id.is_none()
                                || editor.advanced.target_property_id.is_none() =>
                        {
                            Some("Choose a relation and target property.")
                        }
                        _ => None,
                    };
                    if let Some(message) = missing_config {
                        self.database_property_editor.as_mut().unwrap().error =
                            Some(message.into());
                        return false;
                    }
                    let database_id = editor.database_id.clone();
                    let property_id = property.id.clone();
                    let old_kind = property.kind;
                    let needs_confirm = moss_core::is_type_change_lossy(old_kind, target)
                        && (matches!(
                            old_kind,
                            PropertyKind::Formula | PropertyKind::Rollup | PropertyKind::Relation
                        ) || self
                            .store
                            .property_has_values(&database_id, &property_id)
                            .unwrap_or(true));
                    if needs_confirm {
                        if let Some(editor) = &mut self.database_property_editor {
                            editor.pending_kind = Some(target);
                            editor.pane = PropertyPane::ConfirmType;
                            editor.error = None;
                        }
                        return false;
                    }
                    if let Some(editor) = &mut self.database_property_editor {
                        editor.pending_kind = Some(target);
                    }
                    return self.confirm_change_property_type();
                }
            }
        }
        if !editor.rename_only
            && matches!(
                editor.kind,
                PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
            )
        {
            return self.save_advanced_property();
        }
        let name = editor.name_input.text.trim();
        if name.is_empty() {
            self.database_property_editor.as_mut().unwrap().error =
                Some("Enter a property name.".into());
            return false;
        }
        let result = match &editor.property {
            Some(property) => self
                .store
                .rename_property(&editor.database_id, property, name),
            None if editor.insert_relative.is_some() => {
                let options: Vec<String> =
                    if matches!(editor.kind, PropertyKind::Select | PropertyKind::MultiSelect) {
                        editor
                            .options_input
                            .text
                            .split(',')
                            .map(|s| s.trim().to_owned())
                            .collect()
                    } else {
                        Vec::new()
                    };
                let (anchor, after) = editor.insert_relative.as_ref().unwrap();
                self.store.insert_property_relative(
                    &editor.database_id,
                    name,
                    editor.kind,
                    &options,
                    anchor,
                    *after,
                )
            }
            None if matches!(editor.kind, PropertyKind::Select | PropertyKind::MultiSelect) => {
                let options: Vec<String> = editor
                    .options_input
                    .text
                    .split(',')
                    .map(|option| option.trim().to_owned())
                    .collect();
                self.store
                    .add_options_property(&editor.database_id, name, editor.kind, &options)
            }
            None => self
                .store
                .add_property(&editor.database_id, name, editor.kind),
        };
        match result {
            Ok(database) => {
                self.apply_database_property_change(database, None);
                true
            }
            Err(error) => {
                self.database_property_editor.as_mut().unwrap().error =
                    Some(format!("Property is not saved: {error}"));
                false
            }
        }
    }

    pub fn delete_database_property(&mut self) -> bool {
        let Some(editor) = self.database_property_editor.as_ref() else {
            return false;
        };
        let Some(property) = editor.property.as_ref() else {
            return false;
        };
        if property.kind == PropertyKind::Title {
            self.database_property_editor.as_mut().unwrap().error =
                Some("The title property cannot be deleted.".into());
            return false;
        }
        let property_id = property.id.clone();
        match self.store.delete_property(&editor.database_id, property) {
            Ok(database) => {
                self.apply_database_property_change(database, Some(&property_id));
                true
            }
            Err(error) => {
                self.database_property_editor.as_mut().unwrap().error =
                    Some(format!("Property is not deleted: {error}"));
                false
            }
        }
    }

    pub fn close_database_property(&mut self) {
        self.database_property_editor = None;
    }

    pub(crate) fn apply_database_property_change(
        &mut self,
        database: Database,
        deleted: Option<&str>,
    ) {
        let database_id = &database.id;
        let mut affected = Vec::new();
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if tab
                .database
                .as_ref()
                .is_some_and(|db| &db.id == database_id)
            {
                affected.push(index);
                tab.database = Some(database.clone());
                if let Some(id) = deleted {
                    for record in &mut tab.records {
                        record.values.remove(id);
                    }
                }
            }
            if let Some((db, record)) = &mut tab.record_context {
                if &db.id == database_id {
                    *db = database.clone();
                    if let Some(id) = deleted {
                        record.values.remove(id);
                    } else if let Ok(Some(fresh)) = self.store.get_record(&record.id) {
                        *record = fresh;
                    }
                }
            }
        }
        if let Some(peek) = self
            .record_peek
            .as_mut()
            .filter(|p| &p.record.database_id == database_id)
        {
            peek.database = database.clone();
            if let Some(id) = deleted {
                peek.record.values.remove(id);
            } else if let Ok(Some(fresh)) = self.store.get_record(&peek.record.id) {
                peek.record = fresh;
            }
        }
        self.database_property_editor = None;
        let active = self.active;
        let original_filter = self.filter.clone();
        for index in affected {
            self.active = index;
            self.filter = TextEdit::new(&self.tabs[index].title_search);
            self.query_records();
        }
        self.active = active;
        self.filter = original_filter;
    }
}

impl State {
    fn table_property_context(&self) -> Option<(String, moss_core::DatabaseView, Property)> {
        let editor = self.database_property_editor.as_ref()?;
        let property = editor.property.clone()?;
        let tab = self.tab()?;
        let database = tab.database.as_ref()?;
        if database.id != editor.database_id {
            return None;
        }
        let view = database.views.get(tab.view)?.clone();
        Some((database.id.clone(), view, property))
    }

    fn table_action_result(&mut self, result: Result<Database, String>) -> bool {
        match result {
            Ok(database) => {
                self.apply_database_property_change(database, None);
                true
            }
            Err(error) => {
                if let Some(editor) = &mut self.database_property_editor {
                    editor.error = Some(error);
                }
                false
            }
        }
    }

    pub fn insert_table_property(&mut self, after: bool) {
        let Some((_, _, property)) = self.table_property_context() else {
            return;
        };
        self.open_database_property(None);
        if let Some(editor) = &mut self.database_property_editor {
            editor.insert_relative = Some((property.id, after));
        }
    }

    pub fn set_table_sort(&mut self, descending: Option<bool>) {
        let Some((db, view, property)) = self.table_property_context() else {
            return;
        };
        let expected = view.query.sorts;
        let mut next = expected.clone();
        next.retain(|sort| sort.property_id != property.id);
        if let Some(descending) = descending {
            next.push(SortRule {
                property_id: property.id,
                descending,
            });
        }
        let result = self
            .store
            .set_view_sorts(&db, &view.id, expected, next)
            .map_err(|e| e.to_string());
        self.table_action_result(result);
    }

    pub fn set_table_group(&mut self) {
        let Some((db, view, property)) = self.table_property_context() else {
            return;
        };
        let expected = view.query.group_by;
        let next = if expected.as_deref() == Some(property.id.as_str()) {
            None
        } else {
            Some(property.id)
        };
        let result = self
            .store
            .set_view_group(&db, &view.id, expected, next)
            .map_err(|e| e.to_string());
        self.table_action_result(result);
    }

    pub fn set_table_filter(&mut self) {
        let Some((db, view, property)) = self.table_property_context() else {
            return;
        };
        let editor = self.database_property_editor.as_ref().unwrap();
        let op = editor.filter_op;
        let text = editor.filter_input.text.trim();
        let value = if matches!(op, FilterOp::IsEmpty | FilterOp::IsNotEmpty) {
            serde_json::Value::Null
        } else {
            match property.kind {
                PropertyKind::Number => match text
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                {
                    Some(n) => serde_json::Value::Number(n),
                    None => {
                        if let Some(editor) = &mut self.database_property_editor {
                            editor.error = Some("Enter a valid number.".into());
                        }
                        return;
                    }
                },
                PropertyKind::Checkbox => match text.to_ascii_lowercase().as_str() {
                    "true" => serde_json::Value::Bool(true),
                    "false" => serde_json::Value::Bool(false),
                    _ => {
                        if let Some(editor) = &mut self.database_property_editor {
                            editor.error = Some("Enter true or false.".into());
                        }
                        return;
                    }
                },
                PropertyKind::Date => {
                    if chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_err() {
                        if let Some(editor) = &mut self.database_property_editor {
                            editor.error = Some("Enter a date as YYYY-MM-DD.".into());
                        }
                        return;
                    }
                    serde_json::Value::String(text.to_owned())
                }
                _ => serde_json::Value::String(text.to_owned()),
            }
        };
        let rule = FilterExpr::Rule(FilterRule {
            property_id: property.id,
            op,
            value,
        });
        let expected = view.query.filter;
        let next = Some(match expected.clone() {
            Some(FilterExpr::All(mut existing)) => {
                existing.push(rule);
                FilterExpr::All(existing)
            }
            Some(existing) => FilterExpr::All(vec![existing, rule]),
            None => rule,
        });
        let result = self
            .store
            .set_view_filter(&db, &view.id, expected, next)
            .map_err(|e| e.to_string());
        self.table_action_result(result);
    }

    pub fn clear_table_filter(&mut self) {
        let Some((db, view, property)) = self.table_property_context() else {
            return;
        };
        let expected = view.query.filter;
        let next = match expected.clone() {
            Some(FilterExpr::Rule(rule)) if rule.property_id == property.id => None,
            Some(FilterExpr::All(rules)) => {
                let mut kept: Vec<_> = rules.into_iter().filter(|item| !matches!(item, FilterExpr::Rule(rule) if rule.property_id == property.id)).collect();
                match kept.len() {
                    0 => None,
                    1 => kept.pop(),
                    _ => Some(FilterExpr::All(kept)),
                }
            }
            other => other,
        };
        if next == expected {
            if let Some(editor) = &mut self.database_property_editor {
                editor.error = Some(
                    "No direct filter for this property. Nested filters are unchanged.".into(),
                );
            }
            return;
        }
        let result = self
            .store
            .set_view_filter(&db, &view.id, expected, next)
            .map_err(|e| e.to_string());
        self.table_action_result(result);
    }

    pub fn set_table_config(&mut self, key: &'static str, next: serde_json::Value) {
        let Some((db, view, _)) = self.table_property_context() else {
            return;
        };
        let expected = view.config.get(key).cloned();
        let result = self
            .store
            .edit_view_config(&db, &view.id, ViewKind::Table, |config| {
                let map = config.as_object_mut().ok_or("Invalid view settings")?;
                if map.get(key) != expected.as_ref() {
                    return Err("This setting changed in another window. Reopen the menu.".into());
                }
                map.insert(key.into(), next);
                Ok(())
            })
            .map_err(|e| e.to_string());
        self.table_action_result(result);
    }

    pub fn set_table_calculation(&mut self, operation: Option<&str>) {
        let Some((db, view, property)) = self.table_property_context() else {
            return;
        };
        let expected = view
            .config
            .get("calculations")
            .and_then(|v| v.get(&property.id))
            .cloned();
        let id = property.id;
        let next = operation.map(str::to_owned);
        let result = self
            .store
            .edit_view_config(&db, &view.id, ViewKind::Table, |config| {
                let config = config.as_object_mut().ok_or("Invalid view settings")?;
                let values = config
                    .entry("calculations")
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut()
                    .ok_or("Invalid calculation settings")?;
                if values.get(&id) != expected.as_ref() {
                    return Err(
                        "This calculation changed in another window. Reopen the menu.".into(),
                    );
                }
                if let Some(operation) = next {
                    values.insert(id, operation.into());
                } else {
                    values.remove(&id);
                }
                Ok(())
            })
            .map_err(|e| e.to_string());
        if self.table_action_result(result) {
            self.refresh_table_summaries();
        }
    }
}
