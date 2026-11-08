use super::{MAX_BYTES, PROVIDER, render, valid_svg};
use crate::map_view::TilePosition;
use serde_json::Value;
use std::{
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const SOURCE_URL: &str = "https://tiles.openfreemap.org/planet";
const CACHE_BYTES: u64 = 48_000_000;
const CACHE_FILES: usize = 512;
#[derive(Clone)]
struct Source {
    template: String,
    max_zoom: u8,
}

fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Map tile is too large.".into());
    }
    Ok(bytes)
}
fn fresh(path: &Path, days: u64) -> bool {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < Duration::from_secs(days * 86400))
}
fn source(bytes: &[u8]) -> Result<Source, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid map source response.")?;
    let template = value["tiles"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(Value::as_str)
        .ok_or("Missing map source URL.")?;
    let max_zoom = value["maxzoom"]
        .as_u64()
        .filter(|z| *z == 14)
        .ok_or("Unsupported map source zoom.")? as u8;
    if !template.starts_with("https://tiles.openfreemap.org/planet/")
        || !template.ends_with(".pbf")
        || !["{z}", "{x}", "{y}"]
            .iter()
            .all(|part| template.contains(part))
        || template.chars().any(char::is_control)
    {
        return Err("Invalid map source URL.".into());
    }
    Ok(Source {
        template: template.into(),
        max_zoom,
    })
}
fn download(url: &str, _dir: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut child = std::process::Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--compressed",
            "--header",
            "Accept-Encoding: gzip",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-redirs",
            "3",
            "--max-time",
            "5",
            "--max-filesize",
        ])
        .arg(limit.to_string())
        .args(["--user-agent", "Moss/0.1 (native local map; OpenFreeMap)"])
        .arg(url)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    // Bound the decoded body as well as curl's encoded transfer size.
    let mut bytes = Vec::new();
    let result = child
        .stdout
        .take()
        .ok_or("Map transfer output missing.")?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes);
    if result.is_err() || bytes.len() > limit {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Map tile is too large or could not be decoded.".into());
    }
    if !child.wait().is_ok_and(|status| status.success()) {
        return Err("Map tiles are unavailable. Check your connection.".into());
    }
    Ok(bytes)
}
fn trim(dir: &Path, incoming: usize, keep: &Path) -> Result<(), String> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("apple-v1-"))
        .filter_map(|e| {
            Some((
                e.metadata().ok()?.modified().ok()?,
                e.path(),
                e.metadata().ok()?.len(),
            ))
        })
        .take(CACHE_FILES + 1)
        .collect::<Vec<_>>();
    entries.sort_by_key(|e| e.0);
    let mut bytes = entries.iter().map(|e| e.2).sum::<u64>();
    let mut count = entries.len();
    for (_, path, size) in entries {
        if count < CACHE_FILES && bytes + incoming as u64 <= CACHE_BYTES {
            return Ok(());
        }
        if path == keep {
            continue;
        }
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
        bytes = bytes.saturating_sub(size);
        count -= 1;
    }
    if bytes + incoming as u64 > CACHE_BYTES {
        return Err("Map tile cache is full.".into());
    }
    Ok(())
}
fn write(dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
    trim(dir, bytes.len(), path)?;
    let temporary = dir.join(format!(".map-theme-{}.tmp", moss_core::new_id()));
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    let result = std::fs::rename(&temporary, path).map_err(|e| e.to_string());
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
fn resolve(dir: &Path) -> Result<Source, String> {
    static SOURCE: OnceLock<Mutex<Option<(Instant, Source)>>> = OnceLock::new();
    let mut current = SOURCE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "Map source lock failed.")?;
    if let Some((then, source)) = &*current {
        if then.elapsed() < Duration::from_secs(86400) {
            return Ok(source.clone());
        }
    }
    let path = dir.join("apple-v1-source.json");
    let cached = read(&path, 65536).ok().and_then(|b| source(&b).ok());
    if fresh(&path, 1) {
        if let Some(source) = cached.clone() {
            *current = Some((Instant::now(), source.clone()));
            return Ok(source);
        }
    }
    match download(SOURCE_URL, dir, 65536).and_then(|bytes| Ok((source(&bytes)?, bytes))) {
        Ok((source, bytes)) => {
            write(dir, &path, &bytes)?;
            *current = Some((Instant::now(), source.clone()));
            Ok(source)
        }
        Err(error) => cached.ok_or(error),
    }
}
fn rendered_path(dir: &Path, tile: &TilePosition) -> PathBuf {
    dir.join(format!("apple-v1-{}-{}-{}.svg", tile.z, tile.x, tile.y))
}
pub(crate) fn load(dir: &Path, tile: &TilePosition) -> Result<Vec<u8>, String> {
    // Serialize the themed disk budget and network work across Map/Dashboard loaders.
    static LOAD: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = LOAD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Map tile worker lock failed.")?;
    let path = rendered_path(dir, tile);
    let cached = read(&path, MAX_BYTES).ok().filter(|bytes| valid_svg(bytes));
    if fresh(&path, 7) {
        if let Some(bytes) = cached.clone() {
            return Ok(bytes);
        }
    }
    if cfg!(test) || std::env::var("MOSS_MAP_NETWORK").as_deref() == Ok("off") {
        return cached.ok_or_else(|| "Map tiles are disabled in tests or offline.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let result = (|| {
        let source = resolve(dir)?;
        let z = tile.z.min(source.max_zoom);
        let shift = tile.z - z;
        let (x, y) = (tile.x >> shift, tile.y >> shift);
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        source.template.hash(&mut hash);
        PROVIDER.hash(&mut hash);
        let raw_path = dir.join(format!(
            "apple-v1-source-{:016x}-{z}-{x}-{y}.pbf",
            hash.finish()
        ));
        let old = read(&raw_path, MAX_BYTES).ok();
        let bytes = if fresh(&raw_path, 7) && old.is_some() {
            old.unwrap()
        } else {
            let url = source
                .template
                .replace("{z}", &z.to_string())
                .replace("{x}", &x.to_string())
                .replace("{y}", &y.to_string());
            match download(&url, dir, MAX_BYTES) {
                Ok(bytes) => {
                    super::mvt::decode(&bytes)?;
                    write(dir, &raw_path, &bytes)?;
                    bytes
                }
                Err(error) => old.ok_or(error)?,
            }
        };
        let bytes = render(&bytes, tile)?;
        write(dir, &path, &bytes)?;
        Ok(bytes)
    })();
    result.or_else(|error| cached.ok_or(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_rejects_other_hosts_and_invalid_zoom() {
        assert!(
            source(br#"{"tiles":["https://evil.invalid/{z}/{x}/{y}.pbf"],"maxzoom":14}"#).is_err()
        );
        assert!(source(br#"{"tiles":["https://tiles.openfreemap.org/planet/test/{z}/{x}/{y}.pbf"],"maxzoom":19}"#).is_err());
    }
    #[test]
    fn offline_retains_only_valid_generated_svg_and_cache_is_bounded() {
        let directory =
            std::env::temp_dir().join(format!("moss-themed-cache-{}", moss_core::new_id()));
        std::fs::create_dir_all(&directory).unwrap();
        let tile = TilePosition {
            z: 3,
            x: 2,
            y: 4,
            world_x: 2,
            size: 256.,
            left: 0.,
            top: 0.,
        };
        let path = rendered_path(&directory, &tile);
        let bytes = format!(
            "{}<rect width=\"256\" height=\"256\" fill=\"#a2daf2\"/></svg>",
            super::super::SVG_START
        )
        .into_bytes();
        write(&directory, &path, &bytes).unwrap();
        assert_eq!(load(&directory, &tile).unwrap(), bytes);
        std::fs::write(&path, b"<svg><script/></svg>").unwrap();
        assert!(load(&directory, &tile).is_err());
        for i in 0..512 {
            std::fs::write(directory.join(format!("apple-v1-{i}.svg")), b"old").unwrap();
        }
        trim(&directory, 1, &directory.join("new")).unwrap();
        assert!(std::fs::read_dir(&directory).unwrap().count() < 512);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
