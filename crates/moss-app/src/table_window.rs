use crate::{
    records::split_record_appearance,
    state::{State, Tab},
};
use moss_core::{Database, Record, RecordWindow, RecordWindowState, Store, ViewKind};
use rune::{ScrollPos, Tx};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

const ROW: f32 = 38.;
const WINDOW: usize = 256;
const OVERSCAN: usize = 12;
static GENERATION: AtomicU64 = AtomicU64::new(1);

pub struct TableWindow {
    pub total: Option<u64>,
    pending_created: u64,
    pub start: u64,
    pub window_len: usize,
    pub generation: u64,
    pub visible: Range<usize>,
    pub scroll: Rc<Cell<ScrollPos>>,
    pub loading: bool,
    pub error: Option<String>,
    revision: Option<u64>,
    request_id: u64,
    desired_start: Option<u64>,
    invalidate_pending: bool,
    force_reload: bool,
    retry_after: Option<std::time::Instant>,
    deferred: Option<Response>,
    bottom_anchor: Option<f32>,
    pinned: BTreeMap<String, Record>,
}
impl TableWindow {
    pub fn new(initial_len: usize) -> Self {
        Self {
            total: None,
            pending_created: 0,
            start: 0,
            window_len: initial_len,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            visible: 0..initial_len.min(36),
            scroll: Rc::new(Cell::new(ScrollPos::default())),
            loading: false,
            error: None,
            revision: None,
            request_id: 0,
            desired_start: None,
            invalidate_pending: false,
            force_reload: false,
            retry_after: None,
            deferred: None,
            bottom_anchor: None,
            pinned: BTreeMap::new(),
        }
    }

    pub fn displayed_total(&self) -> Option<u64> {
        self.total
            .map(|total| total.saturating_add(self.pending_created))
    }
}

pub fn supported(tab: &Tab) -> bool {
    let Some(view) = tab.database.as_ref().and_then(|db| db.views.get(tab.view)) else {
        return false;
    };
    view.kind == ViewKind::Table
        && view.query.filter.is_none()
        && view.query.sorts.is_empty()
        && view.query.group_by.is_none()
        && tab.title_search.is_empty()
        && tab.offset == 0
        && !view
            .config
            .get("wrap_cells")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
}

#[derive(Clone)]
struct Request {
    page_id: String,
    database: Database,
    workspace: String,
    generation: u64,
    request_id: u64,
    start: u64,
    revision: Option<u64>,
}
struct Loaded {
    window: RecordWindow,
    icons: BTreeMap<String, String>,
    content: BTreeMap<String, bool>,
    titles: BTreeMap<String, String>,
}
struct Response {
    page_id: String,
    generation: u64,
    request_id: u64,
    result: Result<Option<Loaded>, String>,
}

#[derive(Default)]
pub struct TableWindowJobs {
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: Option<mpsc::Receiver<Response>>,
    pending: BTreeMap<String, Request>,
    in_flight: usize,
    next_request: u64,
    worker: Option<std::thread::JoinHandle<()>>,
    stopped: Arc<AtomicBool>,
    has_waker: bool,
    interrupts: Arc<Mutex<Vec<moss_core::InterruptHandle>>>,
}

impl TableWindowJobs {
    fn start(&mut self, path: PathBuf, wake: Option<rune::AppHandle<State>>) {
        if self.sender.is_some() {
            return;
        }
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, receiver) = mpsc::channel();
        self.has_waker = wake.is_some();
        let stopped = self.stopped.clone();
        let interrupts = self.interrupts.clone();
        self.worker = Some(std::thread::spawn(move || {
            let mut reader: Option<Store> = None;
            let mut writer: Option<Store> = None;
            while let Ok(request) = requests.recv() {
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let result = (|| -> Result<Option<Loaded>, String> {
                    if reader.is_none() {
                        let store = Store::open_read_only(path.join("workspace.sqlite3"))
                            .map_err(|e| e.to_string())?;
                        interrupts.lock().unwrap().push(store.interrupt_handle());
                        reader = Some(store);
                    }
                    let store = reader.as_ref().unwrap();
                    let mut window = store
                        .record_window(
                            &request.database.id,
                            request.start,
                            WINDOW as u32,
                            request.revision,
                        )
                        .map_err(|e| e.to_string())?;
                    if matches!(window, RecordWindowState::Stale { .. }) {
                        window = store
                            .record_window(&request.database.id, request.start, WINDOW as u32, None)
                            .map_err(|e| e.to_string())?;
                    }
                    if matches!(window, RecordWindowState::Pending) {
                        if writer.is_none() {
                            let store = Store::open(path.join("workspace.sqlite3"))
                                .map_err(|e| e.to_string())?;
                            interrupts.lock().unwrap().push(store.interrupt_handle());
                            writer = Some(store);
                        }
                        if stopped.load(Ordering::Acquire) {
                            return Ok(None);
                        }
                        if let Err(error) = writer
                            .as_mut()
                            .unwrap()
                            .build_record_rank(&request.database.id)
                        {
                            let message = error.to_string();
                            if message.contains("locked") || message.contains("busy") {
                                return Ok(None);
                            }
                            return Err(message);
                        }
                        if stopped.load(Ordering::Acquire) {
                            return Ok(None);
                        }
                        window = store
                            .record_window(&request.database.id, request.start, WINDOW as u32, None)
                            .map_err(|e| e.to_string())?;
                    }
                    let RecordWindowState::Ready(window) = window else {
                        return Ok(None);
                    };
                    let ids: Vec<_> = window.records.iter().map(|r| r.id.clone()).collect();
                    let (icons, content) = split_record_appearance(
                        store
                            .record_page_appearance(&request.workspace, &ids)
                            .map_err(|e| e.to_string())?,
                    );
                    let titles =
                        crate::relation_titles::load(store, &request.database, &window.records)
                            .map_err(|e| e.to_string())?;
                    Ok(Some(Loaded {
                        window,
                        icons,
                        content,
                        titles,
                    }))
                })();
                if responses
                    .send(Response {
                        page_id: request.page_id,
                        generation: request.generation,
                        request_id: request.request_id,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                if let Some(handle) = &wake {
                    handle.wake();
                }
            }
        }));
        self.sender = Some(sender);
        self.receiver = Some(receiver);
    }
    fn dispatch(&mut self) -> bool {
        if self.in_flight != 0 {
            return true;
        }
        let Some((key, request)) = self.pending.pop_first() else {
            return true;
        };
        let Some(sender) = &self.sender else {
            self.pending.insert(key, request);
            return false;
        };
        match sender.try_send(request) {
            Ok(()) => {
                self.in_flight = 1;
                true
            }
            Err(mpsc::TrySendError::Full(request)) => {
                self.pending.insert(key, request);
                true
            }
            Err(mpsc::TrySendError::Disconnected(_)) => false,
        }
    }
    fn reset_disconnected(&mut self) {
        self.sender.take();
        self.receiver.take();
        self.pending.clear();
        self.in_flight = 0;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Ok(mut handles) = self.interrupts.lock() {
            handles.clear();
        }
        self.has_waker = false;
    }
}
impl Drop for TableWindowJobs {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.sender.take();
        if let Ok(handles) = self.interrupts.lock() {
            for handle in handles.iter() {
                handle.interrupt();
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl State {
    /// Queue one bounded window; a newer visible range supersedes an older request.
    pub fn retry_table_window(&mut self) -> bool {
        let Some(window) = self
            .tabs
            .get_mut(self.active)
            .and_then(|tab| tab.table_window.as_mut())
        else {
            return false;
        };
        window.error = None;
        window.retry_after = None;
        window.desired_start = None;
        let range = window.visible.clone();
        self.request_table_window(range)
    }
    pub fn request_table_window(&mut self, range: Range<usize>) -> bool {
        let Some(tab) = self.tabs.get(self.active) else {
            return false;
        };
        let Some(window) = tab.table_window.as_ref() else {
            return false;
        };
        let total = window.total.map(|v| v as usize);
        let end = total.map_or(range.end, |n| range.end.min(n));
        let start = total.map_or(range.start, |n| range.start.min(n));
        let cached_start = window.start as usize;
        let cached_end = cached_start.saturating_add(window.window_len);
        let covered = window.total.is_some()
            && start >= cached_start
            && end <= cached_end
            && (start < 32 || start >= cached_start + 32)
            && (total.is_some_and(|n| cached_end >= n) || end.saturating_add(32) <= cached_end);
        if covered && !window.invalidate_pending && !window.force_reload {
            return false;
        }
        if window.error.is_some() {
            return false;
        }
        let desired = start.saturating_sub(64) / 64 * 64;
        if window.loading && window.desired_start == Some(desired as u64) {
            return false;
        }
        if window
            .retry_after
            .is_some_and(|when| std::time::Instant::now() < when)
        {
            return false;
        }
        let Some(db) = tab.database.clone() else {
            return false;
        };
        let page_id = tab.page.id.clone();
        let generation = window.generation;
        let revision = window.revision;
        self.table_window_jobs.next_request = self.table_window_jobs.next_request.wrapping_add(1);
        let request_id = self.table_window_jobs.next_request;
        let request = Request {
            page_id: page_id.clone(),
            database: db,
            workspace: self.workspace.id.clone(),
            generation,
            request_id,
            start: desired as u64,
            revision,
        };
        let window = self.tabs[self.active].table_window.as_mut().unwrap();
        window.request_id = request_id;
        window.desired_start = Some(desired as u64);
        window.loading = true;
        window.error = None;
        window.retry_after = None;
        let wake = self
            .handles
            .lock()
            .ok()
            .and_then(|handles| handles.get(&self.workspace.id).cloned());
        self.table_window_jobs.start(self.path.clone(), wake);
        prune_pending(self);
        self.table_window_jobs
            .pending
            .insert(format!("{page_id}:{generation}"), request);
        if !self.table_window_jobs.dispatch() {
            fail_worker(self);
        }
        true
    }
}

/// Mark a window stale after a local write without replacing a live editor.
pub fn mark_changed(s: &mut State, index: usize) {
    if let Some(window) = s
        .tabs
        .get_mut(index)
        .and_then(|tab| tab.table_window.as_mut())
    {
        window.invalidate_pending = true;
    }
}

/// Account for a successful local insert until a fresh rank snapshot replaces the count.
pub fn note_created(s: &mut State, database_id: &str) {
    for tab in &mut s.tabs {
        if tab.database.as_ref().is_some_and(|db| db.id == database_id) {
            if let Some(window) = &mut tab.table_window {
                window.pending_created = window.pending_created.saturating_add(1);
                // Reject any result requested before this insert, even before the next poll.
                window.invalidate_pending = true;
            }
        }
    }
}

fn is_editing(s: &State, index: usize) -> bool {
    index == s.active
        && (s.editing_cell.is_some()
            || s.property_picker.is_some()
            || s.advanced_property.relation_picker.is_some())
}

fn install(s: &mut State, index: usize, response: Response) -> bool {
    let Some(window) = s.tabs[index].table_window.as_ref() else {
        return false;
    };
    if window.generation != response.generation
        || window.request_id != response.request_id
        || window.invalidate_pending
    {
        return false;
    }
    if is_editing(s, index) {
        s.tabs[index].table_window.as_mut().unwrap().deferred = Some(response);
        return false;
    }
    let tab = &mut s.tabs[index];
    let window = tab.table_window.as_mut().unwrap();
    match response.result {
        Ok(Some(loaded)) => {
            let scroll = window.scroll.get();
            if index == s.active
                && scroll.max[1] > 0.
                && (scroll.max[1] - scroll.offset[1]).abs() <= 4.
            {
                window.bottom_anchor = Some(scroll.offset[1]);
            }
            let pins: BTreeSet<_> = tab.created_record_ids.iter().cloned().collect();
            let loaded_ids: BTreeSet<_> =
                loaded.window.records.iter().map(|r| r.id.clone()).collect();
            window.pinned.retain(|id, _| pins.contains(id));
            for record in tab.records.iter().chain(loaded.window.records.iter()) {
                if pins.contains(&record.id) {
                    window.pinned.insert(record.id.clone(), record.clone());
                }
            }
            let pinned: Vec<_> = tab
                .created_record_ids
                .iter()
                .filter(|id| !loaded_ids.contains(*id))
                .filter_map(|id| window.pinned.get(id).cloned())
                .collect();
            let mut icons = loaded.icons;
            let mut content = loaded.content;
            let mut titles = loaded.titles;
            for id in window.pinned.keys() {
                if let Some(icon) = tab.record_icons.get(id) {
                    icons.entry(id.clone()).or_insert_with(|| icon.clone());
                }
                if let Some(has_content) = tab.record_has_content.get(id) {
                    content.entry(id.clone()).or_insert(*has_content);
                }
            }
            let mut needed = BTreeSet::new();
            let mut scanned = 0;
            if let Some(db) = &tab.database {
                'properties: for property in db
                    .properties
                    .iter()
                    .filter(|p| p.kind == moss_core::PropertyKind::Relation)
                {
                    for record in window.pinned.values() {
                        if let Some(ids) = record
                            .values
                            .get(&property.id)
                            .and_then(serde_json::Value::as_array)
                        {
                            for id in ids.iter().filter_map(serde_json::Value::as_str) {
                                scanned += 1;
                                if scanned > 5000 || needed.len() >= 1000 {
                                    break 'properties;
                                }
                                needed.insert(id.to_owned());
                            }
                        }
                    }
                }
            }
            for id in needed {
                if let Some(title) = tab.relation_titles.get(&id) {
                    titles.entry(id).or_insert_with(|| title.clone());
                }
            }
            window.total = Some(loaded.window.total);
            window.pending_created = 0;
            window.revision = Some(loaded.window.revision);
            window.start = loaded.window.start;
            window.window_len = loaded.window.records.len();
            window.loading = false;
            window.error = None;
            window.desired_start = None;
            window.invalidate_pending = false;
            window.force_reload = false;
            tab.records = loaded.window.records;
            tab.records.extend(pinned);
            tab.record_icons = icons;
            tab.record_has_content = content;
            tab.relation_titles = titles;
            true
        }
        Ok(None) => {
            window.loading = true;
            window.desired_start = None;
            window.retry_after = Some(std::time::Instant::now() + Duration::from_millis(250));
            true
        }
        Err(error) => {
            window.loading = false;
            window.desired_start = None;
            window.error = Some(format!("Table rows are not loaded: {error}"));
            true
        }
    }
}

fn prune_pending(s: &mut State) {
    let current: BTreeSet<_> = s
        .tabs
        .iter()
        .filter_map(|tab| {
            tab.table_window
                .as_ref()
                .map(|window| (tab.page.id.clone(), window.generation))
        })
        .collect();
    s.table_window_jobs
        .pending
        .retain(|_, request| current.contains(&(request.page_id.clone(), request.generation)));
}

fn fail_worker(s: &mut State) {
    s.table_window_jobs.reset_disconnected();
    for tab in &mut s.tabs {
        if let Some(window) = &mut tab.table_window {
            if window.loading {
                window.loading = false;
                window.desired_start = None;
                window.error = Some("Table loader stopped. Retry to load rows.".into());
            }
        }
    }
}

pub fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let mut changed = false;
    // The new row window can change the scroll extent after layout. Restore
    // the bottom only if the user has not moved away in the meantime.
    if let Some(window) = s
        .tabs
        .get_mut(s.active)
        .and_then(|tab| tab.table_window.as_mut())
    {
        if let Some(offset) = window.bottom_anchor.take() {
            let scroll = window.scroll.get();
            if (scroll.offset[1] - offset).abs() <= 4. {
                tx.scroll_to_y("db-body", f32::MAX);
                tx.wake_after(Duration::from_millis(4));
            }
        }
    }
    let mut responses = Vec::new();
    let mut disconnected = false;
    if let Some(receiver) = &s.table_window_jobs.receiver {
        loop {
            match receiver.try_recv() {
                Ok(response) => {
                    s.table_window_jobs.in_flight = s.table_window_jobs.in_flight.saturating_sub(1);
                    responses.push(response);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
    }
    for response in responses {
        if let Some(index) = s.tabs.iter().position(|tab| {
            tab.page.id == response.page_id
                && tab
                    .table_window
                    .as_ref()
                    .is_some_and(|window| window.generation == response.generation)
        }) {
            changed |= install(s, index, response);
            if s.tabs[index]
                .table_window
                .as_ref()
                .is_some_and(|window| window.bottom_anchor.is_some())
            {
                tx.wake_after(Duration::from_millis(4));
            }
        }
    }
    prune_pending(s);
    if disconnected || !s.table_window_jobs.dispatch() {
        fail_worker(s);
        changed = true;
    }
    if let Some(index) = s
        .tabs
        .get(s.active)
        .and_then(|tab| tab.table_window.as_ref())
        .map(|_| s.active)
    {
        if !is_editing(s, index) {
            if let Some(response) = s.tabs[index].table_window.as_mut().unwrap().deferred.take() {
                changed |= install(s, index, response);
                if s.tabs[index]
                    .table_window
                    .as_ref()
                    .is_some_and(|window| window.bottom_anchor.is_some())
                {
                    tx.wake_after(Duration::from_millis(4));
                }
            }
            let window = s.tabs[index].table_window.as_mut().unwrap();
            if window.invalidate_pending {
                window.generation = GENERATION.fetch_add(1, Ordering::Relaxed);
                window.revision = None;
                window.desired_start = None;
                window.loading = false;
                window.deferred = None;
                window.invalidate_pending = false;
                window.force_reload = true;
            }
        }
        let editing = is_editing(s, index);
        let edited_id = s
            .editing_cell
            .as_ref()
            .map(|cell| cell.record_id.as_str())
            .or_else(|| {
                s.property_picker
                    .as_ref()
                    .map(|picker| picker.record_id.as_str())
            })
            .or_else(|| {
                s.advanced_property
                    .relation_picker
                    .as_ref()
                    .map(|picker| picker.record_id.as_str())
            });
        let window = s.tabs[index].table_window.as_ref().unwrap();
        let at = window.scroll.get();
        if editing && at.viewport[1] > 0. {
            let first = ((at.offset[1] - ROW).max(0.) / ROW).floor() as usize;
            let last = ((at.offset[1] + at.viewport[1] - ROW).max(0.) / ROW).ceil() as usize;
            let cache_start = window.start as usize;
            let cache_end = cache_start.saturating_add(window.window_len);
            let anchor = edited_id
                .and_then(|id| {
                    s.tabs[index]
                        .records
                        .iter()
                        .position(|record| record.id == id)
                })
                .map(|local| {
                    if local < window.window_len {
                        cache_start + local
                    } else {
                        window.total.unwrap_or(window.window_len as u64) as usize + local
                            - window.window_len
                    }
                });
            let anchor_visible = anchor.is_some_and(|row| row >= first && row < last);
            if (first < cache_start || last > cache_end) && !anchor_visible {
                let row = anchor.unwrap_or(cache_start);
                tx.scroll_to_y(
                    "db-body",
                    (ROW + row as f32 * ROW - at.viewport[1] / 2.).max(0.),
                );
                tx.wake_after(Duration::from_millis(4));
                return changed;
            }
        }
        let window = s.tabs[index].table_window.as_mut().unwrap();
        let first = ((at.offset[1] - ROW).max(0.) / ROW).floor() as usize;
        let last = ((at.offset[1] + at.viewport[1] - ROW).max(0.) / ROW).ceil() as usize;
        let limit = window.total.map_or(window.window_len, |n| n as usize);
        let visible =
            first.saturating_sub(OVERSCAN).min(limit)..last.saturating_add(OVERSCAN).min(limit);
        if visible != window.visible {
            window.visible = visible.clone();
            changed = true;
        }
        if !is_editing(s, index) {
            changed |= s.request_table_window(visible);
        }
        if let Some(window) = &s.tabs[index].table_window {
            if let Some(retry) = window.retry_after {
                tx.wake_after(retry.saturating_duration_since(std::time::Instant::now()));
            } else if window.loading && !s.table_window_jobs.has_waker {
                tx.wake_after(Duration::from_millis(4));
            }
        }
    }
    if (s.table_window_jobs.in_flight != 0 || !s.table_window_jobs.pending.is_empty())
        && !s.table_window_jobs.has_waker
    {
        tx.wake_after(Duration::from_millis(4));
    }
    changed
}

#[cfg(test)]
#[path = "table_window_tests.rs"]
mod tests;
