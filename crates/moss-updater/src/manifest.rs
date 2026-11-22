use crate::{
    MANIFEST_SCHEMA, MAX_ARCHIVE_BYTES,
    crypto::{CryptoError, sign_bytes, verify_bytes},
    version::{Version, is_os_compatible, parse_version},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("invalid manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported manifest schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid semantic version: {0}")]
    InvalidVersion(String),
    #[error("manifest field contains a forbidden control character")]
    ForbiddenCharacter,
    #[error("manifest signature error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("no update package available for platform {0}")]
    UnsupportedPlatform(String),
    #[error("update requires macOS {0} or later")]
    UnsupportedOs(String),
    #[error("invalid download or release URL: {0}")]
    InvalidUrl(String),
    #[error("invalid SHA-256 digest")]
    InvalidDigest,
    #[error("invalid archive size: {0}")]
    InvalidSize(u64),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformArtifact {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestPayload {
    pub schema: u32,
    pub version: String,
    pub published_at: String,
    pub release_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_macos: Option<String>,
    pub platforms: BTreeMap<String, PlatformArtifact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub payload: ManifestPayload,
    pub signature: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedRelease {
    pub version: Version,
    pub published_at: String,
    pub release_url: String,
    pub artifact: PlatformArtifact,
}

fn check_field(value: &str) -> Result<(), ManifestError> {
    if value.contains('\n') || value.contains('\r') || value.contains('|') {
        return Err(ManifestError::ForbiddenCharacter);
    }
    Ok(())
}

pub fn is_valid_url(url: &str, allow_local_urls: bool) -> bool {
    if url.starts_with("https://") && url.len() > "https://".len() {
        return true;
    }
    if allow_local_urls
        && (url.starts_with("http://127.0.0.1:")
            || url.starts_with("http://localhost:")
            || url.starts_with("file://"))
    {
        return true;
    }
    false
}

pub fn is_valid_sha256(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl ManifestPayload {
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, ManifestError> {
        check_field(&self.version)?;
        check_field(&self.published_at)?;
        check_field(&self.release_url)?;
        if let Some(min_macos) = &self.min_macos {
            check_field(min_macos)?;
        }
        let mut out = String::new();
        out.push_str("moss-update-manifest-v1\n");
        out.push_str(&format!("schema={}\n", self.schema));
        out.push_str(&format!("version={}\n", self.version));
        out.push_str(&format!("published_at={}\n", self.published_at));
        out.push_str(&format!("release_url={}\n", self.release_url));
        out.push_str(&format!(
            "min_macos={}\n",
            self.min_macos.as_deref().unwrap_or("")
        ));
        for (platform, artifact) in &self.platforms {
            check_field(platform)?;
            check_field(&artifact.url)?;
            check_field(&artifact.sha256)?;
            out.push_str(&format!(
                "platform={}|url={}|sha256={}|size={}\n",
                platform,
                artifact.url,
                artifact.sha256.to_ascii_lowercase(),
                artifact.size
            ));
        }
        Ok(out.into_bytes())
    }
}

impl Manifest {
    pub fn sign(payload: ManifestPayload, private_key_b64: &str) -> Result<Self, ManifestError> {
        if payload.schema != MANIFEST_SCHEMA {
            return Err(ManifestError::UnsupportedSchema(payload.schema));
        }
        if parse_version(&payload.version).is_none() {
            return Err(ManifestError::InvalidVersion(payload.version.clone()));
        }
        let message = payload.canonical_signing_bytes()?;
        let signature = sign_bytes(private_key_b64, &message)?;
        Ok(Self { payload, signature })
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ManifestError> {
        Ok(serde_json::from_slice(bytes)?)
    }

    pub fn verify(
        &self,
        public_key_b64: &str,
        platform_key: &str,
        current_os: Option<[u64; 3]>,
        allow_local_urls: bool,
    ) -> Result<VerifiedRelease, ManifestError> {
        if self.payload.schema != MANIFEST_SCHEMA {
            return Err(ManifestError::UnsupportedSchema(self.payload.schema));
        }
        let version = parse_version(&self.payload.version)
            .ok_or_else(|| ManifestError::InvalidVersion(self.payload.version.clone()))?;
        let message = self.payload.canonical_signing_bytes()?;
        verify_bytes(public_key_b64, &message, &self.signature)?;

        if !is_valid_url(&self.payload.release_url, allow_local_urls) {
            return Err(ManifestError::InvalidUrl(self.payload.release_url.clone()));
        }
        if !is_os_compatible(self.payload.min_macos.as_deref(), current_os) {
            return Err(ManifestError::UnsupportedOs(
                self.payload.min_macos.clone().unwrap_or_default(),
            ));
        }
        let artifact = self
            .payload
            .platforms
            .get(platform_key)
            .ok_or_else(|| ManifestError::UnsupportedPlatform(platform_key.to_owned()))?;
        if !is_valid_url(&artifact.url, allow_local_urls) {
            return Err(ManifestError::InvalidUrl(artifact.url.clone()));
        }
        if !is_valid_sha256(&artifact.sha256) {
            return Err(ManifestError::InvalidDigest);
        }
        if artifact.size == 0 || artifact.size > MAX_ARCHIVE_BYTES {
            return Err(ManifestError::InvalidSize(artifact.size));
        }

        Ok(VerifiedRelease {
            version,
            published_at: self.payload.published_at.clone(),
            release_url: self.payload.release_url.clone(),
            artifact: PlatformArtifact {
                url: artifact.url.clone(),
                sha256: artifact.sha256.to_ascii_lowercase(),
                size: artifact.size,
            },
        })
    }
}
