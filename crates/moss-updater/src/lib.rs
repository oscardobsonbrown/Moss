pub mod bundle;
pub mod cache;
pub mod crypto;
pub mod http;
pub mod install;
pub mod manifest;
pub mod version;

pub const REPOSITORY: &str = "oscardobsonbrown/Moss";
pub const BUNDLE_ID: &str = "app.moss.desktop";
pub const EXECUTABLE_NAME: &str = "Moss";
pub const HELPER_NAME: &str = "moss-update-helper";
pub const MANIFEST_SCHEMA: u32 = 1;
pub const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/oscardobsonbrown/Moss/releases/latest/download/latest.json";
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

pub const fn current_platform_key() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "macos-arm64"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "macos-x64"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "unsupported"
    }
}
