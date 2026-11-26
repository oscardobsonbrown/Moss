use moss_core::{MapBounds, RecordQuery, Store};
use rune::Waker;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

pub struct Request {
    pub epoch: u64,
    pub context: String,
    pub db: String,
    pub query: RecordQuery,
    pub title: String,
    pub place: String,
}
pub struct Response {
    pub epoch: u64,
    pub context: String,
    pub result: Result<Option<MapBounds>, String>,
}
pub struct FitWorker {
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: mpsc::Receiver<Response>,
    worker: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    interrupt: Arc<Mutex<Option<moss_core::InterruptHandle>>>,
    pending: Option<Request>,
    failure: Option<Response>,
}
impl FitWorker {
    pub fn new(path: PathBuf, wake: Option<Waker>) -> Self {
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (results, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let interrupt = Arc::new(Mutex::new(None));
        let worker_interrupt = interrupt.clone();
        let worker = std::thread::spawn(move || {
            let store = Store::open_read_only(path.join("workspace.sqlite3"));
            if let Ok(opened) = &store {
                *worker_interrupt.lock().unwrap() = Some(opened.interrupt_handle());
            }
            while let Ok(request) = requests.recv() {
                if worker_stop.load(Ordering::Acquire) {
                    break;
                }
                let result = store.as_ref().map_err(ToString::to_string).and_then(|s| {
                    s.map_extent(&request.db, &request.query, &request.title, &request.place)
                        .map_err(|e| e.to_string())
                });
                if worker_stop.load(Ordering::Acquire)
                    || results
                        .send(Response {
                            epoch: request.epoch,
                            context: request.context,
                            result,
                        })
                        .is_err()
                {
                    break;
                }
                if let Some(wake) = &wake {
                    wake();
                }
            }
        });
        Self {
            sender: Some(sender),
            receiver,
            worker: Some(worker),
            stop,
            interrupt,
            pending: None,
            failure: None,
        }
    }
    pub fn submit(&mut self, request: Request) {
        self.pending = Some(request);
        self.pump();
    }
    pub fn cancel(&mut self) {
        self.pending = None;
    }
    fn pump(&mut self) {
        if let Some(request) = self.pending.take() {
            let Some(sender) = &self.sender else {
                return;
            };
            match sender.try_send(request) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(request)) => self.pending = Some(request),
                Err(mpsc::TrySendError::Disconnected(request)) => {
                    self.failure = Some(Response {
                        epoch: request.epoch,
                        context: request.context,
                        result: Err("Map extent worker stopped.".into()),
                    })
                }
            }
        }
    }
    pub fn poll(&mut self) -> Option<Response> {
        let result = self
            .receiver
            .try_recv()
            .ok()
            .or_else(|| self.failure.take());
        self.pump();
        result.or_else(|| self.failure.take())
    }
}
impl Drop for FitWorker {
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
