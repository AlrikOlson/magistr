use super::ConfirmationError;
use crate::jail::WorkspaceKey;
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Workspace-key authenticator for the exact request and finalized byte plan.
///
/// The caller supplies explicit session identity and the original serialized
/// request bytes. These bytes are never parsed or canonicalized. The finalized
/// plan must include the ordered paths, before/after existence and bytes,
/// verification, formatting policy and undo relationships. The mutation
/// pipeline supplies this representation from its private prepared plan.
///
/// Tokens survive owner restart with the same key, but `wb1` tokens from the
/// retained runtime are deliberately invalidated. This is an authenticator,
/// not session admission, replay prevention or a source revision certificate.
pub struct ConfirmationSeal {
    mac: Hmac<Sha256>,
}

impl ConfirmationSeal {
    #[must_use]
    pub fn new(key: &WorkspaceKey) -> Self {
        Self {
            mac: Hmac::<Sha256>::new_from_slice(key.as_bytes())
                .expect("HMAC accepts any key length"),
        }
    }

    #[must_use]
    pub fn seal(
        &self,
        session_id: &str,
        operation: &str,
        serialized_request: &[u8],
        finalized_plan: &[u8],
    ) -> String {
        let tag = self
            .bound_mac(session_id, operation, serialized_request, finalized_plan)
            .finalize()
            .into_bytes();
        let mut token = String::with_capacity(69);
        token.push_str("mgw1.");
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in tag {
            token.push(HEX[(byte >> 4) as usize] as char);
            token.push(HEX[(byte & 15) as usize] as char);
        }
        token
    }

    pub fn verify(
        &self,
        token: &str,
        session_id: &str,
        operation: &str,
        serialized_request: &[u8],
        finalized_plan: &[u8],
    ) -> Result<(), ConfirmationError> {
        let encoded = token
            .strip_prefix("mgw1.")
            .ok_or(ConfirmationError::UnsupportedVersion)?;
        if encoded.len() != 64 {
            return Err(ConfirmationError::Mismatch);
        }
        let mut tag = [0; 32];
        for (target, pair) in tag.iter_mut().zip(encoded.as_bytes().chunks_exact(2)) {
            *target = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
        }
        self.bound_mac(session_id, operation, serialized_request, finalized_plan)
            .verify_slice(&tag)
            .map_err(|_| ConfirmationError::Mismatch)
    }

    fn bound_mac(
        &self,
        session_id: &str,
        operation: &str,
        serialized_request: &[u8],
        finalized_plan: &[u8],
    ) -> Hmac<Sha256> {
        let mut mac = self.mac.clone();
        mac.update(b"magistr.workspace.confirmation.v1\0");
        for field in [
            session_id.as_bytes(),
            operation.as_bytes(),
            serialized_request,
            finalized_plan,
        ] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field);
        }
        mac
    }
}

fn hex_digit(byte: u8) -> Result<u8, ConfirmationError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(ConfirmationError::Mismatch),
    }
}

#[cfg(test)]
#[path = "confirmation_seal.tests.rs"]
mod tests;
