use crate::{
    BUNDLE_ID,
    bundle::verify_bundle,
    version::{Version, format_version, parse_version},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, DirBuilder},
    io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedManifest {
    pub etag: Option<String>,
    pub manifest_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedMarker {
    pub version: Version,
    pub sha256: String,
    pub release_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InstallOutcome {
    Succeeded,
    Failed { reason: String, release_url: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub bundle: PathBuf,
    pub version: Version,
    pub outcome: InstallOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedUpdate {
    pub version: Version,
    pub app_path: PathBuf,
    pub release_url: String,
    pub sha256: String,
}

pub fn default_cache_dir(data_dir: &Path, custom_data_dir: bool) -> PathBuf {
    if custom_data_dir {
        return data_dir.join("update-cache");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join("Library/Caches").join(BUNDLE_ID).join("updates")
}

pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    let write_result = fs::write(&temp, bytes).and_then(|()| fs::rename(&temp, path));
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

pub fn manifest_cache_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("manifest-cache.json")
}

pub fn receipt_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("install-result.json")
}

pub fn install_lock_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("install.lock")
}

pub fn install_log_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("last-install.log")
}

pub fn runner_helper_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("runner").join(crate::HELPER_NAME)
}

pub fn staged_root(cache_dir: &Path) -> PathBuf {
    cache_dir.join("staged")
}

pub fn staged_version_dir(cache_dir: &Path, version: Version) -> PathBuf {
    staged_root(cache_dir).join(format_version(version))
}

pub fn staged_marker_path(cache_dir: &Path, version: Version) -> PathBuf {
    staged_version_dir(cache_dir, version).join("ready.json")
}

pub fn staged_app_path(cache_dir: &Path, version: Version) -> PathBuf {
    staged_version_dir(cache_dir, version).join("Moss.app")
}

pub fn staged_archive_path(cache_dir: &Path, version: Version) -> PathBuf {
    staged_version_dir(cache_dir, version).join("archive.zip")
}

pub fn staged_part_path(cache_dir: &Path, version: Version) -> PathBuf {
    staged_version_dir(cache_dir, version).join("archive.zip.part")
}

pub fn load_cached_manifest(cache_dir: &Path) -> Option<CachedManifest> {
    let bytes = fs::read(manifest_cache_path(cache_dir)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save_cached_manifest(cache_dir: &Path, entry: &CachedManifest) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(entry).map_err(io::Error::other)?;
    atomic_write(&manifest_cache_path(cache_dir), &bytes)
}

pub fn write_staged_marker(cache_dir: &Path, marker: &StagedMarker) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(marker).map_err(io::Error::other)?;
    atomic_write(&staged_marker_path(cache_dir, marker.version), &bytes)
}

pub fn load_verified_staged(cache_dir: &Path, version: Version) -> Option<StagedUpdate> {
    let marker: StagedMarker =
        serde_json::from_slice(&fs::read(staged_marker_path(cache_dir, version)).ok()?).ok()?;
    if marker.version != version {
        return None;
    }
    let app_path = staged_app_path(cache_dir, version);
    verify_bundle(&app_path, version).ok()?;
    Some(StagedUpdate {
        version,
        app_path,
        release_url: marker.release_url,
        sha256: marker.sha256,
    })
}

pub fn find_best_staged(cache_dir: &Path, current: Version) -> Option<StagedUpdate> {
    let root = staged_root(cache_dir);
    let entries = fs::read_dir(&root).ok()?;
    let mut best: Option<StagedUpdate> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(version) = name.to_str().and_then(parse_version) else {
            continue;
        };
        if version <= current {
            continue;
        }
        if let Some(staged) = load_verified_staged(cache_dir, version) {
            if best
                .as_ref()
                .is_none_or(|existing| staged.version > existing.version)
            {
                best = Some(staged);
            }
        }
    }
    best
}

pub fn prune_staged(cache_dir: &Path, keep: &[Version]) {
    let root = staged_root(cache_dir);
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let keep_entry = name
            .to_str()
            .and_then(parse_version)
            .is_some_and(|version| keep.contains(&version));
        if !keep_entry {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

pub fn write_receipt(path: &Path, receipt: &InstallReceipt) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(receipt).map_err(io::Error::other)?;
    atomic_write(path, &bytes)
}

pub fn take_receipt(path: &Path, bundle: &Path) -> Option<InstallReceipt> {
    let receipt: InstallReceipt = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    if receipt.bundle != bundle {
        return None;
    }
    let _ = fs::remove_file(path);
    Some(receipt)
}
