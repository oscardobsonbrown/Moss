use super::TilePosition;
use moss_core::PlaceValue;
use serde_json::Value;
use std::{
    hash::{Hash, Hasher},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime},
};

fn valid_endpoint(value: &str) -> bool {
    value.starts_with("https://") && !value.chars().any(char::is_control)
}
pub(super) fn tile_template() -> String {
    std::env::var("MOSS_MAP_TILE_URL")
        .ok()
        .filter(|s| {
            valid_endpoint(s) && s.contains("{z}") && s.contains("{x}") && s.contains("{y}")
        })
        .unwrap_or_else(|| super::theme::PROVIDER.into())
}
pub(crate) fn attribution() -> &'static str {
    if tile_template() == super::theme::PROVIDER {
        "© OpenStreetMap contributors · © OpenMapTiles"
    } else {
        "© OpenStreetMap contributors"
    }
}
const MAX_TILE_BYTES: u64 = 2 * 1024 * 1024;
fn tile_path(dir: &std::path::Path, template: &str, tile: &TilePosition) -> std::path::PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    template.hash(&mut hasher);
    dir.join(format!(
        "{:016x}-{}-{}-{}.png",
        hasher.finish(),
        tile.z,
        tile.x,
        tile.y
    ))
}
fn read_tile(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_TILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_TILE_BYTES as usize
        || bytes.len() < 33
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes[8..12] != 13u32.to_be_bytes()
        || &bytes[12..16] != b"IHDR"
        || bytes[16..20] != 256u32.to_be_bytes()
        || bytes[20..24] != 256u32.to_be_bytes()
    {
        return Err("Invalid map tile.".into());
    }
    Ok(bytes)
}
pub(super) fn load_tile(
    dir: &std::path::Path,
    template: &str,
    tile: &TilePosition,
) -> Result<Vec<u8>, String> {
    if template == super::theme::PROVIDER {
        return super::theme::load(dir, tile);
    }
    let path = tile_path(dir, template, tile);
    let cached = read_tile(&path).ok();
    let fresh = std::fs::metadata(&path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|m| m.elapsed().ok())
        .is_some_and(|age| age < Duration::from_secs(7 * 86400));
    if fresh && cached.is_some() {
        return Ok(cached.unwrap());
    }
    if cfg!(test) || std::env::var("MOSS_MAP_NETWORK").as_deref() == Ok("off") {
        return cached.ok_or_else(|| "Map tiles are disabled in tests or offline.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if !path.exists() {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".png"))
            .take(513)
            .collect::<Vec<_>>();
        if entries.len() >= 512 {
            // Keep cached tiles for seven days. Only remove an expired entry.
            let expired = entries
                .into_iter()
                .filter_map(|entry| {
                    let modified = entry.metadata().ok()?.modified().ok()?;
                    (modified.elapsed().ok()? >= Duration::from_secs(7 * 86400))
                        .then_some((modified, entry.path()))
                })
                .min_by_key(|(modified, _)| *modified);
            if let Some((_, old)) = expired {
                std::fs::remove_file(old).map_err(|e| e.to_string())?;
            } else {
                return Err("Map tile cache is full.".into());
            }
        }
    }
    let url = template
        .replace("{z}", &tile.z.to_string())
        .replace("{x}", &tile.x.to_string())
        .replace("{y}", &tile.y.to_string());
    let temp = dir.join(format!(".tile-{}.tmp", moss_core::new_id()));
    let mut command = std::process::Command::new("curl");
    command
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-redirs",
            "3",
            "--max-time",
            "4",
            "--max-filesize",
            "2097152",
            "--user-agent",
            "Moss/0.1 (local desktop map)",
            "--output",
        ])
        .arg(&temp);
    if cached.is_some() {
        command.arg("--time-cond").arg(&path);
    }
    let result = command.arg(&url).output();
    if !result.is_ok_and(|output| output.status.success()) {
        let _ = std::fs::remove_file(temp);
        return cached.ok_or_else(|| "Map tiles are unavailable. Check your connection.".into());
    }
    if temp.exists()
        && std::fs::metadata(&temp)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        match read_tile(&temp) {
            Ok(bytes) => {
                if let Err(error) = std::fs::rename(&temp, &path) {
                    let _ = std::fs::remove_file(&temp);
                    return cached.ok_or_else(|| format!("Map tile is not cached: {error}"));
                }
                return Ok(bytes);
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                return cached.ok_or(error);
            }
        }
    }
    let _ = std::fs::remove_file(temp);
    if let Some(bytes) = cached {
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&path) {
            let _ = file.set_times(std::fs::FileTimes::new().set_modified(SystemTime::now()));
        }
        return Ok(bytes);
    }
    Err("Map tile is unavailable.".into())
}
pub(super) fn photon(query: &str) -> Result<Vec<PlaceValue>, String> {
    if cfg!(test) || std::env::var("MOSS_MAP_NETWORK").as_deref() == Ok("off") {
        return Err("Place search is disabled in tests.".into());
    }
    static LAST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let mut last = LAST.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if let Some(then) = *last {
        if let Some(wait) = Duration::from_secs(1).checked_sub(then.elapsed()) {
            std::thread::sleep(wait);
        }
    }
    *last = Some(Instant::now());
    let endpoint = std::env::var("MOSS_PLACE_SEARCH_URL")
        .ok()
        .filter(|s| valid_endpoint(s))
        .unwrap_or_else(|| "https://photon.komoot.io/api/".into());
    let output = std::process::Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-redirs",
            "3",
            "--max-time",
            "8",
            "--max-filesize",
            "2097152",
            "--user-agent",
            "Moss/0.1 (local desktop map)",
            "--get",
            "--data-urlencode",
        ])
        .arg(format!("q={query}"))
        .args(["--data-urlencode", "limit=10"])
        .arg(endpoint)
        .output()
        .map_err(|e| e.to_string())?;
    drop(last);
    if !output.status.success() {
        return Err("Place search is unavailable. Enter coordinates instead.".into());
    }
    let data: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Place search returned invalid data.".to_owned())?;
    let mut places = Vec::new();
    for feature in data["features"].as_array().into_iter().flatten().take(10) {
        let Some(coords) = feature["geometry"]["coordinates"].as_array() else {
            continue;
        };
        let (Some(lon), Some(lat)) = (
            coords.first().and_then(Value::as_f64),
            coords.get(1).and_then(Value::as_f64),
        ) else {
            continue;
        };
        let properties = &feature["properties"];
        let mut parts = Vec::new();
        for key in ["name", "city", "state", "country"] {
            if let Some(value) = properties[key].as_str() {
                if !value.is_empty() && !parts.contains(&value) {
                    parts.push(value);
                }
            }
        }
        let place = PlaceValue {
            name: parts.join(", "),
            latitude: lat,
            longitude: lon,
        };
        if place.valid() {
            places.push(place);
        }
    }
    Ok(places)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tile() -> TilePosition {
        TilePosition {
            z: 3,
            world_x: 2,
            size: 256.,
            x: 2,
            y: 4,
            left: 0.,
            top: 0.,
        }
    }
    fn header() -> Vec<u8> {
        let mut data = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        data.extend(256u32.to_be_bytes());
        data.extend(256u32.to_be_bytes());
        data.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
        data
    }
    #[test]
    fn offline_uses_expired_cached_tile_without_network() {
        let dir = std::env::temp_dir().join(format!("moss-offline-tile-{}", moss_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = tile_path(&dir, "https://example.invalid/{z}/{x}/{y}", &tile());
        let bytes = header();
        std::fs::write(&path, &bytes).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_times(
            std::fs::FileTimes::new()
                .set_modified(SystemTime::now() - Duration::from_secs(8 * 86400)),
        )
        .unwrap();
        assert_eq!(
            load_tile(&dir, "https://example.invalid/{z}/{x}/{y}", &tile()).unwrap(),
            bytes
        );
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn rejects_corrupt_wrong_dimensions_and_oversized_cache() {
        let dir = std::env::temp_dir().join(format!("moss-invalid-tile-{}", moss_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = tile_path(&dir, "https://example.invalid/{z}/{x}/{y}", &tile());
        for bytes in [
            vec![1, 2, 3],
            {
                let mut data = header();
                data[19] = 1;
                data
            },
            vec![0; MAX_TILE_BYTES as usize + 1],
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(load_tile(&dir, "https://example.invalid/{z}/{x}/{y}", &tile()).is_err());
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
