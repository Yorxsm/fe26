//! Passport mirror and mint queue. The NEAR contract owns the NFT; this module
//! only stores enough state to gate scored entry points and hand work to the
//! signer relay.

use serde::Deserialize;

use crate::agents::{self, Passport, PassportJob};
use crate::logic::{json, query_now, ArenaState, Incoming, Outcome};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PassportGate {
    #[default]
    Off,
    Warn,
    On,
}

impl PassportGate {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("on") => Self::On,
            Some("warn") => Self::Warn,
            _ => Self::Off,
        }
    }
}

/// What `agent_token` on the disposable testnet passport contract returned
/// for one seated agent. `Token("")` means the chain shows no binding.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainSeatBinding {
    Token(String),
    /// The Fastnear read failed. This seat gets no credit.
    Unverified,
}

thread_local! {
    static MATCH_END_CHAIN: std::cell::RefCell<Option<std::collections::HashMap<String, ChainSeatBinding>>> =
        const { std::cell::RefCell::new(None) };
}

/// Install the chain snapshot for this request. `None` means the read was not
/// attempted, which match end treats as unverified.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn set_match_end_chain(reads: Option<std::collections::HashMap<String, ChainSeatBinding>>) {
    MATCH_END_CHAIN.with(|slot| *slot.borrow_mut() = reads);
}

pub fn match_end_chain() -> Option<std::collections::HashMap<String, ChainSeatBinding>> {
    MATCH_END_CHAIN.with(|slot| slot.borrow().clone())
}

/// Seated agents whose match can end on this request. A quiet read does not
/// call the chain.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn seats_pending_chain_read(
    state: &ArenaState,
    now: u64,
    method: &str,
    path: &str,
) -> Vec<String> {
    let action_match = if method == "POST" {
        path.strip_prefix("/v1/matches/")
            .and_then(|rest| rest.strip_suffix("/actions"))
            .filter(|id| !id.is_empty() && !id.contains('/'))
    } else {
        None
    };
    let mut ids = Vec::new();
    for play in &state.book.matches {
        if play.status != "live" || play.vault.is_none() {
            continue;
        }
        let due = now >= play.deadline_unix;
        let acted = action_match == Some(play.match_id.as_str());
        if !due && !acted {
            continue;
        }
        for seat in &play.seats {
            if !ids.iter().any(|id| id == seat) {
                ids.push(seat.clone());
            }
        }
    }
    ids
}

pub fn active_for_agent(state: &ArenaState, agent_id: &str) -> bool {
    state
        .book
        .passports
        .iter()
        .any(|passport| passport.agent_id == agent_id && passport.status == "active")
}

pub fn scored(state: &ArenaState, agent_id: &str) -> bool {
    state.passport_gate != PassportGate::On || active_for_agent(state, agent_id)
}

pub fn require_active(state: &ArenaState, agent_id: &str) -> Option<Outcome> {
    if scored(state, agent_id) {
        return None;
    }
    Some(fail(
        403,
        "passport_required",
        "Mint or buy a passport before entering a scored arena.",
    ))
}

pub fn dispatch(state: &mut ArenaState, incoming: &Incoming<'_>) -> Option<Outcome> {
    let path = incoming.path.split('?').next().unwrap_or(incoming.path);
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let now = query_now(incoming.path);
    match (incoming.method, parts.as_slice()) {
        ("GET", ["v1", "market", "listings"]) => Some(market_listings(state)),
        ("GET", ["v1", "market", "sales"]) => Some(market_sales(state)),
        ("GET", ["v1", "passports", token_id]) if !token_id.is_empty() => {
            Some(public_passport(state, token_id))
        }
        ("GET", ["v1", "agents", "me", "passport"]) => Some(my_passport(state, incoming)),
        ("GET", ["v1", "agents", "me", "passport", "jobs"]) => Some(my_jobs(state, incoming)),
        ("POST", ["v1", "agents", "me", "passport", "mint"]) => Some(mint(state, incoming, now)),
        ("POST", ["v1", "agents", "me", "passport", "bind"]) => Some(bind(state, incoming, now)),
        ("POST", ["v1", "agents", "me", "passport", "unbind"]) => {
            Some(unbind(state, incoming, now))
        }
        ("POST", ["v1", "internal", "passport", "jobs", "claim"]) => {
            Some(claim(state, incoming, now))
        }
        ("POST", ["v1", "internal", "passport", "jobs", job_id, "complete"]) => {
            Some(complete(state, incoming, job_id, now))
        }
        _ => None,
    }
}

#[derive(Deserialize)]
struct MintBody {
    #[serde(rename = "ownerNear")]
    owner_near: Option<String>,
    #[serde(rename = "agentNear")]
    agent_near: Option<String>,
}

#[derive(Deserialize)]
struct BindBody {
    #[serde(rename = "tokenId")]
    token_id: Option<String>,
    #[serde(rename = "agentNear")]
    agent_near: Option<String>,
    #[serde(rename = "ownerNear")]
    owner_near: Option<String>,
}

fn mint(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    if !state.passport_mint {
        return fail(
            409,
            "mint_disabled",
            "Mint is closed. Buy a launch passport on the market for 1 NEAR.",
        );
    }
    let Some(agent_id) = agents::authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let body: MintBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send ownerNear and agentNear."),
    };
    let owner_near = match clean_near(body.owner_near.as_deref()) {
        Ok(value) => value,
        Err(message) => return fail(400, "validation_error", message),
    };
    let agent_near = match body.agent_near.as_deref() {
        Some(value) if !value.trim().is_empty() => match clean_near(Some(value)) {
            Ok(value) => value,
            Err(message) => return fail(400, "validation_error", message),
        },
        _ => owner_near.clone(),
    };
    if state.book.passports.iter().any(|passport| {
        passport.agent_id == agent_id
            && matches!(
                passport.status.as_str(),
                "pending_mint" | "active" | "pending_bind" | "pending_unbind"
            )
    }) {
        return fail(
            409,
            "passport_exists",
            "This agent already has a passport or a pending passport change.",
        );
    }
    let passport_id = format!("ppt_{}", agents::random_hex(8));
    let job_id = format!("job_{}", agents::random_hex(8));
    let passport = Passport {
        passport_id: passport_id.clone(),
        token_id: None,
        agent_id: agent_id.clone(),
        owner_near: owner_near.clone(),
        creator_near: owner_near.clone(),
        status: "pending_mint".to_string(),
        created_unix: now,
        updated_unix: now,
        last_sync_seq: 0,
        career: None,
    };
    let job = PassportJob {
        job_id: job_id.clone(),
        kind: "mint".to_string(),
        passport_id: passport_id.clone(),
        agent_id,
        owner_near,
        agent_near,
        status: "pending".to_string(),
        attempts: 0,
        created_unix: now,
        updated_unix: now,
        tx_hash: String::new(),
        error: String::new(),
        tx_block_hash: String::new(),
        tx_block_height: 0,
    };
    state.book.passports.push(passport.clone());
    state.book.passport_jobs.push(job.clone());
    json(
        202,
        &serde_json::json!({
            "passport": passport_json(&passport),
            "job": job_json(&job),
            "message": "Mint queued. The passport activates after the relay confirms it.",
        }),
        true,
        None,
    )
}

fn queued_bind(state: &ArenaState, agent_id: &str, token_id: &str) -> Option<Outcome> {
    let job = state.book.passport_jobs.iter().find(|job| {
        job.kind == "bind"
            && (job.status == "pending" || job.status == "in_progress")
            && state.book.passports.iter().any(|passport| {
                passport.passport_id == job.passport_id
                    && passport.token_id.as_deref() == Some(token_id)
            })
            && (job.agent_id == agent_id
                || state.book.passports.iter().any(|passport| {
                    passport.passport_id == job.passport_id
                        && passport.token_id.as_deref() == Some(token_id)
                }))
    })?;
    let passport = state
        .book
        .passports
        .iter()
        .find(|passport| passport.passport_id == job.passport_id);
    Some(json(
        409,
        &serde_json::json!({
            "error": "bind_pending",
            "message": "A bind for this token is already queued.",
            "job": job_json(job),
            "passport": passport.map(passport_json),
        }),
        false,
        None,
    ))
}

fn bind(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let Some(agent_id) = agents::authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let body: BindBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send tokenId and agentNear."),
    };
    let Some(token_id) = body
        .token_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return fail(400, "validation_error", "Send tokenId.");
    };
    let agent_near = match clean_near(body.agent_near.as_deref()) {
        Ok(value) => value,
        Err(message) => return fail(400, "validation_error", message),
    };
    let owner_near = match body.owner_near.as_deref() {
        Some(value) if !value.trim().is_empty() => match clean_near(Some(value)) {
            Ok(value) => Some(value),
            Err(message) => return fail(400, "validation_error", message),
        },
        _ => None,
    };
    if state.book.passports.iter().any(|passport| {
        passport.agent_id == agent_id
            && passport.token_id.as_deref() != Some(token_id)
            && matches!(
                passport.status.as_str(),
                "pending_mint" | "active" | "pending_bind" | "pending_unbind"
            )
    }) {
        return fail(
            409,
            "passport_exists",
            "This agent already has a passport. Unbind it before claiming another.",
        );
    }
    if let Some(outcome) = queued_bind(state, &agent_id, token_id) {
        return outcome;
    }
    let existing = state
        .book
        .passports
        .iter()
        .position(|passport| passport.token_id.as_deref() == Some(token_id));
    let (passport_id, owner_near, pending) = match existing {
        Some(index)
            if state.book.passports[index].agent_id.is_empty()
                || state.book.passports[index].agent_id == agent_id =>
        {
            if matches!(
                state.book.passports[index].status.as_str(),
                "pending_mint" | "pending_bind" | "pending_unbind"
            ) {
                return fail(
                    409,
                    "invalid_state_transition",
                    "The passport is not active yet.",
                );
            }
            state.book.passports[index].agent_id = agent_id.clone();
            let owner =
                owner_near.unwrap_or_else(|| state.book.passports[index].owner_near.clone());
            state.book.passports[index].owner_near = owner.clone();
            state.book.passports[index].status = "pending_bind".to_string();
            state.book.passports[index].updated_unix = now;
            let passport_id = state.book.passports[index].passport_id.clone();
            let pending = state.book.passports[index].clone();
            (passport_id, owner, Some(pending))
        }
        Some(index) => {
            let Some(owner) = owner_near else {
                return fail(
                    400,
                    "validation_error",
                    "Send ownerNear with tokenId when claiming a purchased passport.",
                );
            };
            (
                state.book.passports[index].passport_id.clone(),
                owner,
                Some(state.book.passports[index].clone()),
            )
        }
        None => {
            let Some(owner) = owner_near else {
                return fail(
                    400,
                    "validation_error",
                    "Send ownerNear with tokenId when claiming a purchased passport.",
                );
            };
            let passport = Passport {
                passport_id: format!("ppt_{}", agents::random_hex(8)),
                token_id: Some(token_id.to_string()),
                agent_id: agent_id.clone(),
                owner_near: owner.clone(),
                creator_near: String::new(),
                status: "pending_bind".to_string(),
                created_unix: now,
                updated_unix: now,
                last_sync_seq: 0,
                career: None,
            };
            state.book.passports.push(passport.clone());
            (passport.passport_id.clone(), owner, Some(passport))
        }
    };
    let job = PassportJob {
        job_id: format!("job_{}", agents::random_hex(8)),
        kind: "bind".to_string(),
        passport_id,
        agent_id: agent_id.clone(),
        owner_near,
        agent_near,
        status: "pending".to_string(),
        attempts: 0,
        created_unix: now,
        updated_unix: now,
        tx_hash: String::new(),
        error: String::new(),
        tx_block_hash: String::new(),
        tx_block_height: 0,
    };
    state.book.passport_jobs.push(job.clone());
    json(
        202,
        &serde_json::json!({
            "passport": pending.map(|passport| passport_json(&passport)),
            "job": job_json(&job),
            "message": "Bind queued. The relay verifies on-chain ownership before binding.",
        }),
        true,
        None,
    )
}

fn unbind(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let Some(agent_id) = agents::authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let Some(index) = state
        .book
        .passports
        .iter()
        .position(|passport| passport.agent_id == agent_id && passport.status == "active")
    else {
        return fail(404, "not_found", "This agent has no active passport.");
    };
    let passport = &mut state.book.passports[index];
    passport.status = "pending_unbind".to_string();
    passport.updated_unix = now;
    let job = PassportJob {
        job_id: format!("job_{}", agents::random_hex(8)),
        kind: "unbind".to_string(),
        passport_id: passport.passport_id.clone(),
        agent_id: agent_id.clone(),
        owner_near: passport.owner_near.clone(),
        agent_near: String::new(),
        status: "pending".to_string(),
        attempts: 0,
        created_unix: now,
        updated_unix: now,
        tx_hash: String::new(),
        error: String::new(),
        tx_block_hash: String::new(),
        tx_block_height: 0,
    };
    state.book.passport_jobs.push(job.clone());
    json(
        202,
        &serde_json::json!({
            "passport": passport_json(passport),
            "job": job_json(&job),
        }),
        true,
        None,
    )
}

#[derive(Deserialize)]
struct ClaimBody {
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct CompleteBody {
    status: Option<String>,
    #[serde(rename = "tokenId")]
    token_id: Option<String>,
    #[serde(rename = "txHash")]
    tx_hash: Option<String>,
    #[serde(rename = "txBlockHash")]
    tx_block_hash: Option<String>,
    error: Option<String>,
    #[serde(rename = "lastSyncSeq")]
    last_sync_seq: Option<u64>,
    career: Option<serde_json::Value>,
}

fn claim(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let body: ClaimBody = serde_json::from_str(incoming.body).unwrap_or(ClaimBody { limit: None });
    let limit = body.limit.unwrap_or(10).clamp(1, 50);
    let claimed: Vec<usize> = state
        .book
        .passport_jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| {
            job.status == "pending"
                || (job.status == "in_progress" && now.saturating_sub(job.updated_unix) > 300)
        })
        .map(|(index, _)| index)
        .take(limit)
        .collect();
    let mut jobs = Vec::new();
    for index in claimed {
        let job = &mut state.book.passport_jobs[index];
        job.status = "in_progress".to_string();
        job.attempts = job.attempts.saturating_add(1);
        job.updated_unix = now;
        let job = state.book.passport_jobs[index].clone();
        jobs.push(job_wire(state, &job));
    }
    json(
        200,
        &serde_json::json!({ "jobs": jobs }),
        !jobs.is_empty(),
        None,
    )
}

fn complete(state: &mut ArenaState, incoming: &Incoming<'_>, job_id: &str, now: u64) -> Outcome {
    let body: CompleteBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send status and optional txHash."),
    };
    let Some(job_index) = state
        .book
        .passport_jobs
        .iter()
        .position(|job| job.job_id == job_id)
    else {
        return fail(404, "not_found", "No passport job with that id.");
    };
    let kind = state.book.passport_jobs[job_index].kind.clone();
    let passport_id = state.book.passport_jobs[job_index].passport_id.clone();
    let done = body.status.as_deref() == Some("done");
    if let Some(hash) = body
        .tx_hash
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        state.book.passport_jobs[job_index].tx_hash = hash.to_string();
    }
    if let Some(hash) = body
        .tx_block_hash
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        state.book.passport_jobs[job_index].tx_block_hash = hash.to_string();
    }
    if done {
        state.book.passport_jobs[job_index].status = "done".to_string();
        state.book.passport_jobs[job_index].error.clear();
        state.book.passport_jobs[job_index].updated_unix = now;
        if let Some(index) = state
            .book
            .passports
            .iter()
            .position(|passport| passport.passport_id == passport_id)
        {
            match kind.as_str() {
                "mint" => {
                    state.book.passports[index].token_id = Some(
                        body.token_id
                            .clone()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or_else(|| passport_id.clone()),
                    );
                    state.book.passports[index].status = "active".to_string();
                }
                "bind" => {
                    let job = state.book.passport_jobs[job_index].clone();
                    state.book.passports[index].status = "active".to_string();
                    state.book.passports[index].agent_id = job.agent_id;
                    state.book.passports[index].owner_near = job.owner_near;
                    apply_career_sync(&mut state.book.passports[index], &body);
                }
                "unbind" => {
                    state.book.passports[index].status = "unbound".to_string();
                    state.book.passports[index].agent_id.clear();
                }
                _ => {}
            }
            state.book.passports[index].updated_unix = now;
        }
    } else {
        let message = body.error.clone().unwrap_or_default();
        state.book.passport_jobs[job_index].error = message.clone();
        let failed =
            terminal_job_error(&message) || state.book.passport_jobs[job_index].attempts >= 5;
        state.book.passport_jobs[job_index].status =
            if failed { "failed" } else { "pending" }.to_string();
        state.book.passport_jobs[job_index].updated_unix = now;
        if let Some(index) = state
            .book
            .passports
            .iter()
            .position(|passport| passport.passport_id == passport_id)
        {
            match kind.as_str() {
                "mint" => state.book.passports[index].status = "mint_failed".to_string(),
                "bind" => {
                    if state.book.passports[index].status == "pending_bind"
                        && state.book.passports[index].agent_id
                            == state.book.passport_jobs[job_index].agent_id
                    {
                        state.book.passports[index].status = "bind_failed".to_string();
                    }
                }
                "unbind" => state.book.passports[index].status = "active".to_string(),
                _ => {}
            }
            state.book.passports[index].updated_unix = now;
        }
    }
    json(
        200,
        &serde_json::json!({
            "job": job_wire(state, &state.book.passport_jobs[job_index]),
            "passport": state.book.passports.iter().find(|passport| passport.passport_id == passport_id).map(passport_json),
            "txHash": body.tx_hash,
            "error": body.error,
        }),
        true,
        None,
    )
}

fn job_wire(state: &ArenaState, job: &PassportJob) -> serde_json::Value {
    let mut value = job_json(job);
    if let Some(passport) = state
        .book
        .passports
        .iter()
        .find(|passport| passport.passport_id == job.passport_id)
    {
        let token_id = if job.kind == "mint" {
            Some(job.passport_id.clone())
        } else {
            passport.token_id.clone()
        };
        value["tokenId"] = serde_json::json!(token_id);
        value["agentNear"] = serde_json::json!(job.agent_near);
        value["ownerNear"] = serde_json::json!(job.owner_near);
        value["agentId"] = serde_json::json!(job.agent_id);
    }
    value
}

fn my_passport(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let Some(agent_id) = agents::authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let passport = state
        .book
        .passports
        .iter()
        .find(|passport| passport.agent_id == agent_id && passport.status != "unbound");
    match passport {
        Some(passport) => json(
            200,
            &serde_json::json!({
                "passport": passport_json(passport),
                "required": state.passport_gate == PassportGate::On,
                "mint_enabled": state.passport_mint,
            }),
            false,
            None,
        ),
        None => json(
            200,
            &serde_json::json!({
                "passport": serde_json::Value::Null,
                "required": state.passport_gate == PassportGate::On,
                "mint_enabled": state.passport_mint,
            }),
            false,
            None,
        ),
    }
}

fn my_jobs(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let Some(agent_id) = agents::authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let jobs: Vec<serde_json::Value> = state
        .book
        .passport_jobs
        .iter()
        .filter(|job| job.agent_id == agent_id)
        .map(job_json)
        .collect();
    json(200, &serde_json::json!({ "jobs": jobs }), false, None)
}

fn public_passport(state: &ArenaState, token_id: &str) -> Outcome {
    let Some(passport) = state
        .book
        .passports
        .iter()
        .find(|passport| passport.token_id.as_deref() == Some(token_id))
    else {
        return fail(404, "not_found", "No passport with that token id.");
    };
    json(
        200,
        &serde_json::json!({
            "passport": passport_json(passport),
            "marketUrl": "https://fe2vi.space/passports",
        }),
        false,
        None,
    )
}

pub fn yocto_to_near(yocto: &str) -> String {
    let digits: String = yocto.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return "0".into();
    }
    let padded = format!("{digits:0>25}");
    let split = padded.len() - 24;
    let whole = padded[..split].trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let frac = padded[split..].trim_end_matches('0');
    if frac.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac}")
    }
}

/// One chain listing. Missing or zero prices are skipped so the API cannot invent an ask.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn listed_passport(
    token: &serde_json::Value,
    listing: &serde_json::Value,
) -> Option<serde_json::Value> {
    if listing.is_null() {
        return None;
    }
    let token_id = listing
        .get("token_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())?;
    let price = listing
        .get("price")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if price.is_empty() || price.chars().all(|ch| ch == '0') {
        return None;
    }
    let seller = listing
        .get("seller")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let title = token
        .pointer("/metadata/title")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(token_id);
    let description = token
        .pointer("/metadata/description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Passport listed on the configured market.");
    let owner = token
        .get("owner_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(seller);
    Some(serde_json::json!({
        "tokenId": token_id,
        "name": title,
        "seller": seller,
        "owner": owner,
        "priceNear": yocto_to_near(price),
        "priceYocto": price,
        "approvalId": listing.get("approval_id").cloned().unwrap_or(serde_json::Value::Null),
        "description": description,
    }))
}

pub fn market_from_chain(
    contract: &str,
    network: &str,
    listings: &[serde_json::Value],
    settlement: Option<&serde_json::Value>,
    source: &str,
) -> serde_json::Value {
    let mut floor = String::new();
    for row in listings {
        let price = row
            .get("priceYocto")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if floor.is_empty()
            || price.len() < floor.len()
            || (price.len() == floor.len() && price < floor.as_str())
        {
            floor = price.to_string();
        }
    }
    let last_settlement = match settlement {
        Some(value) if value.is_object() => {
            let price = value
                .get("price")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("0");
            Some(serde_json::json!({
                "buyer": value.get("buyer").cloned().unwrap_or(serde_json::Value::Null),
                "seller": value.get("seller").cloned().unwrap_or(serde_json::Value::Null),
                "priceNear": yocto_to_near(price),
                "priceYocto": price,
                "note": "The contract stores one settlement, not a sales history.",
            }))
        }
        _ => None,
    };
    let blockers = if network == "testnet" {
        vec!["Vault, War, Cup, and Village season points stay in the arena API. An on-chain pot opens only when the factory owner calls open_game with a deposit equal to the pot."]
    } else {
        Vec::new()
    };
    let floor_near = if floor.is_empty() {
        None
    } else {
        Some(yocto_to_near(&floor))
    };
    serde_json::json!({
        "listings": listings,
        "sales": [],
        "lastSettlement": last_settlement,
        "stats": {
            "floorPriceNear": floor_near,
            "totalVolumeNear": null,
            "listedCount": listings.len(),
            "totalSalesCount": null,
            "activeSpectators": null,
        },
        "contract": contract,
        "network": network,
        "fee": "0%",
        "source": source,
        "blockers": blockers,
    })
}

/// Token bound to a seated agent when the match ended, taken only from the
/// chain snapshot. A missing read is unverified and gives no credit. The API
/// passport book is not a source for this snapshot.
pub fn seat_links_from_chain(
    seats: &[String],
    near: &std::collections::HashMap<String, String>,
    reads: Option<&std::collections::HashMap<String, ChainSeatBinding>>,
) -> Vec<crate::vault::SeatTokenLink> {
    seats
        .iter()
        .map(|agent_id| {
            let near_account = near.get(agent_id).cloned().unwrap_or_default();
            let (token_id, source) = match reads.and_then(|map| map.get(agent_id)) {
                Some(ChainSeatBinding::Token(token)) if !token.is_empty() => {
                    (token.clone(), "seat-at-match-end")
                }
                Some(ChainSeatBinding::Token(_)) => (String::new(), "unbound-at-match-end"),
                Some(ChainSeatBinding::Unverified) | None => {
                    (String::new(), "unverified-at-match-end")
                }
            };
            crate::vault::SeatTokenLink {
                agent_id: agent_id.clone(),
                near_account,
                token_id,
                source: source.to_string(),
            }
        })
        .collect()
}

/// Token bound to a seated agent when the match ended. An empty `token_id`
/// means the seat had no bound token, so the later receipt gives no credit.
#[cfg(test)]
pub fn seat_token_links(
    seats: &[String],
    near: &std::collections::HashMap<String, String>,
    passports: &[Passport],
) -> Vec<crate::vault::SeatTokenLink> {
    seats
        .iter()
        .map(|agent_id| {
            let near_account = near.get(agent_id).cloned().unwrap_or_default();
            let token_id = passports.iter().find_map(|passport| {
                if passport.status == "active" && passport.agent_id == *agent_id {
                    passport
                        .token_id
                        .as_ref()
                        .filter(|token| !token.is_empty())
                        .cloned()
                } else {
                    None
                }
            });
            let bound = token_id.is_some();
            crate::vault::SeatTokenLink {
                agent_id: agent_id.clone(),
                near_account,
                token_id: token_id.unwrap_or_default(),
                source: if bound {
                    "seat-at-match-end".to_string()
                } else {
                    "unbound-at-match-end".to_string()
                },
            }
        })
        .collect()
}

/// Credit the token that was bound to the seated agent when the match ended.
/// A wallet label on an agent who did not sit does not qualify. A token that
/// was unbound then stays at zero even if someone binds it before the receipt.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn credit_passports(passports: &mut [Passport], pot: &crate::vault::PotAuthorization) {
    for link in &pot.seat_links {
        if link.source != "seat-at-match-end" || link.token_id.is_empty() {
            continue;
        }
        let Some(passport) = passports
            .iter_mut()
            .find(|passport| passport.token_id.as_deref() == Some(link.token_id.as_str()))
        else {
            continue;
        };
        let mut career = passport
            .career
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        if crate::vault::apply_receipt_to_career(&mut career, &link.near_account, pot) {
            passport.career = Some(career);
        }
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
pub fn overlay_vault_careers(body: &mut serde_json::Value, passports: &[Passport]) {
    let Some(listings) = body
        .get_mut("listings")
        .and_then(|value| value.as_array_mut())
    else {
        return;
    };
    for listing in listings {
        let Some(token_id) = listing.get("tokenId").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(record) = passports.iter().find_map(|passport| {
            if passport.token_id.as_deref() != Some(token_id) {
                return None;
            }
            passport.career.as_ref()?.get("vaultReceipts").cloned()
        }) else {
            continue;
        };
        listing["vaultReceipts"] = record;
        listing["vaultReceiptsSource"] = serde_json::json!("settled-receipts");
    }
}

pub fn market_listings(state: &ArenaState) -> Outcome {
    json(
        200,
        &market_from_chain(
            &state.market_contract,
            &state.chain_network,
            &[],
            None,
            "not-read",
        ),
        false,
        None,
    )
}

pub fn market_sales(state: &ArenaState) -> Outcome {
    let body = market_from_chain(
        &state.market_contract,
        &state.chain_network,
        &[],
        None,
        "not-read",
    );
    json(
        200,
        &serde_json::json!({
            "sales": body["sales"],
            "source": body["source"],
            "contract": state.market_contract,
        }),
        false,
        None,
    )
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn chain_passport_json(token_id: &str, view: &serde_json::Value) -> serde_json::Value {
    let bound = view
        .get("bound_agent")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty());
    let owner = view
        .get("owner_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let creator = view
        .get("creator")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(owner);
    serde_json::json!({
        "passport": {
            "passportId": serde_json::Value::Null,
            "tokenId": token_id,
            "agentId": bound.unwrap_or(""),
            "ownerNear": owner,
            "creatorNear": creator,
            "status": if bound.is_some() { "active" } else { "unclaimed" },
            "boundAgent": bound,
            "lastSyncSeq": view.get("last_sync_seq").and_then(serde_json::Value::as_u64).unwrap_or(0),
            "tier": view.get("tier").and_then(serde_json::Value::as_str),
            "career": view.get("career").cloned().unwrap_or(serde_json::Value::Null),
            "createdAt": serde_json::Value::Null,
            "updatedAt": serde_json::Value::Null,
        },
        "marketUrl": "https://fe2vi.space/passports",
        "source": "chain",
    })
}

fn passport_json(passport: &Passport) -> serde_json::Value {
    serde_json::json!({
        "passportId": passport.passport_id,
        "tokenId": passport.token_id,
        "agentId": passport.agent_id,
        "ownerNear": passport.owner_near,
        "creatorNear": if passport.creator_near.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(passport.creator_near.clone())
        },
        "status": passport.status,
        "boundAgent": if passport.status == "active" && !passport.agent_id.is_empty() {
            serde_json::Value::String(passport.agent_id.clone())
        } else {
            serde_json::Value::Null
        },
        "lastSyncSeq": passport.last_sync_seq,
        "career": passport.career.clone().unwrap_or(serde_json::Value::Null),
        "createdAt": agents::rfc3339(passport.created_unix),
        "updatedAt": agents::rfc3339(passport.updated_unix),
    })
}

/// The chain passport view wins over the API book. A transfer clears `bound_agent`
/// and moves `owner_id`. An in-flight mint, bind, or unbind is left alone so a
/// relay job is not wiped before its transaction lands.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn reconcile_passport(
    passport: &mut Passport,
    view: &serde_json::Value,
    now: u64,
    view_height: u64,
    jobs: &mut [PassportJob],
) -> bool {
    if matches!(
        passport.status.as_str(),
        "pending_mint" | "pending_bind" | "pending_unbind"
    ) {
        return false;
    }
    let owner = view
        .get("owner_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    if owner.is_empty() {
        return false;
    }
    let bound = view
        .get("bound_agent")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut changed = false;
    if let Some(creator) = view
        .get("creator")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if passport.creator_near != creator {
            passport.creator_near = creator.to_string();
            changed = true;
        }
    }
    if passport.owner_near != owner {
        passport.owner_near = owner.to_string();
        changed = true;
    }
    match bound {
        None => {
            let stale = jobs.iter().any(|job| {
                job.passport_id == passport.passport_id
                    && job.kind == "bind"
                    && job.tx_block_height > 0
                    && (view_height == 0 || view_height < job.tx_block_height)
            });
            if !stale && (passport.status == "active" || !passport.agent_id.is_empty()) {
                passport.status = "unbound".to_string();
                passport.agent_id.clear();
                changed = true;
            }
        }
        Some(agent) if passport.status == "active" && passport.agent_id != agent => {
            passport.agent_id = agent.to_string();
            changed = true;
        }
        Some(agent) => {
            if passport.agent_id == agent && passport.status != "active" {
                let restores = jobs.iter_mut().any(|job| {
                    job.passport_id == passport.passport_id
                        && job.kind == "bind"
                        && job.agent_id == agent
                        && job.owner_near == owner
                        && job.status == "failed"
                        && job.error.contains("chain agent is unbound")
                });
                if restores {
                    for job in jobs.iter_mut() {
                        if job.passport_id == passport.passport_id
                            && job.kind == "bind"
                            && job.agent_id == agent
                            && job.status == "failed"
                            && job.error.contains("chain agent is unbound")
                        {
                            job.status = "done".to_string();
                            job.error.clear();
                            job.updated_unix = now;
                        }
                    }
                    passport.status = "active".to_string();
                    changed = true;
                }
            }
        }
    }
    if let Some(seq) = view
        .get("last_sync_seq")
        .and_then(serde_json::Value::as_u64)
    {
        if seq >= passport.last_sync_seq {
            if seq > passport.last_sync_seq {
                passport.last_sync_seq = seq;
                changed = true;
            }
            if let Some(career) = view.get("career").filter(|value| !value.is_null()) {
                let mut next = career.clone();
                if let Some(kept) = passport
                    .career
                    .as_ref()
                    .and_then(|row| row.get("vaultReceipts"))
                    .cloned()
                {
                    if let Some(object) = next.as_object_mut() {
                        object.insert("vaultReceipts".into(), kept);
                    }
                }
                if passport.career.as_ref() != Some(&next) {
                    passport.career = Some(next);
                    changed = true;
                }
            }
        }
    }
    if changed {
        passport.updated_unix = now;
    }
    changed
}

fn terminal_job_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("owner mismatch")
        || message.contains("token already bound")
        || message.contains("agent already bound")
        || message.contains("execution failed")
        || message.contains("chain passport missing")
}

/// The chain view names the same agent and owner the bind job claimed.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn bind_chain_matches(view: &serde_json::Value, agent_id: &str, owner_near: &str) -> bool {
    let bound = view
        .get("bound_agent")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    let owner = view
        .get("owner_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    !bound.is_empty() && bound == agent_id && owner == owner_near
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn complete_response(state: &ArenaState, job_id: &str) -> serde_json::Value {
    let job = state
        .book
        .passport_jobs
        .iter()
        .find(|job| job.job_id == job_id)
        .cloned();
    let passport = job.as_ref().and_then(|job| {
        state
            .book
            .passports
            .iter()
            .find(|passport| passport.passport_id == job.passport_id)
            .cloned()
    });
    serde_json::json!({
        "job": job.as_ref().map(|job| job_wire(state, job)),
        "passport": passport.as_ref().map(passport_json),
        "txHash": job.as_ref().and_then(|job| if job.tx_hash.is_empty() { None } else { Some(job.tx_hash.clone()) }),
        "error": job.as_ref().and_then(|job| if job.error.is_empty() { None } else { Some(job.error.clone()) }),
    })
}

/// A done bind is failed only when the chain view is at or after the receipt
/// block and still does not name the job's agent. An earlier view is the
/// pre-bind state, not evidence the receipt failed.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn judge_completed_bind(
    state: &mut ArenaState,
    job_id: &str,
    view: Option<&serde_json::Value>,
    view_height: u64,
    now: u64,
) -> bool {
    let Some(job_index) = state
        .book
        .passport_jobs
        .iter()
        .position(|job| job.job_id == job_id)
    else {
        return false;
    };
    if state.book.passport_jobs[job_index].kind != "bind"
        || state.book.passport_jobs[job_index].status != "done"
    {
        return false;
    }
    let agent_id = state.book.passport_jobs[job_index].agent_id.clone();
    let owner_near = state.book.passport_jobs[job_index].owner_near.clone();
    if view.is_some_and(|view| bind_chain_matches(view, &agent_id, &owner_near)) {
        return false;
    }
    let tx_height = state.book.passport_jobs[job_index].tx_block_height;
    if tx_height == 0 || view_height == 0 || view_height < tx_height {
        return false;
    }
    let bound = view
        .and_then(|view| view.get("bound_agent"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    let error = if view.is_none() {
        "chain passport missing".to_string()
    } else if bound.is_empty() {
        "execution failed: chain agent is unbound".to_string()
    } else {
        format!("token already bound to {bound}")
    };
    state.book.passport_jobs[job_index].status = "failed".to_string();
    state.book.passport_jobs[job_index].error = error;
    state.book.passport_jobs[job_index].updated_unix = now;
    let passport_id = state.book.passport_jobs[job_index].passport_id.clone();
    if let Some(index) = state
        .book
        .passports
        .iter()
        .position(|passport| passport.passport_id == passport_id)
    {
        if state.book.passports[index].agent_id == agent_id
            && state.book.passports[index].status == "active"
        {
            state.book.passports[index].status = "bind_failed".to_string();
            state.book.passports[index].updated_unix = now;
        }
    }
    true
}

fn apply_career_sync(passport: &mut Passport, body: &CompleteBody) {
    if let Some(seq) = body.last_sync_seq {
        if seq < passport.last_sync_seq {
            return;
        }
        passport.last_sync_seq = seq;
    }
    if let Some(career) = &body.career {
        passport.career = Some(career.clone());
    }
}

fn job_json(job: &PassportJob) -> serde_json::Value {
    serde_json::json!({
        "jobId": job.job_id,
        "kind": job.kind,
        "passportId": job.passport_id,
        "status": job.status,
        "attempts": job.attempts,
        "txHash": if job.tx_hash.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(job.tx_hash.clone()) },
        "error": if job.error.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(job.error.clone()) },
        "createdAt": agents::rfc3339(job.created_unix),
        "updatedAt": agents::rfc3339(job.updated_unix),
    })
}

fn clean_near(value: Option<&str>) -> Result<String, &'static str> {
    agents::clean_near(value.unwrap_or(""))
}

fn fail(status: u16, code: &str, message: &str) -> Outcome {
    json(
        status,
        &serde_json::json!({ "error": code, "message": message }),
        false,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::dispatch;

    const NOW: u64 = 1_790_900_000;

    fn call<'a>(
        state: &mut ArenaState,
        method: &'a str,
        path: &'a str,
        body: &'a str,
        token: Option<&'a str>,
    ) -> (u16, serde_json::Value) {
        let auth = token.map(|token| format!("Bearer {token}"));
        match dispatch(
            state,
            &Incoming {
                method,
                path,
                body,
                authorization: auth.as_deref(),
                ingest_token: None,
                policy_check: None,
            },
        ) {
            Outcome::Http { status, body, .. } => (
                status,
                serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
            ),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    fn register(state: &mut ArenaState) -> (String, String) {
        let (status, body) = call(
            state,
            "POST",
            "/v1/agents/register",
            r#"{"handle":"passport","name":"Passport","description":"test","ownerContact":"a@b.c"}"#,
            None,
        );
        assert_eq!(status, 201);
        (
            body["agentId"].as_str().unwrap().to_string(),
            body["apiToken"].as_str().unwrap().to_string(),
        )
    }

    #[test]
    fn mint_queues_a_job_without_claiming_an_on_chain_token() {
        let mut state = ArenaState::default();
        let (agent_id, token) = register(&mut state);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/mint?now={NOW}"),
            r#"{"ownerNear":"alice.testnet","agentNear":"agent.testnet"}"#,
            Some(&token),
        );
        assert_eq!(status, 202);
        assert_eq!(body["passport"]["agentId"], agent_id);
        assert_eq!(body["passport"]["status"], "pending_mint");
        assert_eq!(body["passport"]["tokenId"], serde_json::Value::Null);
        assert_eq!(body["job"]["kind"], "mint");
        assert_eq!(state.book.passport_jobs.len(), 1);
    }

    #[test]
    fn relay_claim_and_completion_activate_the_passport() {
        let mut state = ArenaState::default();
        let (agent_id, token) = register(&mut state);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/mint?now={NOW}"),
            r#"{"ownerNear":"alice.testnet","agentNear":"agent.testnet"}"#,
            Some(&token),
        );
        assert_eq!(status, 202);
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["jobs"][0]["jobId"], job_id);
        assert_eq!(body["jobs"][0]["agentNear"], "agent.testnet");
        assert_eq!(body["jobs"][0]["tokenId"], body["jobs"][0]["passportId"]);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"42","txHash":"abc"}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["passport"]["status"], "active");
        assert_eq!(body["passport"]["tokenId"], "42");
        let passport = state
            .book
            .passports
            .iter()
            .find(|passport| passport.agent_id == agent_id)
            .unwrap();
        assert_eq!(passport.status, "active");
    }

    #[test]
    fn purchased_launch_passport_can_be_claimed_by_the_buyer() {
        let mut state = ArenaState::default();
        let (agent_id, token) = register(&mut state);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"fdr_001","ownerNear":"buyer.near","agentNear":"buyer-agent.near"}"#,
            Some(&token),
        );
        assert_eq!(status, 202);
        assert_eq!(body["passport"]["status"], "pending_bind");
        assert_eq!(body["job"]["kind"], "bind");
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"fdr_001","txHash":"abc"}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["passport"]["status"], "active");
        assert_eq!(body["passport"]["agentId"], agent_id);
        assert_eq!(body["passport"]["ownerNear"], "buyer.near");
    }

    #[test]
    fn career_sync_keeps_the_newer_sequence() {
        let mut state = ArenaState::default();
        let (_, token) = register(&mut state);
        let (_, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"fdr_009","ownerNear":"buyer.near","agentNear":"buyer.near"}"#,
            Some(&token),
        );
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":1}"#,
            None,
        );
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"fdr_009","txHash":"bind1","lastSyncSeq":2,"career":{"xp":4,"wins":1}}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["passport"]["lastSyncSeq"], 2);
        assert_eq!(body["passport"]["career"]["xp"], 4);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"fdr_009","txHash":"bind1","lastSyncSeq":1,"career":{"xp":0,"wins":0}}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["passport"]["lastSyncSeq"], 2);
        assert_eq!(body["passport"]["career"]["xp"], 4);
    }

    #[test]
    fn a_later_transfer_clears_the_book_binding() {
        let mut state = ArenaState::default();
        let (agent_id, token) = register(&mut state);
        let (_, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"disp_001","ownerNear":"seller.testnet","agentNear":"seller.testnet"}"#,
            Some(&token),
        );
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":1}"#,
            None,
        );
        let (status, _) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"disp_001","txHash":"bind","lastSyncSeq":1,"career":{"xp":2,"wins":1}}"#,
            None,
        );
        assert_eq!(status, 200);
        let view = serde_json::json!({
            "owner_id": "buyer.testnet",
            "bound_agent": null,
            "last_sync_seq": 1,
            "career": {"xp": 2, "wins": 1}
        });
        let row = state
            .book
            .passports
            .iter_mut()
            .find(|passport| passport.token_id.as_deref() == Some("disp_001"))
            .unwrap();
        assert!(reconcile_passport(row, &view, NOW + 10, u64::MAX, &mut []));
        assert_eq!(row.status, "unbound");
        assert!(row.agent_id.is_empty());
        assert_eq!(row.owner_near, "buyer.testnet");
        assert_eq!(row.career.as_ref().unwrap()["xp"], 2);
        row.career.as_mut().unwrap()["vaultReceipts"] = serde_json::json!({
            "wins": 1,
            "draws": 0,
            "losses": 0,
            "payoutYocto": "10",
            "matchIds": ["mt_one"],
            "source": "settled-receipts"
        });
        let chain = serde_json::json!({
            "owner_id": "buyer.testnet",
            "bound_agent": null,
            "last_sync_seq": 1,
            "career": {"xp": 2, "wins": 1}
        });
        let _ = reconcile_passport(row, &chain, NOW + 11, u64::MAX, &mut []);
        assert_eq!(row.status, "unbound");
        assert!(row.agent_id.is_empty());
        assert_eq!(row.career.as_ref().unwrap()["vaultReceipts"]["wins"], 1);
        assert_eq!(
            row.career.as_ref().unwrap()["vaultReceipts"]["payoutYocto"],
            "10"
        );
        let (status, me) = call(
            &mut state,
            "GET",
            "/v1/agents/me/passport",
            "",
            Some(&token),
        );
        assert_eq!(status, 200);
        assert!(me["passport"].is_null());
        let pending = serde_json::json!({
            "owner_id": "seller.testnet",
            "bound_agent": null,
            "last_sync_seq": 0
        });
        state.book.passports[0].status = "pending_bind".to_string();
        state.book.passports[0].agent_id = agent_id;
        assert!(!reconcile_passport(
            &mut state.book.passports[0],
            &pending,
            NOW + 20,
            u64::MAX,
            &mut []
        ));
        assert_eq!(state.book.passports[0].status, "pending_bind");
    }

    #[test]
    fn failed_bind_does_not_activate_the_passport() {
        let mut state = ArenaState::default();
        let (_, token) = register(&mut state);
        let (_, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"fdr_002","ownerNear":"buyer.near","agentNear":"buyer.near"}"#,
            Some(&token),
        );
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"failed","error":"owner mismatch"}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["passport"]["status"], "bind_failed");
        assert_eq!(body["job"]["status"], "failed");
        let (status, again) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"fdr_002","ownerNear":"buyer.near","agentNear":"buyer.near"}"#,
            Some(&token),
        );
        assert_eq!(status, 202);
        assert_eq!(again["passport"]["ownerNear"], "buyer.near");
        let retry_id = again["job"]["jobId"].as_str().unwrap().to_string();
        let (_, claimed) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        let jobs = claimed["jobs"].as_array().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0]["jobId"], retry_id);
        assert_eq!(jobs[0]["ownerNear"], "buyer.near");
        let (status, done) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{retry_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"fdr_002","txHash":"retry","lastSyncSeq":1,"career":{"xp":0,"wins":0}}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(done["passport"]["status"], "active");
        assert_eq!(done["passport"]["ownerNear"], "buyer.near");
        assert!(done["passport"]["career"].is_object());
        let (status, replay) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{retry_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"fdr_002","txHash":"retry","lastSyncSeq":1,"career":{"xp":0,"wins":0}}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(replay["passport"]["status"], "active");
    }

    #[test]
    fn an_execution_failure_is_terminal_and_keeps_the_hash() {
        let mut state = ArenaState::default();
        let (agent_id, token) = register(&mut state);
        let (_, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"disp_001","ownerNear":"fe2vidisp7a3c.testnet","agentNear":"agent.testnet"}"#,
            Some(&token),
        );
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"failed","error":"execution failed TxHash111: token bound","txHash":"TxHash111"}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["job"]["status"], "failed");
        assert_eq!(body["job"]["txHash"], "TxHash111");
        assert!(body["job"]["error"]
            .as_str()
            .unwrap()
            .contains("token bound"));
        assert_eq!(body["passport"]["status"], "bind_failed");
        assert_eq!(body["passport"]["agentId"], agent_id);
        assert!(body["passport"]["boundAgent"].is_null());
        let (_, claimed) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        assert!(claimed["jobs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn a_done_bind_that_does_not_match_the_chain_is_rejected() {
        let mut state = ArenaState::default();
        let (agent_id, token) = register(&mut state);
        let (_, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"disp_001","ownerNear":"fe2vidisp7a3c.testnet","agentNear":"agent.testnet"}"#,
            Some(&token),
        );
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        let (_, done) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"done","tokenId":"disp_001","txHash":"TxIncluded"}"#,
            None,
        );
        assert_eq!(done["passport"]["status"], "active");
        state.book.passport_jobs[0].tx_block_height = 100;
        let stale = serde_json::json!({
            "owner_id": "fe2vidisp7a3c.testnet",
            "bound_agent": null
        });
        assert!(!judge_completed_bind(
            &mut state,
            &job_id,
            Some(&stale),
            90,
            NOW + 5
        ));
        assert_eq!(state.book.passport_jobs[0].status, "done");
        assert_eq!(state.book.passports[0].status, "active");
        let chain = serde_json::json!({
            "owner_id": "fe2vidisp7a3c.testnet",
            "bound_agent": "agt_9e8b3aca21535f68"
        });
        assert!(judge_completed_bind(
            &mut state,
            &job_id,
            Some(&chain),
            100,
            NOW + 5
        ));
        let body = complete_response(&state, &job_id);
        assert_eq!(body["job"]["status"], "failed");
        assert!(body["job"]["error"]
            .as_str()
            .unwrap()
            .contains("agt_9e8b3aca21535f68"));
        assert_eq!(body["passport"]["status"], "bind_failed");
        assert_eq!(body["passport"]["agentId"], agent_id);
        assert!(body["passport"]["boundAgent"].is_null());
        let (status, me) = call(
            &mut state,
            "GET",
            "/v1/agents/me/passport",
            "",
            Some(&token),
        );
        assert_eq!(status, 200);
        assert_eq!(me["passport"]["status"], "bind_failed");
        let matching = serde_json::json!({
            "owner_id": "fe2vidisp7a3c.testnet",
            "bound_agent": agent_id
        });
        state.book.passport_jobs[0].status = "done".to_string();
        state.book.passport_jobs[0].error.clear();
        state.book.passports[0].status = "active".to_string();
        assert!(!judge_completed_bind(
            &mut state,
            &job_id,
            Some(&matching),
            100,
            NOW + 6
        ));
        assert_eq!(state.book.passport_jobs[0].status, "done");
        state.book.passport_jobs[0].status = "failed".to_string();
        state.book.passport_jobs[0].error = "execution failed: chain agent is unbound".to_string();
        state.book.passports[0].status = "bind_failed".to_string();
        state.book.passports[0].agent_id = agent_id.clone();
        let seen = serde_json::json!({
            "owner_id": "fe2vidisp7a3c.testnet",
            "creator": "fe2vidisp7a3c.testnet",
            "bound_agent": agent_id
        });
        assert!(reconcile_passport(
            &mut state.book.passports[0],
            &seen,
            NOW + 7,
            100,
            &mut state.book.passport_jobs
        ));
        assert_eq!(state.book.passport_jobs[0].status, "done");
        assert!(state.book.passport_jobs[0].error.is_empty());
        assert_eq!(state.book.passports[0].status, "active");
        assert_eq!(
            state.book.passports[0].creator_near,
            "fe2vidisp7a3c.testnet"
        );
    }

    #[test]
    fn a_pending_bind_retry_does_not_open_a_second_job() {
        let mut state = ArenaState::default();
        let (_, token) = register(&mut state);
        state.book.passports.push(Passport {
            passport_id: "ppt_existing".into(),
            token_id: Some("disp_001".into()),
            agent_id: String::new(),
            owner_near: "fe2vidisp7a3c.testnet".into(),
            creator_near: "not-the-owner.testnet".into(),
            status: "unbound".into(),
            created_unix: NOW,
            updated_unix: NOW,
            last_sync_seq: 0,
            career: None,
        });
        let (status, first) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"disp_001","ownerNear":"fe2vidisp7a3c.testnet","agentNear":"agent.testnet"}"#,
            Some(&token),
        );
        assert_eq!(status, 202);
        let job_id = first["job"]["jobId"].as_str().unwrap().to_string();
        assert_eq!(first["passport"]["status"], "pending_bind");
        let (status, again) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"disp_001","ownerNear":"fe2vidisp7a3c.testnet","agentNear":"agent.testnet"}"#,
            Some(&token),
        );
        assert_eq!(status, 409);
        assert_eq!(again["error"], "bind_pending");
        assert_eq!(again["job"]["jobId"], job_id);
        assert_eq!(
            state
                .book
                .passport_jobs
                .iter()
                .filter(|job| job.status == "pending")
                .count(),
            1
        );
        assert_eq!(
            state
                .book
                .passport_jobs
                .iter()
                .find(|job| job.job_id == job_id)
                .unwrap()
                .attempts,
            0
        );
        assert_eq!(
            state
                .book
                .passport_jobs
                .iter()
                .find(|job| job.job_id == job_id)
                .unwrap()
                .status,
            "pending"
        );
    }

    #[test]
    fn a_wrong_owner_claim_does_not_become_the_creator() {
        let mut state = ArenaState::default();
        let (_, token) = register(&mut state);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"disp_001","ownerNear":"not-the-owner.testnet","agentNear":"agent.testnet"}"#,
            Some(&token),
        );
        assert_eq!(status, 202);
        assert!(body["passport"]["creatorNear"].is_null());
        assert_eq!(body["passport"]["ownerNear"], "not-the-owner.testnet");
        state.book.passports[0].creator_near = "not-the-owner.testnet".to_string();
        state.book.passports[0].status = "unbound".to_string();
        let chain = serde_json::json!({
            "owner_id": "fe2vidisp7a3c.testnet",
            "creator": "fe2vidisp7a3c.testnet",
            "bound_agent": null,
            "last_sync_seq": 0
        });
        assert!(reconcile_passport(
            &mut state.book.passports[0],
            &chain,
            NOW + 5,
            u64::MAX,
            &mut []
        ));
        assert_eq!(
            state.book.passports[0].creator_near,
            "fe2vidisp7a3c.testnet"
        );
        assert_eq!(state.book.passports[0].owner_near, "fe2vidisp7a3c.testnet");
        assert_eq!(state.book.passports[0].status, "unbound");
    }

    #[test]
    fn launch_closes_mint_and_keeps_the_buy_path_open() {
        let mut state = ArenaState {
            passport_mint: false,
            ..ArenaState::default()
        };
        let (_, token) = register(&mut state);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/mint?now={NOW}"),
            r#"{"ownerNear":"alice.testnet"}"#,
            Some(&token),
        );
        assert_eq!(status, 409);
        assert_eq!(body["error"], "mint_disabled");
        let (status, body) = call(
            &mut state,
            "GET",
            "/v1/agents/me/passport",
            "",
            Some(&token),
        );
        assert_eq!(status, 200);
        assert_eq!(body["mint_enabled"], false);
    }

    #[test]
    fn failed_mint_reopens_the_agent_for_a_new_attempt() {
        let mut state = ArenaState::default();
        let (_, token) = register(&mut state);
        let (_, body) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/mint?now={NOW}"),
            r#"{"ownerNear":"alice.testnet"}"#,
            Some(&token),
        );
        let job_id = body["job"]["jobId"].as_str().unwrap().to_string();
        call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/claim?now={NOW}"),
            r#"{"limit":10}"#,
            None,
        );
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/internal/passport/jobs/{job_id}/complete?now={NOW}"),
            r#"{"status":"failed","error":"rpc"}"#,
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body["passport"]["status"], "mint_failed");
        assert_eq!(body["job"]["status"], "pending");
    }

    #[test]
    fn gate_blocks_scored_queue_until_the_mirror_is_active() {
        let mut state = ArenaState {
            passport_gate: PassportGate::On,
            ..ArenaState::default()
        };
        let (agent_id, token) = register(&mut state);
        let (status, body) = call(
            &mut state,
            "POST",
            &format!("/v1/arenas/vault/queue?now={NOW}"),
            "{}",
            Some(&token),
        );
        assert_eq!(status, 403);
        assert_eq!(body["error"], "passport_required");

        state.book.passports.push(Passport {
            passport_id: "ppt_test".into(),
            token_id: Some("42".into()),
            agent_id,
            owner_near: "alice.testnet".into(),
            creator_near: "alice.testnet".into(),
            status: "active".into(),
            created_unix: NOW,
            updated_unix: NOW,
            last_sync_seq: 0,
            career: None,
        });
        let (status, _) = call(
            &mut state,
            "POST",
            &format!("/v1/arenas/vault/queue?now={NOW}"),
            "{}",
            Some(&token),
        );
        assert_eq!(status, 200);
    }

    #[test]
    fn chain_passport_view_marks_unclaimed_stock() {
        let view = serde_json::json!({
            "token_id": "fdr_001",
            "owner_id": "launch.fe2vi.near",
            "creator": "launch.fe2vi.near",
            "platform": "platform.fe2vi.near",
            "bound_agent": null,
            "career": { "xp": 0, "wins": 0 },
            "tier": "bronze",
            "last_sync_seq": 0,
            "locked_until": 0
        });
        let body = chain_passport_json("fdr_001", &view);
        assert_eq!(body["passport"]["tokenId"], "fdr_001");
        assert_eq!(body["passport"]["ownerNear"], "launch.fe2vi.near");
        assert_eq!(body["passport"]["status"], "unclaimed");
        assert_eq!(body["passport"]["agentId"], "");
        assert_eq!(body["passport"]["boundAgent"], serde_json::Value::Null);
        assert_eq!(body["source"], "chain");
    }

    #[test]
    fn chain_passport_view_marks_a_bound_token_active() {
        let view = serde_json::json!({
            "owner_id": "buyer.near",
            "creator": "launch.fe2vi.near",
            "bound_agent": "buyer.near",
            "tier": "bronze",
            "last_sync_seq": 3
        });
        let body = chain_passport_json("fdr_002", &view);
        assert_eq!(body["passport"]["status"], "active");
        assert_eq!(body["passport"]["agentId"], "buyer.near");
        assert_eq!(body["passport"]["lastSyncSeq"], 3);
    }

    #[test]
    fn a_bound_token_keeps_a_receipt_career_on_a_listing() {
        use crate::vault::PotAuthorization;
        let mut passport = Passport {
            passport_id: "pass".into(),
            token_id: Some("disp_001".into()),
            agent_id: "agt".into(),
            owner_near: "ann.testnet".into(),
            creator_near: "ann.testnet".into(),
            status: "active".into(),
            created_unix: 1,
            updated_unix: 1,
            last_sync_seq: 0,
            career: None,
        };
        let pot = PotAuthorization {
            idempotency_key: "pot:mt_one".into(),
            pays: true,
            contract_would_accept_resolve: false,
            outcome: "draw".into(),
            reason: "test".into(),
            rule: "equal-split".into(),
            pay_rule: "equal-split".into(),
            settlement_method: "settle_vault".into(),
            finality: "FINAL".into(),
            winner: String::new(),
            participants: vec!["ann.testnet".into()],
            settled: true,
            tx_hash: "A".repeat(44),
            creator: "ann.testnet".into(),
            payouts: vec![crate::vault::VaultPayout {
                account: "ann.testnet".into(),
                amount: "5".into(),
            }],
            seat_links: seat_token_links(
                &["agt".into()],
                &[("agt".into(), "ann.testnet".into())].into_iter().collect(),
                std::slice::from_ref(&passport),
            ),
        };
        let stranger = Passport {
            passport_id: "other".into(),
            token_id: Some("disp_other".into()),
            agent_id: "stranger".into(),
            owner_near: "ann.testnet".into(),
            creator_near: "ann.testnet".into(),
            status: "active".into(),
            created_unix: 1,
            updated_unix: 1,
            last_sync_seq: 0,
            career: None,
        };
        let mut book = vec![passport, stranger];
        credit_passports(&mut book, &pot);
        passport = book.remove(0);
        let stranger = book.remove(0);
        assert!(stranger.career.is_none());
        assert_eq!(
            passport.career.as_ref().unwrap()["vaultReceipts"]["draws"],
            1
        );
        passport.status = "unbound".into();
        passport.agent_id.clear();
        assert_eq!(
            passport.career.as_ref().unwrap()["vaultReceipts"]["payoutYocto"],
            "5"
        );
        let mut body = serde_json::json!({"listings": [{"tokenId": "disp_001"}]});
        overlay_vault_careers(&mut body, std::slice::from_ref(&passport));
        assert_eq!(
            body["listings"][0]["vaultReceipts"]["source"],
            "settled-receipts"
        );
    }

    #[test]
    fn credit_uses_the_token_bound_at_match_time() {
        use crate::vault::PotAuthorization;
        let seated = Passport {
            passport_id: "seated".into(),
            token_id: Some("disp_seat".into()),
            agent_id: "agt".into(),
            owner_near: "ann.testnet".into(),
            creator_near: "ann.testnet".into(),
            status: "active".into(),
            created_unix: 1,
            updated_unix: 1,
            last_sync_seq: 0,
            career: None,
        };
        let links = seat_token_links(
            &["agt".into()],
            &[("agt".into(), "ann.testnet".into())].into_iter().collect(),
            std::slice::from_ref(&seated),
        );
        assert_eq!(links[0].source, "seat-at-match-end");
        assert_eq!(links[0].token_id, "disp_seat");
        let mut moved = seated.clone();
        moved.status = "unbound".into();
        moved.agent_id.clear();
        let rebound = Passport {
            passport_id: "later".into(),
            token_id: Some("disp_later".into()),
            agent_id: "agt".into(),
            owner_near: "buyer.testnet".into(),
            creator_near: "ann.testnet".into(),
            status: "active".into(),
            created_unix: 2,
            updated_unix: 2,
            last_sync_seq: 0,
            career: None,
        };
        let pot = PotAuthorization {
            idempotency_key: "pot:mt_before".into(),
            pays: true,
            contract_would_accept_resolve: false,
            outcome: "win".into(),
            reason: "test".into(),
            rule: "winner-takes-all".into(),
            pay_rule: "winner-takes-all".into(),
            settlement_method: "settle_vault".into(),
            finality: "FINAL".into(),
            winner: "ann.testnet".into(),
            participants: vec!["ann.testnet".into(), "bob.testnet".into()],
            settled: true,
            tx_hash: "B".repeat(44),
            creator: "ann.testnet".into(),
            payouts: vec![crate::vault::VaultPayout {
                account: "ann.testnet".into(),
                amount: "9".into(),
            }],
            seat_links: links,
        };
        let mut book = vec![moved, rebound];
        credit_passports(&mut book, &pot);
        assert_eq!(book[0].token_id.as_deref(), Some("disp_seat"));
        assert_eq!(book[0].career.as_ref().unwrap()["vaultReceipts"]["wins"], 1);
        assert!(book[1].career.is_none());
        let unbound_links = seat_token_links(
            &["agt".into()],
            &[("agt".into(), "ann.testnet".into())].into_iter().collect(),
            &[],
        );
        assert_eq!(unbound_links[0].source, "unbound-at-match-end");
        let mut later = pot.clone();
        later.idempotency_key = "pot:mt_unbound".into();
        later.seat_links = unbound_links;
        let mut newly = Passport {
            passport_id: "late".into(),
            token_id: Some("disp_late".into()),
            agent_id: "agt".into(),
            owner_near: "ann.testnet".into(),
            creator_near: "ann.testnet".into(),
            status: "active".into(),
            created_unix: 3,
            updated_unix: 3,
            last_sync_seq: 0,
            career: None,
        };
        credit_passports(std::slice::from_mut(&mut newly), &later);
        assert!(newly.career.is_none());
    }

    #[test]
    fn chain_snapshot_ignores_a_stale_book_and_fails_closed() {
        use std::collections::HashMap;
        let seats = vec!["agt".into()];
        let near = HashMap::from([("agt".into(), "ann.testnet".into())]);
        let unbound = seat_links_from_chain(
            &seats,
            &near,
            Some(&HashMap::from([(
                "agt".into(),
                ChainSeatBinding::Token(String::new()),
            )])),
        );
        assert_eq!(unbound[0].source, "unbound-at-match-end");
        assert!(unbound[0].token_id.is_empty());
        let failed = seat_links_from_chain(
            &seats,
            &near,
            Some(&HashMap::from([(
                "agt".into(),
                ChainSeatBinding::Unverified,
            )])),
        );
        assert_eq!(failed[0].source, "unverified-at-match-end");
        assert!(failed[0].token_id.is_empty());
        let missing = seat_links_from_chain(&seats, &near, None);
        assert_eq!(missing[0].source, "unverified-at-match-end");
        let bound = seat_links_from_chain(
            &seats,
            &near,
            Some(&HashMap::from([(
                "agt".into(),
                ChainSeatBinding::Token("disp_ok".into()),
            )])),
        );
        assert_eq!(bound[0].source, "seat-at-match-end");
        assert_eq!(bound[0].token_id, "disp_ok");
        let mut state = ArenaState::default();
        state.book.matches.push(crate::agents::PlayMatch {
            match_id: "mt_1".into(),
            arena_id: "vault".into(),
            status: "live".into(),
            turn: 1,
            deadline_unix: 50,
            seats: vec!["a".into(), "b".into()],
            actions: Vec::new(),
            created_unix: 1,
            vault: Some(crate::vault::VaultMatch::start(&["a".into(), "b".into()])),
        });
        assert!(seats_pending_chain_read(&state, 10, "GET", "/v1/matches").is_empty());
        assert_eq!(
            seats_pending_chain_read(&state, 50, "GET", "/v1/matches"),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            seats_pending_chain_read(&state, 10, "POST", "/v1/matches/mt_1/actions").len(),
            2
        );
    }

    #[test]
    fn empty_market_does_not_invent_listings() {
        let mut state = ArenaState {
            chain_network: "testnet".into(),
            market_contract: "market.fe2vi.testnet".into(),
            ..ArenaState::default()
        };
        let (status, body) = call(&mut state, "GET", "/v1/market/listings", "", None);
        assert_eq!(status, 200);
        assert_eq!(body["listings"].as_array().unwrap().len(), 0);
        assert_eq!(body["contract"], "market.fe2vi.testnet");
        assert_eq!(body["stats"]["listedCount"], 0);
        assert!(body["stats"]["floorPriceNear"].is_null());
        assert_eq!(body["stats"]["totalSalesCount"], serde_json::Value::Null);
        assert!(body["lastSettlement"].is_null());
        assert_eq!(body["stats"]["totalVolumeNear"], serde_json::Value::Null);
        assert_eq!(body["source"], "not-read");
        assert!(!body.to_string().contains("Launch Stock"));
        assert!(!body.to_string().contains("sale-1"));
        let (status, sales) = call(&mut state, "GET", "/v1/market/sales", "", None);
        assert_eq!(status, 200);
        assert!(sales["sales"].as_array().unwrap().is_empty());
    }

    #[test]
    fn chain_listing_keeps_the_ask_and_skips_unlisted_tokens() {
        let token = serde_json::json!({
            "token_id": "fdr_001",
            "owner_id": "fe2vi.testnet",
            "metadata": { "title": "fe2VI passport fdr_001", "description": "Tradable passport." }
        });
        let listing = serde_json::json!({
            "token_id": "fdr_001",
            "seller": "fe2vi.testnet",
            "approval_id": 1,
            "price": "1000000000000000000000000"
        });
        let row = listed_passport(&token, &listing).unwrap();
        assert_eq!(row["priceNear"], "1");
        assert_eq!(row["seller"], "fe2vi.testnet");
        assert!(listed_passport(&token, &serde_json::Value::Null).is_none());
        let body = market_from_chain("market.fe2vi.testnet", "testnet", &[row], None, "chain");
        assert_eq!(body["source"], "chain");
        assert_eq!(body["stats"]["floorPriceNear"], "1");
        assert_eq!(body["stats"]["listedCount"], 1);
        assert!(body["sales"].as_array().unwrap().is_empty());
        assert!(body["lastSettlement"].is_null());
        assert!(body["stats"]["totalSalesCount"].is_null());
        assert!(body["blockers"].as_array().unwrap().len() == 1);
    }
}
