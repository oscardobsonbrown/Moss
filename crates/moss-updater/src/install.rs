use crate::{
    bundle::{BundleError, installed_version, run_checked, strip_quarantine, verify_bundle},
    cache::{
        InstallOutcome, InstallReceipt, ensure_private_dir, install_lock_path, install_log_path,
        prune_staged, runner_helper_path, write_receipt,
    },
    version::{Version, default_release_url, format_version},
};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

const PARENT_EXIT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("bundle error: {0}")]
    Bundle(#[from] BundleError),
    #[error("timed out waiting for Moss (pid {0}) to exit")]
    ParentExitTimeout(u32),
    #[error("another Moss update installation is already in progress")]
    LockBusy,
    #[error("installed Moss version changed before swap")]
    VersionChanged,
    #[error("target bundle does not have a valid parent directory")]
    InvalidTarget,
    #[error("could not replace app bundle and restored the previous version: {0}")]
    SwapRolledBack(String),
    #[error("could not replace app bundle and rollback failed ({rollback}): {swap}")]
    SwapAndRollbackFailed { swap: String, rollback: String },
}

#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub parent_pid: u32,
    pub staged_app: PathBuf,
    pub target_app: PathBuf,
    pub expected_current: Version,
    pub next_version: Version,
    pub release_url: String,
    pub receipt_path: PathBuf,
    pub cache_dir: PathBuf,
    pub data_dir: Option<PathBuf>,
    pub relaunch: bool,
}

pub fn spawn_detached_installer(
    helper_src: &Path,
    request: &InstallRequest,
) -> Result<(), InstallError> {
    ensure_private_dir(&request.cache_dir)?;
    let runner_path = runner_helper_path(&request.cache_dir);
    if let Some(parent) = runner_path.parent() {
        ensure_private_dir(parent)?;
    }
    let temp_runner = runner_path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    fs::copy(helper_src, &temp_runner)?;
    fs::set_permissions(&temp_runner, fs::Permissions::from_mode(0o755))?;
    fs::rename(&temp_runner, &runner_path)?;

    let log_path = install_log_path(&request.cache_dir);
    let log_out = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;
    let log_err = log_out.try_clone()?;

    let mut command = Command::new(&runner_path);
    command
        .arg("install")
        .arg("--pid")
        .arg(request.parent_pid.to_string())
        .arg("--staged-app")
        .arg(&request.staged_app)
        .arg("--target-app")
        .arg(&request.target_app)
        .arg("--expected-current")
        .arg(format_version(request.expected_current))
        .arg("--next-version")
        .arg(format_version(request.next_version))
        .arg("--release-url")
        .arg(&request.release_url)
        .arg("--receipt")
        .arg(&request.receipt_path)
        .arg("--cache-dir")
        .arg(&request.cache_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_out))
        .stderr(Stdio::from(log_err));

    if let Some(data_dir) = &request.data_dir {
        command.arg("--data-dir").arg(data_dir);
    }
    if !request.relaunch {
        command.arg("--no-relaunch");
    }

    // SAFETY: setsid is async-signal-safe and detaches the installer helper into its own session
    // so it survives the parent app's termination cleanly.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    command.spawn()?;
    Ok(())
}

fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: kill(pid, 0) checks process existence without sending a signal.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error().raw_os_error();
    err == Some(libc::EPERM)
}

fn wait_for_parent_exit(pid: u32, timeout: Duration) -> Result<(), InstallError> {
    let start = Instant::now();
    while process_alive(pid) {
        if start.elapsed() >= timeout {
            return Err(InstallError::ParentExitTimeout(pid));
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

struct ExclusiveLock {
    file: File,
}

impl ExclusiveLock {
    fn acquire(path: &Path) -> Result<Self, InstallError> {
        if let Some(parent) = path.parent() {
            ensure_private_dir(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;
        file.try_lock().map_err(|_| InstallError::LockBusy)?;
        Ok(Self { file })
    }
}

impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub fn relaunch_bundle(bundle: &Path, data_dir: Option<&Path>) -> Result<(), BundleError> {
    let mut command = Command::new("/usr/bin/open");
    command.args(["-n", "-a"]).arg(bundle);
    if let Some(data_dir) = data_dir {
        command
            .arg("--env")
            .arg(format!("MOSS_DATA_DIR={}", data_dir.display()));
    }
    run_checked(&mut command)?;
    Ok(())
}

fn perform_atomic_swap(
    staged_app: &Path,
    target_app: &Path,
    next_version: Version,
) -> Result<(), InstallError> {
    let parent = target_app.parent().ok_or(InstallError::InvalidTarget)?;
    let work_dir = parent.join(format!(".moss-install-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&work_dir)?;

    let staged_copy = work_dir.join("Moss.app");
    let backup_app = work_dir.join("previous.app");

    let result = (|| -> Result<(), InstallError> {
        run_checked(Command::new("/usr/bin/ditto").arg(staged_app).arg(&staged_copy))?;
        strip_quarantine(&staged_copy);
        verify_bundle(&staged_copy, next_version)?;

        let target_exists = fs::symlink_metadata(target_app).is_ok();
        if target_exists {
            fs::rename(target_app, &backup_app)?;
        }

        if let Err(swap_error) = fs::rename(&staged_copy, target_app) {
            if target_exists {
                if let Err(rollback_error) = fs::rename(&backup_app, target_app) {
                    return Err(InstallError::SwapAndRollbackFailed {
                        swap: swap_error.to_string(),
                        rollback: rollback_error.to_string(),
                    });
                }
            }
            return Err(InstallError::SwapRolledBack(swap_error.to_string()));
        }
        Ok(())
    })();

    let _ = fs::remove_dir_all(&work_dir);
    result
}

pub fn run_detached_install(request: &InstallRequest) -> Result<(), InstallError> {
    let mut stderr = std::io::stderr();
    let _ = writeln!(
        stderr,
        "moss-update-helper: installing {} -> {} for {}",
        format_version(request.expected_current),
        format_version(request.next_version),
        request.target_app.display()
    );

    let outcome = (|| -> Result<(), InstallError> {
        wait_for_parent_exit(request.parent_pid, PARENT_EXIT_TIMEOUT)?;
        let _lock = ExclusiveLock::acquire(&install_lock_path(&request.cache_dir))?;

        if let Some(current) = installed_version(&request.target_app) {
            if current >= request.next_version {
                let _ = writeln!(
                    stderr,
                    "moss-update-helper: target already at {}",
                    format_version(current)
                );
                return Ok(());
            }
            if current != request.expected_current {
                return Err(InstallError::VersionChanged);
            }
        } else {
            return Err(InstallError::VersionChanged);
        }

        verify_bundle(&request.staged_app, request.next_version)?;
        perform_atomic_swap(&request.staged_app, &request.target_app, request.next_version)?;
        prune_staged(&request.cache_dir, &[]);
        Ok(())
    })();

    match outcome {
        Ok(()) => {
            let receipt = InstallReceipt {
                bundle: request.target_app.clone(),
                version: request.next_version,
                outcome: InstallOutcome::Succeeded,
            };
            if let Err(error) = write_receipt(&request.receipt_path, &receipt) {
                let _ = writeln!(stderr, "moss-update-helper: failed to write receipt: {error}");
            }
            if request.relaunch {
                if let Err(error) = relaunch_bundle(&request.target_app, request.data_dir.as_deref()) {
                    let _ = writeln!(stderr, "moss-update-helper: failed to relaunch Moss: {error}");
                }
            }
            Ok(())
        }
        Err(error) => {
            let _ = writeln!(stderr, "moss-update-helper: install failed: {error}");
            let release_url = if request.release_url.is_empty() {
                default_release_url(request.next_version)
            } else {
                request.release_url.clone()
            };
            let receipt = InstallReceipt {
                bundle: request.target_app.clone(),
                version: request.next_version,
                outcome: InstallOutcome::Failed {
                    reason: error.to_string(),
                    release_url,
                },
            };
            if let Err(write_error) = write_receipt(&request.receipt_path, &receipt) {
                let _ = writeln!(
                    stderr,
                    "moss-update-helper: failed to write failure receipt: {write_error}"
                );
            }
            if request.relaunch && request.target_app.exists() {
                let _ = relaunch_bundle(&request.target_app, request.data_dir.as_deref());
            }
            Err(error)
        }
    }
}
