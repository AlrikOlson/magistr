//! Plugin signature verification using ed25519.
//!
//! Plugins in the registry index can optionally carry a base64-encoded
//! ed25519 signature. This module verifies archive integrity against
//! a known public key before installation.

use base64::prelude::*;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use super::types::PluginError;

/// Verify an ed25519 signature over data.
///
/// - `data`: the raw bytes to verify (e.g., the plugin `.tar.gz` archive)
/// - `signature_b64`: base64-encoded 64-byte ed25519 signature
/// - `public_key_b64`: base64-encoded 32-byte ed25519 public key
///
/// Returns `Ok(())` on success, or `PluginError::SignatureInvalid` on failure.
pub fn verify_signature(
    data: &[u8],
    signature_b64: &str,
    public_key_b64: &str,
) -> Result<(), PluginError> {
    let sig_bytes = BASE64_STANDARD
        .decode(signature_b64)
        .map_err(|e| PluginError::SignatureInvalid(format!("invalid signature base64: {e}")))?;

    let key_bytes = BASE64_STANDARD
        .decode(public_key_b64)
        .map_err(|e| PluginError::SignatureInvalid(format!("invalid public key base64: {e}")))?;

    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| PluginError::SignatureInvalid("public key must be exactly 32 bytes".into()))?;

    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| PluginError::SignatureInvalid("signature must be exactly 64 bytes".into()))?;

    let verifying_key = VerifyingKey::from_bytes(&key_array)
        .map_err(|e| PluginError::SignatureInvalid(format!("invalid public key: {e}")))?;

    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(data, &signature)
        .map_err(|e| PluginError::SignatureInvalid(format!("signature verification failed: {e}")))
}

#[cfg(test)]
#[path = "signing.tests.rs"]
mod tests;
