use crate::state::State;
use moss_core::{
    Store, ViewKind,
    chart::{
        Aggregate, ChartConfig, ChartPage, ChartProjection, ChartRecord, ChartType, DateBucket,
    },
};
use rune::Tx;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

struct Request {
    key: String,
    database: String,
    view: String,
    config: ChartConfig,
    search: String,
    page: Option<PageRequest>,
}
struct PageRequest {
    epoch: u64,
    category: Option<String>,
    series: Option<String>,
    after: Option<String>,
    history: Vec<Option<String>>,
}
enum ResultValue {
    Projection(ChartProjection),
    Page {
        epoch: u64,
        page: ChartPage,
        cursor: Option<String>,
        history: Vec<Option<String>>,
    },
}
struct Response {
    key: String,
    result: Result<ResultValue, String>,
    page_epoch: Option<u64>,
}
#[derive(Default)]
pub struct ChartState {
    pub settings_open: bool,
    pub projection: Option<ChartProjection>,
    pub error: Option<String>,
    pub save_error: Option<String>,
    pub loading: bool,
    pub drilldown: Option<usize>,
    pub hovered: Option<usize>,
    pub drill_records: Vec<ChartRecord>,
    pub drill_next: Option<String>,
    pub drill_loading: bool,
    pub drill_error: Option<String>,
    drill_cursor: Option<String>,
    drill_history: Vec<Option<String>>,
    drill_epoch: u64,
    key: String,
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: Option<mpsc::Receiver<Response>>,
    pending: Option<Request>,
    worker: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    interrupt: Arc<Mutex<Option<moss_core::InterruptHandle>>>,
}
impl ChartState {
    pub fn has_previous(&self) -> bool {
        !self.drill_history.is_empty()
    }
    pub fn has_first(&self) -> bool {
        self.drill_cursor.is_some()
    }
    fn start(&mut self, path: PathBuf) {
        if self.sender.is_some() {
            return;
        }
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, receiver) = mpsc::channel();
        let stop = self.stop.clone();
        let interrupt = self.interrupt.clone();
        self.worker = Some(std::thread::spawn(move || {
            let mut store: Option<Store> = None;
            while let Ok(request) = requests.recv() {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let result = (|| {
                    if store.is_none() {
                        let opened = Store::open_read_only(path.join("workspace.sqlite3"))
                            .map_err(|e| e.to_string())?;
                        *interrupt.lock().unwrap() = Some(opened.interrupt_handle());
                        store = Some(opened);
                    }
                    let store = store.as_ref().unwrap();
                    match &request.page {
                        Some(page) => store
                            .chart_records(
                                &request.database,
                                &request.view,
                                &request.config,
                                None,
                                &request.search,
                                page.category.as_deref(),
                                page.series.as_deref(),
                                page.after.as_deref(),
                            )
                            .map(|value| ResultValue::Page {
                                epoch: page.epoch,
                                page: value,
                                cursor: page.after.clone(),
                                history: page.history.clone(),
                            }),
                        None => store
                            .chart_projection(
                                &request.database,
                                &request.view,
                                &request.config,
                                None,
                                &request.search,
                            )
                            .map(ResultValue::Projection),
                    }
                    .map_err(|e| e.to_string())
                })();
                if responses
                    .send(Response {
                        key: request.key,
                        result,
                        page_epoch: request.page.as_ref().map(|page| page.epoch),
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.sender = Some(sender);
        self.receiver = Some(receiver);
    }
    fn dispatch(&mut self) {
        let (Some(sender), Some(request)) = (&self.sender, self.pending.take()) else {
            return;
        };
        match sender.try_send(request) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(request)) => self.pending = Some(request),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.loading = false;
                self.error = Some("Chart worker stopped. Reopen the view.".into());
                self.sender = None;
            }
        }
    }
}
impl Drop for ChartState {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.sender.take();
        if let Some(handle) = self.interrupt.lock().unwrap().as_ref() {
            handle.interrupt();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn config(value: &Value) -> Result<ChartConfig, String> {
    match value.get("chart") {
        None | Some(Value::Null) => Ok(ChartConfig::default()),
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|_| "Chart settings are invalid. Reset them to continue.".into()),
    }
}
pub fn poll(s: &mut State, _time: f64, tx: &mut Tx) -> bool {
    let request = s.tab().and_then(|tab| {
        let db = tab.database.as_ref()?;
        let view = db.views.get(tab.view)?;
        if view.kind != ViewKind::Chart {
            return None;
        }
        let config = config(&view.config);
        Some((
            db.id.clone(),
            view.id.clone(),
            view.query.filter.clone(),
            tab.title_search.clone(),
            config,
        ))
    });
    let count = s.store.local_change_count();
    let state = &mut s.database_views.chart;
    let mut changed = false;
    if let Some((database, view, filter, search, parsed)) = request {
        match parsed {
            Ok(config) => {
                let key = json!([database, view, filter, search, config, count]).to_string();
                if key != state.key {
                    state.key = key.clone();
                    state.projection = None;
                    state.error = None;
                    state.drilldown = None;
                    state.hovered = None;
                    state.drill_epoch = state.drill_epoch.wrapping_add(1);
                    state.drill_loading = false;
                    state.loading = true;
                    state.pending = Some(Request {
                        key,
                        database,
                        view,
                        config,
                        search,
                        page: None,
                    });
                    state.start(s.path.clone());
                    state.dispatch();
                    changed = true;
                }
            }
            Err(error) => {
                state.key = "invalid chart settings".into();
                state.pending = None;
                if state.error.as_deref() != Some(&error) {
                    state.error = Some(error);
                    state.projection = None;
                    state.loading = false;
                    changed = true;
                }
            }
        }
    } else {
        state.pending = None;
        state.key.clear();
        state.projection = None;
        state.loading = false;
    }
    if let Some(receiver) = &state.receiver {
        loop {
            match receiver.try_recv() {
                Ok(response) if response.key == state.key => {
                    match response.result {
                        Ok(ResultValue::Projection(data)) => {
                            state.loading = false;
                            state.projection = Some(data);
                            state.error = None;
                        }
                        Ok(ResultValue::Page {
                            epoch,
                            page,
                            cursor,
                            history,
                        }) if epoch == state.drill_epoch => {
                            state.drill_loading = false;
                            state.drill_records = page.records;
                            state.drill_next = page.next;
                            state.drill_cursor = cursor;
                            state.drill_history = history;
                            state.drill_error = None;
                        }
                        Ok(ResultValue::Page { .. }) => continue,
                        Err(error) if response.page_epoch == Some(state.drill_epoch) => {
                            state.drill_loading = false;
                            state.drill_error = Some(error);
                        }
                        Err(error) if response.page_epoch.is_none() => {
                            state.loading = false;
                            state.projection = None;
                            state.error = Some(error);
                        }
                        Err(_) => continue,
                    }
                    changed = true;
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    state.loading = false;
                    state.error = Some("Chart worker stopped. Reopen the view.".into());
                    changed = true;
                    break;
                }
            }
        }
    }
    state.dispatch();
    if state.loading || state.drill_loading || state.pending.is_some() {
        tx.wake_after(Duration::from_millis(8));
    }
    changed
}
impl State {
    pub fn open_chart_category(&mut self, index: usize) {
        let state = &mut self.database_views.chart;
        let Some(point) = state
            .projection
            .as_ref()
            .and_then(|projection| projection.points.get(index))
        else {
            return;
        };
        state.drilldown = Some(index);
        state.drill_records = point.records.clone();
        state.drill_next = (point.count > point.records.len() as u64)
            .then(|| point.records.last().map(|record| record.id.clone()))
            .flatten();
        state.drill_cursor = None;
        state.drill_history.clear();
        state.drill_loading = false;
        state.drill_error = None;
        state.drill_epoch = state.drill_epoch.wrapping_add(1);
    }
    fn chart_request_page(&mut self, after: Option<String>, history: Vec<Option<String>>) {
        let Some(tab) = self.tab() else { return };
        let Some(db) = &tab.database else { return };
        let Some(view) = db.views.get(tab.view) else {
            return;
        };
        let Ok(config) = config(&view.config) else {
            return;
        };
        let (database, view_id, search) =
            (db.id.clone(), view.id.clone(), tab.title_search.clone());
        let state = &mut self.database_views.chart;
        if state.drill_loading {
            return;
        }
        let Some(point) = state
            .drilldown
            .and_then(|index| state.projection.as_ref()?.points.get(index))
        else {
            return;
        };
        state.drill_epoch = state.drill_epoch.wrapping_add(1);
        state.drill_loading = true;
        state.drill_error = None;
        state.pending = Some(Request {
            key: state.key.clone(),
            database,
            view: view_id,
            config,
            search,
            page: Some(PageRequest {
                epoch: state.drill_epoch,
                category: point.category_key.clone(),
                series: point.series_key.clone(),
                after,
                history,
            }),
        });
        state.dispatch();
    }
    pub fn chart_next_page(&mut self) {
        let state = &self.database_views.chart;
        let Some(next) = state.drill_next.clone() else {
            return;
        };
        let mut history = state.drill_history.clone();
        history.push(state.drill_cursor.clone());
        if history.len() > 256 {
            history.remove(0);
        }
        self.chart_request_page(Some(next), history);
    }
    pub fn chart_previous_page(&mut self) {
        let mut history = self.database_views.chart.drill_history.clone();
        let Some(previous) = history.pop() else {
            return;
        };
        self.chart_request_page(previous, history);
    }
    pub fn chart_first_page(&mut self) {
        self.chart_request_page(None, Vec::new());
    }
    fn update_chart_config(&mut self, change: impl FnOnce(&mut ChartConfig)) {
        let Some((database, view)) = self.tab().and_then(|t| {
            Some((
                t.database.as_ref()?.id.clone(),
                t.database.as_ref()?.views.get(t.view)?.clone(),
            ))
        }) else {
            return;
        };
        let expected = view.config.get("chart").cloned();
        let mut next = match config(&view.config) {
            Ok(c) => c,
            Err(error) => {
                self.database_views.chart.save_error = Some(error);
                return;
            }
        };
        change(&mut next);
        let saved = self
            .store
            .edit_view_config(&database, &view.id, ViewKind::Chart, |value| {
                if value.get("chart").cloned() != expected {
                    return Err("Chart settings changed in another window. Reopen the view.".into());
                }
                if value.is_null() {
                    *value = json!({});
                }
                value
                    .as_object_mut()
                    .ok_or("Invalid view settings")?
                    .insert("chart".into(), json!(next));
                Ok(())
            });
        match saved {
            Ok(db) => {
                crate::ui::property_visibility::sync_database(self, &db);
                self.database_views.chart.key.clear();
                self.database_views.chart.save_error = None;
            }
            Err(error) => {
                self.database_views.chart.save_error =
                    Some(format!("Chart settings are not saved: {error}"))
            }
        }
    }
    pub fn chart_kind(&mut self, kind: ChartType) {
        self.update_chart_config(|c| c.kind = kind);
    }
    pub fn chart_axis(&mut self, id: String) {
        self.update_chart_config(|c| c.category_property_id = Some(id));
    }
    pub fn chart_value(&mut self, id: String) {
        self.update_chart_config(|c| c.value_property_id = Some(id));
    }
    pub fn chart_series(&mut self, id: Option<String>) {
        self.update_chart_config(|c| c.series_property_id = id);
    }
    pub fn chart_aggregate(&mut self, operation: Aggregate) {
        self.update_chart_config(|c| c.aggregation = operation);
    }
    pub fn chart_bucket(&mut self, bucket: DateBucket) {
        self.update_chart_config(|c| c.date_bucket = bucket);
    }
    pub fn reset_chart_config(&mut self) {
        let Some((id, view)) = self.tab().and_then(|t| {
            Some((
                t.database.as_ref()?.id.clone(),
                t.database.as_ref()?.views.get(t.view)?.clone(),
            ))
        }) else {
            return;
        };
        let expected = view.config.get("chart").cloned();
        let result = self
            .store
            .edit_view_config(&id, &view.id, ViewKind::Chart, |value| {
                if value.get("chart").cloned() != expected {
                    return Err("Chart settings changed in another window.".into());
                }
                value
                    .as_object_mut()
                    .ok_or("Invalid view settings")?
                    .remove("chart");
                Ok(())
            });
        match result {
            Ok(db) => {
                crate::ui::property_visibility::sync_database(self, &db);
                self.database_views.chart.key.clear();
                self.database_views.chart.error = None;
                self.database_views.chart.save_error = None;
            }
            Err(error) => self.database_views.chart.save_error = Some(error.to_string()),
        }
    }
}

#[cfg(test)]
#[path = "chart_view_tests.rs"]
mod tests;
