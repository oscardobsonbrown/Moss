use crate::state::State;
use moss_updater::{
    DEFAULT_MANIFEST_URL, HELPER_NAME,
    bundle::{bundle_public_key, can_install_to, extract_archive, installed_version, verify_bundle},
    cache::{
        CachedManifest, InstallOutcome, StagedMarker, StagedUpdate, default_cache_dir,
        ensure_private_dir, find_best_staged, load_cached_manifest, load_verified_staged,
        prune_staged, receipt_path, save_cached_manifest, staged_archive_path, staged_part_path,
        staged_version_dir, take_receipt, write_staged_marker,
    },
    crypto::sha256_file,
    current_platform_key,
    http::{HttpError, ManifestFetch, download_file, fetch_manifest},
    install::{InstallRequest, spawn_detached_installer},
    manifest::{Manifest, ManifestError, VerifiedRelease},
    version::{current_macos_version, default_release_url, format_version, parse_version},
};
use rune::{Tx, Waker};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

pub use moss_updater::version::Version;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);
static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    Current,
    Available(Version),
    Downloading(u8),
    Ready(Version),
    Restarting(Version),
    CheckFailed,
    DownloadFailed,
    Unverified,
    SaveFailed(Version),
    InstallLocation,
    InstallFailed(Version),
}

pub fn version_label(version: Version) -> String {
    format_version(version)
}

pub fn release_url(version: Version) -> String {
    default_release_url(version)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub status: Status,
    pub updated: Option<Version>,
    pub release_url: Option<String>,
}

#[derive(Clone)]
struct RuntimeConfig {
    bundle: Option<PathBuf>,
    helper: Option<PathBuf>,
    cache_dir: PathBuf,
    receipt: PathBuf,
    explicit_data_dir: Option<PathBuf>,
    manifest_url: String,
    public_key: String,
    allow_local_urls: bool,
    curl: PathBuf,
    relaunch_on_install: bool,
}

enum WorkerCommand {
    Check { download: bool },
    DownloadKnown,
}

struct Shared {
    snapshot: Mutex<Snapshot>,
    known_release: Mutex<Option<VerifiedRelease>>,
    staged: Mutex<Option<StagedUpdate>>,
    wakers: Mutex<BTreeMap<u64, Waker>>,
    revision: AtomicU64,
    automatic: AtomicBool,
    stopped: AtomicBool,
    config: RuntimeConfig,
}

#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    requests: Sender<WorkerCommand>,
    pub version: Version,
}

impl Client {
    fn new(
        version: Version,
        automatic: bool,
        initial: Snapshot,
        staged: Option<StagedUpdate>,
        config: RuntimeConfig,
    ) -> (Self, Receiver<WorkerCommand>) {
        let (requests, inbox) = mpsc::channel();
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(initial),
            known_release: Mutex::new(None),
            staged: Mutex::new(staged),
            wakers: Mutex::new(BTreeMap::new()),
            revision: AtomicU64::new(1),
            automatic: AtomicBool::new(automatic),
            stopped: AtomicBool::new(false),
            config,
        });
        (
            Self {
                shared,
                requests,
                version,
            },
            inbox,
        )
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.snapshot.lock().unwrap().clone()
    }

    pub fn release_url_for(&self, version: Version) -> String {
        if let Some(url) = self.shared.snapshot.lock().unwrap().release_url.clone() {
            return url;
        }
        if let Some(staged) = self.shared.staged.lock().unwrap().as_ref() {
            if staged.version == version && !staged.release_url.is_empty() {
                return staged.release_url.clone();
            }
        }
        if let Some(known) = self.shared.known_release.lock().unwrap().as_ref() {
            if known.version == version && !known.release_url.is_empty() {
                return known.release_url.clone();
            }
        }
        default_release_url(version)
    }

    fn wake(&self) {
        self.shared.revision.fetch_add(1, Ordering::Release);
        let wakers: Vec<Waker> = self
            .shared
            .wakers
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();
        for wake in wakers {
            wake();
        }
    }

    fn show(&self, status: Status) {
        let mut view = self.shared.snapshot.lock().unwrap();
        if view.status == status {
            return;
        }
        view.status = status;
        drop(view);
        self.wake();
    }

    fn show_with_url(&self, status: Status, url: Option<String>) {
        let mut view = self.shared.snapshot.lock().unwrap();
        if view.status == status && view.release_url == url {
            return;
        }
        view.status = status;
        if url.is_some() {
            view.release_url = url;
        }
        drop(view);
        self.wake();
    }

    pub fn dismiss(&self) {
        let mut view = self.shared.snapshot.lock().unwrap();
        match view.status {
            Status::DownloadFailed
            | Status::Unverified
            | Status::InstallLocation
            | Status::InstallFailed(_) => {
                view.status = Status::Idle;
                view.release_url = None;
            }
            _ => {
                view.updated = None;
                view.release_url = None;
            }
        }
        drop(view);
        self.wake();
    }

    pub fn automatic(&self) -> bool {
        self.shared.automatic.load(Ordering::Relaxed)
    }

    pub fn keep_editing(&self) {
        if let Status::SaveFailed(version) = self.snapshot().status {
            self.show(Status::Ready(version));
        }
    }

    pub fn set_automatic(&self, enabled: bool) {
        self.shared.automatic.store(enabled, Ordering::Relaxed);
        self.wake();
        if enabled {
            if matches!(self.snapshot().status, Status::Available(_)) {
                self.download_available();
            } else {
                self.check(false);
            }
        }
    }

    pub fn download_available(&self) {
        let has_known = self.shared.known_release.lock().unwrap().is_some();
        if has_known {
            let can_start = {
                let mut view = self.shared.snapshot.lock().unwrap();
                if matches!(
                    view.status,
                    Status::Available(_) | Status::DownloadFailed | Status::InstallFailed(_)
                ) {
                    view.status = Status::Downloading(0);
                    true
                } else {
                    false
                }
            };
            if can_start {
                self.wake();
                if self.requests.send(WorkerCommand::DownloadKnown).is_err() {
                    self.show(Status::DownloadFailed);
                }
                return;
            }
        }
        self.check(true);
    }

    fn begin_manual_check(&self) -> bool {
        let mut view = self.shared.snapshot.lock().unwrap();
        if matches!(
            view.status,
            Status::Checking | Status::Downloading(_) | Status::Restarting(_) | Status::SaveFailed(_)
        ) {
            return false;
        }
        view.status = Status::Checking;
        drop(view);
        self.wake();
        true
    }

    pub fn check(&self, download: bool) {
        if download && matches!(self.snapshot().status, Status::Available(_)) {
            self.download_available();
            return;
        }
        if self.begin_manual_check()
            && self
                .requests
                .send(WorkerCommand::Check { download })
                .is_err()
        {
            self.show(Status::CheckFailed);
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(version: Version, status: Status) -> Self {
        let config = RuntimeConfig {
            bundle: None,
            helper: None,
            cache_dir: std::env::temp_dir().join("moss-updater-fixture"),
            receipt: std::env::temp_dir().join("moss-updater-fixture/receipt.json"),
            explicit_data_dir: None,
            manifest_url: DEFAULT_MANIFEST_URL.into(),
            public_key: String::new(),
            allow_local_urls: true,
            curl: PathBuf::from("/usr/bin/curl"),
            relaunch_on_install: false,
        };
        let (client, _) = Self::new(
            version,
            true,
            Snapshot {
                status,
                updated: None,
                release_url: None,
            },
            None,
            config,
        );
        client
    }
}

pub struct Window {
    pub client: Option<Client>,
    id: u64,
    seen: u64,
    toast: Snapshot,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            client: None,
            id: NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed),
            seen: 0,
            toast: Snapshot::default(),
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(client) = &self.client {
            if let Ok(mut wakers) = client.shared.wakers.lock() {
                wakers.remove(&self.id);
            }
        }
    }
}

impl Window {
    pub fn register(&self, wake: Waker) {
        if let Some(client) = &self.client {
            client.shared.wakers.lock().unwrap().insert(self.id, wake);
        }
    }

    pub fn poll(&mut self, tx: &mut Tx, in_settings: bool) -> bool {
        let Some(client) = &self.client else {
            return false;
        };
        let revision = client.shared.revision.load(Ordering::Acquire);
        if revision == self.seen {
            return false;
        }
        self.seen = revision;
        let mut toast = client.snapshot();
        if matches!(
            toast.status,
            Status::Checking | Status::Current | Status::CheckFailed | Status::Downloading(_)
        ) {
            toast.status = Status::Idle;
        }
        let changed = in_settings || self.toast != toast;
        self.toast = toast;
        if changed {
            tx.instant();
        }
        changed
    }
}

pub fn toggle(s: &mut State) {
    let Some(client) = &s.updates.client else {
        return;
    };
    let enabled = !client.automatic();
    client.set_automatic(enabled);
    s.shared_preferences.lock().unwrap().automatic_updates = Some(enabled);
    s.save_preferences();
}

pub fn restart(s: &mut State, tx: &mut Tx) {
    let Some(client) = s.updates.client.clone() else {
        return;
    };
    let (Status::Ready(next) | Status::SaveFailed(next)) = client.snapshot().status else {
        return;
    };

    let Some(bundle) = client.shared.config.bundle.clone() else {
        client.show(Status::InstallLocation);
        return;
    };
    if !can_install_to(&bundle) {
        client.show(Status::InstallLocation);
        return;
    }

    client.show(Status::Restarting(next));
    if !s.flush_current() {
        client.show(Status::SaveFailed(next));
        return;
    }

    let mut handles = s.handles.lock().unwrap().clone();
    let requester = handles.remove(&s.workspace.id);
    if handles.is_empty() {
        finish_restart(s, tx, &client, next);
        return;
    }
    let Some(requester) = requester else {
        finish_restart(s, tx, &client, next);
        return;
    };

    let expected = handles.len();
    let (done_tx, done_rx) = mpsc::channel();
    for window in handles.into_values() {
        let done_tx = done_tx.clone();
        window.update(move |state, _| {
            let ok = state.flush_current();
            let _ = done_tx.send(ok);
        });
    }
    drop(done_tx);

    thread::spawn(move || {
        let deadline = Instant::now() + FLUSH_TIMEOUT;
        let mut received = 0usize;
        let mut all_ok = true;
        while received < expected {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                all_ok = false;
                break;
            }
            match done_rx.recv_timeout(remaining) {
                Ok(true) => received += 1,
                Ok(false) => {
                    all_ok = false;
                    break;
                }
                Err(_) => {
                    all_ok = false;
                    break;
                }
            }
        }
        if all_ok {
            requester.update(move |state, tx| finish_restart(state, tx, &client, next));
        } else {
            client.show(Status::SaveFailed(next));
        }
    });
}

fn finish_restart(s: &mut State, tx: &mut Tx, client: &Client, next: Version) {
    if !s.flush_current() {
        client.show(Status::SaveFailed(next));
        return;
    }
    s.save_preferences();

    let Some(staged) = client
        .shared
        .staged
        .lock()
        .unwrap()
        .clone()
        .filter(|staged| staged.version == next)
        .or_else(|| load_verified_staged(&client.shared.config.cache_dir, next))
    else {
        client.show(Status::Unverified);
        return;
    };

    if verify_bundle(&staged.app_path, next).is_err() {
        client.show(Status::Unverified);
        return;
    }

    let Some(bundle) = client.shared.config.bundle.clone() else {
        client.show(Status::InstallLocation);
        return;
    };
    let Some(helper) = client.shared.config.helper.clone().filter(|path| path.is_file()) else {
        client.show_with_url(
            Status::InstallFailed(next),
            Some(client.release_url_for(next)),
        );
        return;
    };

    // Stop the local MCP server cleanly before handing off to the detached installer.
    drop(s.local_service.lock().unwrap().take());

    let request = InstallRequest {
        parent_pid: std::process::id(),
        staged_app: staged.app_path,
        target_app: bundle,
        expected_current: client.version,
        next_version: next,
        release_url: client.release_url_for(next),
        receipt_path: client.shared.config.receipt.clone(),
        cache_dir: client.shared.config.cache_dir.clone(),
        data_dir: client.shared.config.explicit_data_dir.clone(),
        relaunch: client.shared.config.relaunch_on_install,
    };

    if let Err(error) = spawn_detached_installer(&helper, &request) {
        eprintln!("Moss update helper failed to start: {error}");
        client.show_with_url(
            Status::InstallFailed(next),
            Some(client.release_url_for(next)),
        );
        return;
    }

    client.shared.stopped.store(true, Ordering::Release);
    tx.quit();
}

pub struct Updater {
    pub client: Client,
}

fn is_disabled_by_env() -> bool {
    std::env::var("MOSS_DISABLE_UPDATES").is_ok_and(|value| {
        let lower = value.trim().to_ascii_lowercase();
        !lower.is_empty() && lower != "0" && lower != "false"
    })
}

fn detect_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe.ancestors().nth(3)?.to_owned();
    if bundle.extension().and_then(|ext| ext.to_str()) == Some("app")
        && exe == bundle.join("Contents/MacOS/Moss")
    {
        Some(bundle)
    } else {
        None
    }
}

fn detect_helper(bundle: Option<&Path>, is_release: bool) -> Option<PathBuf> {
    if !is_release
        && let Some(override_path) = std::env::var_os("MOSS_UPDATE_HELPER").map(PathBuf::from)
        && override_path.is_file()
    {
        return Some(override_path);
    }
    if let Some(bundle) = bundle {
        let bundled = bundle.join("Contents/MacOS").join(HELPER_NAME);
        if bundled.is_file() {
            return Some(bundled);
        }
    }
    if !is_release
        && let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(HELPER_NAME);
        if sibling.is_file() {
            return Some(sibling);
        }
    }
    None
}

impl Updater {
    pub fn start(data_dir: &Path, automatic: bool) -> Option<Self> {
        if is_disabled_by_env() {
            return None;
        }
        let is_release = option_env!("MOSS_RELEASE_BUILD") == Some("1");
        let bundle = if !is_release {
            std::env::var_os("MOSS_UPDATE_TARGET_APP")
                .map(PathBuf::from)
                .or_else(detect_app_bundle)
        } else {
            detect_app_bundle()
        };

        let manifest_override = if !is_release {
            std::env::var("MOSS_UPDATE_MANIFEST_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        } else {
            None
        };

        // Outside release builds and app bundles, only activate when explicitly testing with a manifest URL.
        if is_release && bundle.is_none() {
            return None;
        }
        if !is_release && bundle.is_none() && manifest_override.is_none() {
            return None;
        }

        let public_key = option_env!("MOSS_UPDATE_PUBLIC_KEY")
            .filter(|key| !key.trim().is_empty())
            .map(str::to_owned)
            .or_else(|| bundle.as_deref().and_then(bundle_public_key))
            .or_else(|| {
                (!is_release)
                    .then(|| std::env::var("MOSS_UPDATE_PUBLIC_KEY").ok())
                    .flatten()
                    .filter(|key| !key.trim().is_empty())
            })?;

        let helper = detect_helper(bundle.as_deref(), is_release);
        if is_release && helper.is_none() {
            return None;
        }

        let current = (!is_release)
            .then(|| {
                std::env::var("MOSS_UPDATE_CURRENT_VERSION")
                    .ok()
                    .and_then(|value| parse_version(&value))
            })
            .flatten()
            .or_else(|| bundle.as_deref().and_then(installed_version))
            .or_else(|| option_env!("MOSS_VERSION").and_then(parse_version))
            .or_else(|| parse_version(env!("CARGO_PKG_VERSION")))?;

        let explicit_data_dir = std::env::var_os("MOSS_DATA_DIR").map(PathBuf::from);
        let cache_dir = (!is_release)
            .then(|| std::env::var_os("MOSS_UPDATE_CACHE_DIR").map(PathBuf::from))
            .flatten()
            .unwrap_or_else(|| default_cache_dir(data_dir, explicit_data_dir.is_some()));

        if ensure_private_dir(&cache_dir).is_err() {
            return None;
        }

        let allow_local_urls = manifest_override.is_some();
        let manifest_url = manifest_override.unwrap_or_else(|| DEFAULT_MANIFEST_URL.to_owned());
        let receipt = receipt_path(&cache_dir);

        let config = RuntimeConfig {
            bundle,
            helper,
            cache_dir,
            receipt,
            explicit_data_dir,
            manifest_url,
            public_key,
            allow_local_urls,
            curl: PathBuf::from("/usr/bin/curl"),
            relaunch_on_install: true,
        };

        Some(Self::start_with_config(current, automatic, config))
    }

    fn start_with_config(current: Version, automatic: bool, config: RuntimeConfig) -> Self {
        let mut initial = Snapshot::default();
        if let Some(bundle) = &config.bundle {
            if let Some(receipt) = take_receipt(&config.receipt, bundle) {
                match receipt.outcome {
                    InstallOutcome::Succeeded if receipt.version == current => {
                        initial.updated = Some(current);
                        prune_staged(&config.cache_dir, &[]);
                    }
                    InstallOutcome::Succeeded => {}
                    InstallOutcome::Failed {
                        reason,
                        release_url,
                    } => {
                        eprintln!("Moss previous update failed: {reason}");
                        initial.status = Status::InstallFailed(receipt.version);
                        initial.release_url = Some(release_url);
                    }
                }
            }
        }

        let staged = find_best_staged(&config.cache_dir, current);
        if let Some(staged_update) = &staged {
            prune_staged(&config.cache_dir, &[staged_update.version]);
            if initial.status == Status::Idle {
                initial.status = Status::Ready(staged_update.version);
                initial.release_url = Some(staged_update.release_url.clone());
            }
        } else {
            prune_staged(&config.cache_dir, &[]);
        }

        let (client, inbox) = Client::new(current, automatic, initial, staged, config);
        let worker = client.clone();

        thread::spawn(move || {
            let mut initial_check = true;
            loop {
                let command = if initial_check {
                    initial_check = false;
                    if worker.automatic() {
                        Some((false, WorkerCommand::Check { download: true }))
                    } else {
                        None
                    }
                } else {
                    match inbox.recv_timeout(CHECK_INTERVAL) {
                        Ok(command) => Some((true, command)),
                        Err(mpsc::RecvTimeoutError::Timeout) => worker
                            .automatic()
                            .then_some((false, WorkerCommand::Check { download: true })),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                };

                if worker.shared.stopped.load(Ordering::Acquire) {
                    break;
                }

                let Some((manual, command)) = command else {
                    continue;
                };

                match command {
                    WorkerCommand::Check { download } => {
                        run_check(&worker, manual, download);
                    }
                    WorkerCommand::DownloadKnown => {
                        let known = worker.shared.known_release.lock().unwrap().clone();
                        if let Some(release) = known {
                            let status = download_and_stage(&worker, &release);
                            worker.show_with_url(status, Some(release.release_url));
                        } else {
                            run_check(&worker, true, true);
                        }
                    }
                }
            }
        });

        Self { client }
    }
}

impl Drop for Updater {
    fn drop(&mut self) {
        self.client.shared.stopped.store(true, Ordering::Release);
    }
}

fn fetch_and_verify_manifest(client: &Client) -> Result<VerifiedRelease, Status> {
    let config = &client.shared.config;
    let cached = load_cached_manifest(&config.cache_dir);
    let cached_etag = cached.as_ref().and_then(|entry| entry.etag.as_deref());

    let fetched = fetch_manifest(
        &config.curl,
        &config.manifest_url,
        cached_etag,
        config.allow_local_urls,
        &client.shared.stopped,
    )
    .map_err(|error| {
        if !matches!(error, HttpError::Cancelled) {
            eprintln!("Moss update check: {error}");
        }
        Status::CheckFailed
    })?;

    let (raw_bytes, new_etag, from_network) = match fetched {
        ManifestFetch::NotModified => {
            let Some(cached) = cached else {
                return Err(Status::CheckFailed);
            };
            (cached.manifest_json.into_bytes(), cached.etag, false)
        }
        ManifestFetch::Modified { body, etag } => (body, etag, true),
    };

    let manifest = Manifest::from_json(&raw_bytes).map_err(|error| {
        eprintln!("Moss update manifest parse: {error}");
        Status::Unverified
    })?;

    let verified = manifest
        .verify(
            &config.public_key,
            current_platform_key(),
            current_macos_version(),
            config.allow_local_urls,
        )
        .map_err(|error| {
            eprintln!("Moss update manifest verify: {error}");
            match error {
                ManifestError::UnsupportedOs(_) => Status::Current,
                _ => Status::Unverified,
            }
        })?;

    if from_network {
        if let Ok(manifest_json) = String::from_utf8(raw_bytes) {
            let _ = save_cached_manifest(
                &config.cache_dir,
                &CachedManifest {
                    etag: new_etag,
                    manifest_json,
                },
            );
        }
    }

    Ok(verified)
}

fn download_and_stage(client: &Client, release: &VerifiedRelease) -> Status {
    let config = &client.shared.config;

    // Reuse an already verified staged bundle if it matches this exact release and digest.
    if let Some(existing) = load_verified_staged(&config.cache_dir, release.version) {
        if existing.sha256.eq_ignore_ascii_case(&release.artifact.sha256) {
            *client.shared.staged.lock().unwrap() = Some(existing);
            return Status::Ready(release.version);
        }
    }

    if let Some(bundle) = &config.bundle {
        if !can_install_to(bundle) {
            return Status::InstallLocation;
        }
    }

    let version_dir = staged_version_dir(&config.cache_dir, release.version);
    if ensure_private_dir(&version_dir).is_err() {
        return Status::DownloadFailed;
    }

    let archive_path = staged_archive_path(&config.cache_dir, release.version);
    let part_path = staged_part_path(&config.cache_dir, release.version);

    let archive_already_valid = archive_path.is_file()
        && sha256_file(&archive_path).ok().is_some_and(|(sha, size)| {
            size == release.artifact.size && sha.eq_ignore_ascii_case(&release.artifact.sha256)
        });

    if !archive_already_valid {
        let _ = fs::remove_file(&archive_path);
        client.show(Status::Downloading(0));
        let download_result = download_file(
            &config.curl,
            &release.artifact.url,
            &part_path,
            &archive_path,
            release.artifact.size,
            config.allow_local_urls,
            &client.shared.stopped,
            |percent| client.show(Status::Downloading(percent)),
        );
        if let Err(error) = download_result {
            if !matches!(error, HttpError::Cancelled) {
                eprintln!("Moss update download: {error}");
            }
            return Status::DownloadFailed;
        }
    }

    client.show(Status::Downloading(100));

    match sha256_file(&archive_path) {
        Ok((sha, size))
            if size == release.artifact.size
                && sha.eq_ignore_ascii_case(&release.artifact.sha256) => {}
        Ok(_) => {
            eprintln!("Moss update archive failed SHA-256 verification");
            let _ = fs::remove_file(&archive_path);
            return Status::Unverified;
        }
        Err(error) => {
            eprintln!("Moss update archive read error: {error}");
            let _ = fs::remove_file(&archive_path);
            return Status::Unverified;
        }
    }

    if client.shared.stopped.load(Ordering::Acquire) {
        return Status::DownloadFailed;
    }

    let app_path = match extract_archive(&archive_path, &version_dir) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Moss update extraction failed: {error}");
            let _ = fs::remove_dir_all(&version_dir);
            return Status::Unverified;
        }
    };

    if let Err(error) = verify_bundle(&app_path, release.version) {
        eprintln!("Moss update bundle verification failed: {error}");
        let _ = fs::remove_dir_all(&version_dir);
        return Status::Unverified;
    }

    let marker = StagedMarker {
        version: release.version,
        sha256: release.artifact.sha256.clone(),
        release_url: release.release_url.clone(),
    };
    if let Err(error) = write_staged_marker(&config.cache_dir, &marker) {
        eprintln!("Moss update marker write failed: {error}");
        return Status::DownloadFailed;
    }

    let staged = StagedUpdate {
        version: release.version,
        app_path,
        release_url: release.release_url.clone(),
        sha256: release.artifact.sha256.clone(),
    };
    *client.shared.staged.lock().unwrap() = Some(staged);
    prune_staged(&config.cache_dir, &[release.version]);
    Status::Ready(release.version)
}

fn run_check(client: &Client, manual: bool, download: bool) {
    let existing_staged = client.shared.staged.lock().unwrap().clone();

    let verified = match fetch_and_verify_manifest(client) {
        Ok(release) => release,
        Err(Status::Current) => {
            if let Some(staged) = existing_staged {
                client.show_with_url(Status::Ready(staged.version), Some(staged.release_url));
            } else if manual {
                client.show(Status::Current);
            } else {
                client.show(Status::Idle);
            }
            return;
        }
        Err(error_status) => {
            if let Some(staged) = existing_staged.filter(|_| !manual) {
                client.show_with_url(Status::Ready(staged.version), Some(staged.release_url));
            } else if manual {
                client.show(error_status);
            } else {
                client.show(Status::Idle);
            }
            return;
        }
    };

    if verified.version <= client.version {
        *client.shared.known_release.lock().unwrap() = None;
        *client.shared.staged.lock().unwrap() = None;
        prune_staged(&client.shared.config.cache_dir, &[]);
        client.show(if manual { Status::Current } else { Status::Idle });
        return;
    }

    *client.shared.known_release.lock().unwrap() = Some(verified.clone());

    // If this exact release is already staged and verified, keep or restore Ready immediately.
    if let Some(staged) = existing_staged
        .as_ref()
        .filter(|staged| {
            staged.version == verified.version
                && staged.sha256.eq_ignore_ascii_case(&verified.artifact.sha256)
        })
    {
        if verify_bundle(&staged.app_path, staged.version).is_ok() {
            client.show_with_url(Status::Ready(staged.version), Some(staged.release_url.clone()));
            return;
        }
    }

    if !client.automatic() && !download {
        client.show_with_url(
            Status::Available(verified.version),
            Some(verified.release_url),
        );
        return;
    }

    let status = download_and_stage(client, &verified);
    if !manual && matches!(status, Status::DownloadFailed) {
        if let Some(staged) = existing_staged {
            client.show_with_url(Status::Ready(staged.version), Some(staged.release_url));
            return;
        }
    }
    client.show_with_url(status, Some(verified.release_url));
}

#[cfg(test)]
#[path = "updater_tests.rs"]
mod tests;
