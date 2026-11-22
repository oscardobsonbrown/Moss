use chrono::{SecondsFormat, Utc};
use moss_updater::{
    MANIFEST_SCHEMA,
    cache::atomic_write,
    crypto::{derive_public_key, generate_keypair, sha256_file},
    install::{InstallRequest, run_detached_install},
    manifest::{Manifest, ManifestPayload, PlatformArtifact},
    version::{default_release_url, parse_version},
};
use std::{
    collections::BTreeMap,
    env,
    fs,
    path::PathBuf,
    process::ExitCode,
};

fn usage() -> &'static str {
    "Usage:
  moss-update-helper keygen
  moss-update-helper pubkey --signing-key-env <ENV>
  moss-update-helper manifest --version <X.Y.Z> --archive <PATH> --url <URL> [--release-url <URL>] [--platform <KEY>] [--min-macos <VER>] --signing-key-env <ENV> --output <PATH>
  moss-update-helper verify-manifest --public-key <B64> --manifest <PATH> [--archive <PATH>] [--platform <KEY>] [--allow-local-urls]
  moss-update-helper install --pid <PID> --staged-app <PATH> --target-app <PATH> --expected-current <X.Y.Z> --next-version <X.Y.Z> --receipt <PATH> --cache-dir <PATH> [--release-url <URL>] [--data-dir <PATH>] [--no-relaunch]"
}

fn next_flag(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn cmd_keygen() -> Result<(), String> {
    let pair = generate_keypair().map_err(|error| error.to_string())?;
    println!("MOSS_UPDATE_SIGNING_KEY={}", pair.private_key_b64);
    println!("MOSS_UPDATE_PUBLIC_KEY={}", pair.public_key_b64);
    Ok(())
}

fn cmd_pubkey(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut env_name = String::from("MOSS_UPDATE_SIGNING_KEY");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--signing-key-env" => env_name = next_flag(&mut args, "--signing-key-env")?,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let private = env::var(&env_name)
        .map_err(|_| format!("environment variable {env_name} is not set"))?;
    let public = derive_public_key(&private).map_err(|error| error.to_string())?;
    println!("{public}");
    Ok(())
}

fn cmd_manifest(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut version_raw = None;
    let mut archive = None;
    let mut url = None;
    let mut release_url = None;
    let mut platform = String::from("macos-arm64");
    let mut min_macos = Some(String::from("14.0"));
    let mut signing_key_env = String::from("MOSS_UPDATE_SIGNING_KEY");
    let mut output = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => version_raw = Some(next_flag(&mut args, "--version")?),
            "--archive" => archive = Some(PathBuf::from(next_flag(&mut args, "--archive")?)),
            "--url" => url = Some(next_flag(&mut args, "--url")?),
            "--release-url" => release_url = Some(next_flag(&mut args, "--release-url")?),
            "--platform" => platform = next_flag(&mut args, "--platform")?,
            "--min-macos" => {
                let value = next_flag(&mut args, "--min-macos")?;
                min_macos = (!value.is_empty()).then_some(value);
            }
            "--signing-key-env" => signing_key_env = next_flag(&mut args, "--signing-key-env")?,
            "--output" => output = Some(PathBuf::from(next_flag(&mut args, "--output")?)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let version_str = version_raw
        .ok_or("missing --version")?
        .trim_start_matches('v')
        .to_owned();
    let version = parse_version(&version_str)
        .ok_or_else(|| format!("invalid semantic version: {version_str}"))?;
    let archive_path = archive.ok_or("missing --archive")?;
    let download_url = url.ok_or("missing --url")?;
    let output_path = output.ok_or("missing --output")?;
    let private_key = env::var(&signing_key_env)
        .map_err(|_| format!("environment variable {signing_key_env} is not set"))?;

    let (sha256, size) = sha256_file(&archive_path)
        .map_err(|error| format!("could not hash {}: {error}", archive_path.display()))?;

    let mut platforms = BTreeMap::new();
    platforms.insert(
        platform,
        PlatformArtifact {
            url: download_url,
            sha256,
            size,
        },
    );

    let payload = ManifestPayload {
        schema: MANIFEST_SCHEMA,
        version: version_str,
        published_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        release_url: release_url.unwrap_or_else(|| default_release_url(version)),
        min_macos,
        platforms,
    };
    let manifest = Manifest::sign(payload, &private_key).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    atomic_write(&output_path, &json).map_err(|error| error.to_string())?;
    Ok(())
}

fn cmd_verify_manifest(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut public_key = None;
    let mut manifest_path = None;
    let mut archive_path = None;
    let mut platform = String::from("macos-arm64");
    let mut allow_local_urls = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--public-key" => public_key = Some(next_flag(&mut args, "--public-key")?),
            "--manifest" => manifest_path = Some(PathBuf::from(next_flag(&mut args, "--manifest")?)),
            "--archive" => archive_path = Some(PathBuf::from(next_flag(&mut args, "--archive")?)),
            "--platform" => platform = next_flag(&mut args, "--platform")?,
            "--allow-local-urls" => allow_local_urls = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let public_key = public_key.ok_or("missing --public-key")?;
    let manifest_path = manifest_path.ok_or("missing --manifest")?;
    let bytes = fs::read(&manifest_path)
        .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
    let manifest = Manifest::from_json(&bytes).map_err(|error| error.to_string())?;
    let verified = manifest
        .verify(&public_key, &platform, None, allow_local_urls)
        .map_err(|error| error.to_string())?;

    if let Some(archive_path) = archive_path {
        let (actual_sha, actual_size) = sha256_file(&archive_path)
            .map_err(|error| format!("could not hash {}: {error}", archive_path.display()))?;
        if actual_size != verified.artifact.size {
            return Err(format!(
                "archive size {} did not match manifest size {}",
                actual_size, verified.artifact.size
            ));
        }
        if !actual_sha.eq_ignore_ascii_case(&verified.artifact.sha256) {
            return Err(format!(
                "archive SHA-256 {} did not match manifest {}",
                actual_sha, verified.artifact.sha256
            ));
        }
    }
    Ok(())
}

fn cmd_install(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut parent_pid = None;
    let mut staged_app = None;
    let mut target_app = None;
    let mut expected_current = None;
    let mut next_version = None;
    let mut release_url = String::new();
    let mut receipt_path = None;
    let mut cache_dir = None;
    let mut data_dir = None;
    let mut relaunch = true;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pid" => {
                let raw = next_flag(&mut args, "--pid")?;
                parent_pid = Some(raw.parse::<u32>().map_err(|_| format!("invalid --pid: {raw}"))?);
            }
            "--staged-app" => staged_app = Some(PathBuf::from(next_flag(&mut args, "--staged-app")?)),
            "--target-app" => target_app = Some(PathBuf::from(next_flag(&mut args, "--target-app")?)),
            "--expected-current" => {
                let raw = next_flag(&mut args, "--expected-current")?;
                expected_current = Some(
                    parse_version(&raw)
                        .ok_or_else(|| format!("invalid --expected-current: {raw}"))?,
                );
            }
            "--next-version" => {
                let raw = next_flag(&mut args, "--next-version")?;
                next_version = Some(
                    parse_version(&raw).ok_or_else(|| format!("invalid --next-version: {raw}"))?,
                );
            }
            "--release-url" => release_url = next_flag(&mut args, "--release-url")?,
            "--receipt" => receipt_path = Some(PathBuf::from(next_flag(&mut args, "--receipt")?)),
            "--cache-dir" => cache_dir = Some(PathBuf::from(next_flag(&mut args, "--cache-dir")?)),
            "--data-dir" => data_dir = Some(PathBuf::from(next_flag(&mut args, "--data-dir")?)),
            "--no-relaunch" => relaunch = false,
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let request = InstallRequest {
        parent_pid: parent_pid.ok_or("missing --pid")?,
        staged_app: staged_app.ok_or("missing --staged-app")?,
        target_app: target_app.ok_or("missing --target-app")?,
        expected_current: expected_current.ok_or("missing --expected-current")?,
        next_version: next_version.ok_or("missing --next-version")?,
        release_url,
        receipt_path: receipt_path.ok_or("missing --receipt")?,
        cache_dir: cache_dir.ok_or("missing --cache-dir")?,
        data_dir,
        relaunch,
    };

    run_detached_install(&request).map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("keygen") => cmd_keygen(),
        Some("pubkey") => cmd_pubkey(args),
        Some("manifest") => cmd_manifest(args),
        Some("verify-manifest") => cmd_verify_manifest(args),
        Some("install") => cmd_install(args),
        _ => Err(usage().to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("moss-update-helper: {error}");
            ExitCode::FAILURE
        }
    }
}
