//! Local signing. The secret is borrowed and is not written to logs.

use std::str::FromStr;

use near_crypto::SecretKey;
use near_primitives::hash::CryptoHash;
use near_primitives::transaction::{
    FunctionCallAction, SignedTransaction, Transaction, TransactionV0,
};
use near_primitives::types::AccountId;

pub struct SignedCall<'a> {
    pub signer_id: String,
    pub secret: &'a str,
    pub receiver_id: String,
    pub method: String,
    pub args: Vec<u8>,
    pub gas: u64,
    pub deposit: u128,
    pub nonce: u64,
    pub block_hash: String,
}

pub fn sign_call(call: &SignedCall<'_>) -> Result<Vec<u8>, String> {
    let secret: SecretKey = call
        .secret
        .parse()
        .map_err(|_| "signing key could not be parsed".to_string())?;
    let signer_id: AccountId = call
        .signer_id
        .parse()
        .map_err(|_| "signer account id is invalid".to_string())?;
    let receiver_id: AccountId = call
        .receiver_id
        .parse()
        .map_err(|_| "receiver account id is invalid".to_string())?;
    let block_hash =
        CryptoHash::from_str(&call.block_hash).map_err(|_| "block hash is invalid".to_string())?;
    let transaction = Transaction::V0(TransactionV0 {
        signer_id,
        public_key: secret.public_key(),
        nonce: call.nonce,
        receiver_id,
        block_hash,
        actions: vec![FunctionCallAction {
            method_name: call.method.clone(),
            args: call.args.clone(),
            gas: call.gas,
            deposit: call.deposit,
        }
        .into()],
    });
    let (hash, _) = transaction.get_hash_and_size();
    let signature = secret.sign(hash.as_ref());
    let signed = SignedTransaction::new(signature, transaction);
    borsh::to_vec(&signed)
        .map_err(|err| format!("could not serialize the signed transaction: {err}"))
}
