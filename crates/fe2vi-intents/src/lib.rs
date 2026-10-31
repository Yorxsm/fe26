//! Adapter between fe2VI platform caps and a NEAR Intents agent account.
//!
//! Default mode is a mock. Nothing in this crate contacts the network unless
//! the `live` feature is compiled and [`live_gate`] is true. Even then, fe2VI
//! only prepares a dry quote. It does not broadcast a swap, transfer, or
//! withdrawal. The hosted API is mainnet and moves real funds.
//!
//! API keys (`naa_`) and grant tokens (`ngt_`) are never returned, logged, or
//! stored. A mock grant keeps the hash of a discarded token.

use fe2vi_core::{PlatformCaps, ONE_NEAR, WRAP_NEAR};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MOCK_USD_PER_NEAR_CENTS: u64 = 500;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredGrant {
    pub grant_id: String,
    pub game_id: String,
    pub agent: String,
    pub label: String,
    pub commitment: String,
    pub expires_at_unix: u64,
    pub revoked: bool,
    pub spent_yocto: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedGrant {
    pub grant_id: String,
    pub label: String,
    pub commitment: String,
    pub expires_at_unix: u64,
    pub mode: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentsError {
    UnknownGrant,
    Revoked,
    Expired,
    OverActionCap,
    OverAgentCap,
    SpendRefused,
}

impl IntentsError {
    pub fn code(self) -> &'static str {
        match self {
            Self::UnknownGrant | Self::Revoked | Self::Expired => "agent_grant_required",
            Self::OverActionCap | Self::OverAgentCap => "policy_denied",
            Self::SpendRefused => "spend_refused",
        }
    }
}

/// Both the explicit flag and a non-empty key are required. Either one alone stays mock.
pub fn live_gate(live_flag: &str, api_key: &str) -> bool {
    live_flag == "1" && !api_key.is_empty()
}

/// Strip secret-shaped tokens from a string before it is logged or returned.
pub fn redact_secrets(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        let rest = &text[index..];
        if rest.starts_with("naa_") || rest.starts_with("ngt_") {
            out.push_str("[redacted]");
            index += 4;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || bytes[index] == b'_'
                    || bytes[index] == b'-')
            {
                index += 1;
            }
        } else {
            let ch = rest.chars().next().unwrap_or(' ');
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

/// Whole-NEAR truncation is intentional: the USD sketch is a budget label, not a quote.
pub fn usd_from_yocto(yocto: u128, cents_per_near: u64) -> String {
    let near = yocto / ONE_NEAR;
    let cents = near.saturating_mul(u128::from(cents_per_near));
    let cents = u64::try_from(cents).unwrap_or(u64::MAX);
    format!("{}.{:02}", cents / 100, cents % 100)
}

fn scale_usd(daily: &str, factor: u64) -> String {
    let cents: u64 = daily
        .split_once('.')
        .and_then(|(dollars, frac)| {
            let major = dollars.parse::<u64>().ok()?;
            let minor = format!("{frac:0<2}").chars().take(2).collect::<String>();
            let minor = minor.parse::<u64>().ok()?;
            Some(major.saturating_mul(100).saturating_add(minor))
        })
        .unwrap_or(0);
    let scaled = cents.saturating_mul(factor);
    format!("{}.{:02}", scaled / 100, scaled % 100)
}

/// Platform caps become one complete policy document. Destinations stay `only`
/// with an empty list, so a grant cannot withdraw to an outside address.
pub fn policy_from_caps(caps: &PlatformCaps, cents_per_near: u64) -> serde_json::Value {
    let per_tx = caps.max_per_action_yocto.to_string();
    let daily_near = caps.max_per_agent_yocto.to_string();
    let daily_usd = usd_from_yocto(caps.max_per_agent_yocto, cents_per_near);
    serde_json::json!({
        "frozen": false,
        "actions": ["swap", "transfer", "withdraw"],
        "confidential": false,
        "owner_approval": false,
        "assets": [WRAP_NEAR],
        "limits": {
            "per_transaction": { WRAP_NEAR: per_tx },
            "daily": { WRAP_NEAR: daily_near }
        },
        "max_actions_per_hour": 60,
        "destinations": { "mode": "only", "list": [] },
        "budget": {
            "daily_usd": daily_usd,
            "weekly_usd": scale_usd(&daily_usd, 7),
            "monthly_usd": scale_usd(&daily_usd, 30)
        },
        "timelock_ms": 0
    })
}

pub fn issue_session(
    grants: &mut Vec<StoredGrant>,
    game_id: &str,
    agent: &str,
    now: u64,
    ttl_secs: u64,
) -> IssuedGrant {
    let label = format!("fe2vi:{game_id}:{agent}");
    let material = format!("{label}:{now}:{}", grants.len());
    let digest = Sha256::digest(material.as_bytes());
    let commitment = hex::encode(digest);
    let grant_id = hex::encode(Sha256::digest(commitment.as_bytes()));
    let expires_at_unix = now.saturating_add(ttl_secs);
    grants.push(StoredGrant {
        grant_id: grant_id.clone(),
        game_id: game_id.to_string(),
        agent: agent.to_string(),
        label: label.clone(),
        commitment: commitment.clone(),
        expires_at_unix,
        revoked: false,
        spent_yocto: 0,
    });
    IssuedGrant {
        grant_id,
        label,
        commitment,
        expires_at_unix,
        mode: "mock",
    }
}

pub fn revoke_session(grants: &mut [StoredGrant], grant_id: &str) -> bool {
    if let Some(grant) = grants.iter_mut().find(|grant| grant.grant_id == grant_id) {
        grant.revoked = true;
        true
    } else {
        false
    }
}

/// A mock action inside the caps. This never becomes a chain or API write.
pub fn mock_act(
    grants: &mut [StoredGrant],
    grant_id: &str,
    amount_yocto: u128,
    caps: &PlatformCaps,
    now: u64,
) -> Result<serde_json::Value, IntentsError> {
    let agent_spent = {
        let grant = grants
            .iter()
            .find(|grant| grant.grant_id == grant_id)
            .ok_or(IntentsError::UnknownGrant)?;
        if grant.revoked {
            return Err(IntentsError::Revoked);
        }
        if now >= grant.expires_at_unix {
            return Err(IntentsError::Expired);
        }
        if amount_yocto > caps.max_per_action_yocto {
            return Err(IntentsError::OverActionCap);
        }
        grants
            .iter()
            .filter(|item| item.agent == grant.agent)
            .map(|item| item.spent_yocto)
            .fold(0u128, u128::saturating_add)
    };
    if agent_spent.saturating_add(amount_yocto) > caps.max_per_agent_yocto {
        return Err(IntentsError::OverAgentCap);
    }
    let grant = grants
        .iter_mut()
        .find(|grant| grant.grant_id == grant_id)
        .ok_or(IntentsError::UnknownGrant)?;
    grant.spent_yocto = grant.spent_yocto.saturating_add(amount_yocto);
    Ok(serde_json::json!({
        "ok": true,
        "mode": "mock",
        "dry": true,
        "broadcast": false,
        "grant_id": grant_id,
        "amount_yocto": amount_yocto.to_string()
    }))
}

/// Live mode still refuses a broadcast. Dry quotes are the only remote call.
pub fn refuse_broadcast() -> IntentsError {
    IntentsError::SpendRefused
}

#[cfg(feature = "live")]
mod remote {
    use super::redact_secrets;
    use near_intents_agent_api::AgentApi;

    /// Read-only token list and a dry swap quote. No idempotency key, no grant, no broadcast.
    pub async fn remote_dry_swap(
        base_url: &str,
        api_key: &str,
        agent_id: &str,
        amount: &str,
    ) -> Result<serde_json::Value, String> {
        let api = AgentApi::new(api_key, Some(base_url))
            .map_err(|err| redact_secrets(&err.to_string()))?;
        let tokens = api
            .get_tokens()
            .await
            .map_err(|err| redact_secrets(&err.to_string()))?;
        let destination = tokens
            .as_array()
            .and_then(|list| {
                list.iter().find_map(|token| {
                    let id = token.get("asset_id")?.as_str()?;
                    if id == fe2vi_core::WRAP_NEAR {
                        None
                    } else {
                        Some(id.to_string())
                    }
                })
            })
            .ok_or_else(|| "token list had no second asset".to_string())?;
        let quote = api
            .quote_swap(
                agent_id,
                serde_json::json!({
                    "origin_asset": fe2vi_core::WRAP_NEAR,
                    "destination_asset": destination,
                    "amount": amount,
                }),
            )
            .await
            .map_err(|err| redact_secrets(&err.to_string()))?;
        Ok(quote)
    }
}

#[cfg(feature = "live")]
pub use remote::remote_dry_swap;

#[cfg(test)]
mod tests {
    use super::*;
    use fe2vi_core::default_platform_caps;

    #[test]
    fn caps_map_onto_per_asset_limits_and_a_usd_budget() {
        let caps = default_platform_caps();
        let policy = policy_from_caps(&caps, MOCK_USD_PER_NEAR_CENTS);
        assert_eq!(
            policy["limits"]["per_transaction"][WRAP_NEAR],
            caps.max_per_action_yocto.to_string()
        );
        assert_eq!(
            policy["limits"]["daily"][WRAP_NEAR],
            caps.max_per_agent_yocto.to_string()
        );
        assert_eq!(policy["budget"]["daily_usd"], "25.00");
        assert_eq!(policy["budget"]["weekly_usd"], "175.00");
        assert_eq!(policy["destinations"]["mode"], "only");
        assert!(policy["destinations"]["list"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_revoked_grant_and_an_over_cap_act_are_refused() {
        let caps = default_platform_caps();
        let mut grants = Vec::new();
        let issued = issue_session(&mut grants, "g1", "mason.village.fe2vi.near", 1_000, 60);
        assert!(!issued.commitment.is_empty());
        assert!(!redact_secrets(&format!("{issued:?}")).contains("ngt_"));
        let ok = mock_act(&mut grants, &issued.grant_id, 1, &caps, 1_010).unwrap();
        assert_eq!(ok["broadcast"], false);
        assert_eq!(ok["mode"], "mock");
        assert_eq!(
            mock_act(
                &mut grants,
                &issued.grant_id,
                caps.max_per_action_yocto + 1,
                &caps,
                1_010
            ),
            Err(IntentsError::OverActionCap)
        );
        assert!(revoke_session(&mut grants, &issued.grant_id));
        assert_eq!(
            mock_act(&mut grants, &issued.grant_id, 1, &caps, 1_010),
            Err(IntentsError::Revoked)
        );
    }

    #[test]
    fn the_live_gate_needs_the_flag_and_a_key() {
        assert!(!live_gate("0", "present"));
        assert!(!live_gate("1", ""));
        assert!(live_gate("1", "present"));
        assert_eq!(refuse_broadcast(), IntentsError::SpendRefused);
    }

    #[test]
    fn redaction_removes_secret_prefixes() {
        let cleaned = redact_secrets("header naa_exampletoken extra ngt_othertoken tail");
        assert!(!cleaned.contains("naa_"));
        assert!(!cleaned.contains("ngt_"));
        assert!(cleaned.contains("[redacted]"));
    }
}

#[cfg(all(test, feature = "live"))]
mod live_gate_tests {
    use super::*;

    #[tokio::test]
    async fn a_short_key_fails_before_any_request() {
        let err = remote_dry_swap("https://127.0.0.1:9", "naa_short", "agent", "1")
            .await
            .unwrap_err();
        assert!(!err.contains("naa_"));
        assert!(err.contains("[redacted]") || err.contains("api_key"));
    }
}
