//! Live-only Cup and Village state.
//!
//! The arena never places an order. Agents trade their own Hyperliquid accounts.
//! This module verifies account control, caches public reads, and scores the
//! resulting equity curve. It deliberately has no paper balance or simulated fill.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use fe2vi_market::{AccountState, Fill, LedgerUpdate, PortfolioWindow, Position};
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha3::{Digest as _, Keccak256};

use crate::agents::{authed_id, clean_evm, clean_near, handle_of, random_hex};
use crate::logic::{json, query_now, query_param, ArenaState, Incoming, Outcome};

pub const MIN_EQUITY_USD: f64 = 100.0;
pub const MAX_CUP_SEATS: usize = 8;
const CHALLENGE_SECS: u64 = 600;
const SNAPSHOT_LIMIT: usize = 500;
const FILL_LIMIT: usize = 400;
pub const POLICY_MAX_AGE_SECS: u64 = 24 * 3600;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySnapshot {
    pub agent_id: String,
    pub status: String,
    pub provider_policy_synced: bool,
    pub revision: Option<u64>,
    pub policy_hash: Option<String>,
    pub policy_frozen: bool,
    pub wallet_id: Option<String>,
    pub wallet_near_account: Option<String>,
    pub checked_unix: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyCheck {
    Active(PolicySnapshot),
    Inactive(PolicySnapshot, String),
    Unavailable(String),
}

impl PolicyCheck {
    pub fn active(snapshot: PolicySnapshot) -> Self {
        if snapshot.status == "APPLIED"
            && snapshot.provider_policy_synced
            && !snapshot.policy_frozen
        {
            Self::Active(snapshot)
        } else {
            let message = if snapshot.policy_frozen {
                format!(
                    "NEAR Intents policy is frozen with status {}.",
                    snapshot.status
                )
            } else {
                format!(
                    "NEAR Intents policy is {} with provider_policy_synced={}.",
                    snapshot.status, snapshot.provider_policy_synced
                )
            };
            Self::Inactive(snapshot, message)
        }
    }
}

pub fn valid_near_intents_agent_id(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn policy_snapshot_from_view(
    agent_id: &str,
    view: &serde_json::Value,
    checked_unix: u64,
) -> PolicySnapshot {
    PolicySnapshot {
        agent_id: agent_id.to_string(),
        status: view["status"]
            .as_str()
            .unwrap_or("NONE")
            .to_ascii_uppercase(),
        provider_policy_synced: view["provider_policy_synced"].as_bool().unwrap_or(false),
        revision: view["revision"].as_u64(),
        policy_hash: view["policy_hash"].as_str().map(str::to_string),
        policy_frozen: view["policy"]["frozen"].as_bool().unwrap_or(false),
        wallet_id: view["wallet_id"].as_str().map(str::to_string),
        wallet_near_account: None,
        checked_unix,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveBook {
    #[serde(default)]
    pub links: Vec<LiveLink>,
    #[serde(default)]
    pub challenges: Vec<LinkChallenge>,
    #[serde(default)]
    pub rounds: Vec<CupRound>,
    #[serde(default)]
    pub village: Vec<VillageRow>,
    #[serde(default)]
    pub fills: Vec<LiveFill>,
    #[serde(default)]
    pub awards: Vec<LiveAward>,
    #[serde(default)]
    pub last_tick_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveLink {
    pub agent_id: String,
    pub handle: String,
    pub near_account: String,
    pub derivation_path: String,
    pub hyperliquid_account: String,
    #[serde(default)]
    pub near_intents_agent_id: String,
    #[serde(default)]
    pub policy_status: String,
    #[serde(default)]
    pub provider_policy_synced: bool,
    #[serde(default)]
    pub policy_frozen: bool,
    #[serde(default)]
    pub policy_revision: Option<u64>,
    #[serde(default)]
    pub policy_hash: Option<String>,
    #[serde(default)]
    pub policy_checked_unix: u64,
    pub verified: bool,
    pub linked_unix: u64,
    pub last_sync_unix: u64,
    pub stale: bool,
    pub account_value: f64,
    pub withdrawable: f64,
    pub margin_used: f64,
    pub positions: Vec<Position>,
    pub portfolio: Vec<PortfolioWindow>,
    pub fills: Vec<Fill>,
    pub ledger: Vec<LedgerUpdate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkChallenge {
    pub nonce: String,
    pub agent_id: String,
    pub handle: String,
    pub near_account: String,
    pub derivation_path: String,
    pub hyperliquid_account: String,
    #[serde(default)]
    pub near_intents_agent_id: String,
    pub message: String,
    pub expires_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CupRound {
    pub round_id: String,
    pub start_unix: u64,
    pub end_unix: u64,
    pub status: String,
    pub created_unix: u64,
    pub settled_unix: u64,
    pub seats: Vec<CupSeat>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CupSeat {
    pub agent_id: String,
    pub handle: String,
    pub hyperliquid_account: String,
    pub start_equity: f64,
    pub end_equity: f64,
    pub peak_equity: f64,
    pub max_drawdown_bps: f64,
    pub return_bps: f64,
    pub risk_adjusted_bps: f64,
    pub pnl_usd: f64,
    pub volume_usd: f64,
    pub trades: u32,
    pub enrolled_unix: u64,
    pub eligible: bool,
    pub disqualified: Option<String>,
    pub rank: u32,
    pub points: u32,
    pub snapshots: Vec<EquitySnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquitySnapshot {
    pub t: u64,
    pub equity: f64,
    pub drawdown_bps: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VillageRow {
    pub agent_id: String,
    pub handle: String,
    pub hyperliquid_account: String,
    pub window: String,
    pub pnl_usd: f64,
    pub volume_usd: f64,
    pub account_value: f64,
    pub return_bps: f64,
    pub drawdown_bps: f64,
    pub score_bps: f64,
    pub updated_unix: u64,
    pub stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveFill {
    pub agent_id: String,
    pub handle: String,
    pub coin: String,
    pub side: String,
    pub dir: String,
    pub price: f64,
    pub size: f64,
    pub time: u64,
    pub closed_pnl: f64,
    pub fee: f64,
    pub tid: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAward {
    pub agent_id: String,
    pub handle: String,
    pub room: String,
    pub points: u32,
    pub unix: u64,
    pub label: String,
}

pub struct LinkUpdate {
    pub agent_id: String,
    pub account: AccountState,
    pub portfolio: Vec<PortfolioWindow>,
    pub fills: Vec<Fill>,
    pub ledger: Vec<LedgerUpdate>,
}

impl LiveLink {
    pub fn policy_active(&self) -> bool {
        self.policy_status == "APPLIED" && self.provider_policy_synced && !self.policy_frozen
    }

    pub fn policy_stale(&self, now: u64) -> bool {
        self.policy_checked_unix == 0
            || now.saturating_sub(self.policy_checked_unix) > POLICY_MAX_AGE_SECS
    }

    pub fn policy_active_at(&self, now: u64) -> bool {
        self.policy_active() && !self.policy_stale(now)
    }
}

pub(crate) fn apply_policy_snapshot(link: &mut LiveLink, snapshot: &PolicySnapshot) {
    link.policy_status = snapshot.status.clone();
    link.provider_policy_synced = snapshot.provider_policy_synced;
    link.policy_frozen = snapshot.policy_frozen;
    link.policy_revision = snapshot.revision;
    link.policy_hash = snapshot.policy_hash.clone();
    link.policy_checked_unix = snapshot.checked_unix;
}

pub(crate) fn mark_policy_mismatch(link: &mut LiveLink, checked_unix: u64) {
    link.policy_status = "MISMATCH".to_string();
    link.provider_policy_synced = false;
    link.policy_frozen = false;
    link.policy_revision = None;
    link.policy_hash = None;
    link.policy_checked_unix = checked_unix;
}

fn persist_policy(state: &mut ArenaState, link: &LiveLink) {
    if let Some(stored) = state.live.link_mut(&link.agent_id) {
        stored.policy_status = link.policy_status.clone();
        stored.provider_policy_synced = link.provider_policy_synced;
        stored.policy_frozen = link.policy_frozen;
        stored.policy_revision = link.policy_revision;
        stored.policy_hash = link.policy_hash.clone();
        stored.policy_checked_unix = link.policy_checked_unix;
    }
}

impl LiveBook {
    pub fn link(&self, agent_id: &str) -> Option<&LiveLink> {
        self.links.iter().find(|link| link.agent_id == agent_id)
    }

    pub fn link_by_handle(&self, handle: &str) -> Option<&LiveLink> {
        self.links.iter().find(|link| link.handle == handle)
    }

    pub fn link_mut(&mut self, agent_id: &str) -> Option<&mut LiveLink> {
        self.links.iter_mut().find(|link| link.agent_id == agent_id)
    }

    pub fn public_link(&self, handle: &str) -> Option<serde_json::Value> {
        let link = self.link_by_handle(handle)?;
        Some(link_json(link))
    }

    fn remove_link(&mut self, agent_id: &str) -> bool {
        let before = self.links.len();
        self.links.retain(|link| link.agent_id != agent_id);
        self.village.retain(|row| row.agent_id != agent_id);
        self.links.len() != before
    }

    fn cleanup(&mut self, now: u64) {
        self.challenges
            .retain(|challenge| challenge.expires_unix >= now);
    }

    fn push_award(&mut self, award: LiveAward) {
        if self
            .awards
            .iter()
            .any(|row| row.agent_id == award.agent_id && row.label == award.label)
        {
            return;
        }
        self.awards.push(award);
    }
}

pub fn dispatch_live(state: &mut ArenaState, incoming: &Incoming<'_>) -> Option<Outcome> {
    let path = path_only(incoming.path);
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let now = query_now(incoming.path);
    state.live.cleanup(now);
    match (incoming.method, parts.as_slice()) {
        ("POST", ["v1", "agents", "me", "hyperliquid-link", "challenge"]) => {
            Some(challenge(state, incoming, now))
        }
        ("POST", ["v1", "agents", "me", "hyperliquid-link"]) => Some(link(state, incoming, now)),
        ("POST", ["v1", "agents", "me", "hyperliquid-unlink"]) => {
            Some(unlink(state, incoming, now))
        }
        ("GET", ["v1", "cup", "rounds"]) => Some(rounds(state, now)),
        ("GET", ["v1", "cup", "rounds", "current"]) => Some(current_round(state, now)),
        ("POST", ["v1", "cup", "rounds", id, "join"]) => Some(join(state, incoming, id, now)),
        ("GET", ["v1", "cup", "rounds", id]) => Some(round_detail(state, id, now)),
        ("GET", ["v1", "cup", "rounds", id, "standings"]) => Some(round_detail(state, id, now)),
        ("GET", ["v1", "village", "leaderboard"]) => Some(village_board(state, incoming, now)),
        ("GET", ["v1", "village", "tape"]) => Some(village_tape(state, incoming)),
        ("GET", ["v1", "live", "accounts", handle]) => Some(account_for_handle(state, handle)),
        ("GET", ["v1", "live", "status"]) => Some(status(state, now)),
        _ => None,
    }
}

fn challenge(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    #[derive(Deserialize)]
    struct Body {
        #[serde(rename = "nearAccount")]
        near_account: Option<String>,
        #[serde(rename = "derivationPath")]
        derivation_path: Option<String>,
        #[serde(rename = "hyperliquidAccount")]
        hyperliquid_account: Option<String>,
        #[serde(rename = "nearIntentsAgentId")]
        near_intents_agent_id: Option<String>,
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let body: Body = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send nearAccount, derivationPath, hyperliquidAccount, and nearIntentsAgentId.",
            )
        }
    };
    let Some(near_account) = body
        .near_account
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return fail(400, "validation_error", "nearAccount is required.");
    };
    let near_account = match clean_near(near_account) {
        Ok(value) => value,
        Err(message) => return fail(400, "validation_error", message),
    };
    let Some(path) = body
        .derivation_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return fail(400, "validation_error", "derivationPath is required.");
    };
    if !(1..=120).contains(&path.len())
        || !path
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.'))
    {
        return fail(
            400,
            "validation_error",
            "derivationPath is 1 to 120 letters, numbers, /, -, _, or .",
        );
    }
    let Some(address) = body
        .hyperliquid_account
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return fail(400, "validation_error", "hyperliquidAccount is required.");
    };
    let address = match clean_evm(address) {
        Ok(value) => value,
        Err(message) => return fail(400, "validation_error", message),
    };
    let Some(near_intents_agent_id) = body
        .near_intents_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return fail(400, "validation_error", "nearIntentsAgentId is required.");
    };
    if !valid_near_intents_agent_id(near_intents_agent_id) {
        return fail(
            400,
            "validation_error",
            "nearIntentsAgentId is 64 hex characters.",
        );
    }
    if state
        .live
        .links
        .iter()
        .any(|link| link.agent_id != agent_id && link.hyperliquid_account == address)
    {
        return fail(
            409,
            "address_taken",
            "That Hyperliquid account is already linked to another agent.",
        );
    }
    let handle = handle_of(state, &agent_id);
    let nonce = random_hex(16);
    let expires_unix = now.saturating_add(CHALLENGE_SECS);
    let message = link_message(
        &handle,
        &near_account,
        path,
        &address,
        near_intents_agent_id,
        &nonce,
        expires_unix,
    );
    state.live.challenges.retain(|row| row.agent_id != agent_id);
    state.live.challenges.push(LinkChallenge {
        nonce: nonce.clone(),
        agent_id,
        handle,
        near_account,
        derivation_path: path.to_string(),
        hyperliquid_account: address,
        near_intents_agent_id: near_intents_agent_id.to_string(),
        message: message.clone(),
        expires_unix,
    });
    json(
        200,
        &serde_json::json!({
            "nonce": nonce,
            "message": message,
            "expiresAt": crate::agents::rfc3339(expires_unix),
            "standard": "ethereum-personal-sign",
        }),
        true,
        None,
    )
}

fn link(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    #[derive(Deserialize)]
    struct Body {
        nonce: Option<String>,
        signature: Option<String>,
        #[serde(rename = "nearAccount")]
        near_account: Option<String>,
        #[serde(rename = "derivationPath")]
        derivation_path: Option<String>,
        #[serde(rename = "hyperliquidAccount")]
        hyperliquid_account: Option<String>,
        #[serde(rename = "nearIntentsAgentId")]
        near_intents_agent_id: Option<String>,
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let body: Body = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send nonce, nearAccount, derivationPath, hyperliquidAccount, nearIntentsAgentId, and signature.",
            )
        }
    };
    let Some(nonce) = body.nonce.as_deref().map(str::trim) else {
        return fail(400, "validation_error", "nonce is required.");
    };
    let Some(signature) = body.signature.as_deref().map(str::trim) else {
        return fail(400, "validation_error", "signature is required.");
    };
    let Some(index) = state
        .live
        .challenges
        .iter()
        .position(|row| row.agent_id == agent_id && row.nonce == nonce)
    else {
        return fail(
            404,
            "challenge_not_found",
            "Request a new hyperliquid-link challenge.",
        );
    };
    let challenge = state.live.challenges[index].clone();
    if now > challenge.expires_unix {
        state.live.challenges.remove(index);
        return fail(409, "challenge_expired", "Request a new challenge.");
    }
    if body.near_account.as_deref().map(str::trim) != Some(challenge.near_account.as_str())
        || body.derivation_path.as_deref().map(str::trim)
            != Some(challenge.derivation_path.as_str())
        || body.hyperliquid_account.as_deref().map(str::trim)
            != Some(challenge.hyperliquid_account.as_str())
        || body.near_intents_agent_id.as_deref().map(str::trim)
            != Some(challenge.near_intents_agent_id.as_str())
    {
        return fail(
            400,
            "challenge_mismatch",
            "The link fields do not match the challenge.",
        );
    }
    if !verify_personal_signature(
        &challenge.hyperliquid_account,
        &challenge.message,
        signature,
    ) {
        return fail(
            400,
            "invalid_signature",
            "The signature does not recover to the linked Hyperliquid account.",
        );
    }
    if state.live.links.iter().any(|link| {
        link.agent_id != agent_id && link.hyperliquid_account == challenge.hyperliquid_account
    }) {
        return fail(
            409,
            "address_taken",
            "That Hyperliquid account is already linked to another agent.",
        );
    }
    let policy = match incoming.policy_check {
        Some(PolicyCheck::Active(snapshot))
            if snapshot.agent_id != challenge.near_intents_agent_id =>
        {
            return fail(
                403,
                "policy_mismatch",
                "The NEAR Intents policy check did not match nearIntentsAgentId.",
            )
        }
        Some(PolicyCheck::Active(snapshot))
            if snapshot.status != "APPLIED"
                || !snapshot.provider_policy_synced
                || snapshot.policy_frozen =>
        {
            return fail(
                403,
                "policy_not_ready",
                "The NEAR Intents policy is not APPLIED, synced, and unfrozen.",
            )
        }
        Some(PolicyCheck::Active(snapshot))
            if snapshot.wallet_near_account.as_deref() != Some(challenge.near_account.as_str()) =>
        {
            if snapshot.wallet_near_account.is_none() {
                return fail(
                    503,
                    "policy_verification_unavailable",
                    "The NEAR Intents provider did not return the agent wallet's near_account_id.",
                );
            }
            return fail(
                403,
                "policy_account_mismatch",
                "nearAccount does not match the NEAR Intents agent wallet.",
            );
        }
        Some(PolicyCheck::Active(snapshot)) => snapshot.clone(),
        Some(PolicyCheck::Inactive(_, message)) => {
            return fail(403, "policy_not_ready", message.as_str())
        }
        Some(PolicyCheck::Unavailable(message)) => {
            return fail(503, "policy_verification_unavailable", message.as_str())
        }
        None => {
            return fail(
                503,
                "policy_verification_unavailable",
                "Live linking needs a NEAR Intents policy check from the API edge.",
            )
        }
    };
    state.live.challenges.remove(index);
    let handle = challenge.handle.clone();
    let link = LiveLink {
        agent_id: agent_id.clone(),
        handle: handle.clone(),
        near_account: challenge.near_account.clone(),
        derivation_path: challenge.derivation_path.clone(),
        hyperliquid_account: challenge.hyperliquid_account.clone(),
        near_intents_agent_id: policy.agent_id.clone(),
        policy_status: policy.status.clone(),
        provider_policy_synced: policy.provider_policy_synced,
        policy_frozen: policy.policy_frozen,
        policy_revision: policy.revision,
        policy_hash: policy.policy_hash.clone(),
        policy_checked_unix: policy.checked_unix,
        verified: true,
        linked_unix: now,
        last_sync_unix: 0,
        stale: true,
        account_value: 0.0,
        withdrawable: 0.0,
        margin_used: 0.0,
        positions: Vec::new(),
        portfolio: Vec::new(),
        fills: Vec::new(),
        ledger: Vec::new(),
    };
    state.live.links.retain(|row| row.agent_id != agent_id);
    state.live.links.push(link.clone());
    if let Some(player) = state
        .book
        .players
        .iter_mut()
        .find(|player| player.agent_id == agent_id)
    {
        player.wallets.near_account = Some(challenge.near_account);
        player.wallets.hyperliquid_account = Some(challenge.hyperliquid_account);
        if !player
            .capabilities
            .iter()
            .any(|value| value == "live-trading")
        {
            player.capabilities.push("live-trading".to_string());
        }
    }
    json(200, &link_json(&link), true, None)
}

fn unlink(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    #[derive(Deserialize)]
    struct Body {
        message: Option<String>,
        signature: Option<String>,
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let body: Body = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send message and signature from the linked Hyperliquid key.",
            )
        }
    };
    let Some(link) = state.live.link(&agent_id).cloned() else {
        return fail(404, "not_linked", "No Hyperliquid account is linked.");
    };
    let Some(message) = body.message.as_deref() else {
        return fail(400, "validation_error", "message is required.");
    };
    let Some(signature) = body.signature.as_deref() else {
        return fail(400, "validation_error", "signature is required.");
    };
    if !verify_personal_signature(&link.hyperliquid_account, message, signature) {
        return fail(400, "invalid_signature", "The signature does not match.");
    }
    state.live.remove_link(&agent_id);
    if let Some(player) = state
        .book
        .players
        .iter_mut()
        .find(|player| player.agent_id == agent_id)
    {
        player.wallets.hyperliquid_account = None;
    }
    json(
        200,
        &serde_json::json!({
            "ok": true,
            "handle": link.handle,
            "hyperliquidAccount": link.hyperliquid_account,
            "updatedAt": crate::agents::rfc3339(now),
        }),
        true,
        None,
    )
}

fn rounds(state: &mut ArenaState, now: u64) -> Outcome {
    ensure_round(state, now);
    let rows: Vec<serde_json::Value> = state
        .live
        .rounds
        .iter()
        .rev()
        .take(12)
        .map(|round| round_json(round, now))
        .collect();
    json(
        200,
        &serde_json::json!({ "rounds": rows, "now": now }),
        false,
        None,
    )
}

fn current_round(state: &mut ArenaState, now: u64) -> Outcome {
    let round = ensure_round(state, now);
    json(200, &round_json(round, now), false, None)
}

fn join(state: &mut ArenaState, incoming: &Incoming<'_>, round_id: &str, now: u64) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let Some(link) = state.live.link(&agent_id).cloned() else {
        return fail(
            403,
            "link_required",
            "Link a chain-signature Hyperliquid account before joining the Cup.",
        );
    };
    if !link.verified {
        return fail(
            403,
            "link_unverified",
            "The Hyperliquid link is not verified.",
        );
    }
    let round = ensure_round(state, now);
    if round.round_id != round_id {
        return fail(404, "not_found", "That Cup round is not the current round.");
    }
    if now >= round.start_unix {
        return fail(
            409,
            "round_started",
            "Cup enrollment closes when the round starts.",
        );
    }
    if round.seats.len() >= MAX_CUP_SEATS {
        return fail(409, "round_full", "That Cup round is full.");
    }
    if round.seats.iter().any(|seat| seat.agent_id == agent_id) {
        return json(
            200,
            &serde_json::json!({
                "joined": true,
                "roundId": round.round_id,
                "handle": link.handle,
                "alreadySeated": true,
            }),
            false,
            None,
        );
    }
    let start_unix = round.start_unix;
    let end_unix = round.end_unix;

    let mut link = link;
    if let Some(check) = incoming.policy_check {
        match check {
            PolicyCheck::Active(snapshot) => {
                if snapshot.agent_id != link.near_intents_agent_id {
                    mark_policy_mismatch(&mut link, snapshot.checked_unix);
                    persist_policy(state, &link);
                    return fail_mutated(
                        403,
                        "policy_mismatch",
                        "The NEAR Intents policy check did not match nearIntentsAgentId.",
                        true,
                    );
                }
                if snapshot.status != "APPLIED"
                    || !snapshot.provider_policy_synced
                    || snapshot.policy_frozen
                {
                    apply_policy_snapshot(&mut link, snapshot);
                    persist_policy(state, &link);
                    return fail_mutated(
                        403,
                        "policy_not_ready",
                        "The NEAR Intents policy is not APPLIED, synced, and unfrozen.",
                        true,
                    );
                }
                let Some(wallet_near_account) = snapshot.wallet_near_account.as_deref() else {
                    return fail(
                        503,
                        "policy_verification_unavailable",
                        "The NEAR Intents provider did not return the agent wallet's near_account_id.",
                    );
                };
                if wallet_near_account != link.near_account {
                    mark_policy_mismatch(&mut link, snapshot.checked_unix);
                    persist_policy(state, &link);
                    return fail_mutated(
                        403,
                        "policy_account_mismatch",
                        "nearAccount does not match the NEAR Intents agent wallet.",
                        true,
                    );
                }
                apply_policy_snapshot(&mut link, snapshot);
                persist_policy(state, &link);
            }
            PolicyCheck::Inactive(snapshot, message) => {
                apply_policy_snapshot(&mut link, snapshot);
                persist_policy(state, &link);
                return fail_mutated(403, "policy_not_ready", message.as_str(), true);
            }
            PolicyCheck::Unavailable(message) => {
                return fail(503, "policy_verification_unavailable", message.as_str())
            }
        }
    }
    if link.policy_stale(now) {
        return fail(
            403,
            "policy_stale",
            "The NEAR Intents policy snapshot is older than 24 hours. Relink to recheck it.",
        );
    }
    if !link.policy_active() {
        return fail(
            403,
            "policy_not_verified",
            "Relink after the NEAR Intents policy reports APPLIED and provider_policy_synced.",
        );
    }
    let round = state
        .live
        .rounds
        .iter_mut()
        .find(|row| row.round_id == round_id)
        .unwrap();
    round.seats.push(CupSeat {
        agent_id,
        handle: link.handle.clone(),
        hyperliquid_account: link.hyperliquid_account.clone(),
        start_equity: 0.0,
        end_equity: 0.0,
        peak_equity: 0.0,
        max_drawdown_bps: 0.0,
        return_bps: 0.0,
        risk_adjusted_bps: 0.0,
        pnl_usd: 0.0,
        volume_usd: 0.0,
        trades: 0,
        enrolled_unix: now,
        eligible: false,
        disqualified: None,
        rank: 0,
        points: 0,
        snapshots: Vec::new(),
    });
    json(
        200,
        &serde_json::json!({
            "joined": true,
            "roundId": round_id,
            "handle": link.handle,
            "startsAt": crate::agents::rfc3339(start_unix),
            "endsAt": crate::agents::rfc3339(end_unix),
        }),
        true,
        None,
    )
}

fn round_detail(state: &mut ArenaState, round_id: &str, now: u64) -> Outcome {
    let Some(round) = state
        .live
        .rounds
        .iter()
        .find(|round| round.round_id == round_id)
    else {
        return fail(404, "not_found", "No Cup round with that id.");
    };
    json(200, &round_json(round, now), false, None)
}

fn village_board(state: &ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let window = query_param(incoming.path, "window").unwrap_or("24h");
    let Some(rows) = village_rows(state, window) else {
        return fail(
            400,
            "validation_error",
            "window must be 24h, 7d, 30d, or all.",
        );
    };
    let fresh = state
        .live
        .village
        .iter()
        .any(|row| row.window == window && !row.stale);
    json(
        200,
        &serde_json::json!({
            "window": window,
            "now": now,
            "source": if fresh { "hyperliquid" } else { "stale" },
            "rows": rows,
        }),
        false,
        None,
    )
}

fn village_tape(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let limit = query_param(incoming.path, "limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let rows: Vec<&LiveFill> = state.live.fills.iter().take(limit).collect();
    json(
        200,
        &serde_json::json!({
            "source": "hyperliquid",
            "fills": rows,
        }),
        false,
        None,
    )
}

fn account_for_handle(state: &ArenaState, handle: &str) -> Outcome {
    if let Some(link) = state.live.link_by_handle(handle) {
        return json(200, &link_json(link), false, None);
    }
    if state
        .book
        .players
        .iter()
        .any(|player| player.handle == handle)
    {
        return json(
            200,
            &serde_json::json!({
                "handle": handle,
                "verified": false,
                "linked": false,
                "message": "No chain-signature Hyperliquid account is linked.",
            }),
            false,
            None,
        );
    }
    fail(404, "not_found", "No agent with that handle.")
}

fn status(state: &ArenaState, now: u64) -> Outcome {
    json(
        200,
        &serde_json::json!({
            "now": now,
            "lastTickAt": if state.live.last_tick_unix == 0 { serde_json::Value::Null } else { serde_json::Value::String(crate::agents::rfc3339(state.live.last_tick_unix)) },
            "links": state.live.links.len(),
            "verifiedLinks": state.live.links.iter().filter(|link| link.verified).count(),
            "policyVerifiedLinks": state.live.links.iter().filter(|link| link.policy_active_at(now)).count(),
            "policyStaleLinks": state.live.links.iter().filter(|link| link.policy_stale(now)).count(),
            "rounds": state.live.rounds.len(),
            "fills": state.live.fills.len(),
            "mode": "live-only",
        }),
        false,
        None,
    )
}

fn link_json(link: &LiveLink) -> serde_json::Value {
    serde_json::json!({
        "agentId": link.agent_id,
        "handle": link.handle,
        "nearAccount": link.near_account,
        "derivationPath": link.derivation_path,
        "hyperliquidAccount": link.hyperliquid_account,
        "nearIntentsAgentId": link.near_intents_agent_id,
        "policyStatus": link.policy_status,
        "providerPolicySynced": link.provider_policy_synced,
        "policyFrozen": link.policy_frozen,
        "policyRevision": link.policy_revision,
        "policyHash": link.policy_hash,
        "policyCheckedAt": if link.policy_checked_unix == 0 { serde_json::Value::Null } else { serde_json::Value::String(crate::agents::rfc3339(link.policy_checked_unix)) },
        "verified": link.verified,
        "linkedAt": if link.linked_unix == 0 { serde_json::Value::Null } else { serde_json::Value::String(crate::agents::rfc3339(link.linked_unix)) },
        "lastSyncAt": if link.last_sync_unix == 0 { serde_json::Value::Null } else { serde_json::Value::String(crate::agents::rfc3339(link.last_sync_unix)) },
        "stale": link.stale,
        "accountValue": link.account_value,
        "withdrawable": link.withdrawable,
        "marginUsed": link.margin_used,
        "positions": link.positions,
        "portfolio": link.portfolio,
        "fills": link.fills.iter().take(50).collect::<Vec<_>>(),
    })
}

fn round_json(round: &CupRound, now: u64) -> serde_json::Value {
    let status = if round.settled_unix > 0 {
        "settled"
    } else if now < round.start_unix {
        "open"
    } else if now < round.end_unix {
        "live"
    } else {
        "settled"
    };
    let mut seats: Vec<&CupSeat> = round.seats.iter().collect();
    seats.sort_by(|left, right| {
        right
            .risk_adjusted_bps
            .partial_cmp(&left.risk_adjusted_bps)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.handle.cmp(&right.handle))
    });
    let rows: Vec<serde_json::Value> = seats
        .iter()
        .map(|seat| {
            serde_json::json!({
                "rank": seat.rank,
                "handle": seat.handle,
                "hyperliquidAccount": seat.hyperliquid_account,
                "startEquity": seat.start_equity,
                "endEquity": seat.end_equity,
                "returnBps": seat.return_bps,
                "drawdownBps": seat.max_drawdown_bps,
                "riskAdjustedBps": seat.risk_adjusted_bps,
                "pnlUsd": seat.pnl_usd,
                "volumeUsd": seat.volume_usd,
                "trades": seat.trades,
                "eligible": seat.eligible,
                "disqualified": seat.disqualified,
                "points": seat.points,
                "snapshots": seat.snapshots,
            })
        })
        .collect();
    serde_json::json!({
        "roundId": round.round_id,
        "status": status,
        "startsAt": crate::agents::rfc3339(round.start_unix),
        "endsAt": crate::agents::rfc3339(round.end_unix),
        "startUnix": round.start_unix,
        "endUnix": round.end_unix,
        "seats": rows,
        "seatCount": round.seats.len(),
        "maxSeats": MAX_CUP_SEATS,
        "mode": "live-only",
    })
}

fn village_rows(state: &ArenaState, window: &str) -> Option<Vec<serde_json::Value>> {
    let key = match window {
        "24h" => "24h",
        "7d" => "7d",
        "30d" => "30d",
        "all" => "all",
        _ => return None,
    };
    let mut rows: Vec<&VillageRow> = state
        .live
        .village
        .iter()
        .filter(|row| row.window == key)
        .collect();
    rows.sort_by(|left, right| {
        right
            .score_bps
            .partial_cmp(&left.score_bps)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .pnl_usd
                    .partial_cmp(&left.pnl_usd)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.handle.cmp(&right.handle))
    });
    let mut last_score = None;
    let mut last_rank = 0u32;
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let rank = if last_score == Some(row.score_bps) {
            last_rank
        } else {
            last_rank = index as u32 + 1;
            last_score = Some(row.score_bps);
            last_rank
        };
        out.push(serde_json::json!({
            "rank": rank,
            "handle": row.handle,
            "hyperliquidAccount": row.hyperliquid_account,
            "window": row.window,
            "pnlUsd": row.pnl_usd,
            "volumeUsd": row.volume_usd,
            "accountValue": row.account_value,
            "returnBps": row.return_bps,
            "drawdownBps": row.drawdown_bps,
            "scoreBps": row.score_bps,
            "updatedAt": crate::agents::rfc3339(row.updated_unix),
            "stale": row.stale,
        }));
    }
    Some(out)
}

fn ensure_round(state: &mut ArenaState, now: u64) -> &mut CupRound {
    let slot = fe2vi_core::weekly_slot(now);
    let round_id = format!("cup_{}", slot.start_unix);
    if let Some(index) = state
        .live
        .rounds
        .iter()
        .position(|round| round.round_id == round_id)
    {
        return &mut state.live.rounds[index];
    }
    state.live.rounds.push(CupRound {
        round_id,
        start_unix: slot.start_unix,
        end_unix: slot.end_unix,
        status: if now < slot.start_unix {
            "open"
        } else {
            "live"
        }
        .to_string(),
        created_unix: now,
        settled_unix: 0,
        seats: Vec::new(),
    });
    state.live.rounds.last_mut().expect("round")
}

pub fn apply_link_update(book: &mut LiveBook, update: LinkUpdate, now: u64) {
    let handle = book
        .link(&update.agent_id)
        .map(|link| link.handle.clone())
        .unwrap_or_default();
    if let Some(link) = book.link_mut(&update.agent_id) {
        link.account_value = update.account.account_value;
        link.withdrawable = update.account.withdrawable;
        link.margin_used = update.account.margin_used;
        link.positions = update.account.positions;
        link.portfolio = update.portfolio;
        link.fills = update.fills.clone();
        link.ledger = update.ledger;
        link.last_sync_unix = now;
        link.stale = false;
    }
    for fill in &update.fills {
        if book.fills.iter().any(|row| {
            row.agent_id == update.agent_id && row.tid == fill.tid && row.time == fill.time
        }) {
            continue;
        }
        book.fills.push(LiveFill {
            agent_id: update.agent_id.clone(),
            handle: handle.clone(),
            coin: fill.coin.clone(),
            side: fill.side.clone(),
            dir: fill.dir.clone(),
            price: fill.price,
            size: fill.size,
            time: fill.time,
            closed_pnl: fill.closed_pnl,
            fee: fill.fee,
            tid: fill.tid,
        });
    }
    book.fills.sort_by(|left, right| right.time.cmp(&left.time));
    book.fills.truncate(FILL_LIMIT);
    refresh_village(book, &update.agent_id, now);
}

pub fn mark_link_stale(book: &mut LiveBook, agent_id: &str) {
    if let Some(link) = book.link_mut(agent_id) {
        link.stale = true;
    }
    for row in &mut book.village {
        if row.agent_id == agent_id {
            row.stale = true;
        }
    }
}

pub fn refresh_seats(book: &mut LiveBook, now: u64) {
    let links = book.links.clone();
    for round in &mut book.rounds {
        if now >= round.start_unix && round.settled_unix == 0 && round.status == "open" {
            round.status = "live".to_string();
        }
        for seat in &mut round.seats {
            let link = links
                .iter()
                .find(|link| link.agent_id == seat.agent_id)
                .cloned();
            if seat.start_equity == 0.0 && now >= round.start_unix {
                let Some(link) = link.as_ref() else {
                    seat.disqualified = Some("link_missing".to_string());
                    continue;
                };
                if link.stale || link.last_sync_unix == 0 {
                    continue;
                }
                if !link.positions.is_empty() {
                    seat.disqualified = Some("not_flat_at_start".to_string());
                    seat.start_equity = link.account_value;
                    seat.peak_equity = link.account_value;
                    continue;
                }
                if link.account_value < MIN_EQUITY_USD {
                    seat.disqualified = Some("below_min_equity".to_string());
                    seat.start_equity = link.account_value;
                    seat.peak_equity = link.account_value;
                    continue;
                }
                seat.start_equity = link.account_value;
                seat.peak_equity = link.account_value;
                seat.end_equity = link.account_value;
            }
            let Some(link) = link else {
                continue;
            };
            if link.stale || seat.start_equity <= 0.0 {
                continue;
            }
            if now >= round.start_unix && now <= round.end_unix {
                let equity = link.account_value;
                seat.end_equity = equity;
                if equity > seat.peak_equity {
                    seat.peak_equity = equity;
                }
                if seat.peak_equity > 0.0 {
                    let dd = ((seat.peak_equity - equity) / seat.peak_equity * 10_000.0).max(0.0);
                    if dd > seat.max_drawdown_bps {
                        seat.max_drawdown_bps = dd;
                    }
                }
                let start_ms = round.start_unix.saturating_mul(1000);
                let in_window: Vec<&Fill> = link
                    .fills
                    .iter()
                    .filter(|fill| fill.time >= start_ms)
                    .collect();
                seat.trades = u32::try_from(in_window.len()).unwrap_or(u32::MAX);
                seat.volume_usd = in_window.iter().map(|fill| fill.price * fill.size).sum();
                if link.ledger.iter().any(|update| {
                    update.time >= start_ms
                        && matches!(
                            update.kind.as_str(),
                            "deposit" | "withdraw" | "internalTransfer" | "transfer"
                        )
                }) {
                    seat.disqualified = Some("transfer_during_round".to_string());
                }
                if seat
                    .snapshots
                    .last()
                    .map(|snapshot| now.saturating_sub(snapshot.t) >= 10)
                    .unwrap_or(true)
                {
                    seat.snapshots.push(EquitySnapshot {
                        t: now,
                        equity,
                        drawdown_bps: seat.max_drawdown_bps,
                    });
                    if seat.snapshots.len() > SNAPSHOT_LIMIT {
                        let extra = seat.snapshots.len() - SNAPSHOT_LIMIT;
                        seat.snapshots.drain(0..extra);
                    }
                }
            }
        }
        if now >= round.end_unix && round.settled_unix == 0 {
            for seat in &mut round.seats {
                if seat.disqualified.is_some() {
                    continue;
                }
                let stale = links
                    .iter()
                    .find(|link| link.agent_id == seat.agent_id)
                    .map(|link| link.stale || link.last_sync_unix < round.start_unix)
                    .unwrap_or(true);
                if stale {
                    seat.disqualified = Some("stale_at_end".to_string());
                }
            }
        }
    }
    let ended: Vec<String> = book
        .rounds
        .iter()
        .filter(|round| round.settled_unix == 0 && now >= round.end_unix)
        .map(|round| round.round_id.clone())
        .collect();
    for round_id in ended {
        finalize_round(book, round_id, now);
    }
}

fn finalize_round(book: &mut LiveBook, round_id: String, now: u64) {
    let Some(index) = book
        .rounds
        .iter()
        .position(|round| round.round_id == round_id)
    else {
        return;
    };
    let mut awards = Vec::new();
    {
        let round = &mut book.rounds[index];
        if round.settled_unix > 0 {
            return;
        }
        for seat in &mut round.seats {
            if seat.start_equity > 0.0 {
                seat.return_bps =
                    (seat.end_equity - seat.start_equity) / seat.start_equity * 10_000.0;
                seat.pnl_usd = seat.end_equity - seat.start_equity;
                seat.risk_adjusted_bps =
                    seat.return_bps - 0.5 * (seat.max_drawdown_bps - 200.0).max(0.0);
                seat.eligible = seat.disqualified.is_none() && seat.trades > 0;
            }
        }
        let mut ranked: Vec<&mut CupSeat> = round.seats.iter_mut().collect();
        ranked.sort_by(|left, right| {
            right
                .risk_adjusted_bps
                .partial_cmp(&left.risk_adjusted_bps)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    left.max_drawdown_bps
                        .partial_cmp(&right.max_drawdown_bps)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    right
                        .volume_usd
                        .partial_cmp(&left.volume_usd)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| left.enrolled_unix.cmp(&right.enrolled_unix))
                .then_with(|| left.handle.cmp(&right.handle))
        });
        let points = [10u32, 6, 4, 2, 1];
        for (position, seat) in ranked.iter_mut().enumerate() {
            seat.rank = if seat.eligible {
                position as u32 + 1
            } else {
                0
            };
            if seat.eligible && position < points.len() && seat.risk_adjusted_bps > 0.0 {
                seat.points = points[position];
                awards.push(LiveAward {
                    agent_id: seat.agent_id.clone(),
                    handle: seat.handle.clone(),
                    room: "cup".to_string(),
                    points: seat.points,
                    unix: now,
                    label: format!("cup round {round_id}"),
                });
            }
        }
        round.settled_unix = now;
        round.status = "settled".to_string();
    }
    for award in awards {
        book.push_award(award);
    }
}

fn refresh_village(book: &mut LiveBook, agent_id: &str, now: u64) {
    let Some(link) = book.link(agent_id).cloned() else {
        return;
    };
    book.village.retain(|row| row.agent_id != agent_id);
    for window in &link.portfolio {
        let key = match window.window.as_str() {
            "day" => "24h",
            "week" => "7d",
            "month" => "30d",
            "allTime" => "all",
            _ => continue,
        };
        let return_bps = if window.account_value_start > 0.0 {
            window.pnl / window.account_value_start * 10_000.0
        } else {
            0.0
        };
        let drawdown_bps = history_drawdown_bps(&window.history);
        let score_bps = return_bps - 0.5 * (drawdown_bps - 100.0).max(0.0);
        book.village.push(VillageRow {
            agent_id: agent_id.to_string(),
            handle: link.handle.clone(),
            hyperliquid_account: link.hyperliquid_account.clone(),
            window: key.to_string(),
            pnl_usd: window.pnl,
            volume_usd: window.volume,
            account_value: window.account_value_end,
            return_bps,
            drawdown_bps,
            score_bps,
            updated_unix: now,
            stale: link.stale,
        });
    }
}

fn history_drawdown_bps(history: &[fe2vi_market::EquityPoint]) -> f64 {
    let mut peak = 0.0f64;
    let mut worst = 0.0f64;
    for point in history {
        if point.value > peak {
            peak = point.value;
        }
        if peak > 0.0 {
            worst = worst.max((peak - point.value) / peak * 10_000.0);
        }
    }
    worst
}

fn link_message(
    handle: &str,
    near_account: &str,
    derivation_path: &str,
    hyperliquid_account: &str,
    near_intents_agent_id: &str,
    nonce: &str,
    expires_unix: u64,
) -> String {
    format!(
        "fe2VI link v1\nhandle: {handle}\nnear: {near_account}\npath: {derivation_path}\nhyperliquid: {hyperliquid_account}\nnearIntents: {near_intents_agent_id}\nnonce: {nonce}\nexpires: {expires_unix}"
    )
}

pub fn verify_personal_signature(address: &str, message: &str, signature: &str) -> bool {
    let Some(signature) = hex_bytes(signature) else {
        return false;
    };
    if signature.len() != 65 {
        return false;
    }
    let recovery_byte = match signature[64] {
        value @ 0..=1 => value,
        value @ 27..=28 => value - 27,
        value @ 35..=38 => (value - 35) % 2,
        _ => return false,
    };
    let Some(recovery) = RecoveryId::from_byte(recovery_byte) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature[..64]) else {
        return false;
    };
    let mut hasher = Keccak256::new();
    hasher.update(format!("\x19Ethereum Signed Message:\n{}", message.len()).as_bytes());
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();
    let Ok(key) = VerifyingKey::recover_from_prehash(digest.as_slice(), &signature, recovery)
    else {
        return false;
    };
    let encoded = key.to_encoded_point(false);
    let bytes = encoded.as_bytes();
    if bytes.len() < 65 {
        return false;
    }
    let hash = Keccak256::digest(&bytes[1..]);
    let recovered = format!("0x{}", to_hex(&hash[12..]));
    recovered.eq_ignore_ascii_case(address)
}

fn hex_bytes(value: &str) -> Option<Vec<u8>> {
    let value = value.trim().strip_prefix("0x").unwrap_or(value.trim());
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_nibble(bytes[index])?;
        let low = hex_nibble(bytes[index + 1])?;
        out.push((high << 4) | low);
        index += 2;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn path_only(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

fn fail(status: u16, code: &str, message: &str) -> Outcome {
    fail_mutated(status, code, message, false)
}

fn fail_mutated(status: u16, code: &str, message: &str, mutated: bool) -> Outcome {
    json(
        status,
        &serde_json::json!({ "error": code, "message": message }),
        mutated,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(handle: &str, start: f64, end: f64, dd: f64, trades: u32) -> CupSeat {
        CupSeat {
            agent_id: handle.to_string(),
            handle: handle.to_string(),
            hyperliquid_account: "0x1111111111111111111111111111111111111111".into(),
            start_equity: start,
            end_equity: end,
            peak_equity: start.max(end),
            max_drawdown_bps: dd,
            return_bps: 0.0,
            risk_adjusted_bps: 0.0,
            pnl_usd: 0.0,
            volume_usd: 0.0,
            trades,
            enrolled_unix: 0,
            eligible: false,
            disqualified: None,
            rank: 0,
            points: 0,
            snapshots: Vec::new(),
        }
    }

    #[test]
    fn scoring_rewards_return_and_penalizes_drawdown() {
        let mut book = LiveBook::default();
        book.rounds.push(CupRound {
            round_id: "cup_1".into(),
            start_unix: 100,
            end_unix: 200,
            status: "live".into(),
            created_unix: 0,
            settled_unix: 0,
            seats: vec![
                seat("steady", 1_000.0, 1_100.0, 100.0, 3),
                seat("wild", 1_000.0, 1_100.0, 1_000.0, 3),
                seat("idle", 1_000.0, 1_000.0, 0.0, 0),
            ],
        });
        finalize_round(&mut book, "cup_1".into(), 201);
        let round = &book.rounds[0];
        assert_eq!(round.seats[0].points, 10);
        assert_eq!(round.seats[0].handle, "steady");
        assert!(
            round
                .seats
                .iter()
                .find(|seat| seat.handle == "idle")
                .unwrap()
                .points
                == 0
        );
        assert_eq!(book.awards.len(), 2);
    }

    #[test]
    fn personal_signature_recovers_the_address() {
        use k256::ecdsa::SigningKey;
        let key = SigningKey::from_bytes((&[7u8; 32]).into()).unwrap();
        let public = key.verifying_key().to_encoded_point(false);
        let hash = Keccak256::digest(&public.as_bytes()[1..]);
        let address = format!("0x{}", to_hex(&hash[12..]));
        let message = "fe2VI link v1\nhandle: tape";
        let mut hasher = Keccak256::new();
        hasher.update(format!("\x19Ethereum Signed Message:\n{}", message.len()).as_bytes());
        hasher.update(message.as_bytes());
        let digest = hasher.finalize();
        let (signature, recovery) = key.sign_prehash_recoverable(digest.as_slice()).unwrap();
        let mut bytes = signature.to_bytes().to_vec();
        bytes.push(recovery.to_byte());
        let signature = format!("0x{}", to_hex(&bytes));
        assert!(verify_personal_signature(&address, message, &signature));
        assert!(!verify_personal_signature(&address, "other", &signature));
        let mut legacy = hex_bytes(&signature).unwrap();
        legacy[64] += 27;
        let legacy = format!("0x{}", to_hex(&legacy));
        assert!(verify_personal_signature(&address, message, &legacy));
    }

    #[test]
    fn challenge_links_a_signing_key_to_the_agent() {
        use crate::logic::dispatch;
        use k256::ecdsa::SigningKey;
        let key = SigningKey::from_bytes((&[9u8; 32]).into()).unwrap();
        let public = key.verifying_key().to_encoded_point(false);
        let hash = Keccak256::digest(&public.as_bytes()[1..]);
        let address = format!("0x{}", to_hex(&hash[12..]));
        let now = fe2vi_core::weekly_slot(1_790_900_000)
            .start_unix
            .saturating_sub(30);
        let mut state = crate::logic::ArenaState::default();
        let registered = dispatch(
            &mut state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/agents/register?now={now}"),
                body: r#"{"handle":"linker","name":"Linker","description":"trades"}"#,
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let token = match registered {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 201, "{body}");
                serde_json::from_str::<serde_json::Value>(&body).unwrap()["apiToken"]
                    .as_str()
                    .unwrap()
                    .to_string()
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        };
        let auth = format!("Bearer {token}");
        let near_intents_agent_id = "a".repeat(64);
        let challenge = dispatch(
            &mut state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/agents/me/hyperliquid-link/challenge?now={now}"),
                body: &serde_json::json!({
                    "nearAccount": "linker.near",
                    "derivationPath": "fe2vi/hyperliquid/linker",
                    "hyperliquidAccount": address,
                    "nearIntentsAgentId": near_intents_agent_id,
                })
                .to_string(),
                authorization: Some(&auth),
                ingest_token: None,
                policy_check: None,
            },
        );
        let (nonce, message) = match challenge {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 200, "{body}");
                let value: serde_json::Value = serde_json::from_str(&body).unwrap();
                (
                    value["nonce"].as_str().unwrap().to_string(),
                    value["message"].as_str().unwrap().to_string(),
                )
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        };
        let mut hasher = Keccak256::new();
        hasher.update(format!("\x19Ethereum Signed Message:\n{}", message.len()).as_bytes());
        hasher.update(message.as_bytes());
        let digest = hasher.finalize();
        let (signature, recovery) = key.sign_prehash_recoverable(digest.as_slice()).unwrap();
        let mut signature_bytes = signature.to_bytes().to_vec();
        signature_bytes.push(recovery.to_byte());
        let signature = format!("0x{}", to_hex(&signature_bytes));
        let link_body = serde_json::json!({
            "nonce": nonce,
            "nearAccount": "linker.near",
            "derivationPath": "fe2vi/hyperliquid/linker",
            "hyperliquidAccount": address,
            "nearIntentsAgentId": near_intents_agent_id,
            "signature": signature,
        })
        .to_string();
        let missing = dispatch(
            &mut state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/agents/me/hyperliquid-link?now={now}"),
                body: &link_body,
                authorization: Some(&auth),
                ingest_token: None,
                policy_check: None,
            },
        );
        match missing {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 503, "{body}");
                assert!(body.contains("policy_verification_unavailable"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        let inactive = PolicyCheck::Inactive(
            PolicySnapshot {
                agent_id: near_intents_agent_id.clone(),
                status: "SIGNED".into(),
                provider_policy_synced: false,
                revision: Some(3),
                policy_hash: Some("old".into()),
                policy_frozen: false,
                wallet_id: Some("wallet".into()),
                wallet_near_account: None,
                checked_unix: now,
            },
            "policy is SIGNED".into(),
        );
        let rejected = dispatch(
            &mut state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/agents/me/hyperliquid-link?now={now}"),
                body: &link_body,
                authorization: Some(&auth),
                ingest_token: None,
                policy_check: Some(&inactive),
            },
        );
        match rejected {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 403, "{body}");
                assert!(body.contains("policy_not_ready"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        let mismatched_account = PolicyCheck::Active(PolicySnapshot {
            agent_id: near_intents_agent_id.clone(),
            status: "APPLIED".into(),
            provider_policy_synced: true,
            revision: Some(4),
            policy_hash: Some("hash".into()),
            policy_frozen: false,
            wallet_id: Some("wallet".into()),
            wallet_near_account: Some("other.near".into()),
            checked_unix: now,
        });
        let rejected_account = dispatch(
            &mut state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/agents/me/hyperliquid-link?now={now}"),
                body: &link_body,
                authorization: Some(&auth),
                ingest_token: None,
                policy_check: Some(&mismatched_account),
            },
        );
        match rejected_account {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 403, "{body}");
                assert!(body.contains("policy_account_mismatch"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        let policy = PolicyCheck::Active(PolicySnapshot {
            agent_id: near_intents_agent_id.clone(),
            status: "APPLIED".into(),
            provider_policy_synced: true,
            revision: Some(4),
            policy_hash: Some("hash".into()),
            policy_frozen: false,
            wallet_id: Some("wallet".into()),
            wallet_near_account: Some("linker.near".into()),
            checked_unix: now,
        });
        let linked = dispatch(
            &mut state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/agents/me/hyperliquid-link?now={now}"),
                body: &link_body,
                authorization: Some(&auth),
                ingest_token: None,
                policy_check: Some(&policy),
            },
        );
        match linked {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 200, "{body}");
                let value: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(value["verified"], true);
                assert_eq!(value["hyperliquidAccount"], address);
                assert_eq!(value["policyStatus"], "APPLIED");
                assert_eq!(value["providerPolicySynced"], true);
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        assert!(state.live.links[0].verified);
        assert!(state.live.links[0].policy_active());
        assert_eq!(
            state.book.players[0].wallets.hyperliquid_account,
            Some(address)
        );
    }

    #[test]
    fn policy_view_requires_applied_and_synced() {
        assert!(valid_near_intents_agent_id(&"a".repeat(64)));
        assert!(!valid_near_intents_agent_id("abc"));
        let view = serde_json::json!({
            "status": "APPLIED",
            "provider_policy_synced": true,
            "revision": 7,
            "policy_hash": "0xhash",
            "wallet_id": "wallet",
        });
        let snapshot = policy_snapshot_from_view(&"a".repeat(64), &view, 42);
        assert_eq!(snapshot.status, "APPLIED");
        assert_eq!(snapshot.revision, Some(7));
        assert_eq!(snapshot.policy_hash.as_deref(), Some("0xhash"));
        assert_eq!(snapshot.wallet_id.as_deref(), Some("wallet"));
        assert!(snapshot.wallet_near_account.is_none());
        assert!(matches!(
            PolicyCheck::active(snapshot),
            PolicyCheck::Active(_)
        ));
        let unsigned = policy_snapshot_from_view(
            &"a".repeat(64),
            &serde_json::json!({ "status": "SIGNED", "provider_policy_synced": false }),
            42,
        );
        assert!(matches!(
            PolicyCheck::active(unsigned),
            PolicyCheck::Inactive(..)
        ));
        let frozen = policy_snapshot_from_view(
            &"a".repeat(64),
            &serde_json::json!({
                "status": "APPLIED",
                "provider_policy_synced": true,
                "policy": { "frozen": true },
            }),
            42,
        );
        assert!(frozen.policy_frozen);
        assert!(matches!(
            PolicyCheck::active(frozen),
            PolicyCheck::Inactive(..)
        ));
    }

    fn live_link(agent_id: &str, near_account: &str, checked_unix: u64) -> LiveLink {
        LiveLink {
            agent_id: agent_id.into(),
            handle: "joiner".into(),
            near_account: near_account.into(),
            derivation_path: "fe2vi/hyperliquid/joiner".into(),
            hyperliquid_account: "0x1111111111111111111111111111111111111111".into(),
            near_intents_agent_id: "a".repeat(64),
            policy_status: "APPLIED".into(),
            provider_policy_synced: true,
            policy_frozen: false,
            policy_revision: Some(1),
            policy_hash: Some("old".into()),
            policy_checked_unix: checked_unix,
            verified: true,
            linked_unix: checked_unix,
            last_sync_unix: 0,
            stale: false,
            account_value: 0.0,
            withdrawable: 0.0,
            margin_used: 0.0,
            positions: Vec::new(),
            portfolio: Vec::new(),
            fills: Vec::new(),
            ledger: Vec::new(),
        }
    }

    fn register_agent(state: &mut ArenaState, handle: &str, now: u64) -> (String, String) {
        let outcome = crate::logic::dispatch(
            state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/agents/register?now={now}"),
                body: &format!(
                    r#"{{"handle":"{handle}","name":"{handle}","description":"joins"}}"#
                ),
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let Outcome::Http { status, body, .. } = outcome else {
            panic!("expected http");
        };
        assert_eq!(status, 201, "{body}");
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        (
            value["agentId"].as_str().unwrap().to_string(),
            value["apiToken"].as_str().unwrap().to_string(),
        )
    }

    fn join_with_policy(
        state: &mut ArenaState,
        auth: &str,
        round_id: &str,
        now: u64,
        policy_check: Option<&PolicyCheck>,
    ) -> Outcome {
        crate::logic::dispatch(
            state,
            &crate::logic::Incoming {
                method: "POST",
                path: &format!("/v1/cup/rounds/{round_id}/join?now={now}"),
                body: "{}",
                authorization: Some(auth),
                ingest_token: None,
                policy_check,
            },
        )
    }

    #[test]
    fn policy_recheck_tracks_changes_revocation_and_freeze() {
        let now = 1_000_000;
        let mut link = live_link("agt_test", "joiner.near", now);
        link.policy_checked_unix = now.saturating_sub(POLICY_MAX_AGE_SECS);
        assert!(!link.policy_stale(now));
        link.policy_checked_unix = now.saturating_sub(POLICY_MAX_AGE_SECS + 1);
        assert!(link.policy_stale(now));
        let changed = PolicySnapshot {
            agent_id: "a".repeat(64),
            status: "APPLIED".into(),
            provider_policy_synced: true,
            revision: Some(9),
            policy_hash: Some("new".into()),
            policy_frozen: false,
            wallet_id: Some("wallet".into()),
            wallet_near_account: Some("joiner.near".into()),
            checked_unix: now,
        };
        apply_policy_snapshot(&mut link, &changed);
        assert!(link.policy_active_at(now));
        assert_eq!(link.policy_revision, Some(9));
        assert_eq!(link.policy_hash.as_deref(), Some("new"));

        let revoked = PolicySnapshot {
            status: "SIGNED".into(),
            provider_policy_synced: false,
            revision: Some(10),
            checked_unix: now + 1,
            ..changed.clone()
        };
        apply_policy_snapshot(&mut link, &revoked);
        assert!(!link.policy_active_at(now + 1));
        assert_eq!(link.policy_status, "SIGNED");

        let frozen = PolicySnapshot {
            status: "APPLIED".into(),
            provider_policy_synced: true,
            policy_frozen: true,
            revision: Some(11),
            checked_unix: now + 2,
            ..changed
        };
        assert!(matches!(
            PolicyCheck::active(frozen.clone()),
            PolicyCheck::Inactive(..)
        ));
        apply_policy_snapshot(&mut link, &frozen);
        assert!(!link.policy_active_at(now + 2));
        assert!(link.policy_frozen);
    }

    #[test]
    fn cup_join_rechecks_policy_and_rejects_stale_or_unbound_links() {
        let now = fe2vi_core::weekly_slot(1_790_900_000)
            .start_unix
            .saturating_sub(30);
        let mut state = ArenaState::default();
        let (agent_id, token) = register_agent(&mut state, "joiner", now);
        state.live.links.push(live_link(
            &agent_id,
            "joiner.near",
            now.saturating_sub(POLICY_MAX_AGE_SECS + 1),
        ));
        let round = ensure_round(&mut state, now);
        let round_id = round.round_id.clone();
        let auth = format!("Bearer {token}");

        let stale = join_with_policy(&mut state, &auth, &round_id, now, None);
        let Outcome::Http { status, body, .. } = stale else {
            panic!("expected http");
        };
        assert_eq!(status, 403, "{body}");
        assert!(body.contains("policy_stale"));

        let unbound = PolicyCheck::Active(PolicySnapshot {
            agent_id: "a".repeat(64),
            status: "APPLIED".into(),
            provider_policy_synced: true,
            revision: Some(2),
            policy_hash: Some("new".into()),
            policy_frozen: false,
            wallet_id: Some("wallet".into()),
            wallet_near_account: None,
            checked_unix: now,
        });
        let missing = join_with_policy(&mut state, &auth, &round_id, now, Some(&unbound));
        let Outcome::Http { status, body, .. } = missing else {
            panic!("expected http");
        };
        assert_eq!(status, 503, "{body}");
        assert!(body.contains("policy_verification_unavailable"));

        let fresh = PolicyCheck::Active(PolicySnapshot {
            wallet_near_account: Some("joiner.near".into()),
            ..match unbound {
                PolicyCheck::Active(snapshot) => snapshot,
                _ => unreachable!(),
            }
        });
        let joined = join_with_policy(&mut state, &auth, &round_id, now, Some(&fresh));
        let Outcome::Http { status, body, .. } = joined else {
            panic!("expected http");
        };
        assert_eq!(status, 200, "{body}");
        let stored = state.live.link(&agent_id).unwrap();
        assert_eq!(stored.policy_revision, Some(2));
        assert_eq!(stored.policy_hash.as_deref(), Some("new"));
        assert_eq!(stored.policy_checked_unix, now);
    }

    #[test]
    fn village_rows_keep_the_requested_windows() {
        let mut book = LiveBook::default();
        book.links.push(LiveLink {
            agent_id: "agt".into(),
            handle: "tape".into(),
            near_account: "tape.near".into(),
            derivation_path: "fe2vi/hyperliquid/tape".into(),
            hyperliquid_account: "0x1111111111111111111111111111111111111111".into(),
            near_intents_agent_id: "b".repeat(64),
            policy_status: "APPLIED".into(),
            provider_policy_synced: true,
            policy_frozen: false,
            policy_revision: Some(3),
            policy_hash: Some("hash".into()),
            policy_checked_unix: 1,
            verified: true,
            linked_unix: 1,
            last_sync_unix: 0,
            stale: false,
            account_value: 1_000.0,
            withdrawable: 1_000.0,
            margin_used: 0.0,
            positions: Vec::new(),
            portfolio: vec![PortfolioWindow {
                window: "day".into(),
                account_value_start: 1_000.0,
                account_value_end: 1_100.0,
                pnl: 100.0,
                volume: 5_000.0,
                history: vec![
                    fe2vi_market::EquityPoint {
                        t: 1,
                        value: 1_000.0,
                    },
                    fe2vi_market::EquityPoint { t: 2, value: 900.0 },
                    fe2vi_market::EquityPoint {
                        t: 3,
                        value: 1_100.0,
                    },
                ],
            }],
            fills: Vec::new(),
            ledger: Vec::new(),
        });
        refresh_village(&mut book, "agt", 10);
        assert_eq!(book.village.len(), 1);
        assert_eq!(book.village[0].window, "24h");
        assert!(book.village[0].drawdown_bps > 0.0);
    }
}
