use super::*;
use moss_updater::{
    BUNDLE_ID, MANIFEST_SCHEMA,
    bundle::run_checked,
    cache::{InstallReceipt, write_receipt},
    crypto::{generate_keypair, sha256_file},
    manifest::{Manifest, ManifestPayload, PlatformArtifact},
};
use rune::{Env, Runtime};
use std::{
    collections::BTreeMap,
    fs::DirBuilder,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    process::Command,
};

struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("moss-update-test-{}", uuid::Uuid::new_v4()));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bundled_app(app: &Path, version: &str) {
    let binary = app.join("Contents/MacOS/Moss");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
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

fn write_signed_release(
    root: &Path,
    version_str: &str,
    private_key_b64: &str,
    tamper_archive: bool,
) -> (PathBuf, PathBuf) {
    let src_dir = root.join(format!("build-{version_str}"));
    let app = src_dir.join("Moss.app");
    bundled_app(&app, version_str);

    let archive = root.join(format!("Moss-v{version_str}-macos-arm64.zip"));
    run_checked(
        Command::new("/usr/bin/ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(&app)
            .arg(&archive),
    )
    .unwrap();

    let (sha256, size) = sha256_file(&archive).unwrap();
    if tamper_archive {
        let mut bytes = fs::read(&archive).unwrap();
        bytes[0] ^= 1;
        fs::write(&archive, bytes).unwrap();
    }

    let mut platforms = BTreeMap::new();
    platforms.insert(
        current_platform_key().to_owned(),
        PlatformArtifact {
            url: format!("file://{}", archive.display()),
            sha256,
            size,
        },
    );
    let payload = ManifestPayload {
        schema: MANIFEST_SCHEMA,
        version: version_str.into(),
        published_at: "2026-11-22T09:00:00Z".into(),
        release_url: format!("https://github.com/oscardobsonbrown/Moss/releases/tag/v{version_str}"),
        min_macos: Some("14.0".into()),
        platforms,
    };
    let manifest = Manifest::sign(payload, private_key_b64).unwrap();
    let manifest_path = root.join(format!("latest-{version_str}.json"));
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    (manifest_path, archive)
}

fn wait_for_status(client: &Client, expected: Status) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while client.snapshot().status != expected {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected:?}; current status is {:?}",
            client.snapshot().status
        );
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn signed_manifest_stages_persists_across_restart_and_rejects_tampered_archives() {
    let dir = TestDir::new();
    let root = &dir.0;
    let pair = generate_keypair().unwrap();
    let target_app = root.join("Installed.app");
    bundled_app(&target_app, "0.11.0");
    let cache_dir = root.join("cache");
    ensure_private_dir(&cache_dir).unwrap();

    // 1. Tampered archive is rejected as Unverified and leaves nothing staged.
    let (bad_manifest, _) = write_signed_release(root, "0.12.0", &pair.private_key_b64, true);
    let bad_config = RuntimeConfig {
        bundle: Some(target_app.clone()),
        helper: None,
        cache_dir: cache_dir.clone(),
        receipt: receipt_path(&cache_dir),
        explicit_data_dir: None,
        manifest_url: format!("file://{}", bad_manifest.display()),
        public_key: pair.public_key_b64.clone(),
        allow_local_urls: true,
        curl: PathBuf::from("/usr/bin/curl"),
        relaunch_on_install: false,
    };
    let bad_updater = Updater::start_with_config([0, 11, 0], false, bad_config);
    bad_updater.client.check(true);
    wait_for_status(&bad_updater.client, Status::Unverified);
    assert!(find_best_staged(&cache_dir, [0, 11, 0]).is_none());
    drop(bad_updater);

    // 2. Valid signed release with automatic=false shows Available, then downloads directly on request.
    let (good_manifest, _) = write_signed_release(root, "0.12.0", &pair.private_key_b64, false);
    let good_config = RuntimeConfig {
        bundle: Some(target_app.clone()),
        helper: None,
        cache_dir: cache_dir.clone(),
        receipt: receipt_path(&cache_dir),
        explicit_data_dir: None,
        manifest_url: format!("file://{}", good_manifest.display()),
        public_key: pair.public_key_b64.clone(),
        allow_local_urls: true,
        curl: PathBuf::from("/usr/bin/curl"),
        relaunch_on_install: false,
    };
    let updater = Updater::start_with_config([0, 11, 0], false, good_config.clone());
    updater.client.check(false);
    wait_for_status(&updater.client, Status::Available([0, 12, 0]));
    updater.client.check(true);
    wait_for_status(&updater.client, Status::Ready([0, 12, 0]));

    // Normal quit (drop) must NOT modify the installed bundle (Option A: restart-only install).
    drop(updater);
    assert_eq!(installed_version(&target_app), Some([0, 11, 0]));

    // 3. Reopening Moss restores Ready([0, 12, 0]) immediately from persistent cache!
    let reopened = Updater::start_with_config([0, 11, 0], false, good_config);
    assert_eq!(reopened.client.snapshot().status, Status::Ready([0, 12, 0]));
    drop(reopened);

    // 4. Starting with a Succeeded receipt shows post-update confirmation toast.
    write_receipt(
        &receipt_path(&cache_dir),
        &InstallReceipt {
            bundle: target_app.clone(),
            version: [0, 12, 0],
            outcome: InstallOutcome::Succeeded,
        },
    )
    .unwrap();
    let updated_config = RuntimeConfig {
        bundle: Some(target_app.clone()),
        helper: None,
        cache_dir: cache_dir.clone(),
        receipt: receipt_path(&cache_dir),
        explicit_data_dir: None,
        manifest_url: format!("file://{}", good_manifest.display()),
        public_key: pair.public_key_b64.clone(),
        allow_local_urls: true,
        curl: PathBuf::from("/usr/bin/curl"),
        relaunch_on_install: false,
    };
    let after_update = Updater::start_with_config([0, 12, 0], false, updated_config);
    assert_eq!(after_update.client.snapshot().updated, Some([0, 12, 0]));
    assert_eq!(after_update.client.snapshot().status, Status::Idle);
    // Staged directory was cleaned up after update succeeded.
    assert!(find_best_staged(&cache_dir, [0, 11, 0]).is_none());
}

#[test]
fn newer_release_supersedes_already_ready_staged_update() {
    let dir = TestDir::new();
    let root = &dir.0;
    let pair = generate_keypair().unwrap();
    let target_app = root.join("Installed.app");
    bundled_app(&target_app, "0.11.0");
    let cache_dir = root.join("cache");
    ensure_private_dir(&cache_dir).unwrap();

    let (manifest_12, _) = write_signed_release(root, "0.12.0", &pair.private_key_b64, false);
    let manifest_link = root.join("latest.json");
    fs::copy(&manifest_12, &manifest_link).unwrap();

    let config = RuntimeConfig {
        bundle: Some(target_app.clone()),
        helper: None,
        cache_dir: cache_dir.clone(),
        receipt: receipt_path(&cache_dir),
        explicit_data_dir: None,
        manifest_url: format!("file://{}", manifest_link.display()),
        public_key: pair.public_key_b64.clone(),
        allow_local_urls: true,
        curl: PathBuf::from("/usr/bin/curl"),
        relaunch_on_install: false,
    };

    let updater = Updater::start_with_config([0, 11, 0], true, config);
    wait_for_status(&updater.client, Status::Ready([0, 12, 0]));

    // Publish 0.13.0 and trigger a check while 0.12.0 is already Ready.
    let (manifest_13, _) = write_signed_release(root, "0.13.0", &pair.private_key_b64, false);
    fs::copy(&manifest_13, &manifest_link).unwrap();
    updater.client.check(true);
    wait_for_status(&updater.client, Status::Ready([0, 13, 0]));
    assert_eq!(
        find_best_staged(&cache_dir, [0, 11, 0]).unwrap().version,
        [0, 13, 0]
    );
}

#[test]
fn download_progress_only_appears_in_settings_and_ready_is_persistent() {
    let dir = TestDir::new();
    let mut state = State::open(dir.0.clone(), None).unwrap();
    let client = Client::fixture([0, 11, 0], Status::Downloading(47));
    state.updates.client = Some(client.clone());
    let mut rt = Runtime::new(
        state,
        crate::ui::view,
        (1100., 750.),
        Env {
            system_dark: false,
            scale: 2.,
        },
    );
    rt.frame(0.);
    assert!(rt.find("software-update-toast").is_none());
    assert!(rt.find("update-download-progress").is_none());
    rt.state.modal = Some(crate::state::Modal::Settings);
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("update-download-progress").is_some());
    assert!(rt.find("software-update-toast").is_none());
    client.show(Status::Ready([0, 12, 0]));
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("settings-restart-update").is_some());
    rt.state.modal = None;
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("restart-update").is_some());
    assert!(rt.find("dismiss-update-toast").is_none());
    client.show(Status::InstallFailed([0, 12, 0]));
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("retry-update-install").is_some());
    assert!(rt.find("dismiss-update-toast").is_some());
    client.show(Status::Restarting([0, 12, 0]));
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("software-update-toast").is_none());
}

#[test]
fn restart_stays_in_moss_if_an_edit_cannot_be_saved() {
    let dir = TestDir::new();
    let mut state = State::open(dir.0.clone(), None).unwrap();
    state.new_page(false, None);
    let active = state.active;
    let page = state.tabs[active].page.id.clone();
    state.tabs[active].blocks[0].edit.replace_text("Saved text");
    state.save_blocks();
    state.tabs[active].blocks[0]
        .edit
        .replace_text("Unsaved draft");
    state.tabs[active].dirty_blocks = true;
    let mut external = state.store.get_blocks(&page).unwrap();
    external[0].text = "Change from another window".into();
    state.store.save_blocks(&page, &external).unwrap();

    let target_app = dir.0.join("Installed.app");
    bundled_app(&target_app, "0.11.0");
    let next = [0, 12, 0];
    let config = RuntimeConfig {
        bundle: Some(target_app),
        helper: None,
        cache_dir: dir.0.join("cache"),
        receipt: dir.0.join("cache/install-result.json"),
        explicit_data_dir: None,
        manifest_url: DEFAULT_MANIFEST_URL.into(),
        public_key: String::new(),
        allow_local_urls: true,
        curl: PathBuf::from("/usr/bin/curl"),
        relaunch_on_install: false,
    };
    let (client, _) = Client::new(
        [0, 11, 0],
        true,
        Snapshot {
            status: Status::Ready(next),
            updated: None,
            release_url: None,
        },
        None,
        config,
    );
    state.updates.client = Some(client.clone());
    let mut rt = Runtime::new(
        state,
        crate::ui::view,
        (1100., 750.),
        Env {
            system_dark: true,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let (x, y) = rt.find("restart-update").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.);
    assert_eq!(client.snapshot().status, Status::SaveFailed(next));
    assert!(!client.shared.stopped.load(Ordering::Acquire));
    assert!(rt.state.tabs[active].dirty_blocks);
    assert_eq!(rt.state.tabs[active].blocks[0].edit.text, "Unsaved draft");
    assert!(rt.find("keep-editing-after-update").is_some());
}
