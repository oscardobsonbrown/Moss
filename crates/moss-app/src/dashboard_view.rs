use crate::state::State;
use moss_core::{
    Database, FilterExpr, Record, Store, ViewKind,
    dashboard::{DashboardConfig, DashboardWidget},
};
use rune::Tx;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

#[derive(Clone)]
pub struct WidgetData {
    pub database: Database,
    pub view: moss_core::DatabaseView,
    pub records: Vec<Record>,
    pub has_more: bool,
    pub chart: Option<moss_core::chart::ChartProjection>,
    pub map: Option<moss_core::MapPoints>,
    pub map_base: Option<crate::map_view::MapViewport>,
}
struct Response {
    key: String,
    candidates: Result<Vec<Database>, String>,
    widgets: BTreeMap<String, Result<WidgetData, String>>,
}
#[derive(Clone, Debug)]
pub struct MapDrag {
    pub widget_id: String,
    pub start: crate::map_view::MapViewport,
    pub preview: crate::map_view::MapViewport,
}
pub struct DashboardState {
    pub editing: bool,
    pub adding: bool,
    pub candidates: Vec<Database>,
    pub widgets: BTreeMap<String, Result<WidgetData, String>>,
    pub error: Option<String>,
    pub filtering: bool,
    pub chart_drilldown: Option<(String, usize)>,
    pub chart_settings: Option<String>,
    pub chart_settings_source: Option<Database>,
    pub filter_widget: Option<String>,
    pub filter_property: Option<(String, moss_core::PropertyKind)>,
    pub filter_input: rune::TextEdit,
    pub source_input: rune::TextEdit,
    pub source_search: String,
    pub map_viewports: BTreeMap<String, crate::map_view::MapViewport>,
    pub map_drag: Option<MapDrag>,
    map_bases: BTreeMap<String, crate::map_view::MapViewport>,
    pub map_tiles: BTreeMap<String, crate::map_view::MapTileLoader>,
    pub map_surfaces: BTreeMap<String, crate::map_view::MapSurface>,
    tile_keys: BTreeMap<String, String>,
    waker: Option<rune::Waker>,
    context: String,
    data_signature: String,
    key: String,
    requested: String,
    receiver: Option<mpsc::Receiver<Response>>,
    worker: Option<std::thread::JoinHandle<()>>,
    interrupt: Arc<Mutex<Option<moss_core::InterruptHandle>>>,
    generation: Arc<AtomicU64>,
    cancelled: bool,
}
impl Default for DashboardState {
    fn default() -> Self {
        Self {
            editing: false,
            adding: false,
            candidates: Vec::new(),
            widgets: BTreeMap::new(),
            error: None,
            filtering: false,
            chart_drilldown: None,
            chart_settings: None,
            chart_settings_source: None,
            filter_widget: None,
            filter_property: None,
            filter_input: rune::TextEdit::new(""),
            source_input: rune::TextEdit::new(""),
            source_search: String::new(),
            map_viewports: BTreeMap::new(),
            map_drag: None,
            map_bases: BTreeMap::new(),
            map_tiles: BTreeMap::new(),
            map_surfaces: BTreeMap::new(),
            tile_keys: BTreeMap::new(),
            waker: None,
            context: String::new(),
            data_signature: String::new(),
            key: String::new(),
            requested: String::new(),
            receiver: None,
            worker: None,
            interrupt: Default::default(),
            generation: Default::default(),
            cancelled: false,
        }
    }
}
impl DashboardState {
    pub fn set_waker(&mut self, wake: rune::Waker) {
        self.waker = Some(wake);
    }
    fn cancel_worker(&mut self) {
        if self.receiver.is_some() && !self.cancelled {
            self.cancelled = true;
            self.generation.fetch_add(1, Ordering::Relaxed);
            if let Ok(handle) = self.interrupt.lock() {
                if let Some(handle) = handle.as_ref() {
                    handle.interrupt();
                }
            }
        }
    }
}
impl Drop for DashboardState {
    fn drop(&mut self) {
        if let Ok(handle) = self.interrupt.lock() {
            if let Some(handle) = handle.as_ref() {
                handle.interrupt();
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl State {
    fn dashboard_target(&self) -> Option<(String, String, DashboardConfig)> {
        let tab = self.tab()?;
        let db = tab.database.as_ref()?;
        let view = db.views.get(tab.view)?;
        if view.kind != ViewKind::Dashboard {
            return None;
        }
        Some((
            db.id.clone(),
            view.id.clone(),
            DashboardConfig::read(&view.config).ok()?,
        ))
    }
    pub fn dashboard_save(&mut self, next: DashboardConfig) -> bool {
        let Some((db_id, view_id, old)) = self.dashboard_target() else {
            return false;
        };
        match self.store.save_dashboard(&db_id, &view_id, &old, &next) {
            Ok(db) => {
                self.tabs[self.active].database = Some(db);
                self.database_views.dashboard.error = None;
                true
            }
            Err(error) => {
                self.database_views.dashboard.error = Some(error.to_string());
                false
            }
        }
    }
    pub fn dashboard_retry(&mut self) {
        let state = &mut self.database_views.dashboard;
        state.requested.clear();
        state.key.clear();
        state.error = None;
    }
    pub fn dashboard_map_drag_start(&mut self, id: &str) {
        let state = &mut self.database_views.dashboard;
        let start = state
            .map_viewports
            .get(id)
            .copied()
            .or_else(|| state.map_bases.get(id).copied())
            .unwrap_or_default();
        state.map_drag = Some(MapDrag {
            widget_id: id.into(),
            start,
            preview: start,
        });
    }
    pub fn dashboard_map_drag_move(&mut self, id: &str, dx: f32, dy: f32) {
        if let Some(drag) = self
            .database_views
            .dashboard
            .map_drag
            .as_mut()
            .filter(|drag| drag.widget_id == id)
        {
            drag.preview = drag.start.pan(dx, dy);
        }
    }
    pub fn dashboard_map_drag_end(&mut self, id: &str, dx: f32, dy: f32, cancelled: bool) {
        let state = &mut self.database_views.dashboard;
        if state
            .map_drag
            .as_ref()
            .is_some_and(|drag| drag.widget_id == id)
        {
            let drag = state.map_drag.take().unwrap();
            if !cancelled {
                state
                    .map_viewports
                    .insert(id.into(), drag.start.pan(dx, dy));
            }
        }
    }
    pub fn dashboard_map_zoom(&mut self, id: &str, delta: i8) {
        let state = &mut self.database_views.dashboard;
        let viewport = state
            .map_viewports
            .get(id)
            .copied()
            .or_else(|| state.map_bases.get(id).copied())
            .unwrap_or_default()
            .zoomed(delta);
        state.map_viewports.insert(id.to_owned(), viewport);
    }
    pub fn dashboard_apply_filter(&mut self) {
        let Some((name, kind)) = self.database_views.dashboard.filter_property.clone() else {
            return;
        };
        let text = self.database_views.dashboard.filter_input.text.trim();
        let value = if kind == moss_core::PropertyKind::Number {
            match text.parse::<f64>() {
                Ok(v) if v.is_finite() => serde_json::json!(v),
                _ => {
                    self.database_views.dashboard.error = Some("Enter a valid number.".into());
                    return;
                }
            }
        } else if kind == moss_core::PropertyKind::Checkbox {
            match text.to_ascii_lowercase().as_str() {
                "true" => serde_json::json!(true),
                "false" => serde_json::json!(false),
                _ => {
                    self.database_views.dashboard.error = Some("Enter true or false.".into());
                    return;
                }
            }
        } else if kind == moss_core::PropertyKind::Date {
            if chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_err() {
                self.database_views.dashboard.error = Some("Enter a date as YYYY-MM-DD.".into());
                return;
            }
            serde_json::json!(text)
        } else {
            serde_json::json!(text)
        };
        let Some((_, _, mut config)) = self.dashboard_target() else {
            return;
        };
        if let Some(widget_id) = self.database_views.dashboard.filter_widget.clone() {
            let Some(data) = self
                .database_views
                .dashboard
                .widgets
                .get(&widget_id)
                .and_then(|v| v.as_ref().ok())
            else {
                return;
            };
            let mut matches = data
                .database
                .properties
                .iter()
                .filter(|p| p.name == name && p.kind == kind);
            let Some(property) = matches.next() else {
                return;
            };
            if matches.next().is_some() {
                self.database_views.dashboard.error = Some("Property name is ambiguous.".into());
                return;
            }
            let rule = FilterExpr::Rule(moss_core::FilterRule {
                property_id: property.id.clone(),
                op: moss_core::FilterOp::Eq,
                value,
            });
            let Some(widget) = config.widgets.iter_mut().find(|w| w.id == widget_id) else {
                return;
            };
            let mut query = widget.query.clone().unwrap_or_default();
            query.filter = Some(match query.filter.take() {
                Some(saved) => FilterExpr::All(vec![saved, rule]),
                None => rule,
            });
            widget.query = Some(query);
        } else {
            config
                .global_filters
                .push(moss_core::dashboard::DashboardFilter {
                    property_name: name,
                    property_kind: kind,
                    op: moss_core::FilterOp::Eq,
                    value,
                });
        }
        if self.dashboard_save(config) {
            self.database_views.dashboard.filtering = false;
        }
    }
    pub fn dashboard_remove_filter(&mut self, index: usize) {
        if let Some((_, _, mut config)) = self.dashboard_target() {
            if index < config.global_filters.len() {
                config.global_filters.remove(index);
                self.dashboard_save(config);
            }
        }
    }
    pub fn dashboard_reset_widget_query(&mut self, id: &str) {
        if let Some((_, _, mut config)) = self.dashboard_target() {
            if let Some(widget) = config.widgets.iter_mut().find(|w| w.id == id) {
                widget.query = None;
                self.dashboard_save(config);
            }
        }
    }
    pub fn dashboard_add_widget(&mut self, database_id: String, view_id: String) {
        let Some((_, _, mut config)) = self.dashboard_target() else {
            return;
        };
        config.widgets.push(DashboardWidget {
            id: moss_core::new_id(),
            database_id,
            view_id,
            width: 2,
            height: 300,
            query: None,
            layout: serde_json::Value::Null,
            extra: Default::default(),
        });
        if self.dashboard_save(config) {
            self.database_views.dashboard.adding = false;
        }
    }
    pub fn dashboard_add_figure(&mut self, database_id: String, view_id: String) {
        let Some((_, _, mut config)) = self.dashboard_target() else {
            return;
        };
        let chart = moss_core::chart::ChartConfig {
            kind: moss_core::chart::ChartType::Number,
            ..Default::default()
        };
        config.widgets.push(DashboardWidget {
            id: moss_core::new_id(),
            database_id,
            view_id,
            width: 2,
            height: 300,
            query: None,
            layout: serde_json::json!({"chart": chart}),
            extra: Default::default(),
        });
        if self.dashboard_save(config) {
            self.database_views.dashboard.adding = false;
        }
    }
    pub fn dashboard_toggle_chart_settings(&mut self, id: &str) {
        if self.database_views.dashboard.chart_settings.as_deref() == Some(id) {
            self.database_views.dashboard.chart_settings = None;
            self.database_views.dashboard.chart_settings_source = None;
            return;
        }
        let Some((_, _, config)) = self.dashboard_target() else {
            return;
        };
        let Some(widget) = config.widgets.iter().find(|w| w.id == id) else {
            return;
        };
        match self.store.get_database(&widget.database_id) {
            Ok(Some(db))
                if self
                    .store
                    .get_page(&db.page_id)
                    .ok()
                    .flatten()
                    .is_some_and(|page| {
                        page.workspace_id == self.workspace.id && page.deleted_at.is_none()
                    })
                    && db.views.iter().any(|view| {
                        view.id == widget.view_id && view.kind != ViewKind::Dashboard
                    }) =>
            {
                self.database_views.dashboard.chart_settings = Some(id.to_owned());
                self.database_views.dashboard.chart_settings_source = Some(db);
            }
            Ok(_) => {
                self.database_views.dashboard.error = Some("Widget database is unavailable.".into())
            }
            Err(error) => self.database_views.dashboard.error = Some(error.to_string()),
        }
    }
    fn dashboard_edit_chart(
        &mut self,
        id: &str,
        edit: impl FnOnce(&Database, &mut moss_core::chart::ChartConfig),
    ) {
        let Some((_, _, mut config)) = self.dashboard_target() else {
            return;
        };
        let Some(widget) = config.widgets.iter_mut().find(|w| w.id == id) else {
            return;
        };
        let Ok(Some(db)) = self.store.get_database(&widget.database_id) else {
            return;
        };
        let source = db.views.iter().find(|v| v.id == widget.view_id);
        let chart_source = if widget.layout.get("chart").is_some() {
            &widget.layout
        } else if let Some(view) = source {
            &view.config
        } else {
            return;
        };
        let Ok(mut chart) = crate::chart_view::config(chart_source) else {
            return;
        };
        edit(&db, &mut chart);
        if !widget.layout.is_object() {
            widget.layout = serde_json::json!({});
        }
        widget.layout["chart"] = serde_json::json!(chart);
        self.dashboard_save(config);
    }
    pub fn dashboard_chart_kind(&mut self, id: &str, kind: moss_core::chart::ChartType) {
        self.dashboard_edit_chart(id, |db, chart| {
            chart.kind = kind;
            if kind != moss_core::chart::ChartType::Number && chart.category_property_id.is_none() {
                chart.category_property_id = db
                    .properties
                    .iter()
                    .find(|p| {
                        matches!(
                            p.kind,
                            moss_core::PropertyKind::Status
                                | moss_core::PropertyKind::Select
                                | moss_core::PropertyKind::Date
                        )
                    })
                    .or_else(|| {
                        db.properties
                            .iter()
                            .find(|p| p.kind == moss_core::PropertyKind::Title)
                    })
                    .map(|p| p.id.clone());
            }
        });
    }
    pub fn dashboard_chart_category(&mut self, id: &str, property: String) {
        self.dashboard_edit_chart(id, |_, chart| chart.category_property_id = Some(property));
    }
    pub fn dashboard_chart_bucket(&mut self, id: &str, bucket: moss_core::chart::DateBucket) {
        self.dashboard_edit_chart(id, |_, chart| chart.date_bucket = bucket);
    }
    pub fn dashboard_chart_aggregate(
        &mut self,
        id: &str,
        aggregation: moss_core::chart::Aggregate,
        property: Option<String>,
    ) {
        self.dashboard_edit_chart(id, |_, chart| {
            chart.aggregation = aggregation;
            chart.value_property_id = property;
        });
    }
    pub fn dashboard_remove_widget(&mut self, id: &str) {
        if let Some((_, _, mut c)) = self.dashboard_target() {
            c.widgets.retain(|w| w.id != id);
            self.dashboard_save(c);
        }
    }
    pub fn dashboard_duplicate_widget(&mut self, id: &str) {
        if let Some((_, _, mut c)) = self.dashboard_target() {
            if let Some(i) = c.widgets.iter().position(|w| w.id == id) {
                let mut w = c.widgets[i].clone();
                w.id = moss_core::new_id();
                c.widgets.insert(i + 1, w);
                self.dashboard_save(c);
            }
        }
    }
    pub fn dashboard_move_widget(&mut self, id: &str, delta: isize) {
        if let Some((_, _, mut c)) = self.dashboard_target() {
            if let Some(i) = c.widgets.iter().position(|w| w.id == id) {
                let target = (i as isize + delta).clamp(0, c.widgets.len() as isize - 1) as usize;
                if target != i {
                    let w = c.widgets.remove(i);
                    c.widgets.insert(target, w);
                    self.dashboard_save(c);
                }
            }
        }
    }
    pub fn dashboard_reorder_widget(&mut self, id: &str, before: &str) {
        if id == before {
            return;
        }
        if let Some((_, _, mut config)) = self.dashboard_target() {
            if let Some(index) = config.widgets.iter().position(|w| w.id == id) {
                let widget = config.widgets.remove(index);
                let target = config
                    .widgets
                    .iter()
                    .position(|w| w.id == before)
                    .unwrap_or(config.widgets.len());
                config.widgets.insert(target, widget);
                self.dashboard_save(config);
            }
        }
    }
    pub fn dashboard_resize_width(&mut self, id: &str, delta: i8) {
        if let Some((_, _, mut config)) = self.dashboard_target() {
            if let Some(widget) = config.widgets.iter_mut().find(|w| w.id == id) {
                widget.width = (widget.width as i8 + delta).clamp(1, 4) as u8;
                self.dashboard_save(config);
            }
        }
    }
    pub fn dashboard_resize_widget(&mut self, id: &str, delta: i16) {
        if let Some((_, _, mut c)) = self.dashboard_target() {
            if let Some(w) = c.widgets.iter_mut().find(|w| w.id == id) {
                w.height = (w.height as i16 + delta).clamp(120, 1200) as u16;
                self.dashboard_save(c);
            }
        }
    }
}
fn load(
    store: &Store,
    workspace: &str,
    search: &str,
    config: &DashboardConfig,
    viewports: &BTreeMap<String, crate::map_view::MapViewport>,
    bases: &BTreeMap<String, crate::map_view::MapViewport>,
    cancelled: &dyn Fn() -> bool,
) -> (
    Result<Vec<Database>, String>,
    BTreeMap<String, Result<WidgetData, String>>,
) {
    if cancelled() {
        return (Err("Dashboard request cancelled".into()), BTreeMap::new());
    }
    let candidates = store
        .search_relation_databases(workspace, search, 200)
        .map_err(|e| e.to_string());
    let widgets = config
        .widgets
        .iter()
        .map(|widget| {
            let value = (|| -> Result<WidgetData, String> {
                if cancelled() {
                    return Err("Dashboard request cancelled".into());
                }
                let db = store
                    .get_database(&widget.database_id)
                    .map_err(|e| e.to_string())?
                    .ok_or("Database missing")?;
                let page = store
                    .get_page(&db.page_id)
                    .map_err(|e| e.to_string())?
                    .ok_or("Database page missing")?;
                if page.workspace_id != workspace
                    || page.deleted_at.is_some()
                    || page.kind != moss_core::PageKind::Database
                {
                    return Err("Database is unavailable".into());
                }
                let view = db
                    .views
                    .iter()
                    .find(|v| v.id == widget.view_id && v.kind != ViewKind::Dashboard)
                    .cloned()
                    .ok_or("View missing")?;
                let mut query = view.query.clone();
                if let Some(extra) = widget.query.as_ref().and_then(|q| q.filter.clone()) {
                    query.filter = Some(match query.filter.take() {
                        Some(saved) => FilterExpr::All(vec![saved, extra]),
                        None => extra,
                    });
                }
                if let Some(override_query) = &widget.query {
                    if !override_query.sorts.is_empty() {
                        query.sorts = override_query.sorts.clone();
                    }
                }
                if let Some(extra) = config.filter_for(&db) {
                    query.filter = Some(match query.filter.take() {
                        Some(saved) => FilterExpr::All(vec![saved, extra]),
                        None => extra,
                    });
                }
                let chart = if view.kind == ViewKind::Chart || widget.layout.get("chart").is_some()
                {
                    let chart_config =
                        crate::chart_view::config(if widget.layout.get("chart").is_some() {
                            &widget.layout
                        } else {
                            &view.config
                        })?;
                    let mut extras = Vec::new();
                    if let Some(filter) = config.filter_for(&db) {
                        extras.push(filter);
                    }
                    if let Some(filter) = widget.query.as_ref().and_then(|q| q.filter.clone()) {
                        extras.push(filter);
                    }
                    let extra = (!extras.is_empty()).then_some(FilterExpr::All(extras));
                    Some(
                        store
                            .chart_projection(&db.id, &view.id, &chart_config, extra.as_ref(), "")
                            .map_err(|e| e.to_string())?,
                    )
                } else {
                    None
                };
                let map_base = (view.kind == ViewKind::Map && chart.is_none()).then(|| {
                    crate::map_view::MapViewport::from_config(&view.config)
                        .with_config(&widget.layout)
                });
                let map = if view.kind == ViewKind::Map && chart.is_none() {
                    let place = widget
                        .layout
                        .get("map_place_property")
                        .or_else(|| view.config.get("map_place_property"))
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            db.properties
                                .iter()
                                .find(|p| p.kind == moss_core::PropertyKind::Place)
                                .map(|p| p.id.as_str())
                        })
                        .ok_or("Choose a Place property in the Map view")?;
                    let base = map_base.expect("Map source has a viewport");
                    let viewport = if bases.get(&widget.id) == Some(&base) {
                        viewports.get(&widget.id).copied().unwrap_or(base)
                    } else {
                        base
                    };
                    let viewport = crate::map_view::MapViewport {
                        width: (170. * f64::from(widget.width) - 20.).max(100.),
                        height: (f64::from(widget.height) - 80.).max(40.),
                        ..viewport
                    };
                    Some(
                        store
                            .map_points(&db.id, &query, "", place, viewport.bounds(), 500)
                            .map_err(|e| e.to_string())?,
                    )
                } else {
                    None
                };
                query.offset = 0;
                query.limit = 101;
                let mut records = if chart.is_some() || map.is_some() {
                    Vec::new()
                } else {
                    store
                        .query_records(&db.id, &query)
                        .map_err(|e| e.to_string())?
                };
                let has_more = records.len() > 100;
                records.truncate(100);
                Ok(WidgetData {
                    database: db,
                    view,
                    records,
                    has_more,
                    chart,
                    map,
                    map_base,
                })
            })();
            (widget.id.clone(), value)
        })
        .collect();
    (candidates, widgets)
}
pub fn poll(s: &mut State, _time: f64, tx: &mut Tx) -> bool {
    let Some((db, view, config)) = s.dashboard_target() else {
        let state = &mut s.database_views.dashboard;
        state.context.clear();
        state.data_signature.clear();
        state.key.clear();
        state.requested.clear();
        state.cancel_worker();
        state.map_tiles.clear();
        state.map_surfaces.clear();
        state.tile_keys.clear();
        return false;
    };
    let change_count = s.store.local_change_count();
    let state = &mut s.database_views.dashboard;
    let context = format!("{db}:{view}");
    let mut changed = false;
    if state.context != context {
        state.cancel_worker();
        state.key.clear();
        state.requested.clear();
        state.context = context;
        state.data_signature.clear();
        state.editing = false;
        state.adding = false;
        state.filtering = false;
        state.filter_widget = None;
        state.filter_property = None;
        state.chart_drilldown = None;
        state.source_search.clear();
        state.source_input.replace_text("");
        state.filter_input.replace_text("");
        state.error = None;
        state.widgets.clear();
        state.candidates.clear();
        state.map_tiles.clear();
        state.map_surfaces.clear();
        state.tile_keys.clear();
        state.map_viewports.clear();
        state.map_drag = None;
        state.map_bases.clear();
        changed = true;
    }
    let key = format!(
        "{}:{}:{}:{}:{:?}:{}",
        db,
        view,
        change_count,
        serde_json::to_string(&config).unwrap_or_default(),
        state.map_viewports,
        state.source_search
    );
    let data_signature = format!(
        "{}:{}:{}:{}:{}",
        db,
        view,
        change_count,
        serde_json::to_string(&config).unwrap_or_default(),
        state.source_search
    );
    if state.receiver.is_some() && state.requested != key {
        state.cancel_worker();
    }
    if let Some(receiver) = &state.receiver {
        match receiver.try_recv() {
            Ok(response) => {
                if !state.cancelled && response.key == key {
                    state.key = response.key;
                    state.data_signature = data_signature.clone();
                    match response.candidates {
                        Ok(candidates) => {
                            state.candidates = candidates;
                            state.error = None;
                        }
                        Err(error) => {
                            state.candidates.clear();
                            state.error = Some(format!("Database search failed: {error}"));
                        }
                    }
                    for (id, result) in &response.widgets {
                        if let Ok(data) = result {
                            if let Some(base) = data.map_base {
                                if state.map_bases.get(id) != Some(&base) {
                                    state.map_viewports.remove(id);
                                }
                                state.map_bases.insert(id.clone(), base);
                            }
                        }
                    }
                    state.widgets = response.widgets;
                    changed = true;
                }
                state.receiver = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                state.error = Some("Dashboard loading failed.".into());
                state.receiver = None;
                changed = true;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if state.receiver.is_none() {
            if let Some(worker) = state.worker.take() {
                let _ = worker.join();
            }
            if let Ok(mut handle) = state.interrupt.lock() {
                *handle = None;
            }
        }
    }
    if state.map_drag.is_none()
        && state.receiver.is_none()
        && state.key != key
        && state.requested != key
    {
        let path = s.path.join("workspace.sqlite3");
        let workspace = s.workspace.id.clone();
        let requested = key.clone();
        let viewports = state.map_viewports.clone();
        let bases = state.map_bases.clone();
        let request_config = config.clone();
        let search = state.source_search.clone();
        let (sender, receiver) = mpsc::channel();
        let interrupt = state.interrupt.clone();
        let generation = state.generation.clone();
        let epoch = generation.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        state.cancelled = false;
        state.worker = Some(std::thread::spawn(move || {
            let (candidates, widgets) = match Store::open_read_only(path) {
                Ok(store) => {
                    if let Ok(mut handle) = interrupt.lock() {
                        *handle = Some(store.interrupt_handle());
                    }
                    load(
                        &store,
                        &workspace,
                        &search,
                        &request_config,
                        &viewports,
                        &bases,
                        &|| generation.load(Ordering::Relaxed) != epoch,
                    )
                }
                Err(e) => (
                    Err(e.to_string()),
                    BTreeMap::from([("".into(), Err(e.to_string()))]),
                ),
            };
            let _ = sender.send(Response {
                key: requested,
                candidates,
                widgets,
            });
        }));
        state.requested = key;
        if state.data_signature != data_signature {
            state.widgets.clear();
            state.candidates.clear();
        }
        state.receiver = Some(receiver);
        changed = true;
    }
    let map_ids: std::collections::BTreeSet<_> = config
        .widgets
        .iter()
        .filter(|widget| {
            state
                .widgets
                .get(&widget.id)
                .and_then(|v| v.as_ref().ok())
                .is_some_and(|v| v.map.is_some())
        })
        .map(|w| w.id.clone())
        .collect();
    let retained_ids: std::collections::BTreeSet<_> =
        config.widgets.iter().map(|w| w.id.clone()).collect();
    let dark_hint = s.prefs.dark.unwrap_or(false);
    state.map_tiles.retain(|id, _| retained_ids.contains(id));
    state.map_surfaces.retain(|id, _| retained_ids.contains(id));
    state.tile_keys.retain(|id, _| retained_ids.contains(id));
    state.map_bases.retain(|id, _| retained_ids.contains(id));
    state
        .map_viewports
        .retain(|id, _| retained_ids.contains(id));
    for widget in config.widgets.iter().filter(|w| map_ids.contains(&w.id)) {
        let Some(waker) = state.waker.clone() else {
            continue;
        };
        let viewport = crate::map_view::MapViewport {
            width: (170. * f64::from(widget.width) - 20.).max(100.),
            height: (f64::from(widget.height) - 80.).max(40.),
            ..state
                .map_drag
                .as_ref()
                .filter(|drag| drag.widget_id == widget.id)
                .map(|drag| drag.preview)
                .or_else(|| state.map_viewports.get(&widget.id).copied())
                .or_else(|| state.map_bases.get(&widget.id).copied())
                .unwrap_or_default()
        };
        let tile_key = format!("{:?}", viewport);
        if crate::map_view::surface::use_maplibre() && !state.map_tiles.contains_key(&widget.id) {
            if !state.map_surfaces.contains_key(&widget.id) {
                if let Some(surface) = crate::map_view::MapSurface::start(
                    viewport,
                    s.path.clone(),
                    dark_hint,
                    2.0,
                    waker,
                ) {
                    state.map_surfaces.insert(widget.id.clone(), surface);
                    state.tile_keys.insert(widget.id.clone(), tile_key.clone());
                    changed = true;
                }
            }
            if let Some(surface) = state.map_surfaces.get(&widget.id) {
                if state.tile_keys.get(&widget.id) != Some(&tile_key) {
                    surface.set_viewport(viewport);
                    state.tile_keys.insert(widget.id.clone(), tile_key);
                    changed = true;
                }
            }
        } else {
            let loader = state
                .map_tiles
                .entry(widget.id.clone())
                .or_insert_with(|| crate::map_view::MapTileLoader::new(waker));
            if state.tile_keys.get(&widget.id) != Some(&tile_key) {
                loader.request(viewport, s.path.clone());
                state.tile_keys.insert(widget.id.clone(), tile_key);
                changed = true;
            }
            changed |= loader.poll();
        }
    }
    if state.receiver.is_some() {
        tx.wake_after(Duration::from_millis(30));
    }
    changed
}

#[cfg(test)]
#[path = "dashboard_view_tests.rs"]
mod tests;
