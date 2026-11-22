use crate::REPOSITORY;
use std::process::Command;

pub type Version = [u64; 3];

pub fn parse_version(input: &str) -> Option<Version> {
    let trimmed = input.trim();
    let mut parts = trimmed.split('.');
    let mut version = [0; 3];
    for number in &mut version {
        let part = parts.next()?;
        if part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        *number = part.parse().ok()?;
    }
    parts.next().is_none().then_some(version)
}

pub fn format_version([major, minor, patch]: Version) -> String {
    format!("{major}.{minor}.{patch}")
}

pub fn default_release_url(version: Version) -> String {
    format!(
        "https://github.com/{REPOSITORY}/releases/tag/v{}",
        format_version(version)
    )
}

pub fn parse_os_version(input: &str) -> Option<[u64; 3]> {
    let mut parts = input.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().map_or(Some(0), |part| part.parse().ok())?;
    let patch = parts.next().map_or(Some(0), |part| part.parse().ok())?;
    if parts.next().is_some() {
        return None;
    }
    Some([major, minor, patch])
}

pub fn current_macos_version() -> Option<[u64; 3]> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = std::str::from_utf8(&output.stdout).ok()?;
        parse_os_version(text)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn is_os_compatible(min_required: Option<&str>, current_os: Option<[u64; 3]>) -> bool {
    let Some(min_required) = min_required.filter(|value| !value.trim().is_empty()) else {
        return true;
    };
    let Some(required) = parse_os_version(min_required) else {
        return false;
    };
    let Some(current) = current_os else {
        return true;
    };
    current >= required
}
