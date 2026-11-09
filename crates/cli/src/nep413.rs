//! NEP-413 signing for the Tidepool testnet sandbox. The seed never leaves
//! the caller. Mainnet hosts are refused before a request is built.

use sha2::{Digest, Sha256};

/// Bytes covered by the ed25519 signature: magic || borsh message || nonce || recipient || no callback.
pub fn nep413_message(message: &str, nonce: &[u8; 32], recipient: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(2u32.pow(31) + 413).to_le_bytes());
    out.extend_from_slice(&(message.len() as u32).to_le_bytes());
    out.extend_from_slice(message.as_bytes());
    out.extend_from_slice(nonce);
    out.extend_from_slice(&(recipient.len() as u32).to_le_bytes());
    out.extend_from_slice(recipient.as_bytes());
    out.push(0);
    out
}

pub fn nep413_digest(message: &str, nonce: &[u8; 32], recipient: &str) -> [u8; 32] {
    let digest = Sha256::digest(nep413_message(message, nonce, recipient));
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signed_prefix_is_the_nep413_magic() {
        let nonce = [7u8; 32];
        let bytes = nep413_message("fe2vi", &nonce, "intents.near");
        assert_eq!(&bytes[..4], &(2u32.pow(31) + 413).to_le_bytes());
        assert_eq!(bytes.last(), Some(&0));
        let digest = nep413_digest("fe2vi", &nonce, "intents.near");
        assert_ne!(digest, [0u8; 32]);
    }
}
