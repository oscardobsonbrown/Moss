use crate::state::State;
use rune::{Tx, Waker};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, DirBuilder, File},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

const REPOSITORY: &str = "oscardobsonbrown/Moss";
const BUNDLE_ID: &str = "app.moss.desktop";
const INSTALLER: &str = include_str!("../../../scripts/install-macos.sh");
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
pub type Version = [u64; 3];
static SAVE_FAILED: AtomicBool = AtomicBool::new(false);

pub fn defer_install() {
    SAVE_FAILED.store(true, Ordering::Relaxed);
}

fn version(input: &str) -> Option<Version> {
    let mut parts = input.split('.');
    let mut version = [0; 3];
    for number in &mut version {
        let part = parts.next()?;
        if part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        *number = part.parse().ok()?;
    }
    parts.next().is_none().then_some(version)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    digest: Option<String>,
    #[serde(default)]
    size: u64,
}

fn output(command: &mut Command) -> Result<Output, String> {
    let result = command.output().map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "{} exited with {}",
            command.get_program().to_string_lossy(),
            result.status
        ));
    }
    Ok(result)
}

fn plist(bundle: &Path, key: &str) -> Option<String> {
    let result = output(
        Command::new("/usr/libexec/PlistBuddy")
            .arg("-c")
            .arg(format!("Print :{key}"))
            .arg(bundle.join("Contents/Info.plist")),
    )
    .ok()?;
    Some(String::from_utf8(result.stdout).ok()?.trim().into())
}

fn installed_version(bundle: &Path) -> Option<Version> {
    if plist(bundle, "CFBundleIdentifier")?.as_str() != BUNDLE_ID {
        return None;
    }
    version(&plist(bundle, "CFBundleShortVersionString")?)
}

fn download(curl: &Path, timeout: &str) -> Command {
    let mut command = Command::new(curl);
    command.args([
        "--disable",
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "10",
        "--max-time",
        timeout,
        "--user-agent",
        "Moss-Updater",
    ]);
    command
}

fn verify_archive(archive: &Path, expected_digest: &str) -> Result<(), String> {
    let checksum = output(
        Command::new("/usr/bin/shasum")
            .args(["-a", "256"])
            .arg(archive),
    )?;
    let matches = std::str::from_utf8(&checksum.stdout)
        .ok()
        .and_then(|checksum| checksum.split_whitespace().next())
        .is_some_and(|checksum| checksum.eq_ignore_ascii_case(expected_digest));
    if !matches {
        return Err("Moss update archive does not match GitHub's checksum".into());
    }
    Ok(())
}

fn verify_bundle(bundle: &Path, next: Version) -> Result<(), String> {
    if bundle.is_symlink()
        || installed_version(bundle) != Some(next)
        || plist(bundle, "CFBundleExecutable").as_deref() != Some("Moss")
    {
        return Err("Moss update bundle has the wrong identity or version".into());
    }
    output(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(bundle),
    )?;
    Ok(())
}

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
}

pub fn version_label([major, minor, patch]: Version) -> String {
    format!("{major}.{minor}.{patch}")
}

pub fn release_url(version: Version) -> String {
    format!(
        "https://github.com/{REPOSITORY}/releases/tag/v{}",
        version_label(version)
    )
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub status: Status,
    pub updated: Option<Version>,
}

struct Shared {
    snapshot: Mutex<Snapshot>,
    wakers: Mutex<Vec<Waker>>,
    revision: AtomicU64,
    automatic: AtomicBool,
    restart: AtomicBool,
    stopped: AtomicBool,
}

#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    requests: Sender<bool>,
    pub version: Version,
}

impl Client {
    fn new(version: Version, automatic: bool, updated: Option<Version>) -> (Self, Receiver<bool>) {
        let (requests, inbox) = mpsc::channel();
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Snapshot {
                status: Status::Idle,
                updated,
            }),
            wakers: Mutex::new(Vec::new()),
            revision: AtomicU64::new(1),
            automatic: AtomicBool::new(automatic),
            restart: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
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
        *self.shared.snapshot.lock().unwrap()
    }

    fn wake(&self) {
        self.shared.revision.fetch_add(1, Ordering::Release);
        for wake in self.shared.wakers.lock().unwrap().iter() {
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

    pub fn dismiss(&self) {
        let mut view = self.shared.snapshot.lock().unwrap();
        match view.status {
            Status::DownloadFailed | Status::Unverified | Status::InstallLocation => {
                view.status = Status::Idle;
            }
            _ => view.updated = None,
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
            self.check(false);
        }
    }

    fn begin_check(&self) -> bool {
        let mut view = self.shared.snapshot.lock().unwrap();
        if matches!(
            view.status,
            Status::Checking
                | Status::Downloading(_)
                | Status::Ready(_)
                | Status::Restarting(_)
                | Status::SaveFailed(_)
        ) {
            return false;
        }
        view.status = Status::Checking;
        drop(view);
        self.wake();
        true
    }

    pub fn check(&self, download: bool) {
        if self.begin_check() && self.requests.send(download).is_err() {
            self.show(Status::CheckFailed);
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(version: Version, status: Status) -> Self {
        let (client, _) = Self::new(version, true, None);
        client.show(status);
        client
    }
}

#[derive(Default)]
pub struct Window {
    pub client: Option<Client>,
    seen: u64,
    toast: Snapshot,
}

impl Window {
    pub fn register(&self, wake: Waker) {
        if let Some(client) = &self.client {
            client.shared.wakers.lock().unwrap().push(wake);
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
    client.show(Status::Restarting(next));
    if !s.flush_current() {
        client.show(Status::SaveFailed(next));
        return;
    }
    let mut handles = s.handles.lock().unwrap().clone();
    let requester = handles.remove(&s.workspace.id);
    let Some(requester) = requester.filter(|_| !handles.is_empty()) else {
        finish_restart(s, tx, &client, next);
        return;
    };
    let remaining = Arc::new(AtomicUsize::new(handles.len()));
    let failed = Arc::new(AtomicBool::new(false));
    for window in handles.into_values() {
        let (client, remaining, failed, requester) = (
            client.clone(),
            remaining.clone(),
            failed.clone(),
            requester.clone(),
        );
        window.update(move |s, _| {
            if !s.flush_current() {
                failed.store(true, Ordering::Release);
            }
            if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                if failed.load(Ordering::Acquire) {
                    client.show(Status::SaveFailed(next));
                } else {
                    requester.update(move |s, tx| finish_restart(s, tx, &client, next));
                }
            }
        });
    }
}

fn finish_restart(s: &mut State, tx: &mut Tx, client: &Client, next: Version) {
    if !s.flush_current() {
        client.show(Status::SaveFailed(next));
        return;
    }
    s.save_preferences();
    SAVE_FAILED.store(false, Ordering::Relaxed);
    client.shared.restart.store(true, Ordering::Release);
    tx.quit();
}

fn latest(curl: &Path) -> Result<Release, String> {
    let metadata = output(download(curl, "30").args([
        "--header",
        "Accept: application/vnd.github+json",
        "--max-filesize",
        "4194304",
        &format!("https://api.github.com/repos/{REPOSITORY}/releases/latest"),
    ]))?;
    serde_json::from_slice(&metadata.stdout).map_err(|error| error.to_string())
}

struct Package<'a> {
    next: Version,
    name: &'a str,
    digest: &'a str,
    size: u64,
}

impl Release {
    fn package(&self, current: Version) -> Result<Option<Package<'_>>, Status> {
        let Some(next) = self.tag_name.strip_prefix('v').and_then(version) else {
            return Err(Status::Unverified);
        };
        if self.draft || self.prerelease || next <= current {
            return Ok(None);
        }
        let expected = format!("Moss-{}-macos-arm64.zip", self.tag_name);
        let archive = self
            .assets
            .iter()
            .find(|asset| asset.name == expected && asset.state == "uploaded")
            .ok_or(Status::Unverified)?;
        let digest = archive
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .filter(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or(Status::Unverified)?;
        let signed_asset = format!("{expected}.sig");
        if archive.size == 0
            || archive.size > 536_870_912
            || !self.assets.iter().any(|asset| {
                asset.name == signed_asset && asset.state == "uploaded" && asset.size == 64
            })
        {
            return Err(Status::Unverified);
        }
        Ok(Some(Package {
            next,
            name: &archive.name,
            digest,
            size: archive.size,
        }))
    }
}

fn fetch_file(
    curl: &Path,
    url: &str,
    destination: &Path,
    size: u64,
    client: &Client,
    progress: bool,
) -> Result<(), String> {
    let _ = fs::remove_file(destination);
    let mut process = download(curl, "600")
        .args(["--max-filesize", &size.to_string(), "--output"])
        .arg(destination)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    loop {
        match process.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(status)) => return Err(format!("Download exited with {status}")),
            Ok(None) => {}
            Err(error) => {
                let _ = process.kill();
                let _ = process.wait();
                return Err(error.to_string());
            }
        }
        if client.shared.stopped.load(Ordering::Acquire) {
            let _ = process.kill();
            let _ = process.wait();
            return Err("Moss is quitting".into());
        }
        if progress && let Ok(file) = destination.metadata() {
            client.show(Status::Downloading(
                (file.len().saturating_mul(100) / size).min(99) as u8,
            ));
        }
        thread::sleep(Duration::from_millis(120));
    }
    if destination
        .metadata()
        .map_err(|error| error.to_string())?
        .len()
        != size
    {
        return Err("Download has the wrong file size".into());
    }
    Ok(())
}

fn stage(
    curl: &Path,
    helper: &Path,
    key: &str,
    dir: &Path,
    release: &Package,
    client: &Client,
) -> Result<(), Status> {
    let archive = dir.join("update.zip");
    let signature = dir.join("update.sig");
    let url = format!(
        "https://github.com/{REPOSITORY}/releases/download/v{}/{}",
        version_label(release.next),
        release.name
    );
    fetch_file(curl, &format!("{url}.sig"), &signature, 64, client, false)
        .map_err(|_| Status::DownloadFailed)?;
    fetch_file(curl, &url, &archive, release.size, client, true)
        .map_err(|_| Status::DownloadFailed)?;
    client.show(Status::Downloading(100));
    verify_archive(&archive, release.digest).map_err(|_| Status::Unverified)?;
    output(
        Command::new(helper)
            .args(["verify", key])
            .arg(&signature)
            .arg(&archive),
    )
    .map_err(|_| Status::Unverified)?;
    if client.shared.stopped.load(Ordering::Acquire) {
        return Err(Status::DownloadFailed);
    }
    let bundle = dir.join("Moss.app");
    let _ = fs::remove_dir_all(&bundle);
    output(
        Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(&archive)
            .arg(dir),
    )
    .map_err(|_| Status::Unverified)?;
    verify_bundle(&bundle, release.next).map_err(|_| Status::Unverified)?;
    Ok(())
}

#[derive(Deserialize, Serialize)]
struct Installed {
    bundle: PathBuf,
    version: Version,
}

fn take_installed(path: &Path, bundle: &Path, current: Version) -> Option<Version> {
    let report = serde_json::from_slice::<Installed>(&fs::read(path).ok()?).ok()?;
    if report.bundle != bundle || report.version != current {
        return None;
    }
    fs::remove_file(path).ok()?;
    Some(report.version)
}

pub struct Updater {
    bundle: PathBuf,
    current: Version,
    helper: PathBuf,
    work: PathBuf,
    result: PathBuf,
    data: PathBuf,
    pub client: Client,
}

impl Updater {
    pub fn start(data: &Path, automatic: bool) -> Option<Self> {
        if option_env!("MOSS_RELEASE_BUILD") != Some("1")
            || std::env::args_os().len() != 1
            || std::env::var_os("MOSS_DISABLE_UPDATES").is_some()
        {
            return None;
        }
        let key = option_env!("MOSS_UPDATE_PUBLIC_KEY").filter(|key| !key.is_empty())?;
        let exe = std::env::current_exe().ok()?;
        let bundle = exe.ancestors().nth(3)?.to_owned();
        if bundle.extension()? != "app" || exe != bundle.join("Contents/MacOS/Moss") {
            return None;
        }
        let current = version(option_env!("MOSS_VERSION")?)?;
        let helper = bundle.join("Contents/MacOS/moss-update-helper");
        if !helper.is_file() {
            return None;
        }
        let result = data.join("update-result.json");
        let updated = take_installed(&result, &bundle, current);
        let work = std::env::temp_dir().join(format!("moss-update-{}", uuid::Uuid::new_v4()));
        DirBuilder::new().mode(0o700).create(&work).ok()?;
        let (client, inbox) = Client::new(current, automatic, updated);
        let (worker, staging, verifier, target) =
            (client.clone(), work.clone(), helper.clone(), bundle.clone());
        thread::spawn(move || {
            let mut scheduled = true;
            loop {
                let manual = if scheduled {
                    None
                } else {
                    match inbox.recv_timeout(CHECK_INTERVAL) {
                        Ok(force) => Some(force),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                };
                scheduled = false;
                if worker.shared.stopped.load(Ordering::Acquire) {
                    break;
                }
                if manual.is_none() && (!worker.automatic() || !worker.begin_check()) {
                    continue;
                }
                let status = check_update(&worker, manual, &target, &staging, &verifier, key);
                worker.show(status);
            }
        });
        Some(Self {
            bundle,
            current,
            helper,
            work,
            result,
            data: data.into(),
            client,
        })
    }

    fn install(&self, next: Version) -> Result<(), String> {
        let lock = File::open(self.bundle.join("Contents/MacOS/Moss"))
            .map_err(|error| error.to_string())?;
        lock.try_lock().map_err(|error| error.to_string())?;
        if installed_version(&self.bundle) != Some(self.current) {
            return Err("The installed version changed".into());
        }
        let source = self.work.join("Moss.app");
        verify_bundle(&source, next)?;
        output(
            Command::new("/bin/sh")
                .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .args(["-c", INSTALLER, "moss-update"])
                .arg(source)
                .arg(&self.bundle),
        )?;
        let result = serde_json::to_vec(&Installed {
            bundle: self.bundle.clone(),
            version: next,
        })
        .unwrap();
        let temp = self
            .result
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        if let Err(error) = fs::write(&temp, result).and_then(|_| fs::rename(&temp, &self.result)) {
            let _ = fs::remove_file(temp);
            eprintln!("Could not save the Moss update result: {error}");
        }
        Ok(())
    }

    fn relaunch(&self) {
        if let Err(error) = output(
            Command::new("/usr/bin/open")
                .args(["-n", "-a"])
                .arg(&self.bundle)
                .arg("--env")
                .arg(format!("MOSS_DATA_DIR={}", self.data.display())),
        ) {
            eprintln!("Could not reopen Moss: {error}");
        }
    }

    pub fn install_on_quit(self) {
        self.client.shared.stopped.store(true, Ordering::Release);
        let restart = self.client.shared.restart.load(Ordering::Acquire);
        if SAVE_FAILED.load(Ordering::Relaxed) {
            if restart {
                self.relaunch();
            }
            return;
        }
        let next = match self.client.snapshot().status {
            Status::Ready(next) => next,
            Status::Restarting(next) if restart => next,
            _ => return,
        };
        let recovery = self.work.join("moss-update-recovery");
        let helper = if fs::copy(&self.helper, &recovery).is_ok() {
            &recovery
        } else {
            &self.helper
        };
        match self.install(next) {
            Ok(()) if restart => self.relaunch(),
            Ok(()) => {}
            Err(error) => {
                eprintln!("Moss update install: {error}");
                if installed_version(&self.bundle).is_some_and(|version| version > self.current) {
                    if restart {
                        self.relaunch();
                    }
                    return;
                }
                if output(Command::new(helper).arg("recover").arg(release_url(next))).is_ok() {
                    self.relaunch();
                }
            }
        }
    }
}

fn check_update(
    worker: &Client,
    manual: Option<bool>,
    target: &Path,
    staging: &Path,
    verifier: &Path,
    key: &str,
) -> Status {
    let curl = Path::new("/usr/bin/curl");
    let release = match latest(curl) {
        Ok(release) => release,
        Err(error) => {
            eprintln!("Moss update check: {error}");
            return if manual.is_some() {
                Status::CheckFailed
            } else {
                Status::Idle
            };
        }
    };
    let package = match release.package(worker.version) {
        Ok(Some(package)) => package,
        Ok(None) => {
            return if manual.is_some() {
                Status::Current
            } else {
                Status::Idle
            };
        }
        Err(error) if manual.is_some() => return error,
        // A release can appear on GitHub before its signature is uploaded.
        Err(_) => return Status::Idle,
    };
    if !worker.automatic() && manual != Some(true) {
        return Status::Available(package.next);
    }
    let probe = target.with_file_name(format!(".moss-update-{}", uuid::Uuid::new_v4()));
    if fs::create_dir(&probe)
        .and_then(|_| fs::remove_dir(&probe))
        .is_err()
    {
        return Status::InstallLocation;
    }
    worker.show(Status::Downloading(0));
    match stage(curl, verifier, key, staging, &package, worker) {
        Ok(()) => Status::Ready(package.next),
        Err(error) => error,
    }
}

impl Drop for Updater {
    fn drop(&mut self) {
        self.client.shared.stopped.store(true, Ordering::Release);
        let _ = fs::remove_dir_all(&self.work);
    }
}

#[cfg(test)]
#[path = "updater_tests.rs"]
mod tests;
