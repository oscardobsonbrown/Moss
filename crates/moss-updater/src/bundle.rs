use crate::{
    BUNDLE_ID, EXECUTABLE_NAME,
    version::{Version, parse_version},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("command `{program}` failed: {detail}")]
    Command { program: String, detail: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("bundle identity, executable, or version does not match")]
    InvalidMetadata,
    #[error("extracted archive does not contain Moss.app")]
    MissingAppBundle,
    #[error("codesign verification failed: {0}")]
    Codesign(String),
}

pub fn run_checked(command: &mut Command) -> Result<Output, BundleError> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command.output().map_err(|error| BundleError::Command {
        program: program.clone(),
        detail: error.to_string(),
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if stderr.is_empty() {
            format!("exited with {}", output.status)
        } else {
            format!("exited with {}: {stderr}", output.status)
        };
        return Err(BundleError::Command { program, detail });
    }
    Ok(output)
}

pub fn plist_value(bundle: &Path, key: &str) -> Option<String> {
    let info_plist = bundle.join("Contents/Info.plist");
    let result = run_checked(
        Command::new("/usr/libexec/PlistBuddy")
            .arg("-c")
            .arg(format!("Print :{key}"))
            .arg(&info_plist),
    )
    .ok()?;
    let value = String::from_utf8(result.stdout).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub fn installed_version(bundle: &Path) -> Option<Version> {
    if plist_value(bundle, "CFBundleIdentifier").as_deref() != Some(BUNDLE_ID) {
        return None;
    }
    parse_version(&plist_value(bundle, "CFBundleShortVersionString")?)
}

pub fn bundle_public_key(bundle: &Path) -> Option<String> {
    plist_value(bundle, "MossUpdatePublicKey")
}

pub fn is_translocated_or_readonly_mount(bundle: &Path) -> bool {
    let text = bundle.to_string_lossy();
    text.contains("/AppTranslocation/") || text.starts_with("/Volumes/")
}

pub fn can_install_to(bundle: &Path) -> bool {
    if is_translocated_or_readonly_mount(bundle) {
        return false;
    }
    let Some(parent) = bundle.parent() else {
        return false;
    };
    let probe = parent.join(format!(".moss-update-probe-{}", uuid::Uuid::new_v4()));
    match fs::create_dir(&probe) {
        Ok(()) => {
            let _ = fs::remove_dir(&probe);
            true
        }
        Err(_) => false,
    }
}

pub fn verify_bundle(bundle: &Path, expected_version: Version) -> Result<(), BundleError> {
    let meta = fs::symlink_metadata(bundle)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(BundleError::InvalidMetadata);
    }
    if installed_version(bundle) != Some(expected_version) {
        return Err(BundleError::InvalidMetadata);
    }
    if plist_value(bundle, "CFBundleExecutable").as_deref() != Some(EXECUTABLE_NAME) {
        return Err(BundleError::InvalidMetadata);
    }
    let executable = bundle.join("Contents/MacOS").join(EXECUTABLE_NAME);
    if !executable.is_file() {
        return Err(BundleError::InvalidMetadata);
    }
    run_checked(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(bundle),
    )
    .map_err(|error| BundleError::Codesign(error.to_string()))?;
    Ok(())
}

pub fn extract_archive(archive: &Path, target_dir: &Path) -> Result<PathBuf, BundleError> {
    let temp = target_dir.join(format!(".extract-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp)?;
    let extract_result = run_checked(
        Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(archive)
            .arg(&temp),
    );
    if let Err(error) = extract_result {
        let _ = fs::remove_dir_all(&temp);
        return Err(error);
    }
    let extracted_app = temp.join("Moss.app");
    if !extracted_app.is_dir() {
        let _ = fs::remove_dir_all(&temp);
        return Err(BundleError::MissingAppBundle);
    }
    let final_app = target_dir.join("Moss.app");
    let _ = fs::remove_dir_all(&final_app);
    if let Err(error) = fs::rename(&extracted_app, &final_app) {
        let _ = fs::remove_dir_all(&temp);
        return Err(BundleError::Io(error));
    }
    let _ = fs::remove_dir_all(&temp);
    Ok(final_app)
}

pub fn strip_quarantine(bundle: &Path) {
    let _ = Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(bundle)
        .output();
}
