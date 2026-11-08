use crate::state::State;
use moss_core::{FilterExpr, Store, ViewKind};
use serde_json::Value;
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

#[derive(Clone, Debug, PartialEq)]
pub enum SummaryValue {
    Loading,
    Ready(Value),
    Error(String),
}

struct Request {
    page_id: String,
    view_id: String,
    database_id: String,
    epoch: u64,
    filter: Option<FilterExpr>,
    search: String,
    operations: BTreeMap<String, String>,
}
struct Response {
    page_id: String,
    view_id: String,
    epoch: u64,
    values: BTreeMap<String, SummaryValue>,
}
#[derive(Default)]
pub struct SummaryJobs {
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: Option<mpsc::Receiver<Response>>,
    pending: BTreeMap<String, Request>,
    in_flight: usize,
    next_epoch: u64,
    worker: Option<std::thread::JoinHandle<()>>,
    stopped: Arc<AtomicBool>,
    interrupt: Arc<Mutex<Option<moss_core::InterruptHandle>>>,
}
impl SummaryJobs {
    fn start(&mut self, path: PathBuf) {
        if self.sender.is_some() {
            return;
        }
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, receiver) = mpsc::channel();
        let stopped = self.stopped.clone();
        let interrupt = self.interrupt.clone();
        self.worker = Some(std::thread::spawn(move || {
            let mut store = None;
            while let Ok(request) = requests.recv() {
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let opened = if store.is_none() {
                    match Store::open_read_only(path.join("workspace.sqlite3")) {
                        Ok(value) => {
                            *interrupt.lock().unwrap() = Some(value.interrupt_handle());
                            store = Some(value);
                            Ok(())
                        }
                        Err(error) => Err(error.to_string()),
                    }
                } else {
                    Ok(())
                };
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let values = request
                    .operations
                    .iter()
                    .map(|(id, operation)| {
                        let result = if stopped.load(Ordering::Acquire) {
                            Err("Calculation cancelled".to_owned())
                        } else {
                            opened.as_ref().map_err(Clone::clone).and_then(|_| {
                                store
                                    .as_ref()
                                    .unwrap()
                                    .summarize_records(
                                        &request.database_id,
                                        request.filter.as_ref(),
                                        &request.search,
                                        id,
                                        operation,
                                    )
                                    .map_err(|error| error.to_string())
                            })
                        };
                        (
                            id.clone(),
                            match result {
                                Ok(value) => SummaryValue::Ready(value),
                                Err(error) => SummaryValue::Error(error),
                            },
                        )
                    })
                    .collect();
                if responses
                    .send(Response {
                        page_id: request.page_id,
                        view_id: request.view_id,
                        epoch: request.epoch,
                        values,
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
        let Some(sender) = &self.sender else { return };
        while let Some((key, request)) = self.pending.pop_first() {
            match sender.try_send(request) {
                Ok(()) => self.in_flight += 1,
                Err(mpsc::TrySendError::Full(request)) => {
                    self.pending.insert(key, request);
                    break;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => break,
            }
        }
    }
}

impl Drop for SummaryJobs {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.sender.take();
        if let Some(handle) = self.interrupt.lock().unwrap().as_ref() {
            handle.interrupt();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl State {
    pub fn refresh_table_summaries(&mut self) {
        self.refresh_table_summaries_for(self.active);
    }
    pub fn refresh_table_summaries_for(&mut self, index: usize) {
        let Some(tab) = self.tabs.get_mut(index) else {
            return;
        };
        let Some(db) = &tab.database else { return };
        let Some(view) = db.views.get(tab.view) else {
            return;
        };
        let operations: BTreeMap<String, String> = view
            .config
            .get("calculations")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|map| map.iter())
            .filter_map(|(id, op)| op.as_str().map(|op| (id.clone(), op.to_owned())))
            .filter(|(id, _)| db.properties.iter().any(|p| &p.id == id))
            .collect();
        if view.kind != ViewKind::Table || operations.is_empty() {
            tab.summary_values.clear();
            tab.summary_key.clear();
            return;
        }
        let key = serde_json::json!([
            db.id,
            view.id,
            view.query.filter,
            tab.title_search,
            operations,
            self.store.local_change_count()
        ])
        .to_string();
        if tab.summary_key == key {
            return;
        }
        tab.summary_key = key;
        self.summary_jobs.next_epoch += 1;
        tab.summary_epoch = self.summary_jobs.next_epoch;
        tab.summary_values = operations
            .keys()
            .map(|id| (id.clone(), SummaryValue::Loading))
            .collect();
        let request = Request {
            page_id: tab.page.id.clone(),
            view_id: view.id.clone(),
            database_id: db.id.clone(),
            epoch: tab.summary_epoch,
            filter: view.query.filter.clone(),
            search: tab.title_search.clone(),
            operations,
        };
        self.summary_jobs
            .pending
            .insert(request.page_id.clone(), request);
        self.summary_jobs.start(self.path.clone());
        self.summary_jobs.dispatch();
    }
}

pub fn poll(s: &mut State, tx: &mut rune::Tx) -> bool {
    let mut changed = false;
    if let Some(receiver) = &s.summary_jobs.receiver {
        while let Ok(result) = receiver.try_recv() {
            s.summary_jobs.in_flight = s.summary_jobs.in_flight.saturating_sub(1);
            if let Some(tab) = s.tabs.iter_mut().find(|tab| {
                tab.page.id == result.page_id
                    && tab.summary_epoch == result.epoch
                    && tab
                        .database
                        .as_ref()
                        .and_then(|db| db.views.get(tab.view))
                        .is_some_and(|view| view.id == result.view_id)
            }) {
                tab.summary_values = result.values;
                changed = true;
            }
        }
    }
    s.summary_jobs.dispatch();
    if s.summary_jobs.in_flight > 0 || !s.summary_jobs.pending.is_empty() {
        tx.wake_after(Duration::from_millis(30));
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_state_joins_a_queued_summary_reader() {
        for _ in 0..10 {
            let path =
                std::env::temp_dir().join(format!("moss-summary-drop-{}", moss_core::new_id()));
            let mut s = State::open(path.clone(), None).unwrap();
            s.new_page(true, None);
            let tab = &mut s.tabs[s.active];
            let db = tab.database.as_mut().unwrap();
            let title = db
                .properties
                .iter()
                .find(|p| p.kind == moss_core::PropertyKind::Title)
                .unwrap()
                .id
                .clone();
            db.views[tab.view].config["calculations"] = serde_json::json!({title: "count_all"});
            s.refresh_table_summaries();
            drop(s);
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn old_results_cannot_replace_a_newer_view_request() {
        let path = std::env::temp_dir().join(format!("moss-summary-{}", moss_core::new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let tab = &mut s.tabs[s.active];
        let page_id = tab.page.id.clone();
        let view_id = tab.database.as_ref().unwrap().views[tab.view].id.clone();
        tab.summary_epoch = 42;
        tab.summary_values
            .insert("value".into(), SummaryValue::Loading);
        let (sender, receiver) = mpsc::channel();
        s.summary_jobs.receiver = Some(receiver);
        let mut tx = rune::Tx::default();
        sender
            .send(Response {
                page_id: page_id.clone(),
                view_id: view_id.clone(),
                epoch: 41,
                values: BTreeMap::from([("value".into(), SummaryValue::Ready(Value::from(999)))]),
            })
            .unwrap();
        assert!(!poll(&mut s, &mut tx));
        assert_eq!(
            s.tab().unwrap().summary_values["value"],
            SummaryValue::Loading
        );
        sender
            .send(Response {
                page_id,
                view_id,
                epoch: 42,
                values: BTreeMap::from([("value".into(), SummaryValue::Ready(Value::from(1)))]),
            })
            .unwrap();
        assert!(poll(&mut s, &mut tx));
        assert_eq!(
            s.tab().unwrap().summary_values["value"],
            SummaryValue::Ready(Value::from(1))
        );
        drop(s);
        std::fs::remove_dir_all(path).unwrap();
    }
}
