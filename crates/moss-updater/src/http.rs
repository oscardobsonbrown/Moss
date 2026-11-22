use crate::MAX_MANIFEST_BYTES;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("operation cancelled")]
    Cancelled,
    #[error("HTTP request failed with status {status}: {detail}")]
    Status { status: u16, detail: String },
    #[error("HTTP command failed: {0}")]
    Command(String),
    #[error("response exceeded size limit")]
    TooLarge,
    #[error("downloaded file size {actual} did not match expected {expected}")]
    SizeMismatch { actual: u64, expected: u64 },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ManifestFetch {
    NotModified,
    Modified { body: Vec<u8>, etag: Option<String> },
}

fn local_file_path(url: &str) -> Option<PathBuf> {
    url.strip_prefix("file://").map(PathBuf::from)
}

fn base_curl(curl: &Path, timeout_secs: u64, allow_local_urls: bool) -> Command {
    let mut command = Command::new(curl);
    command.args([
        "--disable",
        "--silent",
        "--show-error",
        "--location",
        "--connect-timeout",
        "10",
        "--max-time",
        &timeout_secs.to_string(),
        "--user-agent",
        "Moss-Updater/2",
    ]);
    if allow_local_urls {
        command.args(["--proto", "=https,http", "--proto-redir", "=https,http"]);
    } else {
        command.args(["--proto", "=https", "--proto-redir", "=https"]);
    }
    command
}

fn parse_etag(headers: &str) -> Option<String> {
    let mut last = None;
    for line in headers.lines() {
        let line = line.trim();
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("etag") {
                let value = value.trim();
                if !value.is_empty() {
                    last = Some(value.to_owned());
                }
            }
        }
    }
    last
}

pub fn fetch_manifest(
    curl: &Path,
    url: &str,
    etag: Option<&str>,
    allow_local_urls: bool,
    stop: &AtomicBool,
) -> Result<ManifestFetch, HttpError> {
    if stop.load(Ordering::Acquire) {
        return Err(HttpError::Cancelled);
    }
    if allow_local_urls && let Some(path) = local_file_path(url) {
        let meta = fs::metadata(&path)?;
        if meta.len() > MAX_MANIFEST_BYTES {
            return Err(HttpError::TooLarge);
        }
        let body = fs::read(&path)?;
        return Ok(ManifestFetch::Modified { body, etag: None });
    }

    let temp_dir = std::env::temp_dir().join(format!("moss-manifest-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;
    let headers_path = temp_dir.join("headers.txt");
    let body_path = temp_dir.join("body.json");

    let mut command = base_curl(curl, 30, allow_local_urls);
    command
        .args([
            "--max-filesize",
            &MAX_MANIFEST_BYTES.to_string(),
            "--dump-header",
        ])
        .arg(&headers_path)
        .arg("--output")
        .arg(&body_path)
        .args(["--write-out", "%{http_code}"]);
    if let Some(etag) = etag {
        command.args(["--header", &format!("If-None-Match: {etag}")]);
    }
    command.arg(url);

    let output = command
        .output()
        .map_err(|error| HttpError::Command(error.to_string()));
    let result = (|| -> Result<ManifestFetch, HttpError> {
        let output = output?;
        if stop.load(Ordering::Acquire) {
            return Err(HttpError::Cancelled);
        }
        let status_text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let status: u16 = status_text.parse().unwrap_or(0);
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(HttpError::Command(if detail.is_empty() {
                format!("curl exited with {}", output.status)
            } else {
                detail
            }));
        }
        match status {
            304 => Ok(ManifestFetch::NotModified),
            200..=299 => {
                let body = fs::read(&body_path)?;
                if body.len() as u64 > MAX_MANIFEST_BYTES {
                    return Err(HttpError::TooLarge);
                }
                let headers = fs::read_to_string(&headers_path).unwrap_or_default();
                Ok(ManifestFetch::Modified {
                    body,
                    etag: parse_etag(&headers),
                })
            }
            _ => {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                Err(HttpError::Status { status, detail })
            }
        }
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    result
}

fn copy_local_file<F>(
    source: &Path,
    part: &Path,
    final_path: &Path,
    expected_size: u64,
    stop: &AtomicBool,
    mut on_progress: F,
) -> Result<(), HttpError>
where
    F: FnMut(u8),
{
    let mut reader = File::open(source)?;
    let mut writer = File::create(part)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut copied = 0u64;
    let mut last_percent = 0u8;
    loop {
        if stop.load(Ordering::Acquire) {
            drop(writer);
            let _ = fs::remove_file(part);
            return Err(HttpError::Cancelled);
        }
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        writer.write_all(&buffer[..count])?;
        copied += count as u64;
        if copied > expected_size {
            drop(writer);
            let _ = fs::remove_file(part);
            return Err(HttpError::SizeMismatch {
                actual: copied,
                expected: expected_size,
            });
        }
        let percent = ((copied.saturating_mul(100)) / expected_size).min(99) as u8;
        if percent > last_percent {
            last_percent = percent;
            on_progress(percent);
        }
    }
    writer.flush()?;
    drop(writer);
    if copied != expected_size {
        let _ = fs::remove_file(part);
        return Err(HttpError::SizeMismatch {
            actual: copied,
            expected: expected_size,
        });
    }
    fs::rename(part, final_path)?;
    Ok(())
}

fn run_curl_download<F>(
    curl: &Path,
    url: &str,
    part: &Path,
    expected_size: u64,
    resume: bool,
    allow_local_urls: bool,
    stop: &AtomicBool,
    on_progress: &mut F,
) -> Result<u16, HttpError>
where
    F: FnMut(u8),
{
    let stderr_path = part.with_extension("stderr");
    let status_path = part.with_extension("status");
    let _ = fs::remove_file(&stderr_path);
    let _ = fs::remove_file(&status_path);
    let stderr_file = File::create(&stderr_path)?;
    let status_file = File::create(&status_path)?;

    let mut command = base_curl(curl, 600, allow_local_urls);
    command.args([
        "--max-filesize",
        &expected_size.to_string(),
        "--write-out",
        "%{http_code}",
    ]);
    if resume {
        command.args(["--continue-at", "-"]);
    }
    command
        .arg("--output")
        .arg(part)
        .arg(url)
        .stdout(Stdio::from(status_file))
        .stderr(Stdio::from(stderr_file));

    let mut child = command
        .spawn()
        .map_err(|error| HttpError::Command(error.to_string()))?;

    let mut last_percent = part
        .metadata()
        .ok()
        .map_or(0, |meta| (meta.len().saturating_mul(100) / expected_size).min(99) as u8);

    let exit = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(HttpError::Command(error.to_string()));
            }
        }
        if stop.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(HttpError::Cancelled);
        }
        if let Ok(meta) = part.metadata() {
            let percent = (meta.len().saturating_mul(100) / expected_size).min(99) as u8;
            if percent > last_percent {
                last_percent = percent;
                on_progress(percent);
            }
        }
        thread::sleep(Duration::from_millis(100));
    };

    let status_code = fs::read_to_string(&status_path)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or(0);
    let stderr_text = fs::read_to_string(&stderr_path).unwrap_or_default();
    let _ = fs::remove_file(&stderr_path);
    let _ = fs::remove_file(&status_path);

    let exit_status = exit?;
    if !exit_status.success() {
        if status_code == 416 {
            return Ok(416);
        }
        let detail = stderr_text.trim();
        return Err(HttpError::Command(if detail.is_empty() {
            format!("curl exited with {exit_status} (HTTP {status_code})")
        } else {
            detail.to_owned()
        }));
    }
    Ok(status_code)
}

pub fn download_file<F>(
    curl: &Path,
    url: &str,
    part_path: &Path,
    final_path: &Path,
    expected_size: u64,
    allow_local_urls: bool,
    stop: &AtomicBool,
    mut on_progress: F,
) -> Result<(), HttpError>
where
    F: FnMut(u8),
{
    if expected_size == 0 {
        return Err(HttpError::SizeMismatch {
            actual: 0,
            expected: 0,
        });
    }
    if let Some(parent) = part_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if allow_local_urls && let Some(source) = local_file_path(url) {
        return copy_local_file(&source, part_path, final_path, expected_size, stop, on_progress);
    }

    let existing_len = part_path.metadata().ok().map_or(0, |meta| meta.len());
    let should_resume = existing_len > 0 && existing_len < expected_size;
    if existing_len >= expected_size {
        let _ = fs::remove_file(part_path);
    }

    let status = run_curl_download(
        curl,
        url,
        part_path,
        expected_size,
        should_resume,
        allow_local_urls,
        stop,
        &mut on_progress,
    )?;

    let actual_size = part_path.metadata().ok().map_or(0, |meta| meta.len());
    if should_resume && (status == 416 || actual_size != expected_size) {
        // Server rejected range or file changed; retry cleanly from byte 0 once.
        let _ = fs::remove_file(part_path);
        let retry_status = run_curl_download(
            curl,
            url,
            part_path,
            expected_size,
            false,
            allow_local_urls,
            stop,
            &mut on_progress,
        )?;
        if !(200..=299).contains(&retry_status) {
            let _ = fs::remove_file(part_path);
            return Err(HttpError::Status {
                status: retry_status,
                detail: "download retry failed".into(),
            });
        }
    } else if !(200..=299).contains(&status) {
        let _ = fs::remove_file(part_path);
        return Err(HttpError::Status {
            status,
            detail: "download failed".into(),
        });
    }

    let final_size = part_path.metadata()?.len();
    if final_size != expected_size {
        let _ = fs::remove_file(part_path);
        return Err(HttpError::SizeMismatch {
            actual: final_size,
            expected: expected_size,
        });
    }
    fs::rename(part_path, final_path)?;
    Ok(())
}
