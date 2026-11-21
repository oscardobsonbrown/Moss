use super::*;
use rune::{Env, Runtime};
use std::os::unix::fs::PermissionsExt;

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
    output(
        Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-"])
            .arg(app),
    )
    .unwrap();
}

#[test]
fn only_newer_stable_signed_arm64_packages_are_selected() {
    let mut release = Release {
        tag_name: "v0.10.0".into(),
        draft: false,
        prerelease: false,
        assets: vec![
            Asset {
                name: "Moss-v0.10.0-macos-arm64.zip".into(),
                state: "uploaded".into(),
                digest: Some(format!("sha256:{}", "ab".repeat(32))),
                size: 300,
            },
            Asset {
                name: "Moss-v0.10.0-macos-arm64.zip.sig".into(),
                state: "uploaded".into(),
                digest: None,
                size: 64,
            },
        ],
    };
    assert_eq!(
        release.package([0, 9, 0]).unwrap().unwrap().next,
        [0, 10, 0]
    );
    assert!(release.package([0, 10, 0]).unwrap().is_none());
    assert!(release.package([1, 0, 0]).unwrap().is_none());
    release.prerelease = true;
    assert!(release.package([0, 9, 0]).unwrap().is_none());
    release.prerelease = false;
    release.assets[1].size = 63;
    assert!(matches!(
        release.package([0, 9, 0]),
        Err(Status::Unverified)
    ));
    release.assets[1].size = 64;
    release.assets[0].name = "Moss-v0.10.0-macos-x64.zip".into();
    assert!(matches!(
        release.package([0, 9, 0]),
        Err(Status::Unverified)
    ));
    release.assets[0].name = "Moss-v0.10.0-macos-arm64.zip".into();
    release.assets.pop();
    assert!(matches!(
        release.package([0, 9, 0]),
        Err(Status::Unverified)
    ));
    assert!(version("0.10.0-beta").is_none());
}

#[test]
fn verified_update_preserves_data_and_rejects_a_replaced_archive() {
    let dir = TestDir::new();
    let root = &dir.0;
    let old_app = root.join("Installed.app");
    let new_app = root.join("Moss.app");
    let work = root.join("pending");
    fs::create_dir(&work).unwrap();
    bundled_app(&old_app, "0.9.0");
    bundled_app(&new_app, "0.10.0");
    let data = root.join("workspace.sqlite3");
    fs::write(&data, "local data").unwrap();
    let helper = root.join("helper");
    let helper_source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/moss-update-helper.swift");
    output(
        Command::new("xcrun")
            .args(["swiftc", "-O"])
            .arg(helper_source)
            .arg("-o")
            .arg(&helper),
    )
    .unwrap();
    let secret = root.join("test-key");
    let key = output(Command::new(&helper).arg("keygen").arg(&secret)).unwrap();
    let key = String::from_utf8(key.stdout).unwrap();
    let archive = root.join("fixture.zip");
    let sig = root.join("fixture.sig");
    output(
        Command::new("/usr/bin/ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(new_app)
            .arg(&archive),
    )
    .unwrap();
    output(
        Command::new(&helper)
            .arg("sign")
            .arg(secret)
            .arg(&archive)
            .arg(&sig),
    )
    .unwrap();
    let curl = root.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
set -eu
dir=$(/usr/bin/dirname "$0")
while [ "$#" -gt 0 ]; do
case "$1" in
    --output) target=$2; shift ;;
    https://*.zip.sig) exec /bin/cp "$dir/fixture.sig" "$target" ;;
    https://*.zip) exec /bin/cp "$dir/fixture.zip" "$target" ;;
esac
shift
done
exit 1
"#,
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();
    let (client, _) = Client::new([0, 9, 0], true, None);
    let staged = |contents: &[u8]| {
        fs::write(&archive, contents).unwrap();
        let hash = output(
            Command::new("/usr/bin/shasum")
                .args(["-a", "256"])
                .arg(&archive),
        )
        .unwrap();
        let digest = std::str::from_utf8(&hash.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap();
        let package = Package {
            next: [0, 10, 0],
            name: "Moss-v0.10.0-macos-arm64.zip",
            digest,
            size: contents.len() as u64,
        };
        stage(&curl, &helper, key.trim(), &work, &package, &client)
    };
    let original = fs::read(&archive).unwrap();
    let mut tampered = original.clone();
    tampered[0] ^= 1;
    assert_eq!(staged(&tampered), Err(Status::Unverified));
    assert!(!work.join("Moss.app").exists());
    staged(&original).unwrap();
    client.show(Status::Ready([0, 10, 0]));
    assert_eq!(installed_version(&old_app), Some([0, 9, 0]));
    let result = root.join("update-result.json");
    let updater = Updater {
        bundle: old_app.clone(),
        current: [0, 9, 0],
        helper,
        work,
        result: result.clone(),
        data: root.clone(),
        client,
    };
    updater.install_on_quit();
    assert_eq!(installed_version(&old_app), Some([0, 10, 0]));
    assert_eq!(fs::read_to_string(&data).unwrap(), "local data");
    assert_eq!(take_installed(&result, &old_app, [0, 9, 0]), None);
    assert_eq!(
        take_installed(&result, &old_app, [0, 10, 0]),
        Some([0, 10, 0])
    );
    assert_eq!(take_installed(&result, &old_app, [0, 10, 0]), None);
}

#[test]
fn download_progress_only_appears_in_settings_and_ready_is_persistent() {
    let dir = TestDir::new();
    let mut state = State::open(dir.0.clone(), None).unwrap();
    let client = Client::fixture([0, 9, 0], Status::Downloading(47));
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
    client.show(Status::Ready([0, 10, 0]));
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("settings-restart-update").is_some());
    rt.state.modal = None;
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find("restart-update").is_some());
    assert!(rt.find("dismiss-update-toast").is_none());
    client.show(Status::Restarting([0, 10, 0]));
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

    let next = [0, 10, 0];
    let client = Client::fixture([0, 9, 0], Status::Ready(next));
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
    assert!(!client.shared.restart.load(Ordering::Acquire));
    assert!(rt.state.tabs[active].dirty_blocks);
    assert_eq!(rt.state.tabs[active].blocks[0].edit.text, "Unsaved draft");
    assert!(rt.find("keep-editing-after-update").is_some());
}
