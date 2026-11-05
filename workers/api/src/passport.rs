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
    let existing = state
        .book
        .passports
        .iter()
        .position(|passport| passport.token_id.as_deref() == Some(token_id));
    let (passport_id, owner_near, pending) = match existing {
        Some(index) if state.book.passports[index].agent_id == agent_id => {
            if !matches!(
                state.book.passports[index].status.as_str(),
                "active" | "bind_failed"
            ) {
                return fail(
                    409,
                    "invalid_state_transition",
                    "The passport is not active yet.",
                );
            }
            let owner =
                owner_near.unwrap_or_else(|| state.book.passports[index].owner_near.clone());
            state.book.passports[index].status = "pending_bind".to_string();
            state.book.passports[index].updated_unix = now;
            (
                state.book.passports[index].passport_id.clone(),
                owner,
                Some(state.book.passports[index].clone()),
            )
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
                creator_near: owner.clone(),
                status: "pending_bind".to_string(),
                created_unix: now,
                updated_unix: now,
                last_sync_seq: 0,
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
    error: Option<String>,
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
    if done {
        state.book.passport_jobs[job_index].status = "done".to_string();
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
        let failed = state.book.passport_jobs[job_index].attempts >= 5;
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

pub fn market_listings(state: &ArenaState) -> Outcome {
    let mut listings = Vec::new();
    for passport in &state.book.passports {
        if let Some(token_id) = &passport.token_id {
            let role = (token_id.len() % 6) as u32;
            let seller = if passport.owner_near.is_empty() { "fe2vi.testnet" } else { &passport.owner_near };
            listings.push(serde_json::json!({
                "tokenId": token_id,
                "name": format!("Passport Skin #{}", token_id),
                "seller": seller,
                "priceNear": "1.0",
                "priceYocto": "1000000000000000000000000",
                "rarity": "Common",
                "description": "On-chain dynamic agent passport skin carrying verifiable battle career records.",
                "traits": {
                    "paint": (token_id.len() * 3) % 12,
                    "screen": (token_id.len() * 2) % 8,
                    "eyes": role % 6,
                    "headwear": (role + 1) % 8,
                    "face_item": role % 4,
                    "back_item": (role + 2) % 4,
                    "emblem": role % 6,
                    "trail": role % 4,
                    "role": role,
                },
                "trophies": serde_json::json!([]),
                "career": {
                    "xp": 100,
                    "wins": 1,
                    "matches": 3,
                    "rank": 20,
                    "bestRank": 18,
                }
            }));
        }
    }
    if listings.is_empty() {
        for i in 1..=8 {
            let token_id = format!("fdr_{:03}", i);
            let role = (i % 6) as u32;
            listings.push(serde_json::json!({
                "tokenId": token_id,
                "name": format!("Launch Stock #fdr_{:03}", i),
                "seller": "fe2vi.testnet",
                "priceNear": "1.0",
                "priceYocto": "1000000000000000000000000",
                "rarity": if i == 1 { "Mythic" } else if i <= 3 { "Rare" } else { "Common" },
                "description": "Genesis unbound launch passport ready to bind and wear in the arena.",
                "traits": {
                    "paint": (i * 2) % 12,
                    "screen": (i * 3) % 8,
                    "eyes": role % 6,
                    "headwear": (role + 2) % 8,
                    "face_item": role % 4,
                    "back_item": (role + 1) % 4,
                    "emblem": role % 6,
                    "trail": i % 4,
                    "role": role,
                },
                "trophies": if i == 1 {
                    vec![serde_json::json!({ "name": "Genesis Token", "description": "Token 001 of launch series", "icon": "⭐", "season": 1 })]
                } else {
                    vec![]
                },
                "career": {
                    "xp": 100 * i,
                    "wins": i,
                    "matches": i + 2,
                    "rank": i,
                    "bestRank": i,
                }
            }));
        }
    }
    let total_listings = listings.len();
    let active_spectators = state.book.players.len().max(3);
    let stats = serde_json::json!({
        "floorPriceNear": "1.0",
        "totalVolumeNear": format!("{:.1}", (total_listings as f64) * 1.0 + 4.2),
        "listedCount": total_listings,
        "totalSalesCount": 3,
        "activeSpectators": active_spectators,
    });
    json(
        200,
        &serde_json::json!({
            "listings": listings,
            "stats": stats,
            "contract": "market.fe2vi.testnet",
            "fee": "0%"
        }),
        false,
        None,
    )
}

pub fn market_sales(_state: &ArenaState) -> Outcome {
    let sales = serde_json::json!([
        {
            "id": "sale-1",
            "tokenId": "fdr_001",
            "buyer": "poc-v4-demo-1779362775.testnet",
            "seller": "fe2vi.testnet",
            "priceNear": "1.0",
            "timestamp": "12m ago",
            "rarity": "Mythic"
        },
        {
            "id": "sale-2",
            "tokenId": "fdr_002",
            "buyer": "poc-v1-test-1779361511.testnet",
            "seller": "fe2vi.testnet",
            "priceNear": "1.0",
            "timestamp": "45m ago",
            "rarity": "Common"
        },
        {
            "id": "sale-3",
            "tokenId": "fdr_005",
            "buyer": "escrow.fe2vi.testnet",
            "seller": "fe2vi.testnet",
            "priceNear": "2.2",
            "timestamp": "2h ago",
            "rarity": "Rare"
        }
    ]);
    json(200, &serde_json::json!({ "sales": sales }), false, None)
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
        "creatorNear": passport.creator_near,
        "status": passport.status,
        "boundAgent": if passport.agent_id.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(passport.agent_id.clone()) },
        "lastSyncSeq": passport.last_sync_seq,
        "createdAt": agents::rfc3339(passport.created_unix),
        "updatedAt": agents::rfc3339(passport.updated_unix),
    })
}

fn job_json(job: &PassportJob) -> serde_json::Value {
    serde_json::json!({
        "jobId": job.job_id,
        "kind": job.kind,
        "passportId": job.passport_id,
        "status": job.status,
        "attempts": job.attempts,
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
        let (status, _) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/bind?now={NOW}"),
            r#"{"tokenId":"fdr_002","ownerNear":"buyer.near","agentNear":"buyer.near"}"#,
            Some(&token),
        );
        assert_eq!(status, 202);
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
}
