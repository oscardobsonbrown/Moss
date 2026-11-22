use crate::state::State;
use moss_core::Store;
use rune::{Tx, Waker};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Default)]
pub struct ArchiveTransfer {
    pub busy: bool,
    pub status: Option<String>,
    wake: Option<Waker>,
    receiver: Option<mpsc::Receiver<Result<bool, String>>>,
    cancel: Option<Arc<AtomicBool>>,
}
impl ArchiveTransfer {
    pub fn set_waker(&mut self, wake: Waker) {
        self.wake = Some(wake);
    }
}
impl Drop for ArchiveTransfer {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl State {
    pub fn cancel_archive_transfer(&mut self) {
        if let Some(cancel) = &self.archive_transfer.cancel {
            cancel.store(true, Ordering::Relaxed);
            self.archive_transfer.status = Some("Cancelling archive transfer…".into());
        }
    }
    pub fn transfer_archive(&mut self, path: PathBuf, importing: bool) {
        if self.archive_transfer.busy || !self.flush_current() {
            return;
        }
        self.archive_transfer.busy = true;
        self.archive_transfer.status = Some(
            if importing {
                "Importing workspace…"
            } else {
                "Exporting workspace…"
            }
            .into(),
        );
        let db = self.path.join("workspace.sqlite3");
        let workspace = self.workspace.id.clone();
        let wake = self.archive_transfer.wake.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.archive_transfer.cancel = Some(cancel.clone());
        let (sender, receiver) = mpsc::channel();
        self.archive_transfer.receiver = Some(receiver);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if importing {
                    Store::open(db)
                        .map_err(|e| e.to_string())
                        .and_then(|mut store| {
                            store
                                .import_workspace_from_file_cancellable(path, &cancel)
                                .map(|_| true)
                                .map_err(|e| e.to_string())
                        })
                } else {
                    Store::open_read_only(db)
                        .map_err(|e| e.to_string())
                        .and_then(|store| {
                            store
                                .export_workspace_to_file_cancellable(&workspace, path, &cancel)
                                .map(|_| false)
                                .map_err(|e| e.to_string())
                        })
                }
            }))
            .unwrap_or_else(|_| Err("Archive transfer stopped unexpectedly.".into()));
            let _ = sender.send(result);
            if let Some(wake) = wake {
                wake();
            }
        });
    }
}
pub fn poll(s: &mut State, _: f64, tx: &mut Tx) -> bool {
    let Some(result) = s
        .archive_transfer
        .receiver
        .as_ref()
        .and_then(|rx| match rx.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Archive transfer stopped.".into())),
            Err(mpsc::TryRecvError::Empty) => None,
        })
    else {
        return false;
    };
    s.archive_transfer.receiver = None;
    s.archive_transfer.cancel = None;
    s.archive_transfer.busy = false;
    s.archive_transfer.status = Some(match result {
        Ok(true) => {
            if let Ok(workspaces) = s.store.list_workspaces() {
                s.workspaces = workspaces;
            }
            "Workspace imported. Restart Moss to open it.".into()
        }
        Ok(false) => "Workspace exported.".into(),
        Err(error) if error.contains("archive transfer cancelled") => {
            "Archive transfer cancelled.".into()
        }
        Err(error) => format!("Archive transfer failed: {error}"),
    });
    tx.instant();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};
    use std::time::{Duration, Instant};

    fn wait(rt: &mut Runtime<State>) {
        let until = Instant::now() + Duration::from_secs(10);
        while rt.state.archive_transfer.busy && Instant::now() < until {
            rt.frame(0.01);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!rt.state.archive_transfer.busy, "archive task timed out");
    }
    #[test]
    fn archive_transfer_runs_off_thread_and_restores_as_a_new_workspace() {
        let path = std::env::temp_dir().join(format!("moss-transfer-{}", moss_core::new_id()));
        let state = State::open(path.clone(), None).unwrap();
        let before = state.workspaces.len();
        let mut rt = Runtime::new(
            state,
            crate::ui::view,
            (1000., 800.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.poll = Some(crate::poll);
        rt.state.open_settings();
        rt.state
            .select_settings_section(crate::settings::SettingsSection::ImportExport);
        rt.frame(0.01);
        assert!(rt.find("export-workspace").is_some());
        assert!(rt.find("import-workspace").is_some());
        let archive = path.with_extension("moss");
        rt.state.transfer_archive(archive.clone(), false);
        wait(&mut rt);
        assert_eq!(
            rt.state.archive_transfer.status.as_deref(),
            Some("Workspace exported.")
        );
        assert!(rt.find("archive-transfer-status").is_some());
        assert!(archive.exists());
        rt.state.transfer_archive(archive.clone(), true);
        wait(&mut rt);
        assert_eq!(rt.state.workspaces.len(), before + 1);
        assert_eq!(
            rt.state.archive_transfer.status.as_deref(),
            Some("Workspace imported. Restart Moss to open it.")
        );
        rt.state.transfer_archive(path.join("missing.moss"), true);
        wait(&mut rt);
        assert!(
            rt.state
                .archive_transfer
                .status
                .as_deref()
                .unwrap()
                .starts_with("Archive transfer failed:")
        );
        assert_eq!(rt.state.workspaces.len(), before + 1);
        drop(rt);
        std::fs::remove_file(archive).unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn dropping_transfer_state_requests_worker_cancellation() {
        let flag = Arc::new(AtomicBool::new(false));
        let mut state = ArchiveTransfer::default();
        state.cancel = Some(flag.clone());
        drop(state);
        assert!(flag.load(Ordering::Relaxed));
    }
}
