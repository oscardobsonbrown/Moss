use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("could not generate random key material: {0}")]
    Random(String),
    #[error("invalid base64 encoding")]
    Base64,
    #[error("invalid Ed25519 key length")]
    KeyLength,
    #[error("invalid Ed25519 public key")]
    PublicKey,
    #[error("invalid Ed25519 signature length")]
    SignatureLength,
    #[error("Ed25519 signature verification failed")]
    SignatureMismatch,
}

pub struct Keypair {
    pub private_key_b64: String,
    pub public_key_b64: String,
}

pub fn generate_keypair() -> Result<Keypair, CryptoError> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|error| CryptoError::Random(error.to_string()))?;
    let signing = SigningKey::from_bytes(&seed);
    let verifying = signing.verifying_key();
    Ok(Keypair {
        private_key_b64: STANDARD.encode(signing.to_bytes()),
        public_key_b64: STANDARD.encode(verifying.to_bytes()),
    })
}

pub fn decode_signing_key(private_b64: &str) -> Result<SigningKey, CryptoError> {
    let raw = STANDARD
        .decode(private_b64.trim())
        .map_err(|_| CryptoError::Base64)?;
    let bytes: [u8; 32] = raw.try_into().map_err(|_| CryptoError::KeyLength)?;
    Ok(SigningKey::from_bytes(&bytes))
}

pub fn decode_verifying_key(public_b64: &str) -> Result<VerifyingKey, CryptoError> {
    let raw = STANDARD
        .decode(public_b64.trim())
        .map_err(|_| CryptoError::Base64)?;
    let bytes: [u8; 32] = raw.try_into().map_err(|_| CryptoError::KeyLength)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| CryptoError::PublicKey)
}

pub fn derive_public_key(private_b64: &str) -> Result<String, CryptoError> {
    let signing = decode_signing_key(private_b64)?;
    Ok(STANDARD.encode(signing.verifying_key().to_bytes()))
}

pub fn sign_bytes(private_b64: &str, message: &[u8]) -> Result<String, CryptoError> {
    let signing = decode_signing_key(private_b64)?;
    let signature = signing.sign(message);
    Ok(STANDARD.encode(signature.to_bytes()))
}

pub fn verify_bytes(
    public_b64: &str,
    message: &[u8],
    signature_b64: &str,
) -> Result<(), CryptoError> {
    let verifying = decode_verifying_key(public_b64)?;
    let raw = STANDARD
        .decode(signature_b64.trim())
        .map_err(|_| CryptoError::Base64)?;
    let bytes: [u8; 64] = raw.try_into().map_err(|_| CryptoError::SignatureLength)?;
    let signature = Signature::from_bytes(&bytes);
    verifying
        .verify(message, &signature)
        .map_err(|_| CryptoError::SignatureMismatch)
}

pub fn sha256_file(path: &Path) -> io::Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        total += count as u64;
    }
    Ok((hex::encode(hasher.finalize()), total))
}
