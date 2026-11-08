use crate::state::State;
use moss_core::{
    Database, Property, Record, Store, ViewKind,
    board::{BOARD_PAGE_SIZE, BoardProjection, bucket_key, group_property, subgroup_property},
};
use rune::Tx;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
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
    search: String,
    offsets: BTreeMap<String, u64>,
}
struct Response {
    key: String,
    result: Result<BoardProjection, String>,
}
#[derive(Default)]
pub struct BoardState {
    pub projection: Option<BoardProjection>,
    pub loading: bool,
    pub error: Option<String>,
    pub frozen: bool,
    pub offsets: BTreeMap<String, u64>,
    database: String,
    view: String,
    key: String,
    context: String,
    pending: Option<Request>,
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: Option<mpsc::Receiver<Response>>,
    worker: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    interrupt: Arc<Mutex<Option<moss_core::InterruptHandle>>>,
}
impl BoardState {
    pub fn projection_for(&self, database: &str, view: &str) -> Option<&BoardProjection> {
        (self.database == database && self.view == view)
            .then_some(self.projection.as_ref())
            .flatten()
    }
    pub fn invalidate(&mut self) {
        self.key.clear();
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
                    store
                        .as_ref()
                        .unwrap()
                        .query_board(
                            &request.database,
                            &request.view,
                            &request.search,
                            &request.offsets,
                        )
                        .map_err(|e| e.to_string())
                })();
                if stop.load(Ordering::Acquire)
                    || responses
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
        self.sender = Some(sender);
        self.receiver = Some(receiver);
    }
    fn dispatch(&mut self) {
        if let Some(request) = self.pending.take() {
            if let Some(sender) = &self.sender {
                match sender.try_send(request) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(request)) => self.pending = Some(request),
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        self.loading = false;
                        self.error =
                            Some("The Board reader stopped. Reopen this workspace.".into());
                    }
                }
            }
        }
    }
}
impl Drop for BoardState {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.sender.take();
        if let Ok(interrupt) = self.interrupt.lock() {
            if let Some(interrupt) = interrupt.as_ref() {
                interrupt.interrupt();
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn poll(s: &mut State, _time: f64, tx: &mut Tx) -> bool {
    if s.database_views.board.frozen {
        return false;
    }
    let request = s.tab().and_then(|tab| {
        let db = tab.database.as_ref()?;
        let view = db.views.get(tab.view)?;
        if view.kind != ViewKind::Board {
            return None;
        }
        Some((
            db.id.clone(),
            view.id.clone(),
            json!([
                db.id,
                view.id,
                view.query,
                view.config.get("board_subgroup_by"),
                tab.title_search
            ])
            .to_string(),
            tab.title_search.clone(),
            json!([db, view, tab.title_search, s.store.local_change_count()]).to_string(),
        ))
    });
    let state = &mut s.database_views.board;
    let mut changed = false;
    if let Some((database, view, context, search, version)) = request {
        if state.context != context {
            state.database = database.clone();
            state.view = view.clone();
            state.context = context;
            state.offsets.clear();
            state.projection = None;
            state.key.clear();
        }
        let key = json!([version, state.offsets]).to_string();
        if state.key != key {
            state.key = key.clone();
            state.error = None;
            state.loading = true;
            state.pending = Some(Request {
                key,
                database,
                view,
                search,
                offsets: state.offsets.clone(),
            });
            state.start(s.path.clone());
            state.dispatch();
            changed = true;
        }
    } else {
        state.context.clear();
        state.database.clear();
        state.view.clear();
        state.key.clear();
        state.pending = None;
        state.projection = None;
        state.loading = false;
        state.frozen = false;
    }
    if let Some(receiver) = &state.receiver {
        while let Ok(response) = receiver.try_recv() {
            if response.key != state.key {
                continue;
            }
            state.loading = false;
            match response.result {
                Ok(projection) => {
                    let mut corrected = false;
                    state.offsets.retain(|key, offset| {
                        let total = projection
                            .buckets
                            .iter()
                            .find(|bucket| bucket_key(&bucket.group, &bucket.subgroup) == *key)
                            .map(|bucket| bucket.total)
                            .unwrap_or(0);
                        let last = total.saturating_sub(1) / BOARD_PAGE_SIZE as u64
                            * BOARD_PAGE_SIZE as u64;
                        if *offset > last {
                            *offset = last;
                            corrected = true;
                        }
                        *offset > 0
                    });
                    if corrected {
                        state.key.clear();
                        state.loading = true;
                    } else {
                        state.projection = Some(projection);
                    }
                }
                Err(error) => state.error = Some(error),
            }
            changed = true;
        }
    }
    state.dispatch();
    if state.loading || state.pending.is_some() {
        tx.wake_after(Duration::from_millis(16));
    }
    changed
}

fn project_move(
    projection: &mut BoardProjection,
    record: &Record,
    rank: Vec<u8>,
    matches: bool,
    before: Option<&str>,
) {
    let mut removed = false;
    for bucket in &mut projection.buckets {
        if let Some(index) = bucket.records.iter().position(|old| old.id == record.id) {
            bucket.records.remove(index);
            bucket.total = bucket.total.saturating_sub(1);
            projection
                .group_totals
                .entry(moss_core::board::group_key(&bucket.group))
                .and_modify(|count| *count = count.saturating_sub(1));
            projection
                .subgroup_totals
                .entry(moss_core::board::group_key(&bucket.subgroup))
                .and_modify(|count| *count = count.saturating_sub(1));
            removed = true;
        }
    }
    projection.order.insert(record.id.clone(), rank);
    if !matches {
        if removed {
            projection.total = projection.total.saturating_sub(1);
        }
        return;
    }
    let group = moss_core::board::group_value(record, &projection.group_property);
    let subgroup = projection
        .subgroup_property
        .as_ref()
        .map(|p| moss_core::board::group_value(record, p))
        .unwrap_or(Value::Null);
    *projection
        .group_totals
        .entry(moss_core::board::group_key(&group))
        .or_default() += 1;
    *projection
        .subgroup_totals
        .entry(moss_core::board::group_key(&subgroup))
        .or_default() += 1;
    if !removed {
        projection.total += 1;
    }
    if !projection.groups.contains(&group) {
        projection.groups.push(group.clone());
    }
    if !projection.subgroups.contains(&subgroup) {
        projection.subgroups.push(subgroup.clone());
    }
    let index = projection
        .buckets
        .iter()
        .position(|b| b.group == group && b.subgroup == subgroup)
        .unwrap_or_else(|| {
            projection.buckets.push(moss_core::board::BoardBucket {
                group,
                subgroup,
                total: 0,
                offset: 0,
                records: Vec::new(),
            });
            projection.buckets.len() - 1
        });
    let bucket = &mut projection.buckets[index];
    bucket.total += 1;
    let index = before
        .and_then(|id| bucket.records.iter().position(|r| r.id == id))
        .unwrap_or(bucket.records.len());
    bucket.records.insert(index, record.clone());
    bucket.records.truncate(BOARD_PAGE_SIZE);
}

fn board_context(s: &State) -> Option<(Database, moss_core::DatabaseView)> {
    let tab = s.tab()?;
    let db = tab.database.as_ref()?;
    let view = db.views.get(tab.view)?;
    (view.kind == ViewKind::Board).then(|| (db.clone(), view.clone()))
}
impl State {
    pub fn page_board_bucket(&mut self, group: &Value, subgroup: &Value, forward: bool) {
        let key = bucket_key(group, subgroup);
        let state = &mut self.database_views.board;
        let offset = state.offsets.get(&key).copied().unwrap_or(0);
        let next = if forward {
            offset.saturating_add(BOARD_PAGE_SIZE as u64)
        } else {
            offset.saturating_sub(BOARD_PAGE_SIZE as u64)
        };
        if next == 0 {
            state.offsets.remove(&key);
        } else {
            state.offsets.insert(key, next);
        }
        state.invalidate();
    }
    pub fn commit_board_move(
        &mut self,
        record: &Record,
        property: &Property,
        subproperty: Option<&Property>,
        group: Value,
        subgroup: Value,
        before: Option<String>,
    ) -> bool {
        if !self.finish_cell(false) || !self.close_peek() {
            return false;
        }
        let Some((db, view)) = board_context(self) else {
            return false;
        };
        if record.database_id != db.id
            || group_property(&db, &view).ok() != Some(property)
            || subgroup_property(&db, &view).ok().flatten() != subproperty
        {
            self.error = Some("Board grouping changed; try the move again.".into());
            return false;
        }
        let rank = self
            .database_views
            .board
            .projection_for(&db.id, &view.id)
            .and_then(|p| p.order.get(&record.id))
            .cloned()
            .unwrap_or_else(|| record.id.as_bytes().to_vec());
        match self.store.move_board_record(
            &db.id,
            &view,
            record,
            property,
            subproperty,
            group,
            subgroup,
            before.as_deref(),
            &rank,
        ) {
            Ok(updated) => {
                self.sync_record(&updated);
                let search = self
                    .tab()
                    .map(|tab| tab.title_search.as_str())
                    .unwrap_or("");
                let matches =
                    self.store
                        .board_record_matches(&db.id, &view.id, &updated.id, search);
                let rank = self.store.board_record_rank(&view.id, &updated.id);
                if view.query.sorts.is_empty() {
                    if let (Ok(matches), Ok(rank), Some(projection)) =
                        (matches, rank, self.database_views.board.projection.as_mut())
                    {
                        project_move(projection, &updated, rank, matches, before.as_deref());
                    } else {
                        self.database_views.board.projection = None;
                    }
                }
                self.database_views.board.invalidate();
                self.error = None;
                true
            }
            Err(error) => {
                let error = format!("Card was not moved: {error}");
                self.database_views.board.error = Some(error.clone());
                self.error = Some(error);
                false
            }
        }
    }
    pub fn save_board_group_name(
        &mut self,
        property: &Property,
        old: Option<&Value>,
        name: &str,
    ) -> bool {
        if !self.finish_cell(false) || !self.close_peek() {
            return false;
        }
        let Some((db, _)) = board_context(self) else {
            return false;
        };
        match self
            .store
            .save_board_group_name(&db.id, property, old, name)
        {
            Ok(db) => {
                for tab in &mut self.tabs {
                    if tab.database.as_ref().is_some_and(|old| old.id == db.id) {
                        tab.database = Some(db.clone());
                    }
                }
                self.database_views.board.invalidate();
                self.query_records();
                true
            }
            Err(error) => {
                self.error = Some(format!("Group was not saved: {error}"));
                false
            }
        }
    }
    pub fn trash_board_card(&mut self, record_id: &str) -> bool {
        if !self.finish_cell(false) || !self.close_peek() {
            return false;
        }
        let Some((db, _)) = board_context(self) else {
            return false;
        };
        let belongs = self
            .store
            .get_record(record_id)
            .ok()
            .flatten()
            .is_some_and(|record| record.database_id == db.id && record.deleted_at.is_none());
        if !belongs {
            self.error = Some("Card is unavailable in this database.".into());
            return false;
        }
        match self.store.delete_record(record_id) {
            Ok(()) => {
                self.database_views.board.invalidate();
                self.query_records()
            }
            Err(error) => {
                self.error = Some(format!("Card was not moved to Trash: {error}"));
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "board_view_tests.rs"]
mod tests;
