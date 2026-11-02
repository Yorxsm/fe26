//! Vault v2: a simultaneous, asymmetric-information heist round.
//!
//! Every seat gets a private brief and a public action. Orders resolve together,
//! so intent and outcome are separate records. The engine is deterministic and
//! has no randomness after role assignment.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const MAX_ROUNDS: u32 = 8;
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
}

impl VaultMatch {
    pub fn start(seats: &[String]) -> Self {
        let roster: Vec<VaultSeat> = seats
            .iter()
            .enumerate()
            .map(|(index, agent_id)| {
                let (role, objective, target, brief) = if index % 2 == 0 {
                    (
                        "thief",
                        "loot",
                        None,
                        "Take at least two loot. Every steal raises the alarm; cover your heat.",
                    )
                } else {
                    (
                        "guard",
                        "expose",
                        seats.first().cloned(),
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
                    let before = self.seats[index].heat;
                    self.seats[index].heat = before.saturating_sub(1);
                    if before > 0 {
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
                    if let Some(t) = self.seats.iter().position(|seat| seat.agent_id == target) {
                        self.seats[t].heat = self.seats[t].heat.saturating_add(1);
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
                        seat.heat = seat.heat.saturating_add(1);
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
        self.pending.clear();
        self.round = self.round.saturating_add(1);
        if self.round > MAX_ROUNDS || self.alarm >= MAX_ALARM || self.loot == 0 {
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

    pub fn score(&self, agent_id: &str) -> i32 {
        self.seat(agent_id)
            .map(|seat| seat.points - seat.heat as i32 + if seat.objective_met { 5 } else { 0 })
            .unwrap_or(0)
    }

    pub fn public_json(&self, handles: &HashMap<String, String>) -> serde_json::Value {
        let status = if self.ended {
            "settled"
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
                    "brief": if self.ended { serde_json::Value::String(seat.brief.clone()) } else { serde_json::Value::Null },
                    "intel": seat.intel,
                    "tools": seat.tools,
                    "heat": seat.heat,
                    "points": seat.points,
                    "lootTaken": seat.loot_taken,
                    "score": self.score(&seat.agent_id),
                    "objectiveMet": seat.objective_met,
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
            "locks": self.locks,
            "openLocks": self.locks.iter().filter(|open| **open).count(),
            "loot": self.loot,
            "alarm": self.alarm,
            "seats": seats,
            "results": results,
            "revealed": self.ended,
        })
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
}
