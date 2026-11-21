//! Ed25519 signing for published profile bundles.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::util::{set_private, write_atomic};

pub struct Signer {
    key: SigningKey,
    trusted_dir: PathBuf,
}

impl Signer {
    pub fn load_or_create(key_path: &Path, trusted_dir: &Path) -> Result<Self> {
        fs::create_dir_all(trusted_dir)?;
        let key = if key_path.exists() {
            let hex_seed = fs::read_to_string(key_path).with_context(|| format!("read {}", key_path.display()))?;
            let bytes = hex::decode(hex_seed.trim()).context("signing key is not hex")?;
            let seed: [u8; 32] = bytes.try_into().map_err(|_| anyhow!("signing key must be 32 bytes"))?;
            SigningKey::from_bytes(&seed)
        } else {
            let mut seed = [0u8; 32];
            getrandom::fill(&mut seed).map_err(|e| anyhow!("random: {e}"))?;
            write_atomic(key_path, format!("{}\n", hex::encode(seed)).as_bytes())?;
            set_private(key_path)?;
            SigningKey::from_bytes(&seed)
        };
        Ok(Self { key, trusted_dir: trusted_dir.to_path_buf() })
    }

    pub fn public_key(&self) -> String {
        B64.encode(self.key.verifying_key().to_bytes())
    }

    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.key.verifying_key().to_bytes())
    }

    /// Signs the canonical JSON of `body` (object keys sorted).
    pub fn sign(&self, body: &Value) -> String {
        let bytes = serde_json::to_vec(body).unwrap_or_default();
        B64.encode(self.key.sign(&bytes).to_bytes())
    }

    /// Public keys trusted for imports: our own plus `trusted-keys/*.pub`.
    pub fn trusted_keys(&self) -> Vec<String> {
        let mut keys = vec![self.public_key()];
        if let Ok(rd) = fs::read_dir(&self.trusted_dir) {
            for e in rd.flatten() {
                if e.path().extension().is_some_and(|x| x == "pub")
                    && let Ok(k) = fs::read_to_string(e.path())
                {
                    keys.push(k.trim().to_string());
                }
            }
        }
        keys
    }
}

pub fn fingerprint_of(public_key: &[u8]) -> String {
    let d = Sha256::digest(public_key);
    format!("ed25519:{}", hex::encode(&d[..12]))
}

pub fn verify(public_key_b64: &str, body: &Value, signature_b64: &str) -> Result<()> {
    let pk: [u8; 32] =
        B64.decode(public_key_b64.trim())?.try_into().map_err(|_| anyhow!("public key must be 32 bytes"))?;
    let sig: [u8; 64] =
        B64.decode(signature_b64.trim())?.try_into().map_err(|_| anyhow!("signature must be 64 bytes"))?;
    let vk = VerifyingKey::from_bytes(&pk)?;
    let bytes = serde_json::to_vec(body)?;
    if vk.verify(&bytes, &Signature::from_bytes(&sig)).is_err() {
        bail!("signature does not match the bundle");
    }
    Ok(())
}
