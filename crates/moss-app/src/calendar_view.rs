use crate::state::{State, Tab};
use chrono::{Datelike, Duration as Days, NaiveDate};
use moss_core::{
    Database, DatabaseView, Property, PropertyKind, Store, ViewKind, calendar::CalendarProjection,
};
use rune::{GesturePhase, ScrollPos, Tx, WheelEvent};
use std::{
    cell::Cell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub fn date_property<'a>(db: &'a Database, view: &DatabaseView) -> Option<&'a Property> {
    match view
        .config
        .get("calendar_date_property")
        .and_then(|v| v.as_str())
    {
        Some(id) => db
            .properties
            .iter()
            .find(|p| p.id == id && p.kind == PropertyKind::Date),
        None => db.properties.iter().find(|p| p.kind == PropertyKind::Date),
    }
}
pub fn week(view: &DatabaseView) -> bool {
    view.config.get("calendar_mode").and_then(|v| v.as_str()) == Some("week")
}
pub fn bounds(tab: &Tab, view: &DatabaseView, monday: bool) -> Option<(NaiveDate, NaiveDate)> {
    let first = if week(view) {
        tab.calendar_month
    } else {
        tab.calendar_month.with_day(1)?
    };
    let weekday = |date: NaiveDate| {
        if monday {
            date.weekday().num_days_from_monday()
        } else {
            date.weekday().num_days_from_sunday()
        }
    };
    let start = first.checked_sub_signed(Days::days(weekday(first) as i64))?;
    let end = if week(view) {
        start.checked_add_signed(Days::days(6))?
    } else {
        let next = first.checked_add_months(chrono::Months::new(1))?;
        let last = next.checked_sub_signed(Days::days(1))?;
        last.checked_add_signed(Days::days((6 - weekday(last)) as i64))?
    };
    Some((start, end))
}
pub fn date_range(
    record: &moss_core::Record,
    property: &Property,
) -> Option<(NaiveDate, NaiveDate)> {
    let value = record.values.get(&property.id)?;
    let start = value
        .as_str()
        .or_else(|| value.get("start").and_then(|v| v.as_str()))?;
    let start = NaiveDate::parse_from_str(start.get(..10)?, "%Y-%m-%d").ok()?;
    let end = value
        .get("end")
        .and_then(|v| v.as_str())
        .and_then(|v| v.get(..10))
        .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
        .unwrap_or(start);
    (end >= start).then_some((start, end))
}
struct Request {
    key: String,
    database: String,
    view: String,
    search: String,
    undated_search: String,
    start: NaiveDate,
    end: NaiveDate,
    offset: u64,
}
struct Response {
    key: String,
    result: Result<CalendarProjection, String>,
}
#[derive(Default)]
pub struct CalendarState {
    pub projection: Option<CalendarProjection>,
    pub loading: bool,
    pub error: Option<String>,
    pub day_rects: BTreeMap<NaiveDate, [f32; 4]>,
    pub property_anchor: Option<String>,
    pub drag: Option<CalendarDrag>,
    pub schedule: Option<CalendarSchedule>,
    pub suppress_press: bool,
    pub offset: u64,
    pub scroll: Rc<Cell<ScrollPos>>,
    wheel_delta: f32,
    wheel_changed: bool,
    key: String,
    context: String,
    projection_key: String,
    database: String,
    view: String,
    pending: Option<Request>,
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: Option<mpsc::Receiver<Response>>,
    worker: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    interrupt: Arc<Mutex<Option<moss_core::InterruptHandle>>>,
}

impl State {
    pub fn calendar_wheel(&mut self, event: WheelEvent, tx: &mut Tx) -> bool {
        let calendar = &mut self.database_views.calendar;
        if event.phase == GesturePhase::Began && !event.momentum {
            calendar.wheel_delta = 0.;
            calendar.wheel_changed = false;
        }
        if calendar.drag.is_some() || self.undated_open || event.mods.cmd || event.mods.ctrl {
            return false;
        }
        if event.precise && calendar.wheel_changed {
            return true;
        }
        if !event.dy.is_finite() || event.dy == 0. || event.dx.abs() > event.dy.abs() {
            return false;
        }
        let position = calendar.scroll.get();
        let forward = event.dy < 0.;
        let at_edge = if forward {
            position.offset[1] >= position.max[1] - 1.
        } else {
            position.offset[1] <= 1.
        };
        if !at_edge {
            calendar.wheel_delta = 0.;
            return false;
        }
        if event.precise {
            if calendar.wheel_delta.signum() != event.dy.signum() {
                calendar.wheel_delta = 0.;
            }
            calendar.wheel_delta += event.dy;
            if calendar.wheel_delta.abs() < 55. {
                return true;
            }
            calendar.wheel_changed = true;
        }
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let is_week = tab
            .database
            .as_ref()
            .and_then(|db| db.views.get(tab.view))
            .is_some_and(week);
        let next = if is_week {
            tab.calendar_month
                .checked_add_signed(Days::days(if forward { 7 } else { -7 }))
        } else if forward {
            tab.calendar_month
                .checked_add_months(chrono::Months::new(1))
        } else {
            tab.calendar_month
                .checked_sub_months(chrono::Months::new(1))
        };
        if let Some(next) = next {
            tab.calendar_month = next;
            self.save_preferences();
            tx.scroll_to_instant("db-body", 0., if forward { 0. } else { f32::MAX });
            tx.instant();
        }
        true
    }
}
pub struct CalendarSchedule {
    pub record: moss_core::Record,
    pub property_id: String,
    pub context: (String, String),
}
pub struct CalendarDrag {
    pub record_id: String,
    pub property_id: String,
    pub from: NaiveDate,
    pub expected: serde_json::Value,
    pub target: Option<NaiveDate>,
    pub moved: bool,
    pub context: (String, String),
    pub edge: Option<bool>,
    pub undated: bool,
    pub record: moss_core::Record,
    pub card_date: NaiveDate,
    pub card_end: NaiveDate,
    pub origin: [f32; 2],
    pub delta: [f32; 2],
    pub width: f32,
    pub height: f32,
}

fn shift_endpoint(value: &str, days: i64) -> Option<String> {
    let date = NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()?;
    let date = date.checked_add_signed(Days::days(days))?;
    Some(format!("{date}{}", value.get(10..)?))
}
fn shift_value(value: &serde_json::Value, days: i64) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::String(text) => Some(shift_endpoint(text, days)?.into()),
        serde_json::Value::Object(object) => {
            let mut result = object.clone();
            let start = object.get("start")?.as_str()?;
            result.insert("start".into(), shift_endpoint(start, days)?.into());
            if let Some(end) = object.get("end").filter(|v| !v.is_null()) {
                result.insert("end".into(), shift_endpoint(end.as_str()?, days)?.into());
            }
            Some(result.into())
        }
        _ => None,
    }
}
fn resize_value(
    value: &serde_json::Value,
    target: NaiveDate,
    start_edge: bool,
) -> Option<serde_json::Value> {
    let original_start = value
        .as_str()
        .or_else(|| value.get("start").and_then(|v| v.as_str()))?;
    let original_end = value
        .get("end")
        .and_then(|v| v.as_str())
        .unwrap_or(original_start);
    let start = NaiveDate::parse_from_str(original_start.get(..10)?, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(original_end.get(..10)?, "%Y-%m-%d").ok()?;
    if (start_edge && target > end) || (!start_edge && target < start) {
        return None;
    }
    let mut object = value.as_object().cloned().unwrap_or_default();
    object.insert(
        "start".into(),
        if start_edge {
            shift_endpoint(original_start, (target - start).num_days())?
        } else {
            original_start.into()
        }
        .into(),
    );
    object.insert(
        "end".into(),
        if start_edge {
            original_end.into()
        } else {
            shift_endpoint(original_end, (target - end).num_days())?
        }
        .into(),
    );
    Some(object.into())
}

impl State {
    pub fn choose_calendar_undated(&mut self, record: &moss_core::Record, property_id: &str) {
        let Some(tab) = self.tab() else {
            return;
        };
        let Some(db) = &tab.database else {
            return;
        };
        let Some(view) = db.views.get(tab.view) else {
            return;
        };
        let context = (db.id.clone(), view.id.clone());
        self.database_views.calendar.schedule = Some(CalendarSchedule {
            record: record.clone(),
            property_id: property_id.into(),
            context,
        });
        self.undated_open = false;
    }
    pub fn schedule_calendar_record(&mut self, date: NaiveDate) {
        let Some(schedule) = self.database_views.calendar.schedule.take() else {
            return;
        };
        let valid = self
            .tab()
            .and_then(|tab| {
                tab.database.as_ref().and_then(|db| {
                    db.views.get(tab.view).map(|view| {
                        (
                            db.id.as_str(),
                            view.id.as_str(),
                            date_property(db, view).map(|p| p.id.as_str()),
                        )
                    })
                })
            })
            .is_some_and(|(db, view, property)| {
                db == schedule.context.0
                    && view == schedule.context.1
                    && property == Some(schedule.property_id.as_str())
            });
        if !valid {
            self.error = Some("The calendar date property changed. Choose the page again.".into());
            return;
        }
        let expected = schedule
            .record
            .values
            .get(&schedule.property_id)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        if !expected.is_null() && expected.as_str() != Some("") {
            self.error = Some("This date needs the date editor.".into());
            return;
        }
        match self.store.set_calendar_date_if_unchanged(
            &schedule.record.id,
            &schedule.property_id,
            &expected,
            serde_json::json!(date.to_string()),
        ) {
            Ok(record) => {
                self.sync_record(&record);
                self.database_views.calendar.invalidate();
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Date is not saved: {error}")),
        }
    }
    pub fn drag_calendar_record(
        &mut self,
        record: &moss_core::Record,
        property_id: &str,
        from: NaiveDate,
        through: NaiveDate,
        allow_resize: bool,
        card_rect: Option<[f32; 4]>,
        event: rune::Drag,
        tx: &mut Tx,
    ) {
        if event.phase == rune::Phase::Down {
            self.database_views.calendar.suppress_press = false;
            let Some(tab) = self.tab() else {
                return;
            };
            let Some(db) = &tab.database else {
                return;
            };
            let Some(view) = db.views.get(tab.view) else {
                return;
            };
            let context = (db.id.clone(), view.id.clone());
            let expected = record
                .values
                .get(property_id)
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let range = self
                .record_database(&record.id)
                .and_then(|db| db.properties.iter().find(|p| p.id == property_id))
                .and_then(|prop| date_range(record, prop));
            let edge = range.filter(|_| allow_resize).and_then(|(start, end)| {
                if from == start && event.start_x < 7. {
                    Some(true)
                } else if through == end && event.start_x > event.w - 7. {
                    Some(false)
                } else {
                    None
                }
            });
            let undated = expected.is_null() || expected.as_str() == Some("");
            let point = event.window_point();
            let rect = card_rect.unwrap_or([
                point[0] - event.start_x,
                point[1] - event.start_y,
                event.w,
                event.h,
            ]);
            let from = self
                .database_views
                .calendar
                .day_rects
                .iter()
                .find(|(_, rect)| {
                    point[0] >= rect[0]
                        && point[0] < rect[0] + rect[2]
                        && point[1] >= rect[1]
                        && point[1] < rect[1] + rect[3]
                })
                .map(|(date, _)| *date)
                .unwrap_or(from);
            self.database_views.calendar.drag = Some(CalendarDrag {
                record_id: record.id.clone(),
                property_id: property_id.into(),
                from,
                expected,
                target: None,
                moved: false,
                context,
                edge,
                undated,
                record: record.clone(),
                card_date: from,
                card_end: through,
                origin: [rect[0], rect[1]],
                delta: [0., 0.],
                width: rect[2],
                height: rect[3],
            });
        }
        let point = event.window_point();
        let calendar = &mut self.database_views.calendar;
        let target = calendar
            .day_rects
            .iter()
            .find(|(_, rect)| {
                point[0] >= rect[0]
                    && point[0] < rect[0] + rect[2]
                    && point[1] >= rect[1]
                    && point[1] < rect[1] + rect[3]
            })
            .map(|(date, _)| *date);
        if let Some(drag) = &mut calendar.drag {
            drag.moved |= event.dx.abs().max(event.dy.abs()) >= 5.;
            drag.target = target;
            drag.delta = [event.dx, event.dy];
        }
        if event.phase == rune::Phase::Up {
            let Some(drag) = calendar.drag.take() else {
                return;
            };
            calendar.suppress_press = drag.moved;
            let valid_context = self
                .tab()
                .and_then(|tab| {
                    tab.database.as_ref().and_then(|db| {
                        db.views.get(tab.view).map(|view| {
                            (
                                db.id.as_str(),
                                view.id.as_str(),
                                date_property(db, view).map(|p| p.id.as_str()),
                            )
                        })
                    })
                })
                .is_some_and(|(db, view, property)| {
                    db == drag.context.0
                        && view == drag.context.1
                        && property == Some(drag.property_id.as_str())
                });
            if !valid_context && drag.moved && !event.cancelled() {
                self.error =
                    Some("The calendar date property changed. Move the page again.".into());
            }
            if !event.cancelled() && drag.moved && valid_context {
                if let Some(target) = drag.target {
                    let delta = (target - drag.from).num_days();
                    if delta != 0 || drag.undated {
                        let next = if drag.undated {
                            Some(serde_json::json!(target.to_string()))
                        } else {
                            match drag.edge {
                                Some(start) => resize_value(&drag.expected, target, start),
                                None => shift_value(&drag.expected, delta),
                            }
                        };
                        if let Some(next) = next {
                            match self.store.set_calendar_date_if_unchanged(
                                &drag.record_id,
                                &drag.property_id,
                                &drag.expected,
                                next,
                            ) {
                                Ok(record) => {
                                    self.sync_record(&record);
                                    if let Some(projection) =
                                        &mut self.database_views.calendar.projection
                                    {
                                        if let Some(current) = projection
                                            .records
                                            .iter_mut()
                                            .find(|r| r.id == record.id)
                                        {
                                            *current = record;
                                        }
                                    }
                                    self.database_views.calendar.invalidate();
                                    if drag.undated {
                                        self.undated_open = false;
                                    }
                                    self.error = None;
                                }
                                Err(error) => {
                                    self.error = Some(format!("Date is not saved: {error}"))
                                }
                            }
                        } else {
                            self.error =
                                Some("The date cannot be moved or resized to that day.".into());
                        }
                    }
                }
            }
        }
        tx.instant();
    }
}
impl CalendarState {
    pub fn projection_for(&self, db: &str, view: &str) -> Option<&CalendarProjection> {
        (self.database == db && self.view == view && self.projection_key == self.key)
            .then_some(self.projection.as_ref())
            .flatten()
    }
    pub fn invalidate(&mut self) {
        self.key.clear();
    }
    fn start(&mut self, path: std::path::PathBuf) {
        if self.sender.is_some() {
            return;
        }
        let (send, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, recv) = mpsc::channel();
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
                    store
                        .as_ref()
                        .unwrap()
                        .query_calendar(
                            &request.database,
                            &request.view,
                            &request.search,
                            &request.undated_search,
                            request.start,
                            request.end,
                            request.offset,
                        )
                        .map_err(|e| e.to_string())
                })();
                if responses
                    .send(Response {
                        key: request.key,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.sender = Some(send);
        self.receiver = Some(recv);
    }
}
impl Drop for CalendarState {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.sender.take();
        if let Ok(interrupt) = self.interrupt.lock() {
            if let Some(i) = interrupt.as_ref() {
                i.interrupt();
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let context = s.tab().and_then(|tab| {
        let db = tab.database.as_ref()?;
        let view = db.views.get(tab.view)?;
        if view.kind != ViewKind::Calendar || date_property(db, view).is_none() {
            return None;
        }
        let (start, end) = bounds(tab, view, s.prefs.start_week_on_monday)?;
        Some((
            db.id.clone(),
            view.id.clone(),
            tab.title_search.clone(),
            start,
            end,
            view.config.clone(),
            view.query.clone(),
        ))
    });
    let undated_search = if s.undated_open {
        s.undated_search.text.clone()
    } else {
        String::new()
    };
    let path = s.path.clone();
    let generation = s.store.local_change_count();
    let state = &mut s.database_views.calendar;
    let Some((database, view, search, start, end, config, query)) = context else {
        state.key.clear();
        state.projection = None;
        state.pending = None;
        state.loading = false;
        state.drag = None;
        state.schedule = None;
        return false;
    };
    if state
        .drag
        .as_ref()
        .is_some_and(|drag| drag.context.0 == database && drag.context.1 == view)
    {
        if state.loading {
            tx.wake_after(Duration::from_millis(16));
        }
        return false;
    }
    state.drag = None;
    let context = format!(
        "{database}/{view}/{search}/{start}/{end}/{config}/{:?}",
        query
    );
    if state.context != context {
        state.offset = 0;
        state.context = context.clone();
    }
    let key = format!("{context}/{undated_search}/{generation}/{}", state.offset);
    if state.key != key {
        if state.loading {
            if let Ok(interrupt) = state.interrupt.lock() {
                if let Some(interrupt) = interrupt.as_ref() {
                    interrupt.interrupt();
                }
            }
        }
        if state.database != database || state.view != view {
            state.projection = None;
            state.day_rects.clear();
        }
        state.database = database.clone();
        state.view = view.clone();
        state.key = key.clone();
        state.error = None;
        state.loading = true;
        state
            .day_rects
            .retain(|date, _| *date >= start && *date <= end);
        state.start(path);
        state.pending = Some(Request {
            key,
            database,
            view,
            search,
            undated_search,
            start,
            end,
            offset: state.offset,
        });
    }
    let mut changed = false;
    if let Some(receiver) = &state.receiver {
        while let Ok(response) = receiver.try_recv() {
            if response.key != state.key {
                continue;
            }
            state.loading = false;
            match response.result {
                Ok(projection) => {
                    if state.offset != projection.offset {
                        state.offset = projection.offset;
                        state.key.clear();
                    }
                    state.projection = Some(projection);
                    state.projection_key = response.key;
                }
                Err(error) => {
                    state.error = Some(error);
                    state.projection = None;
                }
            }
            changed = true;
        }
    }
    if let Some(request) = state.pending.take() {
        if let Some(sender) = &state.sender {
            match sender.try_send(request) {
                Ok(()) => (),
                Err(mpsc::TrySendError::Full(request)) => state.pending = Some(request),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    state.loading = false;
                    state.error = Some("Calendar reader stopped.".into());
                }
            }
        }
    }
    if state.loading || state.pending.is_some() {
        tx.wake_after(Duration::from_millis(16));
    }
    changed
}
