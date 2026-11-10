//! A NEAR function-call signature. The secret is borrowed and is not returned.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn public_key(secret: &str) -> Result<String, String> {
    let key = signing_key(secret)?;
    Ok(format!(
        "ed25519:{}",
        bs58::encode(key.verifying_key().to_bytes()).into_string()
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn sign_function_call(
    signer_id: &str,
    secret: &str,
    receiver_id: &str,
    method: &str,
    args: &[u8],
    gas: u64,
    deposit: u128,
    nonce: u64,
    block_hash_hex: &str,
) -> Result<Vec<u8>, String> {
    let secret_key = signing_key(secret)?;
    let public = secret_key.verifying_key().to_bytes();
    let block_hash = decode_hash(block_hash_hex)?;
    let mut tx = Vec::new();
    push_str(&mut tx, signer_id);
    tx.push(0);
    tx.extend_from_slice(&public);
    tx.extend_from_slice(&nonce.to_le_bytes());
    push_str(&mut tx, receiver_id);
    tx.extend_from_slice(&block_hash);
    push_u32(&mut tx, 1);
    tx.push(2);
    push_str(&mut tx, method);
    push_u32(&mut tx, args.len() as u32);
    tx.extend_from_slice(args);
    tx.extend_from_slice(&gas.to_le_bytes());
    tx.extend_from_slice(&deposit.to_le_bytes());
    let hash = Sha256::digest(&tx);
    let signature = secret_key.sign(&hash);
    let mut signed = tx;
    signed.push(0);
    signed.extend_from_slice(&signature.to_bytes());
    Ok(signed)
}

fn signing_key(secret: &str) -> Result<ed25519_dalek::SigningKey, String> {
    let raw = secret
        .trim()
        .strip_prefix("ed25519:")
        .unwrap_or(secret.trim());
    let bytes = bs58::decode(raw)
        .into_vec()
        .map_err(|_| "signing key could not be parsed".to_string())?;
    if bytes.len() < 32 {
        return Err("signing key could not be parsed".into());
    }
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&bytes[..32]);
    Ok(ed25519_dalek::SigningKey::from_bytes(&seed))
}

fn decode_hash(text: &str) -> Result<[u8; 32], String> {
    let bytes = bs58::decode(text.trim())
        .into_vec()
        .map_err(|_| "block hash is invalid".to_string())?;
    if bytes.len() != 32 {
        return Err("block hash is invalid".into());
    }
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}

fn push_str(out: &mut Vec<u8>, text: &str) {
    push_u32(out, text.len() as u32);
    out.extend_from_slice(text.as_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signed_bytes_match_the_near_transaction_encoding() {
        let seed = [7_u8; 32];
        let key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let raw = {
            let mut bytes = key.to_bytes().to_vec();
            bytes.extend_from_slice(&key.verifying_key().to_bytes());
            format!("ed25519:{}", bs58::encode(bytes).into_string())
        };
        let block = bs58::encode([9_u8; 32]).into_string();
        let ours = sign_function_call(
            "fe2virecdb1e91.testnet",
            &raw,
            "passport.fe2virecdb1e91.testnet",
            "set_playbook_hash",
            br#"{"token_id":"rec_001"}"#,
            30_000_000_000_000,
            0,
            4,
            &block,
        )
        .unwrap();
        let signed = crate::near_sign::tests::reference(
            &raw,
            "fe2virecdb1e91.testnet",
            "passport.fe2virecdb1e91.testnet",
            "set_playbook_hash",
            br#"{"token_id":"rec_001"}"#,
            4,
            &block,
        );
        assert_eq!(ours, signed);
    }

    fn reference(
        secret: &str,
        signer: &str,
        receiver: &str,
        method: &str,
        args: &[u8],
        nonce: u64,
        block: &str,
    ) -> Vec<u8> {
        use near_crypto::SecretKey;
        use near_primitives::hash::CryptoHash;
        use near_primitives::transaction::{
            FunctionCallAction, SignedTransaction, Transaction, TransactionV0,
        };
        use near_primitives::types::AccountId;
        use std::str::FromStr;
        let secret: SecretKey = secret.parse().unwrap();
        let transaction = Transaction::V0(TransactionV0 {
            signer_id: signer.parse::<AccountId>().unwrap(),
            public_key: secret.public_key(),
            nonce,
            receiver_id: receiver.parse().unwrap(),
            block_hash: CryptoHash::from_str(block).unwrap(),
            actions: vec![FunctionCallAction {
                method_name: method.into(),
                args: args.to_vec(),
                gas: 30_000_000_000_000,
                deposit: 0,
            }
            .into()],
        });
        let (hash, _) = transaction.get_hash_and_size();
        let signature = secret.sign(hash.as_ref());
        near_primitives::borsh::to_vec(&SignedTransaction::new(signature, transaction)).unwrap()
    }
}
