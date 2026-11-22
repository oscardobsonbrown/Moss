use moss_updater::{
    BUNDLE_ID, MANIFEST_SCHEMA,
    bundle::{installed_version, run_checked, verify_bundle},
    cache::{
        InstallOutcome, StagedMarker, find_best_staged, prune_staged, receipt_path,
        staged_app_path, take_receipt, write_staged_marker,
    },
    crypto::{derive_public_key, generate_keypair, sha256_file},
    install::{InstallRequest, spawn_detached_installer},
    manifest::{Manifest, ManifestError, ManifestPayload, PlatformArtifact},
    version::{format_version, is_os_compatible, parse_version},
};
use std::{
    collections::BTreeMap,
    fs::{self, DirBuilder},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("moss-updater-int-{}", uuid::Uuid::new_v4()));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_signed_bundle(app: &Path, version: &str, marker: &str) {
    let binary = app.join("Contents/MacOS/Moss");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, format!("#!/bin/sh\necho '{marker}'\n")).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        app.join("Contents/Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
<key>CFBundleExecutable</key><string>Moss</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
</dict></plist>"#
        ),
    )
    .unwrap();
    run_checked(
        Command::new("/usr/bin/codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(app),
    )
    .unwrap();
}

#[test]
fn manifest_signing_verification_and_rejection_rules() {
    let pair = generate_keypair().unwrap();
    assert_eq!(derive_public_key(&pair.private_key_b64).unwrap(), pair.public_key_b64);
    let other_pair = generate_keypair().unwrap();

    let mut platforms = BTreeMap::new();
    platforms.insert(
        "macos-arm64".into(),
        PlatformArtifact {
            url: "https://github.com/oscardobsonbrown/Moss/releases/download/v0.12.0/Moss-v0.12.0-macos-arm64.zip".into(),
            sha256: "ab".repeat(32),
            size: 1024,
        },
    );
    let payload = ManifestPayload {
        schema: MANIFEST_SCHEMA,
        version: "0.12.0".into(),
        published_at: "2026-11-22T08:00:00Z".into(),
        release_url: "https://github.com/oscardobsonbrown/Moss/releases/tag/v0.12.0".into(),
        min_macos: Some("14.0".into()),
        platforms,
    };

    let manifest = Manifest::sign(payload.clone(), &pair.private_key_b64).unwrap();
    let verified = manifest
        .verify(&pair.public_key_b64, "macos-arm64", Some([15, 0, 0]), false)
        .unwrap();
    assert_eq!(verified.version, [0, 12, 0]);
    assert_eq!(verified.artifact.size, 1024);

    // Wrong public key is rejected.
    assert!(matches!(
        manifest.verify(&other_pair.public_key_b64, "macos-arm64", Some([15, 0, 0]), false),
        Err(ManifestError::Crypto(_))
    ));

    // Tampered payload field is rejected.
    let mut tampered = manifest.clone();
    tampered.payload.version = "0.13.0".into();
    assert!(matches!(
        tampered.verify(&pair.public_key_b64, "macos-arm64", Some([15, 0, 0]), false),
        Err(ManifestError::Crypto(_))
    ));

    // Missing platform is rejected.
    assert!(matches!(
        manifest.verify(&pair.public_key_b64, "macos-x64", Some([15, 0, 0]), false),
        Err(ManifestError::UnsupportedPlatform(_))
    ));

    // Older macOS is rejected cleanly as UnsupportedOs.
    assert!(matches!(
        manifest.verify(&pair.public_key_b64, "macos-arm64", Some([13, 6, 0]), false),
        Err(ManifestError::UnsupportedOs(_))
    ));

    // HTTP URL is rejected unless allow_local_urls is enabled.
    let mut local_payload = payload;
    local_payload.platforms.get_mut("macos-arm64").unwrap().url =
        "http://127.0.0.1:8080/Moss.zip".into();
    let local_manifest = Manifest::sign(local_payload, &pair.private_key_b64).unwrap();
    assert!(matches!(
        local_manifest.verify(&pair.public_key_b64, "macos-arm64", Some([15, 0, 0]), false),
        Err(ManifestError::InvalidUrl(_))
    ));
    assert!(
        local_manifest
            .verify(&pair.public_key_b64, "macos-arm64", Some([15, 0, 0]), true)
            .is_ok()
    );
}

#[test]
fn version_and_os_parsing_helpers() {
    assert_eq!(parse_version("0.11.0"), Some([0, 11, 0]));
    assert_eq!(format_version([1, 2, 3]), "1.2.3");
    assert_eq!(parse_version("0.01.0"), None);
    assert_eq!(parse_version("0.11.0-rc1"), None);
    assert!(is_os_compatible(Some("14.0"), Some([14, 0, 0])));
    assert!(is_os_compatible(Some("14.0"), Some([15, 1, 2])));
    assert!(!is_os_compatible(Some("15.0"), Some([14, 7, 1])));
    assert!(is_os_compatible(None, Some([13, 0, 0])));
}

#[cfg(target_os = "macos")]
#[test]
fn detached_helper_waits_for_parent_swaps_atomically_and_writes_receipt() {
    let dir = TestDir::new();
    let root = &dir.0;
    let cache = root.join("cache");
    let target_app = root.join("Installed.app");
    let staged_app = staged_app_path(&cache, [0, 12, 0]);
    let data_file = root.join("workspace.sqlite3");

    make_signed_bundle(&target_app, "0.11.0", "old-version");
    make_signed_bundle(&staged_app, "0.12.0", "new-version");
    fs::write(&data_file, "important workspace data").unwrap();
    write_staged_marker(
        &cache,
        &StagedMarker {
            version: [0, 12, 0],
            sha256: "cd".repeat(32),
            release_url: "https://github.com/oscardobsonbrown/Moss/releases/tag/v0.12.0".into(),
        },
    )
    .unwrap();

    // Persistent staged update discovery finds 0.12.0.
    let best = find_best_staged(&cache, [0, 11, 0]).unwrap();
    assert_eq!(best.version, [0, 12, 0]);

    // Spawn a short-lived dummy parent process so the detached helper must wait for it to exit.
    let mut dummy_parent = Command::new("/bin/sleep").arg("0.3").spawn().unwrap();
    let parent_pid = dummy_parent.id();

    let helper_bin = PathBuf::from(env!("CARGO_BIN_EXE_moss-update-helper"));
    let receipt = receipt_path(&cache);
    let request = InstallRequest {
        parent_pid,
        staged_app: staged_app.clone(),
        target_app: target_app.clone(),
        expected_current: [0, 11, 0],
        next_version: [0, 12, 0],
        release_url: "https://github.com/oscardobsonbrown/Moss/releases/tag/v0.12.0".into(),
        receipt_path: receipt.clone(),
        cache_dir: cache.clone(),
        data_dir: None,
        relaunch: false,
    };

    spawn_detached_installer(&helper_bin, &request).unwrap();
    assert!(dummy_parent.wait().unwrap().success());

    let deadline = Instant::now() + Duration::from_secs(10);
    while !receipt.is_file() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for detached installer receipt"
        );
        thread::sleep(Duration::from_millis(50));
    }

    let taken = take_receipt(&receipt, &target_app).unwrap();
    assert_eq!(taken.version, [0, 12, 0]);
    assert_eq!(taken.outcome, InstallOutcome::Succeeded);
    assert_eq!(installed_version(&target_app), Some([0, 12, 0]));
    verify_bundle(&target_app, [0, 12, 0]).unwrap();
    assert_eq!(
        fs::read_to_string(&data_file).unwrap(),
        "important workspace data"
    );
    assert!(take_receipt(&receipt, &target_app).is_none());
    // Staged directory was pruned after install.
    assert!(find_best_staged(&cache, [0, 11, 0]).is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn detached_helper_rejects_tampered_staged_bundle_and_preserves_existing_app() {
    let dir = TestDir::new();
    let root = &dir.0;
    let cache = root.join("cache");
    let target_app = root.join("Installed.app");
    let staged_app = staged_app_path(&cache, [0, 12, 0]);

    make_signed_bundle(&target_app, "0.11.0", "old-version");
    make_signed_bundle(&staged_app, "0.12.0", "new-version");
    // Tamper after signing.
    fs::write(staged_app.join("Contents/MacOS/Moss"), "#!/bin/sh\necho tampered\n").unwrap();

    let helper_bin = PathBuf::from(env!("CARGO_BIN_EXE_moss-update-helper"));
    let receipt = receipt_path(&cache);
    let request = InstallRequest {
        parent_pid: 0,
        staged_app,
        target_app: target_app.clone(),
        expected_current: [0, 11, 0],
        next_version: [0, 12, 0],
        release_url: "https://github.com/oscardobsonbrown/Moss/releases/tag/v0.12.0".into(),
        receipt_path: receipt.clone(),
        cache_dir: cache.clone(),
        data_dir: None,
        relaunch: false,
    };

    spawn_detached_installer(&helper_bin, &request).unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    while !receipt.is_file() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for failure receipt"
        );
        thread::sleep(Duration::from_millis(50));
    }

    let taken = take_receipt(&receipt, &target_app).unwrap();
    assert_eq!(taken.version, [0, 12, 0]);
    assert!(matches!(taken.outcome, InstallOutcome::Failed { .. }));
    // Original app is completely untouched and still valid.
    assert_eq!(installed_version(&target_app), Some([0, 11, 0]));
    verify_bundle(&target_app, [0, 11, 0]).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn helper_cli_generates_and_verifies_signed_manifest() {
    let dir = TestDir::new();
    let root = &dir.0;
    let helper_bin = PathBuf::from(env!("CARGO_BIN_EXE_moss-update-helper"));
    let pair = generate_keypair().unwrap();

    let archive = root.join("Moss-v0.12.0-macos-arm64.zip");
    fs::write(&archive, b"test-zip-contents").unwrap();
    let manifest = root.join("latest.json");

    run_checked(
        Command::new(&helper_bin)
            .env("TEST_SIGNING_KEY", &pair.private_key_b64)
            .args([
                "manifest",
                "--version",
                "0.12.0",
                "--archive",
            ])
            .arg(&archive)
            .args([
                "--url",
                "https://github.com/oscardobsonbrown/Moss/releases/download/v0.12.0/Moss-v0.12.0-macos-arm64.zip",
                "--signing-key-env",
                "TEST_SIGNING_KEY",
                "--output",
            ])
            .arg(&manifest),
    )
    .unwrap();

    let pubkey_out = run_checked(
        Command::new(&helper_bin)
            .env("TEST_SIGNING_KEY", &pair.private_key_b64)
            .args(["pubkey", "--signing-key-env", "TEST_SIGNING_KEY"]),
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(pubkey_out.stdout).unwrap().trim(),
        pair.public_key_b64
    );

    run_checked(
        Command::new(&helper_bin)
            .args(["verify-manifest", "--public-key", &pair.public_key_b64, "--manifest"])
            .arg(&manifest)
            .arg("--archive")
            .arg(&archive),
    )
    .unwrap();

    let (sha, size) = sha256_file(&archive).unwrap();
    let parsed = Manifest::from_json(&fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(parsed.payload.platforms["macos-arm64"].sha256, sha);
    assert_eq!(parsed.payload.platforms["macos-arm64"].size, size);

    // Pruning keeps requested version only.
    let cache = root.join("prune-cache");
    make_signed_bundle(&staged_app_path(&cache, [0, 12, 0]), "0.12.0", "a");
    make_signed_bundle(&staged_app_path(&cache, [0, 13, 0]), "0.13.0", "b");
    prune_staged(&cache, &[[0, 13, 0]]);
    assert!(!staged_app_path(&cache, [0, 12, 0]).exists());
    assert!(staged_app_path(&cache, [0, 13, 0]).exists());
}

#[test]
fn http_manifest_etag_304_and_resumable_download() {
    use moss_updater::http::{ManifestFetch, download_file, fetch_manifest};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, atomic::AtomicBool},
    };

    let dir = TestDir::new();
    let root = &dir.0;
    let payload_bytes: Vec<u8> = (0..128 * 1024).map(|i| (i % 251) as u8).collect();
    let manifest_body = br#"{"schema":1}"#.to_vec();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_payload = payload_bytes.clone();
    let server_manifest = manifest_body.clone();
    let server = thread::spawn(move || {
        // Handle 5 sequential requests:
        // 1) GET /latest.json -> 200 with ETag
        // 2) GET /latest.json with If-None-Match -> 304
        // 3) GET /archive.zip with Range -> 206 Partial Content
        // 4) GET /archive-416.zip with Range -> 416 Range Not Satisfiable
        // 5) GET /archive-416.zip without Range -> 200 OK
        for _ in 0..5 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req_buf = [0u8; 4096];
            let n = stream.read(&mut req_buf).unwrap();
            let req = String::from_utf8_lossy(&req_buf[..n]);
            let first_line = req.lines().next().unwrap_or("");
            let path = first_line.split_whitespace().nth(1).unwrap_or("/");
            let if_none_match = req.lines().find_map(|line| {
                let (k, v) = line.split_once(':')?;
                k.eq_ignore_ascii_case("if-none-match").then(|| v.trim().to_owned())
            });
            let range_start = req.lines().find_map(|line| {
                let (k, v) = line.split_once(':')?;
                if !k.eq_ignore_ascii_case("range") {
                    return None;
                }
                v.trim()
                    .strip_prefix("bytes=")?
                    .strip_suffix('-')?
                    .parse::<usize>()
                    .ok()
            });

            match path {
                "/latest.json" if if_none_match.as_deref() == Some("\"moss-etag-1\"") => {
                    let resp = "HTTP/1.1 304 Not Modified\r\nETag: \"moss-etag-1\"\r\nConnection: close\r\n\r\n";
                    stream.write_all(resp.as_bytes()).unwrap();
                }
                "/latest.json" => {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nETag: \"moss-etag-1\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        server_manifest.len()
                    );
                    stream.write_all(resp.as_bytes()).unwrap();
                    stream.write_all(&server_manifest).unwrap();
                }
                "/archive.zip" => {
                    let start = range_start.unwrap_or(0);
                    let slice = &server_payload[start..];
                    let resp = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        start,
                        server_payload.len() - 1,
                        server_payload.len(),
                        slice.len()
                    );
                    stream.write_all(resp.as_bytes()).unwrap();
                    stream.write_all(slice).unwrap();
                }
                "/archive-416.zip" if range_start.is_some() => {
                    let resp = "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    stream.write_all(resp.as_bytes()).unwrap();
                }
                "/archive-416.zip" => {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        server_payload.len()
                    );
                    stream.write_all(resp.as_bytes()).unwrap();
                    stream.write_all(&server_payload).unwrap();
                }
                _ => {
                    stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n").unwrap();
                }
            }
        }
    });

    let curl = Path::new("/usr/bin/curl");
    let stop = Arc::new(AtomicBool::new(false));
    let manifest_url = format!("http://127.0.0.1:{port}/latest.json");

    let first = fetch_manifest(curl, &manifest_url, None, true, &stop).unwrap();
    let etag = match first {
        ManifestFetch::Modified { body, etag } => {
            assert_eq!(body, manifest_body);
            assert_eq!(etag.as_deref(), Some("\"moss-etag-1\""));
            etag.unwrap()
        }
        ManifestFetch::NotModified => panic!("expected Modified on first fetch"),
    };

    let second = fetch_manifest(curl, &manifest_url, Some(&etag), true, &stop).unwrap();
    assert_eq!(second, ManifestFetch::NotModified);

    // Resumable download from partial file.
    let part = root.join("resume.zip.part");
    let final_path = root.join("resume.zip");
    fs::write(&part, &payload_bytes[..4096]).unwrap();
    download_file(
        curl,
        &format!("http://127.0.0.1:{port}/archive.zip"),
        &part,
        &final_path,
        payload_bytes.len() as u64,
        true,
        &stop,
        |_| {},
    )
    .unwrap();
    assert_eq!(fs::read(&final_path).unwrap(), payload_bytes);

    // Recovery from HTTP 416 on stale partial file.
    let part_416 = root.join("retry.zip.part");
    let final_416 = root.join("retry.zip");
    fs::write(&part_416, &payload_bytes[..1024]).unwrap();
    download_file(
        curl,
        &format!("http://127.0.0.1:{port}/archive-416.zip"),
        &part_416,
        &final_416,
        payload_bytes.len() as u64,
        true,
        &stop,
        |_| {},
    )
    .unwrap();
    assert_eq!(fs::read(&final_416).unwrap(), payload_bytes);

    server.join().unwrap();
}
