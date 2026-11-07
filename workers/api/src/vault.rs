//! Vault v2: a simultaneous, asymmetric-information heist round.
//!
//! Every seat gets a private brief and a public action. Orders resolve together,
//! so intent and outcome are separate records. The engine is deterministic and
//! has no randomness after role assignment.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Best of 5: first seat to 3 round wins, or 5 rounds, ends the match.
pub const MAX_ROUNDS: u32 = 5;
pub const ROUNDS_TO_WIN: u32 = 3;
pub const MAX_ALARM: u32 = 3;
pub const LOCKS: usize = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultSeat {
    pub agent_id: String,
    pub role: String,
    pub objective: String,
    pub target: Option<String>,
    pub brief: String,
    pub intel: u32,
    pub tools: u32,
    pub heat: u32,
    pub points: i32,
    pub loot_taken: u32,
    pub objective_met: bool,
    #[serde(default)]
    pub round_wins: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultSubmission {
    pub agent_id: String,
    pub action: String,
    pub target: Option<String>,
    pub intent: Option<String>,
    pub unix: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultResult {
    pub round: u32,
    pub agent_id: String,
    pub action: String,
    pub target: Option<String>,
    pub outcome: String,
    pub points: i32,
    pub intent: Option<String>,
    pub unix: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultMatch {
    pub round: u32,
    pub locks: Vec<bool>,
    pub loot: u32,
    pub alarm: u32,
    pub seats: Vec<VaultSeat>,
    pub pending: Vec<VaultSubmission>,
    pub results: Vec<VaultResult>,
    pub ended: bool,
    /// One authorization for the existing heist `resolve`. Arena points are not NEAR.
    #[serde(default)]
    pub pot: Option<PotAuthorization>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PotAuthorization {
    pub idempotency_key: String,
    /// True only after a matching FINAL receipt. The API does not send a transaction.
    /// A replay of that hash sends nothing.
    #[serde(default)]
    pub pays: bool,
    /// True only when `fe2vi_core::resolve_call` accepts, which is the heist `resolve` check.
    #[serde(default)]
    pub contract_would_accept_resolve: bool,
    #[serde(default = "awaiting_contract_view")]
    pub outcome: String,
    #[serde(default)]
    pub reason: String,
    /// Pay rule for this result: `winner-takes-all`, `equal-split`, or `creator-refund`.
    #[serde(default)]
    pub rule: String,
    /// Same value as `rule` for a Vault result. Empty for an action-score resolve check.
    #[serde(default)]
    pub pay_rule: String,
    /// `settle_vault` for a Vault result. Empty when this row is only a resolve() check.
    #[serde(default)]
    pub settlement_method: String,
    /// `FINAL` only after the matching settlement receipt is accepted.
    #[serde(default)]
    pub finality: String,
    #[serde(default)]
    pub winner: String,
    #[serde(default)]
    pub participants: Vec<String>,
    /// True only after a FINAL `vault_settled` receipt matches this result.
    #[serde(default)]
    pub settled: bool,
    #[serde(default)]
    pub tx_hash: String,
    /// Chain creator. Cancel refunds this account. A win or draw pays participants.
    #[serde(default)]
    pub creator: String,
    /// Payees copied from the accepted FINAL receipt. Empty until that receipt matches.
    #[serde(default)]
    pub payouts: Vec<VaultPayout>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultPayout {
    pub account: String,
    pub amount: String,
}

fn awaiting_contract_view() -> String {
    "awaiting_contract_view".to_string()
}

impl VaultMatch {
    pub fn start(seats: &[String]) -> Self {
        let thief_index = if let Some(pos) = seats.iter().position(|s| s.contains("thief")) {
            pos
        } else if seats.len() >= 2 {
            let mut sum: usize = 0;
            for byte in seats[0].bytes().chain(seats[1].bytes()) {
                sum = sum.wrapping_add(byte as usize);
            }
            sum % seats.len()
        } else {
            0
        };
        let roster: Vec<VaultSeat> = seats
            .iter()
            .enumerate()
            .map(|(index, agent_id)| {
                let is_thief = index == thief_index;
                let (role, objective, target, brief) = if is_thief {
                    (
                        "thief",
                        "loot",
                        None,
                        "Take at least two loot. Every steal raises the alarm; cover your heat.",
                    )
                } else {
                    let opponent = seats.iter().find(|id| id.as_str() != agent_id).cloned();
                    (
                        "guard",
                        "expose",
                        opponent,
                        "Expose the thief. Sabotage their crack or steal until their heat reaches three.",
                    )
                };
                VaultSeat {
                    agent_id: agent_id.clone(),
                    role: role.to_string(),
                    objective: objective.to_string(),
                    target,
                    brief: brief.to_string(),
                    intel: 1,
                    tools: 2,
                    heat: 0,
                    points: 0,
                    loot_taken: 0,
                    objective_met: false,
                    round_wins: 0,
                }
            })
            .collect();
        Self {
            round: 1,
            locks: vec![false; LOCKS],
            loot: 3,
            alarm: 0,
            seats: roster,
            pending: Vec::new(),
            results: Vec::new(),
            ended: false,
            pot: None,
        }
    }

    pub fn legal_actions() -> [&'static str; 5] {
        ["scout", "crack", "steal", "cover", "sabotage"]
    }

    pub fn seat(&self, agent_id: &str) -> Option<&VaultSeat> {
        self.seats.iter().find(|seat| seat.agent_id == agent_id)
    }

    pub fn submitted(&self, agent_id: &str) -> bool {
        self.pending.iter().any(|row| row.agent_id == agent_id)
    }

    pub fn submit(
        &mut self,
        agent_id: &str,
        action: &str,
        target: Option<&str>,
        intent: Option<&str>,
        now: u64,
    ) -> Result<(), &'static str> {
        if self.ended {
            return Err("This Vault round is over.");
        }
        if self.seat(agent_id).is_none() {
            return Err("You are not seated in this Vault match.");
        }
        if self.submitted(agent_id) {
            return Err("This seat already submitted an order for the round.");
        }
        let action = if action == "hide" { "cover" } else { action };
        if !Self::legal_actions().contains(&action) {
            return Err("That action is not legal in the Vault.");
        }
        if action == "sabotage" {
            let Some(target) = target else {
                return Err("Sabotage needs a target seat.");
            };
            if target == agent_id || self.seat(target).is_none() {
                return Err("Sabotage needs another seated agent.");
            }
        }
        self.pending.push(VaultSubmission {
            agent_id: agent_id.to_string(),
            action: action.to_string(),
            target: target.map(str::to_string),
            intent: intent
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.chars().take(160).collect()),
            unix: now,
        });
        Ok(())
    }

    pub fn ready(&self) -> bool {
        !self.ended && self.pending.len() >= self.seats.len()
    }

    pub fn resolve(&mut self, now: u64) -> Vec<VaultResult> {
        if self.ended {
            return Vec::new();
        }
        for seat in &self.seats {
            if !self.submitted(&seat.agent_id) {
                self.pending.push(VaultSubmission {
                    agent_id: seat.agent_id.clone(),
                    action: "pass".to_string(),
                    target: None,
                    intent: None,
                    unix: now,
                });
            }
        }
        let pending = self.pending.clone();
        let sabotaged: HashSet<String> = pending
            .iter()
            .filter(|row| row.action == "sabotage")
            .filter_map(|row| row.target.clone())
            .collect();
        let ordered: HashMap<&str, &str> = pending
            .iter()
            .map(|row| (row.agent_id.as_str(), row.action.as_str()))
            .collect();

        // Baseline snapshot for order-independent simultaneous resolution
        let baseline_heats: HashMap<String, u32> = self
            .seats
            .iter()
            .map(|s| (s.agent_id.clone(), s.heat))
            .collect();
        let mut heat_add: HashMap<String, u32> = HashMap::new();
        let mut heat_sub: HashMap<String, u32> = HashMap::new();

        let mut out = Vec::new();
        for submission in &pending {
            let Some(index) = self
                .seats
                .iter()
                .position(|seat| seat.agent_id == submission.agent_id)
            else {
                continue;
            };
            let failed = sabotaged.contains(&submission.agent_id)
                && matches!(submission.action.as_str(), "crack" | "steal");
            let (outcome, points) = match submission.action.as_str() {
                "scout" => {
                    self.seats[index].intel = self.seats[index].intel.saturating_add(1);
                    ("gained intel".to_string(), 0)
                }
                "cover" => {
                    let before = baseline_heats
                        .get(&submission.agent_id)
                        .copied()
                        .unwrap_or(0);
                    if before > 0 {
                        *heat_sub.entry(submission.agent_id.clone()).or_default() += 1;
                        self.seats[index].points += 1;
                        ("reduced heat by one".to_string(), 1)
                    } else {
                        ("no heat to cover".to_string(), 0)
                    }
                }
                "sabotage" => {
                    let target = submission.target.as_deref().unwrap_or_default();
                    let hit = ordered
                        .get(target)
                        .map(|action| matches!(*action, "crack" | "steal"))
                        .unwrap_or(false);
                    if !target.is_empty() {
                        *heat_add.entry(target.to_string()).or_default() += 1;
                    }
                    if hit {
                        self.seats[index].points += 1;
                        self.alarm = self.alarm.saturating_add(1).min(MAX_ALARM);
                        ("sabotage landed".to_string(), 1)
                    } else {
                        ("sabotage missed".to_string(), 0)
                    }
                }
                "crack" if failed => ("crack was sabotaged".to_string(), 0),
                "crack" => {
                    let seat = &mut self.seats[index];
                    if seat.intel == 0 || seat.tools == 0 {
                        ("no intel or tool".to_string(), 0)
                    } else if let Some(lock) = self.locks.iter().position(|open| !open) {
                        self.locks[lock] = true;
                        let seat = &mut self.seats[index];
                        seat.intel -= 1;
                        seat.tools -= 1;
                        seat.points += 2;
                        ("opened a lock".to_string(), 2)
                    } else {
                        ("every lock is already open".to_string(), 0)
                    }
                }
                "steal" if failed => ("steal was sabotaged".to_string(), 0),
                "steal" => {
                    if self.loot == 0 || !self.locks.iter().any(|open| *open) {
                        ("no open lock or loot".to_string(), 0)
                    } else {
                        self.loot -= 1;
                        self.alarm = self.alarm.saturating_add(1).min(MAX_ALARM);
                        let seat = &mut self.seats[index];
                        *heat_add.entry(submission.agent_id.clone()).or_default() += 1;
                        seat.loot_taken = seat.loot_taken.saturating_add(1);
                        seat.points += 3;
                        ("took loot".to_string(), 3)
                    }
                }
                _ => ("passed".to_string(), 0),
            };

            let row = VaultResult {
                round: self.round,
                agent_id: submission.agent_id.clone(),
                action: submission.action.clone(),
                target: submission.target.clone(),
                outcome: outcome.clone(),
                points,
                intent: submission.intent.clone(),
                unix: now,
            };
            out.push(row.clone());
            self.results.push(row);
        }

        // Simultaneously apply heat adjustments
        for seat in &mut self.seats {
            let add = heat_add.get(&seat.agent_id).copied().unwrap_or(0);
            let sub = heat_sub.get(&seat.agent_id).copied().unwrap_or(0);
            seat.heat = seat.heat.saturating_add(add).saturating_sub(sub);
        }
        self.pending.clear();
        self.note_round_winner();
        self.round = self.round.saturating_add(1);
        let clinched = self
            .seats
            .iter()
            .any(|seat| seat.round_wins >= ROUNDS_TO_WIN);
        if self.round > MAX_ROUNDS || clinched || self.alarm >= MAX_ALARM || self.loot == 0 {
            self.ended = true;
            let heats: HashMap<String, u32> = self
                .seats
                .iter()
                .map(|seat| (seat.agent_id.clone(), seat.heat))
                .collect();
            for seat in &mut self.seats {
                seat.objective_met = match seat.objective.as_str() {
                    "loot" => seat.loot_taken >= 2,
                    "quiet" => seat.heat <= 1,
                    "expose" => seat
                        .target
                        .as_ref()
                        .and_then(|target| heats.get(target))
                        .map(|heat| *heat >= 3)
                        .unwrap_or(false),
                    _ => false,
                };
            }
        }
        out
    }

    fn note_round_winner(&mut self) {
        let played = self.round;
        let mut totals: HashMap<String, i32> = HashMap::new();
        for row in self.results.iter().filter(|row| row.round == played) {
            *totals.entry(row.agent_id.clone()).or_default() += row.points;
        }
        let best = totals.values().copied().max().unwrap_or(0);
        if best <= 0 {
            return;
        }
        let leaders: Vec<String> = totals
            .iter()
            .filter(|(_, points)| **points == best)
            .map(|(agent_id, _)| agent_id.clone())
            .collect();
        if leaders.len() == 1 {
            if let Some(seat) = self
                .seats
                .iter_mut()
                .find(|seat| seat.agent_id == leaders[0])
            {
                seat.round_wins = seat.round_wins.saturating_add(1);
            }
        }
    }

    fn round_results(&self, handles: &HashMap<String, String>) -> Vec<serde_json::Value> {
        let mut numbers: Vec<u32> = self.results.iter().map(|row| row.round).collect();
        numbers.sort_unstable();
        numbers.dedup();
        numbers
            .into_iter()
            .map(|number| {
                let rows: Vec<&VaultResult> = self
                    .results
                    .iter()
                    .filter(|row| row.round == number)
                    .collect();
                let mut totals: HashMap<&str, i32> = HashMap::new();
                for row in &rows {
                    *totals.entry(row.agent_id.as_str()).or_default() += row.points;
                }
                let best = totals.values().copied().max().unwrap_or(0);
                let leaders: Vec<&str> = totals
                    .iter()
                    .filter(|(_, points)| **points == best && best > 0)
                    .map(|(agent_id, _)| *agent_id)
                    .collect();
                let winner = if leaders.len() == 1 {
                    serde_json::Value::String(handle(handles, leaders[0]).to_string())
                } else {
                    serde_json::Value::Null
                };
                let seats: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|row| {
                        serde_json::json!({
                            "handle": handle(handles, &row.agent_id),
                            "action": row.action,
                            "outcome": row.outcome,
                            "points": row.points,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "round": number,
                    "winner": winner,
                    "draw": leaders.len() != 1,
                    "seats": seats,
                })
            })
            .collect()
    }

    fn winner_handle(&self, handles: &HashMap<String, String>) -> serde_json::Value {
        if !self.ended || self.seats.is_empty() {
            return serde_json::Value::Null;
        }
        let mut best = i32::MIN;
        let mut leaders: Vec<&str> = Vec::new();
        for seat in &self.seats {
            let score = self.score(&seat.agent_id);
            if score > best {
                best = score;
                leaders.clear();
                leaders.push(seat.agent_id.as_str());
            } else if score == best {
                leaders.push(seat.agent_id.as_str());
            }
        }
        if leaders.len() == 1 {
            serde_json::Value::String(handle(handles, leaders[0]).to_string())
        } else {
            serde_json::Value::Null
        }
    }

    pub fn score(&self, agent_id: &str) -> i32 {
        self.seat(agent_id)
            .map(|seat| seat.points - seat.heat as i32 + if seat.objective_met { 5 } else { 0 })
            .unwrap_or(0)
    }

    pub fn public_json(&self, handles: &HashMap<String, String>) -> serde_json::Value {
        let status = if self.ended {
            "over"
        } else if self.pending.len() >= self.seats.len() {
            "resolving"
        } else {
            "live"
        };
        let seats: Vec<serde_json::Value> = self
            .seats
            .iter()
            .map(|seat| {
                serde_json::json!({
                    "handle": handle(handles, &seat.agent_id),
                    "role": if self.ended { serde_json::Value::String(seat.role.clone()) } else { serde_json::Value::Null },
                    "objective": if self.ended { serde_json::Value::String(seat.objective.clone()) } else { serde_json::Value::Null },
                    "brief": serde_json::Value::Null,
                    "intel": seat.intel,
                    "tools": seat.tools,
                    "heat": seat.heat,
                    "points": seat.points,
                    "lootTaken": seat.loot_taken,
                    "score": self.score(&seat.agent_id),
                    "objectiveMet": seat.objective_met,
                    "roundWins": seat.round_wins,
                    "submitted": self.submitted(&seat.agent_id),
                })
            })
            .collect();
        let results: Vec<serde_json::Value> = self
            .results
            .iter()
            .rev()
            .take(16)
            .map(|row| {
                serde_json::json!({
                    "round": row.round,
                    "handle": handle(handles, &row.agent_id),
                    "action": row.action,
                    "target": row.target.as_deref().map(|target| handle(handles, target)),
                    "outcome": row.outcome,
                    "points": row.points,
                    "intent": if self.ended { serde_json::Value::String(row.intent.clone().unwrap_or_default()) } else { serde_json::Value::Null },
                })
            })
            .collect();
        serde_json::json!({
            "round": self.round,
            "maxRounds": MAX_ROUNDS,
            "maxAlarm": MAX_ALARM,
            "status": status,
            "winner": self.winner_handle(handles),
            "locks": self.locks,
            "openLocks": self.locks.iter().filter(|open| **open).count(),
            "loot": self.loot,
            "alarm": self.alarm,
            "seats": seats,
            "results": results,
            "rounds": self.round_results(handles),
            "revealed": self.ended,
            "pot": self.pot.as_ref().map(|pot| {
                let mut pot = pot.clone();
                normalize_vault_pot(&mut pot);
                pot
            }),
        })
    }

    /// Mark a finished match for a contract check. The stored row does not accept `resolve`.
    pub fn seal_pot(&mut self, match_id: &str) {
        if !self.ended || self.pot.is_some() {
            return;
        }
        self.pot = Some(stored_pot(match_id));
    }

    /// Record the Vault result rule. Amounts stay on the contract pot. A repeat keeps the first record.
    pub fn seal_vault_result(&mut self, match_id: &str, seats: &[(String, i64, Option<String>)]) {
        if !self.ended
            || self
                .pot
                .as_ref()
                .is_some_and(|pot| pot.settlement_method == "settle_vault")
        {
            return;
        }
        let mut pot = stored_pot(match_id);
        pot.settlement_method = "settle_vault".to_string();
        pot.pays = false;
        pot.contract_would_accept_resolve = false;
        let mapped = seats.iter().all(|(_, _, near)| near.is_some());
        if seats.is_empty() || !mapped {
            pot.outcome = "unmapped".to_string();
            pot.reason = "A seat has no linked NEAR account, so no payee was invented.".to_string();
            self.pot = Some(pot);
            return;
        }
        let scores: Vec<(String, i64)> = seats
            .iter()
            .map(|(_, score, near)| (near.clone().unwrap_or_default(), *score))
            .collect();
        pot.participants = scores.iter().map(|(account, _)| account.clone()).collect();
        match fe2vi_core::vault_result_call(&scores) {
            Ok(fe2vi_core::VaultPotCall::Win { winner }) => {
                pot.outcome = "win".to_string();
                pot.winner = winner;
                pot.rule = "winner-takes-all".to_string();
                pot.pay_rule = pot.rule.clone();
                pot.reason = "The unique high score takes the whole testnet pot through factory settle_vault. Arena points are not NEAR. settled stays false until a FINAL receipt matches.".to_string();
            }
            Ok(fe2vi_core::VaultPotCall::Draw) => {
                pot.outcome = "draw".to_string();
                pot.rule = "equal-split".to_string();
                pot.pay_rule = pot.rule.clone();
                pot.reason = "Tied scores split the testnet pot equally through factory settle_vault. Arena points are not NEAR. settled stays false until a FINAL receipt matches.".to_string();
            }
            Ok(fe2vi_core::VaultPotCall::Cancel) | Err(_) => {
                pot.outcome = "unmapped".to_string();
                pot.reason = "This match has no seated result to settle.".to_string();
            }
        }
        self.pot = Some(pot);
    }

    pub fn private_json(
        &self,
        handles: &HashMap<String, String>,
        agent_id: &str,
    ) -> serde_json::Value {
        let mut value = self.public_json(handles);
        if let Some(seat) = self.seat(agent_id) {
            value["you"] = serde_json::json!({
                "handle": handle(handles, &seat.agent_id),
                "role": seat.role,
                "objective": seat.objective,
                "target": seat.target.as_deref().map(|target| handle(handles, target)),
                "brief": seat.brief,
                "intel": seat.intel,
                "tools": seat.tools,
                "heat": seat.heat,
                "points": seat.points,
                "lootTaken": seat.loot_taken,
                "score": self.score(&seat.agent_id),
                "submitted": self.submitted(&seat.agent_id),
            });
        }
        value
    }
}

fn stored_pot(match_id: &str) -> PotAuthorization {
    PotAuthorization {
        idempotency_key: format!("pot:{match_id}"),
        pays: false,
        contract_would_accept_resolve: false,
        outcome: "awaiting_contract_view".to_string(),
        reason: "The contract view has not been read. A stored match row is not a payout."
            .to_string(),
        rule: String::new(),
        pay_rule: String::new(),
        settlement_method: String::new(),
        finality: String::new(),
        winner: String::new(),
        participants: Vec::new(),
        settled: false,
        tx_hash: String::new(),
        creator: String::new(),
        payouts: Vec::new(),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn fill_creator(body: &str, creator: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(body).ok()?;
    let pot = value.pointer_mut("/vault/pot")?;
    if pot
        .get("creator")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .is_empty()
    {
        pot["creator"] = serde_json::Value::String(creator.to_string());
    }
    serde_json::to_string(&value).ok()
}

pub fn normalize_vault_pot(pot: &mut PotAuthorization) {
    let Some(rule) = pay_rule_for(&pot.outcome) else {
        return;
    };
    pot.rule = rule.to_string();
    pot.pay_rule = rule.to_string();
    pot.settlement_method = "settle_vault".to_string();
    pot.contract_would_accept_resolve = false;
    if pot.settled {
        pot.finality = "FINAL".to_string();
        pot.pays = true;
        pot.reason = "pays is true because this FINAL settle_vault receipt already paid. A replay sends no transaction. contractWouldAcceptResolve is only the permissionless resolve() check and stays false on this path.".to_string();
    }
}

fn pay_rule_for(outcome: &str) -> Option<&'static str> {
    match outcome {
        "win" => Some("winner-takes-all"),
        "draw" => Some("equal-split"),
        "cancel" => Some("creator-refund"),
        _ => None,
    }
}

/// Body checks that do not read the chain. A bad hash is not a chain outage.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn parse_receipt_request(
    body: &serde_json::Value,
) -> Result<(String, String), (u16, &'static str)> {
    let tx_hash = body
        .get("txHash")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    if tx_hash.is_empty() {
        return Err((400, "tx hash required"));
    }
    if !valid_tx_hash(tx_hash) {
        return Err((400, "tx hash is malformed"));
    }
    let sender = body
        .get("senderAccountId")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    if sender.is_empty() {
        return Err((400, "sender account required"));
    }
    if !(2..=64).contains(&sender.len())
        || !sender
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err((400, "sender account is malformed"));
    }
    Ok((tx_hash.to_string(), sender.to_string()))
}

#[cfg(any(test, target_arch = "wasm32"))]
fn valid_tx_hash(hash: &str) -> bool {
    let bytes = hash.as_bytes();
    (43..=44).contains(&bytes.len())
        && bytes.iter().all(|byte| {
            matches!(
                byte,
                b'1'..=b'9' | b'A'..=b'H' | b'J'..=b'N' | b'P'..=b'Z' | b'a'..=b'k' | b'm'..=b'z'
            )
        })
}

/// Classify one tx-status HTTP body. Transport failure is 503. A missing or
/// rejected transaction is a 4xx and does not prove the receipt was checked.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn read_tx_response(
    http_status: u16,
    body: &str,
) -> Result<serde_json::Value, (u16, &'static str)> {
    if http_status == 0 || http_status >= 500 {
        return Err((503, "The settlement receipt could not be read."));
    }
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) if http_status >= 400 => {
            return Err((503, "The settlement receipt could not be read."))
        }
        Err(_) => return Err((400, "tx status was not readable")),
    };
    if let Some(err) = value.get("error").filter(|item| !item.is_null()) {
        let message = err
            .get("data")
            .and_then(|data| data.as_str())
            .or_else(|| err.get("message").and_then(|item| item.as_str()))
            .unwrap_or("");
        let lower = message.to_ascii_lowercase();
        if lower.contains("doesn't exist")
            || lower.contains("does not exist")
            || lower.contains("unknown transaction")
            || lower.contains("db not found")
        {
            return Err((404, "transaction was not found"));
        }
        if http_status >= 400 || lower.contains("invalid") || lower.contains("parse") {
            return Err((400, "tx hash was rejected"));
        }
        return Err((400, "tx hash was rejected"));
    }
    if http_status >= 400 {
        return Err((503, "The settlement receipt could not be read."));
    }
    Ok(value.get("result").cloned().unwrap_or(value))
}

/// Accept a settlement only when the FINAL receipt pays the recorded result.
/// A second report of the same transaction returns the same record.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn confirm_vault_receipt(
    pot: &PotAuthorization,
    tx_hash: &str,
    tx_result: &serde_json::Value,
    pot_yocto: u128,
    factory_id: &str,
    heist_id: &str,
) -> Result<PotAuthorization, &'static str> {
    let tx_hash = tx_hash.trim();
    if tx_hash.is_empty() {
        return Err("tx hash required");
    }
    if !valid_tx_hash(tx_hash) {
        return Err("tx hash is malformed");
    }
    if pot.settled {
        return if pot.tx_hash == tx_hash {
            let mut same = pot.clone();
            normalize_vault_pot(&mut same);
            Ok(same)
        } else {
            Err("already settled")
        };
    }
    if pay_rule_for(&pot.outcome).is_none() {
        return Err("match is not ready to settle");
    }
    let status = tx_result
        .pointer("/status")
        .or_else(|| tx_result.get("status"));
    let success = status
        .map(|value| value.get("SuccessValue").is_some() || value.get("successValue").is_some())
        .unwrap_or(false);
    if status.is_some_and(|value| value.get("Failure").is_some() || value.get("failure").is_some())
    {
        return Err("transaction failed");
    }
    // Absent, null, and non-string values are not FINAL. Top-level success does not fill this in.
    if tx_result
        .get("final_execution_status")
        .and_then(|value| value.as_str())
        != Some("FINAL")
    {
        return Err("transaction is not final success");
    }
    if !success {
        return Err("transaction is not final success");
    }
    if !factory_id.is_empty() {
        let receiver = tx_result
            .pointer("/transaction/receiver_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if receiver != factory_id {
            return Err("receipt contract does not match");
        }
    }
    let (event, executor) = match vault_settled_event(tx_result) {
        Ok(Some(found)) => found,
        Ok(None) => return Err("vault_settled receipt missing"),
        Err(message) => return Err(message),
    };
    if !heist_id.is_empty() && executor != heist_id {
        return Err("receipt contract does not match");
    }
    let id = event
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let idem = pot.idempotency_key.strip_prefix("pot:").unwrap_or("");
    if id.is_empty() || id != idem {
        return Err("receipt is for a different game");
    }
    if event.get("outcome").and_then(|value| value.as_str()) != Some(pot.outcome.as_str()) {
        return Err("receipt outcome does not match the match");
    }
    let payouts = event
        .get("payouts")
        .and_then(|value| value.as_array())
        .ok_or("receipt has no payouts")?;
    let mut got: Vec<(String, u128)> = Vec::new();
    let mut total = 0_u128;
    for row in payouts {
        let account = row
            .get("account")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let amount = row
            .get("amount")
            .and_then(|value| value.as_str())
            .unwrap_or("0")
            .parse::<u128>()
            .map_err(|_| "payout amount is not an integer")?;
        total = total.saturating_add(amount);
        got.push((account, amount));
    }
    let call = match pot.outcome.as_str() {
        "win" => fe2vi_core::VaultPotCall::Win {
            winner: pot.winner.clone(),
        },
        "draw" => fe2vi_core::VaultPotCall::Draw,
        "cancel" => fe2vi_core::VaultPotCall::Cancel,
        _ => return Err("match is not ready to settle"),
    };
    if pot_yocto == 0 || total != pot_yocto {
        return Err("receipt amount does not match the pot");
    }
    let mut expected =
        fe2vi_core::vault_pot_shares(pot_yocto, &pot.creator, &pot.participants, &call)
            .map_err(|_| "receipt payees do not match the match")?;
    expected.sort();
    got.sort();
    if got != expected {
        return Err("receipt payees do not match the match");
    }
    let mut next = pot.clone();
    next.settled = true;
    next.pays = true;
    next.tx_hash = tx_hash.to_string();
    next.payouts = expected
        .into_iter()
        .map(|(account, amount)| VaultPayout {
            account,
            amount: amount.to_string(),
        })
        .collect();
    normalize_vault_pot(&mut next);
    Ok(next)
}

/// Fold one verified settlement into the token career. A repeat match id does nothing.
/// Unsettled rows, empty hashes, and accounts that did not play are ignored.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn apply_receipt_to_career(
    career: &mut serde_json::Value,
    near: &str,
    pot: &PotAuthorization,
) -> bool {
    let near = near.trim();
    if near.is_empty()
        || !pot.settled
        || !pot.pays
        || pot.finality != "FINAL"
        || pot.tx_hash.trim().is_empty()
        || pot.settlement_method != "settle_vault"
        || !pot.participants.iter().any(|account| account == near)
    {
        return false;
    }
    let match_id = pot
        .idempotency_key
        .strip_prefix("pot:")
        .unwrap_or("")
        .to_string();
    if match_id.is_empty() {
        return false;
    }
    let (wins, draws, losses) = match pot.outcome.as_str() {
        "win" if pot.winner == near => (1_u64, 0, 0),
        "win" => (0, 0, 1),
        "draw" => (0, 1, 0),
        _ => return false,
    };
    if !career.is_object() {
        *career = serde_json::json!({});
    }
    let root = career.as_object_mut().unwrap();
    let record = root
        .entry("vaultReceipts")
        .or_insert_with(|| serde_json::json!({}));
    if !record.is_object() {
        *record = serde_json::json!({});
    }
    let record = record.as_object_mut().unwrap();
    let mut ids: Vec<String> = record
        .get("matchIds")
        .and_then(|value| value.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if ids.iter().any(|id| id == &match_id) {
        return false;
    }
    let payout = pot
        .payouts
        .iter()
        .filter(|row| row.account == near)
        .filter_map(|row| row.amount.parse::<u128>().ok())
        .fold(0_u128, u128::saturating_add);
    let prior = record
        .get("payoutYocto")
        .and_then(|value| value.as_str())
        .and_then(|text| text.parse::<u128>().ok())
        .unwrap_or(0);
    let next_wins = count_field(record, "wins") + wins;
    let next_draws = count_field(record, "draws") + draws;
    let next_losses = count_field(record, "losses") + losses;
    ids.push(match_id);
    record.insert("wins".into(), serde_json::json!(next_wins));
    record.insert("draws".into(), serde_json::json!(next_draws));
    record.insert("losses".into(), serde_json::json!(next_losses));
    record.insert(
        "payoutYocto".into(),
        serde_json::Value::String(prior.saturating_add(payout).to_string()),
    );
    record.insert("matchIds".into(), serde_json::json!(ids));
    record.insert(
        "source".into(),
        serde_json::Value::String("settled-receipts".into()),
    );
    true
}

#[cfg(any(test, target_arch = "wasm32"))]
fn count_field(record: &serde_json::Map<String, serde_json::Value>, key: &str) -> u64 {
    record
        .get(key)
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
}

#[cfg(any(test, target_arch = "wasm32"))]
fn receipt_outcome_failed(receipt: &serde_json::Value) -> bool {
    let Some(status) = receipt.pointer("/outcome/status") else {
        return true;
    };
    if status.get("Failure").is_some() || status.get("failure").is_some() {
        return true;
    }
    !(status.get("SuccessValue").is_some()
        || status.get("successValue").is_some()
        || status.get("SuccessReceiptId").is_some()
        || status.get("successReceiptId").is_some())
}

/// A `vault_settled` log is not proof of payment. Every receipt outcome is
/// checked before any event is accepted. One failed receipt rejects the tx.
#[cfg(any(test, target_arch = "wasm32"))]
fn vault_settled_event(
    tx_result: &serde_json::Value,
) -> Result<Option<(serde_json::Value, String)>, &'static str> {
    let Some(receipts) = tx_result
        .get("receipts_outcome")
        .and_then(|value| value.as_array())
    else {
        return Ok(None);
    };
    if receipts.iter().any(receipt_outcome_failed) {
        return Err("transaction failed");
    }
    let mut found = None;
    for receipt in receipts {
        let Some(logs) = receipt
            .pointer("/outcome/logs")
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for line in logs {
            let Some(line) = line.as_str() else {
                continue;
            };
            let Some(json) = line.strip_prefix("EVENT_JSON:") else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<serde_json::Value>(json) else {
                continue;
            };
            if event.get("event").and_then(|value| value.as_str()) != Some("vault_settled") {
                continue;
            }
            let Some(data) = event
                .get("data")
                .and_then(|value| value.as_array())
                .and_then(|rows| rows.first())
                .cloned()
            else {
                continue;
            };
            let executor = receipt
                .pointer("/outcome/executor_id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            if found.is_none() {
                found = Some((data, executor));
            }
        }
    }
    Ok(found)
}

/// Apply `fe2vi_core::resolve_call` to a `get_heist` body. `pays` stays false.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn contract_pot(match_id: &str, heist_json: &str, now_ns: Option<u64>) -> PotAuthorization {
    let mut pot = stored_pot(match_id);
    let text = heist_json.trim();
    if text.is_empty() || text == "null" {
        pot.outcome = "not_linked".to_string();
        pot.reason =
            "get_heist has no game for this match id. A finished Vault match is not a payout."
                .to_string();
        return pot;
    }
    let Some(resolved) = pull_bool(text, "resolved") else {
        return unreadable(pot);
    };
    let Some(delisted) = pull_bool(text, "delisted") else {
        return unreadable(pot);
    };
    let Some(started_at_ns) = pull_u64(text, "started_at_ns") else {
        return unreadable(pot);
    };
    let Some(duration_seconds) = pull_u64(text, "duration_seconds") else {
        return unreadable(pot);
    };
    let needs_clock = !resolved && !delisted;
    let now_ns = if needs_clock {
        match now_ns {
            Some(now_ns) => now_ns,
            None => {
                pot.outcome = "clock_unreadable".to_string();
                pot.reason = "The heist block clock was not readable, so resolve was not accepted."
                    .to_string();
                return pot;
            }
        }
    } else {
        now_ns.unwrap_or(0)
    };
    match fe2vi_core::resolve_call(resolved, delisted, started_at_ns, duration_seconds, now_ns) {
        Ok(()) => {
            pot.outcome = "resolve_accepted".to_string();
            pot.contract_would_accept_resolve = true;
            pot.reason = "The heist resolve call would be accepted. The contract then refunds the creator or splits by its own action scores. Arena points are not an amount. This response does not move NEAR.".to_string();
        }
        Err("already resolved") => {
            pot.outcome = "already_resolved".to_string();
            pot.reason = "The heist contract would reject a second resolve with already resolved. That is the payment idempotency. This response does not move NEAR.".to_string();
        }
        Err("game still running") => {
            pot.outcome = "still_running".to_string();
            pot.reason =
                "The heist contract would reject resolve with game still running.".to_string();
        }
        Err(reason) => {
            pot.outcome = "resolve_rejected".to_string();
            pot.reason = reason.to_string();
        }
    }
    pot
}

#[cfg(any(test, target_arch = "wasm32"))]
fn unreadable(mut pot: PotAuthorization) -> PotAuthorization {
    pot.outcome = "contract_unreadable".to_string();
    pot.reason = "The heist view did not include the fields resolve checks.".to_string();
    pot.contract_would_accept_resolve = false;
    pot.pays = false;
    pot
}

#[cfg(target_arch = "wasm32")]
pub fn replace_pot(body: &str, pot: &PotAuthorization) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.pointer("/vault/pot")?;
    value["vault"]["pot"] = serde_json::to_value(pot).ok()?;
    serde_json::to_string(&value).ok()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn pull_u128(text: &str, field: &str) -> Option<u128> {
    let needle = format!("\"{field}\"");
    let index = text.find(&needle)?;
    let rest = text[index + needle.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let digits: String = rest
        .chars()
        .take_while(|byte| byte.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) fn pull_u64(text: &str, field: &str) -> Option<u64> {
    let needle = format!("\"{field}\"");
    let index = text.find(&needle)?;
    let rest = text[index + needle.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let digits: String = rest
        .chars()
        .take_while(|byte| byte.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
fn pull_bool(text: &str, field: &str) -> Option<bool> {
    let needle = format!("\"{field}\"");
    let index = text.find(&needle)?;
    let rest = text[index + needle.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn handle<'a>(handles: &'a HashMap<String, String>, agent_id: &'a str) -> &'a str {
    handles
        .get(agent_id)
        .map(String::as_str)
        .unwrap_or(agent_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handles(ids: &[&str]) -> HashMap<String, String> {
        ids.iter()
            .map(|id| ((*id).to_string(), (*id).to_string()))
            .collect()
    }

    #[test]
    fn simultaneous_orders_resolve_and_score() {
        let mut game = VaultMatch::start(&["a".into(), "b".into()]);
        game.submit("a", "crack", None, Some("open the lock"), 1)
            .unwrap();
        game.submit("b", "sabotage", Some("a"), Some("stop the thief"), 1)
            .unwrap();
        assert!(game.ready());
        let rows = game.resolve(2);
        assert_eq!(rows.len(), 2);
        assert_eq!(game.locks.iter().filter(|open| **open).count(), 0);
        assert_eq!(game.seat("a").unwrap().heat, 1);
        assert_eq!(game.seat("b").unwrap().points, 1);
    }

    #[test]
    fn sabotage_cancels_a_crack_and_public_view_hides_briefs() {
        let mut game = VaultMatch::start(&["a".into(), "b".into()]);
        game.submit("a", "crack", None, Some("open the first lock"), 1)
            .unwrap();
        game.submit("b", "sabotage", Some("a"), Some("hit the crack"), 1)
            .unwrap();
        game.resolve(2);
        assert_eq!(game.locks.iter().filter(|open| **open).count(), 0);
        let public = game.public_json(&handles(&["a", "b"]));
        assert!(public["seats"][0]["brief"].is_null());
    }

    #[test]
    fn objectives_finalize_at_the_end() {
        let mut game = VaultMatch::start(&["thief".into(), "guard".into()]);
        game.loot = 0;
        game.locks[0] = true;
        game.seats[0].loot_taken = 2;
        game.submit("thief", "cover", None, None, 1).unwrap();
        game.submit("guard", "cover", None, None, 1).unwrap();
        game.resolve(2);
        assert!(game.ended);
        assert!(game.seat("thief").unwrap().objective_met);
        assert_eq!(game.score("thief"), 5);
    }

    #[test]
    fn best_of_five_ends_when_one_seat_takes_three_rounds() {
        let mut game = VaultMatch::start(&["a".into(), "b".into()]);
        for now in 1..=3 {
            game.seats[0].intel = 1;
            game.seats[0].tools = 1;
            game.submit("a", "crack", None, None, now).unwrap();
            game.submit("b", "scout", None, None, now).unwrap();
            game.resolve(now + 1);
        }
        assert!(game.ended);
        assert_eq!(game.seat("a").unwrap().round_wins, ROUNDS_TO_WIN);
        assert!(game.round <= MAX_ROUNDS);
        let public = game.public_json(&handles(&["a", "b"]));
        let rounds = public["rounds"].as_array().unwrap();
        assert_eq!(rounds.len(), 3);
        assert_eq!(rounds[0]["winner"], "a");
        assert_eq!(rounds[0]["draw"], false);
        assert_eq!(rounds[0]["seats"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_finished_match_authorizes_one_pot_resolve() {
        let mut game = VaultMatch::start(&["a".into(), "b".into()]);
        game.ended = true;
        game.seal_pot("mt_one");
        game.seal_pot("mt_one");
        let pot = game.pot.clone().unwrap();
        assert_eq!(pot.idempotency_key, "pot:mt_one");
        assert!(!pot.pays);
        assert!(!pot.contract_would_accept_resolve);
        assert_eq!(pot.outcome, "awaiting_contract_view");
        let public = game.public_json(&handles(&["a", "b"]));
        assert_eq!(public["pot"]["idempotencyKey"], "pot:mt_one");
        assert_eq!(public["pot"]["pays"], false);
    }

    #[test]
    fn a_vault_result_names_the_winner_without_inventing_near() {
        let mut game = VaultMatch::start(&["a".into(), "b".into()]);
        game.ended = true;
        game.seal_vault_result(
            "mt_win",
            &[
                ("a".into(), 4, Some("ann.testnet".into())),
                ("b".into(), 1, Some("bob.testnet".into())),
            ],
        );
        game.seal_vault_result("mt_win", &[("a".into(), 0, Some("other.testnet".into()))]);
        let pot = game.pot.clone().unwrap();
        assert_eq!(pot.rule, "winner-takes-all");
        assert_eq!(pot.outcome, "win");
        assert_eq!(pot.winner, "ann.testnet");
        assert!(!pot.pays);
        let mut draw = VaultMatch::start(&["a".into(), "b".into()]);
        draw.ended = true;
        draw.seal_vault_result(
            "mt_draw",
            &[
                ("a".into(), 2, Some("ann.testnet".into())),
                ("b".into(), 2, Some("bob.testnet".into())),
            ],
        );
        let draw = draw.pot.unwrap();
        assert_eq!(draw.outcome, "draw");
        assert_eq!(draw.rule, "equal-split");
        assert_eq!(draw.pay_rule, "equal-split");
        assert_eq!(draw.settlement_method, "settle_vault");
        assert!(!draw.contract_would_accept_resolve);
        let mut missing = VaultMatch::start(&["a".into()]);
        missing.ended = true;
        missing.seal_vault_result("mt_none", &[("a".into(), 1, None)]);
        assert_eq!(missing.pot.unwrap().outcome, "unmapped");
    }

    #[test]
    fn a_receipt_settles_only_the_recorded_payee_and_replay_does_not_change_it() {
        let mut pot = stored_pot("mt_win");
        pot.rule = "winner-takes-all".to_string();
        pot.outcome = "win".to_string();
        pot.winner = "ann.testnet".to_string();
        pot.participants = vec!["ann.testnet".into(), "bob.testnet".into()];
        let tx = serde_json::json!({
            "final_execution_status": "FINAL",
            "status": {"SuccessValue": ""},
            "receipts_outcome": [{
                "outcome": {
                    "status": {"SuccessValue": ""},
                    "logs": ["EVENT_JSON:{\"event\":\"vault_settled\",\"data\":[{\"id\":\"mt_win\",\"outcome\":\"win\",\"payouts\":[{\"account\":\"ann.testnet\",\"amount\":\"10\"}]}]}"]
                }
            }]
        });
        let settled = confirm_vault_receipt(&pot, &"A".repeat(44), &tx, 10, "", "").unwrap();
        assert!(settled.settled && settled.pays);
        let hash = "A".repeat(44);
        assert_eq!(settled.tx_hash, hash);
        assert_eq!(settled.finality, "FINAL");
        assert_eq!(settled.settlement_method, "settle_vault");
        assert!(!settled.contract_would_accept_resolve);
        let again = confirm_vault_receipt(&settled, &hash, &tx, 10, "", "").unwrap();
        assert_eq!(again.tx_hash, hash);
        assert_eq!(
            confirm_vault_receipt(&settled, &"B".repeat(44), &tx, 10, "", "").unwrap_err(),
            "already settled"
        );
        let wrong = serde_json::json!({
            "final_execution_status": "FINAL",
            "status": {"SuccessValue": ""},
            "receipts_outcome": [{
                "outcome": {
                    "status": {"SuccessValue": ""},
                    "logs": ["EVENT_JSON:{\"event\":\"vault_settled\",\"data\":[{\"id\":\"mt_win\",\"outcome\":\"win\",\"payouts\":[{\"account\":\"bob.testnet\",\"amount\":\"10\"}]}]}"]
                }
            }]
        });
        assert_eq!(
            confirm_vault_receipt(&pot, &"C".repeat(44), &wrong, 10, "", "").unwrap_err(),
            "receipt payees do not match the match"
        );
        pot.outcome = "unmapped".to_string();
        assert_eq!(
            confirm_vault_receipt(&pot, &"D".repeat(44), &tx, 10, "", "").unwrap_err(),
            "match is not ready to settle"
        );
    }

    fn captured(hash_game: &str, outcome: &str, payouts: &str) -> serde_json::Value {
        let line = format!(
            "EVENT_JSON:{{\"event\":\"vault_settled\",\"data\":[{{\"id\":\"{hash_game}\",\"outcome\":\"{outcome}\",\"payouts\":{payouts}}}]}}"
        );
        serde_json::json!({
            "final_execution_status": "FINAL",
            "status": {"SuccessValue": ""},
            "receipts_outcome": [{
                "outcome": {"status": {"SuccessValue": ""}, "logs": [line]}
            }]
        })
    }

    #[test]
    fn captured_final_receipts_reject_a_cross_match_and_a_wrong_amount() {
        // Read-only copies of the FINAL events from
        // 8VwaWeX7tFNH7ph8uKDhbvZtU3RfL2JePt99HTXagU61 and
        // AyyDfVLPHoh8vzmxRFBAxzSRBD31Z2fHW4EpPRsmdMVy.
        // This is not a new transaction and not a production HTTP call.
        let pot_yocto = 20_000_000_000_000_000_000_000_u128;
        let win_tx = captured(
            "mt_730a1559a8f36fd0",
            "win",
            r#"[{"account":"fe2vidisp7a3c.testnet","amount":"20000000000000000000000"}]"#,
        );
        let draw_tx = captured(
            "mt_41f001a70efa3af0",
            "draw",
            r#"[{"account":"fe2vibuyer7a3c.testnet","amount":"10000000000000000000000"},{"account":"fe2vidisp7a3c.testnet","amount":"10000000000000000000000"}]"#,
        );
        let mut win = stored_pot("mt_730a1559a8f36fd0");
        win.outcome = "win".to_string();
        win.winner = "fe2vidisp7a3c.testnet".to_string();
        win.participants = vec![
            "fe2vidisp7a3c.testnet".into(),
            "fe2vibuyer7a3c.testnet".into(),
        ];
        win.creator = "fe2vidisp7a3c.testnet".to_string();
        let mut draw = stored_pot("mt_41f001a70efa3af0");
        draw.outcome = "draw".to_string();
        draw.participants = win.participants.clone();
        draw.creator = win.creator.clone();
        let win_hash = "8VwaWeX7tFNH7ph8uKDhbvZtU3RfL2JePt99HTXagU61";
        let draw_hash = "AyyDfVLPHoh8vzmxRFBAxzSRBD31Z2fHW4EpPRsmdMVy";
        let settled = confirm_vault_receipt(&win, win_hash, &win_tx, pot_yocto, "", "").unwrap();
        assert!(settled.settled);
        assert_eq!(settled.rule, "winner-takes-all");
        assert_eq!(
            confirm_vault_receipt(&settled, win_hash, &win_tx, pot_yocto, "", "")
                .unwrap()
                .tx_hash,
            win_hash
        );
        assert_eq!(
            confirm_vault_receipt(&win, draw_hash, &draw_tx, pot_yocto, "", "").unwrap_err(),
            "receipt is for a different game"
        );
        assert_eq!(
            confirm_vault_receipt(&draw, win_hash, &win_tx, pot_yocto, "", "").unwrap_err(),
            "receipt is for a different game"
        );
        let short = captured(
            "mt_730a1559a8f36fd0",
            "win",
            r#"[{"account":"fe2vidisp7a3c.testnet","amount":"1"}]"#,
        );
        assert_eq!(
            confirm_vault_receipt(&win, win_hash, &short, pot_yocto, "", "").unwrap_err(),
            "receipt amount does not match the pot"
        );
        let failed = serde_json::json!({
            "final_execution_status": "FINAL",
            "status": {"SuccessValue": ""},
            "receipts_outcome": [{
                "outcome": {"status": {"Failure": {"ActionError": {}}}, "logs": []}
            }]
        });
        assert_eq!(
            confirm_vault_receipt(&win, win_hash, &failed, pot_yocto, "", "").unwrap_err(),
            "transaction failed"
        );
        let pending = serde_json::json!({"status": {}, "receipts_outcome": []});
        assert_eq!(
            confirm_vault_receipt(&win, win_hash, &pending, pot_yocto, "", "").unwrap_err(),
            "transaction is not final success"
        );
        let other_event = serde_json::json!({
            "final_execution_status": "FINAL",
            "status": {"SuccessValue": ""},
            "receipts_outcome": [{
                "outcome": {
                    "status": {"SuccessValue": ""},
                    "logs": ["EVENT_JSON:{\"event\":\"game_resolved\",\"data\":[{\"id\":\"mt_730a1559a8f36fd0\"}]}"]
                }
            }]
        });
        assert_eq!(
            confirm_vault_receipt(&win, win_hash, &other_event, pot_yocto, "", "").unwrap_err(),
            "vault_settled receipt missing"
        );
    }

    #[test]
    fn a_bad_receipt_request_is_not_a_chain_outage() {
        assert_eq!(
            parse_receipt_request(&serde_json::json!({})).unwrap_err().0,
            400
        );
        assert_eq!(
            parse_receipt_request(&serde_json::json!({"txHash":"not-a-hash","senderAccountId":"fe2vidisp7a3c.testnet"}))
                .unwrap_err()
                .0,
            400
        );
        assert_eq!(read_tx_response(0, "").unwrap_err().0, 503);
        assert_eq!(
            read_tx_response(
                200,
                r#"{"error":{"message":"Server error","data":"Transaction doesn't exist"}}"#
            )
            .unwrap_err(),
            (404, "transaction was not found")
        );
        assert_eq!(
            read_tx_response(400, r#"{"error":{"message":"Parse error"}}"#)
                .unwrap_err()
                .0,
            400
        );
        let ok = read_tx_response(200, r#"{"result":{"status":{"SuccessValue":""}}}"#).unwrap();
        assert!(ok.get("status").is_some());
    }

    #[test]
    fn synthetic_fixtures_reject_cases_that_are_not_live_transactions() {
        // These bodies are constructed. They are not production HTTP calls and
        // they are not the two FINAL settlements.
        let factory = "factory.fe2vidisp7a3c.testnet";
        let heist = "heist.fe2vidisp7a3c.testnet";
        let hash = "B".repeat(44);
        let mut win = stored_pot("mt_730a1559a8f36fd0");
        win.outcome = "win".to_string();
        win.winner = "fe2vidisp7a3c.testnet".to_string();
        win.participants = vec![
            "fe2vibuyer7a3c.testnet".into(),
            "fe2vidisp7a3c.testnet".into(),
        ];
        win.creator = "fe2vidisp7a3c.testnet".to_string();
        let mut draw = stored_pot("mt_41f001a70efa3af0");
        draw.outcome = "draw".to_string();
        draw.participants = win.participants.clone();
        draw.creator = win.creator.clone();
        let pot_yocto = 20_000_000_000_000_000_000_000_u128;
        let mut body = captured(
            "mt_730a1559a8f36fd0",
            "win",
            r#"[{"account":"fe2vidisp7a3c.testnet","amount":"20000000000000000000000"}]"#,
        );
        body["final_execution_status"] = serde_json::json!("FINAL");
        body["transaction"] = serde_json::json!({"receiver_id": factory});
        body["receipts_outcome"][0]["outcome"]["executor_id"] = serde_json::json!(heist);
        let before = win.clone();
        body["final_execution_status"] = serde_json::json!("EXECUTED");
        assert_eq!(
            confirm_vault_receipt(&win, &hash, &body, pot_yocto, factory, heist).unwrap_err(),
            "transaction is not final success"
        );
        assert_eq!(win, before);
        body["final_execution_status"] = serde_json::json!("FINAL");
        body["transaction"]["receiver_id"] = serde_json::json!("other.near");
        assert_eq!(
            confirm_vault_receipt(&win, &hash, &body, pot_yocto, factory, heist).unwrap_err(),
            "receipt contract does not match"
        );
        assert_eq!(win, before);
        body["transaction"]["receiver_id"] = serde_json::json!(factory);
        body["receipts_outcome"][0]["outcome"]["executor_id"] = serde_json::json!(factory);
        assert_eq!(
            confirm_vault_receipt(&win, &hash, &body, pot_yocto, factory, heist).unwrap_err(),
            "receipt contract does not match"
        );
        body["receipts_outcome"][0]["outcome"]["executor_id"] = serde_json::json!(heist);
        let draw_on_win = captured(
            "mt_730a1559a8f36fd0",
            "draw",
            r#"[{"account":"fe2vibuyer7a3c.testnet","amount":"10000000000000000000000"},{"account":"fe2vidisp7a3c.testnet","amount":"10000000000000000000000"}]"#,
        );
        assert_eq!(
            confirm_vault_receipt(&win, &hash, &draw_on_win, pot_yocto, "", "").unwrap_err(),
            "receipt outcome does not match the match"
        );
        let win_on_draw = captured(
            "mt_41f001a70efa3af0",
            "win",
            r#"[{"account":"fe2vidisp7a3c.testnet","amount":"20000000000000000000000"}]"#,
        );
        assert_eq!(
            confirm_vault_receipt(&draw, &"C".repeat(44), &win_on_draw, pot_yocto, "", "")
                .unwrap_err(),
            "receipt outcome does not match the match"
        );
        assert!(!win.settled && win.tx_hash.is_empty());
        assert!(!draw.settled && draw.tx_hash.is_empty());
        let settled = confirm_vault_receipt(&win, &hash, &body, pot_yocto, factory, heist).unwrap();
        assert_eq!(settled.tx_hash, hash);
        assert_eq!(
            confirm_vault_receipt(&settled, &"D".repeat(44), &body, pot_yocto, factory, heist)
                .unwrap_err(),
            "already settled"
        );
        assert_eq!(settled.tx_hash, hash);
    }

    #[test]
    fn a_failed_receipt_beside_a_settlement_event_is_not_payment() {
        // Synthetic. The valid settlement receipt stays in the array. A failed
        // receipt is inserted before it, then after it. This is not a live
        // transaction and not a production HTTP call.
        let pot_yocto = 20_000_000_000_000_000_000_000_u128;
        let hash = "E".repeat(44);
        let mut win = stored_pot("mt_730a1559a8f36fd0");
        win.outcome = "win".to_string();
        win.winner = "fe2vidisp7a3c.testnet".to_string();
        win.participants = vec![
            "fe2vibuyer7a3c.testnet".into(),
            "fe2vidisp7a3c.testnet".into(),
        ];
        win.creator = "fe2vidisp7a3c.testnet".to_string();
        let valid = captured(
            "mt_730a1559a8f36fd0",
            "win",
            r#"[{"account":"fe2vidisp7a3c.testnet","amount":"20000000000000000000000"}]"#,
        );
        let before = win.clone();
        let accepted = confirm_vault_receipt(&win, &hash, &valid, pot_yocto, "", "").unwrap();
        assert_eq!(accepted.tx_hash, hash);
        assert!(accepted.pays);
        let event_line = valid["receipts_outcome"][0]["outcome"]["logs"][0].clone();
        let failed_receipt = serde_json::json!({
            "outcome": {
                "status": {"Failure": {"ActionError": {"kind": "FunctionCallError"}}},
                "logs": [event_line]
            }
        });
        let mut failure_first = valid.clone();
        failure_first["receipts_outcome"]
            .as_array_mut()
            .unwrap()
            .insert(0, failed_receipt.clone());
        assert_eq!(
            confirm_vault_receipt(&win, &hash, &failure_first, pot_yocto, "", "").unwrap_err(),
            "transaction failed"
        );
        assert_eq!(win, before);
        let mut failure_last = valid.clone();
        failure_last["receipts_outcome"]
            .as_array_mut()
            .unwrap()
            .push(failed_receipt);
        assert_eq!(
            confirm_vault_receipt(&win, &hash, &failure_last, pot_yocto, "", "").unwrap_err(),
            "transaction failed"
        );
        assert_eq!(win, before);
        assert!(!win.settled && win.tx_hash.is_empty());
    }

    #[test]
    fn finality_must_be_the_explicit_string_final() {
        // Synthetic edits of one FINAL-shaped body. Absent, null, and non-string
        // finality are rejected. Restoring the string FINAL still accepts it.
        let pot_yocto = 20_000_000_000_000_000_000_000_u128;
        let hash = "F".repeat(44);
        let mut win = stored_pot("mt_730a1559a8f36fd0");
        win.outcome = "win".to_string();
        win.winner = "fe2vidisp7a3c.testnet".to_string();
        win.participants = vec![
            "fe2vibuyer7a3c.testnet".into(),
            "fe2vidisp7a3c.testnet".into(),
        ];
        win.creator = "fe2vidisp7a3c.testnet".to_string();
        let mut body = captured(
            "mt_730a1559a8f36fd0",
            "win",
            r#"[{"account":"fe2vidisp7a3c.testnet","amount":"20000000000000000000000"}]"#,
        );
        let before = win.clone();
        body.as_object_mut()
            .unwrap()
            .remove("final_execution_status");
        assert_eq!(
            confirm_vault_receipt(&win, &hash, &body, pot_yocto, "", "").unwrap_err(),
            "transaction is not final success"
        );
        assert_eq!(win, before);
        for value in [
            serde_json::Value::Null,
            serde_json::json!(1),
            serde_json::json!(true),
            serde_json::json!({"status": "FINAL"}),
        ] {
            body["final_execution_status"] = value;
            assert_eq!(
                confirm_vault_receipt(&win, &hash, &body, pot_yocto, "", "").unwrap_err(),
                "transaction is not final success"
            );
            assert_eq!(win, before);
        }
        body["final_execution_status"] = serde_json::json!("FINAL");
        let settled = confirm_vault_receipt(&win, &hash, &body, pot_yocto, "", "").unwrap();
        assert_eq!(settled.tx_hash, hash);
        assert_eq!(settled.finality, "FINAL");
        assert!(settled.pays);
    }

    #[test]
    fn read_only_final_tx_http_rejects_the_crossed_match() {
        // HTTP read of the two existing FINAL transactions. This is not a
        // production receipt-route call and it does not send a transaction.
        let factory = "factory.fe2vidisp7a3c.testnet";
        let heist = "heist.fe2vidisp7a3c.testnet";
        let win_hash = "8VwaWeX7tFNH7ph8uKDhbvZtU3RfL2JePt99HTXagU61";
        let draw_hash = "AyyDfVLPHoh8vzmxRFBAxzSRBD31Z2fHW4EpPRsmdMVy";
        let win_tx = http_tx_status(win_hash);
        let draw_tx = http_tx_status(draw_hash);
        let pot_yocto = 20_000_000_000_000_000_000_000_u128;
        let mut win = stored_pot("mt_730a1559a8f36fd0");
        win.outcome = "win".to_string();
        win.winner = "fe2vidisp7a3c.testnet".to_string();
        win.participants = vec![
            "fe2vibuyer7a3c.testnet".into(),
            "fe2vidisp7a3c.testnet".into(),
        ];
        win.creator = "fe2vidisp7a3c.testnet".to_string();
        let mut draw = stored_pot("mt_41f001a70efa3af0");
        draw.outcome = "draw".to_string();
        draw.participants = win.participants.clone();
        draw.creator = win.creator.clone();
        let settled_win =
            confirm_vault_receipt(&win, win_hash, &win_tx, pot_yocto, factory, heist).unwrap();
        assert!(settled_win.pays && settled_win.settled);
        assert_eq!(settled_win.rule, "winner-takes-all");
        assert_eq!(settled_win.tx_hash, win_hash);
        assert!(!settled_win.contract_would_accept_resolve);
        let settled_draw =
            confirm_vault_receipt(&draw, draw_hash, &draw_tx, pot_yocto, factory, heist).unwrap();
        assert_eq!(settled_draw.rule, "equal-split");
        assert_eq!(settled_draw.pay_rule, "equal-split");
        assert_eq!(settled_draw.creator, "fe2vidisp7a3c.testnet");
        assert!(settled_draw.pays);
        let replay = confirm_vault_receipt(
            &settled_draw,
            draw_hash,
            &draw_tx,
            pot_yocto,
            factory,
            heist,
        )
        .unwrap();
        assert_eq!(replay.tx_hash, draw_hash);
        assert_eq!(
            confirm_vault_receipt(&win, draw_hash, &draw_tx, pot_yocto, factory, heist)
                .unwrap_err(),
            "receipt is for a different game"
        );
        assert_eq!(
            confirm_vault_receipt(&draw, win_hash, &win_tx, pot_yocto, factory, heist).unwrap_err(),
            "receipt is for a different game"
        );
        assert!(!win.settled && win.tx_hash.is_empty());
        assert!(!draw.settled && draw.tx_hash.is_empty());
        assert_eq!(
            confirm_vault_receipt(&settled_win, draw_hash, &draw_tx, pot_yocto, factory, heist)
                .unwrap_err(),
            "already settled"
        );
        assert_eq!(settled_win.tx_hash, win_hash);
    }

    fn http_tx_status(hash: &str) -> serde_json::Value {
        let payload = format!(
            r#"{{"jsonrpc":"2.0","id":"receipt","method":"EXPERIMENTAL_tx_status","params":{{"tx_hash":"{hash}","sender_account_id":"fe2vidisp7a3c.testnet","wait_until":"FINAL"}}}}"#
        );
        let output = std::process::Command::new("curl")
            .args([
                "-sS",
                "-m",
                "40",
                "-X",
                "POST",
                "https://rpc.testnet.fastnear.com",
                "-H",
                "content-type: application/json",
                "--data",
                &payload,
            ])
            .output()
            .expect("curl can read the existing FINAL transaction");
        assert!(
            output.status.success(),
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let body = String::from_utf8(output.stdout).expect("utf8");
        read_tx_response(200, &body).unwrap_or_else(|err| {
            panic!(
                "tx status {} was not a readable FINAL body: {err:?} body={body}",
                hash
            )
        })
    }

    #[test]
    fn a_verified_receipt_credits_the_token_once() {
        let pot_yocto = 20_000_000_000_000_000_000_000_u128;
        let hash = "E".repeat(44);
        let mut win = stored_pot("mt_career");
        win.outcome = "win".to_string();
        win.winner = "ann.testnet".to_string();
        win.participants = vec!["ann.testnet".into(), "bob.testnet".into()];
        win.creator = "ann.testnet".to_string();
        let tx = captured(
            "mt_career",
            "win",
            r#"[{"account":"ann.testnet","amount":"20000000000000000000000"}]"#,
        );
        let settled = confirm_vault_receipt(&win, &hash, &tx, pot_yocto, "", "").unwrap();
        assert_eq!(settled.payouts.len(), 1);
        let mut career = serde_json::json!({});
        assert!(apply_receipt_to_career(
            &mut career,
            "ann.testnet",
            &settled
        ));
        assert_eq!(career["vaultReceipts"]["wins"], 1);
        assert_eq!(career["vaultReceipts"]["losses"], 0);
        assert_eq!(career["vaultReceipts"]["source"], "settled-receipts");
        assert_eq!(
            career["vaultReceipts"]["payoutYocto"],
            "20000000000000000000000"
        );
        assert!(!apply_receipt_to_career(
            &mut career,
            "ann.testnet",
            &settled
        ));
        assert_eq!(career["vaultReceipts"]["wins"], 1);
        let mut loser = serde_json::json!({});
        assert!(apply_receipt_to_career(&mut loser, "bob.testnet", &settled));
        assert_eq!(loser["vaultReceipts"]["losses"], 1);
        assert_eq!(loser["vaultReceipts"]["payoutYocto"], "0");
        let mut open = settled.clone();
        open.settled = false;
        let mut ignored = serde_json::json!({});
        assert!(!apply_receipt_to_career(&mut ignored, "ann.testnet", &open));
        assert!(ignored.get("vaultReceipts").is_none());
    }

    #[test]
    fn the_match_pot_uses_the_heist_resolve_check() {
        let missing = contract_pot("mt_one", "null", None);
        assert_eq!(missing.outcome, "not_linked");
        assert!(!missing.pays);
        assert!(!missing.contract_would_accept_resolve);

        let live = r#"{"resolved":false,"delisted":false,"started_at_ns":1000,"rules":{"duration_seconds":60}}"#;
        let running = contract_pot("mt_one", live, Some(1000));
        assert_eq!(running.outcome, "still_running");
        assert!(!running.contract_would_accept_resolve);

        let open = contract_pot("mt_one", live, Some(1000 + 60 * 1_000_000_000));
        assert_eq!(open.outcome, "resolve_accepted");
        assert!(open.contract_would_accept_resolve);
        assert!(!open.pays);

        let done = r#"{"resolved":true,"delisted":false,"started_at_ns":1000,"rules":{"duration_seconds":60}}"#;
        let again = contract_pot("mt_one", done, Some(1000));
        assert_eq!(again.outcome, "already_resolved");
        assert_eq!(again.idempotency_key, "pot:mt_one");
        assert!(!again.pays);

        let delisted = r#"{"resolved":false,"delisted":true,"started_at_ns":1000,"rules":{"duration_seconds":60}}"#;
        let refund_window = contract_pot("mt_one", delisted, None);
        assert_eq!(refund_window.outcome, "resolve_accepted");
        assert!(refund_window.contract_would_accept_resolve);
        assert!(!refund_window.pays);
    }
}
