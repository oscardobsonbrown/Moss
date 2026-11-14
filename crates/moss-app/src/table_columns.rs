use crate::state::State;
use moss_core::{Database, Property, PropertyKind};
use rune::{Drag, Phase};
use serde_json::Value;

pub struct Resize {
    database: String,
    view: String,
    property: String,
    expected: Option<Value>,
    start: f32,
    pub width: f32,
}

pub fn width(s: &State, db: &Database, view: usize, property: &Property) -> f32 {
    let Some(view) = db.views.get(view) else {
        return 200.;
    };
    if let Some(drag) = &s.database_views.table_resize {
        if drag.database == db.id && drag.view == view.id && drag.property == property.id {
            return drag.width;
        }
    }
    view.config
        .get("column_widths")
        .and_then(|widths| widths.get(&property.id))
        .and_then(Value::as_f64)
        .filter(|width| width.is_finite() && (96.0..=1000.0).contains(width))
        .map(|width| width as f32)
        .unwrap_or(if property.kind == PropertyKind::Title {
            280.
        } else {
            200.
        })
}

impl State {
    pub fn resize_table_column(&mut self, property: &str, drag: &Drag) {
        if drag.cancelled() {
            self.database_views.table_resize = None;
            return;
        }
        if drag.phase == Phase::Down {
            let Some(tab) = self.tab() else { return };
            let Some(db) = &tab.database else { return };
            let Some(view) = db.views.get(tab.view) else {
                return;
            };
            let Some(prop) = db.properties.iter().find(|p| p.id == property) else {
                return;
            };
            self.database_views.table_resize = Some(Resize {
                database: db.id.clone(),
                view: view.id.clone(),
                property: property.to_owned(),
                expected: view
                    .config
                    .get("column_widths")
                    .and_then(|v| v.get(property))
                    .cloned(),
                start: width(self, db, tab.view, prop),
                width: width(self, db, tab.view, prop),
            });
            return;
        }
        let Some(resize) = &mut self.database_views.table_resize else {
            return;
        };
        if resize.property != property {
            return;
        }
        resize.width = (resize.start + drag.dx).clamp(96., 1000.);
        if drag.phase == Phase::Up {
            let resize = self.database_views.table_resize.take().unwrap();
            if (resize.width - resize.start).abs() < 0.5 {
                return;
            }
            match self.store.set_table_column_width(
                &resize.database,
                &resize.view,
                &resize.property,
                resize.expected,
                resize.width,
            ) {
                Ok(db) => self.apply_table_column_change(db, &resize.view, false),
                Err(error) => self.error = Some(format!("Column width is not saved: {error}")),
            }
        }
    }
}

pub struct Move {
    database: String,
    view: String,
    property: String,
    expected_layout: Option<Value>,
    expected_title: Option<Value>,
    expected_columns: Vec<String>,
    pub before: Option<String>,
    pub active: bool,
    pub dx: f32,
    rects: Vec<(String, [f32; 4])>,
}

impl State {
    pub fn move_table_column(&mut self, property: &str, drag: &Drag, rects: &[(String, [f32; 4])]) {
        if drag.cancelled() {
            self.database_views.table_move = None;
            return;
        }
        if drag.phase == Phase::Down {
            let Some(tab) = self.tab() else { return };
            let Some(db) = &tab.database else { return };
            let Some(view) = db.views.get(tab.view) else {
                return;
            };
            self.database_views.table_move = Some(Move {
                database: db.id.clone(),
                view: view.id.clone(),
                property: property.to_owned(),
                expected_layout: moss_core::record_layout::saved(db).cloned(),
                expected_title: view.config.get("title_position").cloned(),
                expected_columns: moss_core::record_layout::columns_for_view(db, tab.view)
                    .iter()
                    .map(|p| p.id.clone())
                    .collect(),
                before: None,
                active: false,
                dx: 0.,
                rects: rects.to_vec(),
            });
            return;
        }
        let Some(moving) = &mut self.database_views.table_move else {
            return;
        };
        if moving.property != property {
            return;
        }
        if drag.dx.hypot(drag.dy) >= 4. {
            moving.active = true;
            moving.dx = drag.dx;
            let x = drag.window_point()[0];
            // shortcut: off-screen columns need manual scrolling; add edge scrolling for long tables.
            moving.before = moving
                .rects
                .iter()
                .find(|(id, r)| id != property && x < r[0] + r[2] / 2.)
                .map(|(id, _)| id.clone());
        }
        if drag.phase == Phase::Up {
            let moving = self.database_views.table_move.take().unwrap();
            if !moving.active {
                return;
            }
            match self.store.reorder_table_column(
                &moving.database,
                &moving.view,
                &moving.property,
                moving.before.as_deref(),
                moving.expected_layout,
                moving.expected_title,
                &moving.expected_columns,
            ) {
                Ok(db) => self.apply_table_column_change(db, &moving.view, true),
                Err(error) => self.error = Some(format!("Column order is not saved: {error}")),
            }
        }
    }
}

/// Return the pointer offset or the distance a neighbouring column moves aside.
pub fn drag_offset(
    s: &State,
    db: &Database,
    view: usize,
    property: &Property,
    columns: &[&Property],
) -> Option<(f32, bool)> {
    let moving = s.database_views.table_move.as_ref()?;
    if !moving.active || moving.database != db.id || db.views.get(view)?.id != moving.view {
        return None;
    }
    let source = columns.iter().position(|p| p.id == moving.property)?;
    let current = columns.iter().position(|p| p.id == property.id)?;
    if source == current {
        return Some((moving.dx, true));
    }
    let target = moving
        .before
        .as_ref()
        .and_then(|id| {
            columns
                .iter()
                .filter(|p| p.id != moving.property)
                .position(|p| &p.id == id)
        })
        .unwrap_or(columns.len() - 1);
    let shift = width(s, db, view, columns[source]);
    let offset = if current < source && current >= target {
        shift
    } else if current > source && current <= target {
        -shift
    } else {
        0.
    };
    Some((offset, false))
}

fn copy_field(old: &mut Value, new: &Value, key: &str) -> bool {
    if old.is_null() {
        *old = serde_json::json!({});
    }
    let Some(old) = old.as_object_mut() else {
        return false;
    };
    let Some(new) = new.as_object() else {
        return false;
    };
    match new.get(key) {
        Some(value) => {
            old.insert(key.to_owned(), value.clone());
        }
        None => {
            old.remove(key);
        }
    }
    true
}

fn only_column_change(old: &Database, new: &Database, view: &str, reordered: bool) -> bool {
    let mut expected = old.clone();
    let Some(old_view) = expected.views.iter_mut().find(|v| v.id == view) else {
        return false;
    };
    let Some(new_view) = new.views.iter().find(|v| v.id == view) else {
        return false;
    };
    if !copy_field(
        &mut old_view.config,
        &new_view.config,
        if reordered {
            "title_position"
        } else {
            "column_widths"
        },
    ) {
        return false;
    }
    if reordered {
        let Some(old_title) = expected
            .properties
            .iter_mut()
            .find(|p| p.kind == PropertyKind::Title)
        else {
            return false;
        };
        let Some(new_title) = new
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Title)
        else {
            return false;
        };
        if !copy_field(
            &mut old_title.config,
            &new_title.config,
            "record_page_layout",
        ) {
            return false;
        }
    }
    expected == *new
}

impl State {
    fn apply_table_column_change(&mut self, database: Database, view: &str, reordered: bool) {
        if self
            .tabs
            .iter()
            .filter_map(|tab| tab.database.as_ref())
            .filter(|db| db.id == database.id)
            .any(|old| !only_column_change(old, &database, view, reordered))
        {
            self.apply_database_property_change(database, None);
            return;
        }
        for tab in &mut self.tabs {
            if tab.database.as_ref().is_some_and(|db| db.id == database.id) {
                tab.database = Some(database.clone());
            }
            if let Some((db, _)) = &mut tab.record_context {
                if db.id == database.id {
                    *db = database.clone();
                }
            }
        }
        if let Some(peek) = self
            .record_peek
            .as_mut()
            .filter(|peek| peek.database.id == database.id)
        {
            peek.database = database;
        }
    }
}
