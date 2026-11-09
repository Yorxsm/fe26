//! Hyperliquid testnet orders. The agent key is read from the environment or
//! a file and is never printed. The arena does not see it. Mainnet is refused.

use std::time::{SystemTime, UNIX_EPOCH};

use fe2vi_core::{refuse_testnet, HYPERLIQUID_TESTNET_EXCHANGE, HYPERLIQUID_TESTNET_INFO};
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

pub const TESTNET_EXCHANGE: &str = HYPERLIQUID_TESTNET_EXCHANGE;
pub const TESTNET_INFO: &str = HYPERLIQUID_TESTNET_INFO;

/// HIP-3 asset id: 100000 + dex index * 10000 + index inside that dex meta.
pub fn hip3_asset_id(dex_index: u32, coin_index: u32) -> u32 {
    100_000 + dex_index.saturating_mul(10_000) + coin_index
}

pub fn order_action(
    asset: u32,
    is_buy: bool,
    price: &str,
    size: &str,
    tif: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "order",
        "orders": [{
            "a": asset,
            "b": is_buy,
            "p": price,
            "s": size,
            "r": false,
            "t": { "limit": { "tif": tif } }
        }],
        "grouping": "na"
    })
}

pub fn cancel_action(asset: u32, oid: u64) -> serde_json::Value {
    serde_json::json!({
        "type": "cancel",
        "cancels": [{ "a": asset, "o": oid }]
    })
}

/// Msgpack the action the way the Hyperliquid L1 hash expects, then append the
/// nonce and the no-vault marker.
pub fn action_hash_preimage(action: &serde_json::Value, nonce: u64) -> Result<Vec<u8>, String> {
    let mut packed = pack_action(action)?;
    packed.extend_from_slice(&nonce.to_be_bytes());
    packed.push(0);
    Ok(packed)
}

fn pack_action(action: &serde_json::Value) -> Result<Vec<u8>, String> {
    match action.get("type").and_then(|value| value.as_str()) {
        Some("order") => pack_order(action),
        Some("cancel") => pack_cancel(action),
        _ => Err("only order and cancel are signed".into()),
    }
}

fn pack_order(action: &serde_json::Value) -> Result<Vec<u8>, String> {
    let order = action
        .pointer("/orders/0")
        .ok_or("order action needs one order")?;
    let asset = order
        .get("a")
        .and_then(|value| value.as_u64())
        .ok_or("asset missing")? as u32;
    let is_buy = order
        .get("b")
        .and_then(|value| value.as_bool())
        .ok_or("side missing")?;
    let price = order
        .get("p")
        .and_then(|value| value.as_str())
        .ok_or("price missing")?;
    let size = order
        .get("s")
        .and_then(|value| value.as_str())
        .ok_or("size missing")?;
    let tif = order
        .pointer("/t/limit/tif")
        .and_then(|value| value.as_str())
        .ok_or("tif missing")?;
    let mut order_bytes = pack_map(&[
        ("a", pack_uint(u64::from(asset))),
        ("b", vec![if is_buy { 0xc3 } else { 0xc2 }]),
        ("p", pack_str(price)),
        ("s", pack_str(size)),
        ("r", vec![0xc2]),
        (
            "t",
            pack_map(&[("limit", pack_map(&[("tif", pack_str(tif))]))]),
        ),
    ]);
    let mut orders = vec![0x91];
    orders.append(&mut order_bytes);
    Ok(pack_map(&[
        ("type", pack_str("order")),
        ("orders", orders),
        ("grouping", pack_str("na")),
    ]))
}

fn pack_cancel(action: &serde_json::Value) -> Result<Vec<u8>, String> {
    let row = action
        .pointer("/cancels/0")
        .ok_or("cancel action needs one cancel")?;
    let asset = row
        .get("a")
        .and_then(|value| value.as_u64())
        .ok_or("asset missing")? as u32;
    let oid = row
        .get("o")
        .and_then(|value| value.as_u64())
        .ok_or("oid missing")?;
    let cancel = pack_map(&[("a", pack_uint(u64::from(asset))), ("o", pack_uint(oid))]);
    let mut cancels = vec![0x91];
    cancels.extend(cancel);
    Ok(pack_map(&[
        ("type", pack_str("cancel")),
        ("cancels", cancels),
    ]))
}

fn pack_map(pairs: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0x80 | pairs.len() as u8];
    for (key, value) in pairs {
        out.extend(pack_str(key));
        out.extend(value);
    }
    out
}

fn pack_str(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() + 2);
    if bytes.len() < 32 {
        out.push(0xa0 | bytes.len() as u8);
    } else {
        out.push(0xd9);
        out.push(bytes.len() as u8);
    }
    out.extend_from_slice(bytes);
    out
}

fn pack_uint(value: u64) -> Vec<u8> {
    if value < 128 {
        vec![value as u8]
    } else if value <= u64::from(u8::MAX) {
        vec![0xcc, value as u8]
    } else if value <= u64::from(u16::MAX) {
        let bytes = (value as u16).to_be_bytes();
        vec![0xcd, bytes[0], bytes[1]]
    } else {
        let bytes = (value as u32).to_be_bytes();
        vec![0xce, bytes[0], bytes[1], bytes[2], bytes[3]]
    }
}

pub fn sign_l1(
    secret_hex: &str,
    action: &serde_json::Value,
    nonce: u64,
) -> Result<serde_json::Value, String> {
    let secret = decode_key(secret_hex)?;
    let preimage = action_hash_preimage(action, nonce)?;
    let connection = Keccak256::digest(&preimage);
    let digest = eip712_agent_digest(&connection, "b");
    let key = SigningKey::from_bytes((&secret).into())
        .map_err(|_| "agent key is not a secp256k1 scalar".to_string())?;
    let (signature, recovery) = key
        .sign_prehash_recoverable(digest.as_slice())
        .map_err(|_| "could not sign the order".to_string())?;
    let bytes = signature.to_bytes();
    let r = format!("0x{}", hex_encode(&bytes[..32]));
    let s = format!("0x{}", hex_encode(&bytes[32..]));
    let v = 27 + u64::from(recovery.to_byte());
    Ok(serde_json::json!({ "r": r, "s": s, "v": v }))
}

pub fn exchange_body(
    action: serde_json::Value,
    nonce: u64,
    signature: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "action": action,
        "nonce": nonce,
        "signature": signature
    })
}

pub fn submit_exchange(url: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    refuse_testnet(url).map_err(|err| err.to_string())?;
    if !url.contains("hyperliquid-testnet") {
        return Err("order host must be the Hyperliquid testnet".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client
        .post(url)
        .json(body)
        .send()
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().map_err(|err| err.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or(serde_json::json!({ "raw": text }));
    if !status.is_success() {
        return Err(format!("hyperliquid testnet returned {status}"));
    }
    Ok(value)
}

pub fn post_info(body: &serde_json::Value) -> Result<serde_json::Value, String> {
    refuse_testnet(TESTNET_INFO).map_err(|err| err.to_string())?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client
        .post(TESTNET_INFO)
        .json(body)
        .send()
        .map_err(|err| err.to_string())?;
    let text = response.text().map_err(|err| err.to_string())?;
    serde_json::from_str(&text).map_err(|_| "hyperliquid info was not json".to_string())
}

/// Resolve a coin such as `BTC` or `xyz:TSLA` to an asset id on testnet.
pub fn resolve_asset(coin: &str) -> Result<(u32, String), String> {
    let coin = coin.trim();
    if let Some((dex, name)) = coin.split_once(':') {
        let dexes = post_info(&serde_json::json!({ "type": "perpDexs" }))?;
        let list = dexes.as_array().ok_or("perpDexs was not a list")?;
        let dex_index = list
            .iter()
            .position(|row| row.get("name").and_then(|value| value.as_str()) == Some(dex))
            .ok_or_else(|| format!("dex {dex} is not on Hyperliquid testnet"))?
            as u32;
        let meta = post_info(&serde_json::json!({ "type": "meta", "dex": dex }))?;
        let universe = meta
            .get("universe")
            .and_then(|value| value.as_array())
            .ok_or("dex meta had no universe")?;
        let coin_index = universe
            .iter()
            .position(|row| {
                row.get("name").and_then(|value| value.as_str()) == Some(coin)
                    || row.get("name").and_then(|value| value.as_str()) == Some(name)
            })
            .ok_or_else(|| format!("{coin} is not in dex {dex}"))? as u32;
        let full = universe[coin_index as usize]
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or(coin)
            .to_string();
        return Ok((hip3_asset_id(dex_index, coin_index), full));
    }
    let meta = post_info(&serde_json::json!({ "type": "meta" }))?;
    let universe = meta
        .get("universe")
        .and_then(|value| value.as_array())
        .ok_or("meta had no universe")?;
    let index = universe
        .iter()
        .position(|row| row.get("name").and_then(|value| value.as_str()) == Some(coin))
        .ok_or_else(|| format!("{coin} is not a testnet perp"))? as u32;
    Ok((index, coin.to_string()))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn address_of(secret_hex: &str) -> Result<String, String> {
    let secret = decode_key(secret_hex)?;
    let key = SigningKey::from_bytes((&secret).into())
        .map_err(|_| "agent key is not a secp256k1 scalar".to_string())?;
    let public = key.verifying_key().to_encoded_point(false);
    let hash = Keccak256::digest(&public.as_bytes()[1..]);
    Ok(format!("0x{}", hex_encode(&hash[12..])))
}

fn eip712_agent_digest(connection_id: &[u8], source: &str) -> [u8; 32] {
    let domain_type = Keccak256::digest(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    );
    let name = Keccak256::digest(b"Exchange");
    let version = Keccak256::digest(b"1");
    let mut chain = [0u8; 32];
    chain[30] = 0x05;
    chain[31] = 0x39;
    let domain = keccak_words(&[
        domain_type.as_slice(),
        name.as_slice(),
        version.as_slice(),
        &chain,
        &[0u8; 32],
    ]);
    let agent_type = Keccak256::digest(b"Agent(string source,bytes32 connectionId)");
    let source_hash = Keccak256::digest(source.as_bytes());
    let mut connection = [0u8; 32];
    connection.copy_from_slice(&connection_id[..32]);
    let message = keccak_words(&[agent_type.as_slice(), source_hash.as_slice(), &connection]);
    let mut prefix = vec![0x19, 0x01];
    prefix.extend_from_slice(&domain);
    prefix.extend_from_slice(&message);
    let digest = Keccak256::digest(&prefix);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn keccak_words(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for part in parts {
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn decode_key(secret_hex: &str) -> Result<[u8; 32], String> {
    let text = secret_hex.trim().trim_start_matches("0x");
    if text.len() != 64 {
        return Err("FE2VI_HL_AGENT_KEY must be 32 bytes of hex".into());
    }
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        let pair = &text[index * 2..index * 2 + 2];
        *byte = u8::from_str_radix(pair, 16).map_err(|_| "agent key is not hex".to_string())?;
    }
    Ok(out)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKED: &str = "83a474797065a56f72646572a66f72646572739186a16100a162c3a170a131a173a5302e303031a172c2a17481a56c696d697481a3746966a3416c6fa867726f7570696e67a26e610000018bcfe5680000";

    #[test]
    fn hip3_ids_match_the_published_formula() {
        assert_eq!(hip3_asset_id(1, 0), 110_000);
        assert_eq!(hip3_asset_id(65, 1), 750_001);
    }

    #[test]
    fn order_preimage_matches_msgpack() {
        let action = order_action(0, true, "1", "0.001", "Alo");
        let packed = action_hash_preimage(&action, 1_700_000_000_000).unwrap();
        assert_eq!(hex_encode(&packed), PACKED);
        let big = order_action(750_001, true, "1", "0.001", "Alo");
        let big_packed = pack_action(&big).unwrap();
        assert_eq!(
            hex_encode(&big_packed),
            "83a474797065a56f72646572a66f72646572739186a161ce000b71b1a162c3a170a131a173a5302e303031a172c2a17481a56c696d697481a3746966a3416c6fa867726f7570696e67a26e61"
        );
        let cancel = cancel_action(4, 777);
        assert_eq!(
            hex_encode(&pack_action(&cancel).unwrap()),
            "82a474797065a663616e63656ca763616e63656c739182a16104a16fcd0309"
        );
    }

    #[test]
    fn a_signature_recovers_the_agent_address_and_mainnet_is_refused() {
        let secret = "07".repeat(32);
        let action = order_action(4, true, "1", "0.001", "Alo");
        let signature = sign_l1(&secret, &action, 1_700_000_000_000).unwrap();
        assert!(signature["r"].as_str().unwrap().starts_with("0x"));
        let v = signature["v"].as_u64().unwrap();
        assert!(v == 27 || v == 28);
        let address = address_of(&secret).unwrap();
        assert!(address.starts_with("0x"));
        assert_eq!(address.len(), 42);
        assert!(refuse_testnet("https://api.hyperliquid.xyz/exchange").is_err());
        assert!(submit_exchange(
            "https://api.hyperliquid.xyz/exchange",
            &serde_json::json!({})
        )
        .is_err());
    }
}
