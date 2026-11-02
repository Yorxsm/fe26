//! Network side of the live Cup/Village. Durable Object state stays pure and
//! host-testable; this module is the only place that calls Hyperliquid.

use worker::{Date, Env, Fetch, Headers, Method, Request, RequestInit};

use crate::live::{
    apply_link_update, apply_policy_snapshot, mark_link_stale, mark_policy_mismatch, refresh_seats,
    LinkUpdate, PolicyCheck, POLICY_MAX_AGE_SECS,
};
use crate::logic::ArenaState;

const HISTORY_MS: u64 = 30 * 24 * 3600 * 1000;
const DEFAULT_INTENTS_URL: &str = "https://api.agentsonintents.com";
const POLICY_REFRESH_SECS: u64 = POLICY_MAX_AGE_SECS / 2;
const POLICY_REFRESH_LIMIT: usize = 8;

/// Ask the NEAR Intents provider for the policy view before a live link.
/// The partner key stays in the Worker secret store and never reaches the client.
pub async fn policy_check(env: &Env, agent_id: &str) -> PolicyCheck {
    let (key, base) = match intents_config(env) {
        Ok(config) => config,
        Err(message) => return PolicyCheck::Unavailable(message),
    };
    let policy_url = format!("{base}/v1/agents/{agent_id}/policy");
    let view = match get_json(&key, &policy_url, "policy").await {
        Ok(view) => view,
        Err(message) => return PolicyCheck::Unavailable(message),
    };
    let checked_unix = Date::now().as_millis() / 1000;
    let mut snapshot = crate::live::policy_snapshot_from_view(agent_id, &view, checked_unix);
    if snapshot.status != "APPLIED" || !snapshot.provider_policy_synced || snapshot.policy_frozen {
        return PolicyCheck::active(snapshot);
    }

    let wallet_url = format!("{base}/v1/agents/{agent_id}/wallet");
    let wallet = match get_json(&key, &wallet_url, "wallet").await {
        Ok(wallet) => wallet,
        Err(message) => return PolicyCheck::Unavailable(message),
    };
    let Some(policy_wallet) = snapshot
        .wallet_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return PolicyCheck::Unavailable(
            "NEAR Intents policy lookup did not return wallet_id.".into(),
        );
    };
    let Some(wallet_wallet) = wallet["wallet_id"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return PolicyCheck::Unavailable(
            "NEAR Intents wallet lookup did not return wallet_id.".into(),
        );
    };
    if policy_wallet != wallet_wallet {
        return PolicyCheck::Unavailable(
            "NEAR Intents policy and wallet belong to different wallets.".into(),
        );
    }
    let Some(near_account) = wallet["near_account_id"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return PolicyCheck::Unavailable(
            "NEAR Intents wallet lookup did not return near_account_id.".into(),
        );
    };
    snapshot.wallet_near_account = Some(near_account.to_ascii_lowercase());
    PolicyCheck::active(snapshot)
}

fn intents_config(env: &Env) -> Result<(String, String), String> {
    let key = env
        .var("NEAR_INTENTS_AGENT_API_KEY")
        .ok()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err(
            "Live linking is disabled: NEAR_INTENTS_AGENT_API_KEY is not configured.".into(),
        );
    }
    let base = env
        .var("NEAR_INTENTS_AGENT_API_URL")
        .ok()
        .map(|value| value.to_string())
        .unwrap_or_else(|| DEFAULT_INTENTS_URL.to_string());
    let base = base.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err("NEAR Intents API URL is empty.".into());
    }
    Ok((key, base))
}

async fn get_json(key: &str, url: &str, label: &str) -> Result<serde_json::Value, String> {
    let headers = Headers::new();
    if headers.set("x-api-key", key).is_err() || headers.set("accept", "application/json").is_err()
    {
        return Err("Could not build the NEAR Intents request.".into());
    }
    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);
    let request = Request::new_with_init(url, &init)
        .map_err(|_| "Could not build the NEAR Intents request.".to_string())?;
    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|_| format!("NEAR Intents {label} lookup failed."))?;
    let status = response.status_code();
    let text = response
        .text()
        .await
        .map_err(|_| format!("NEAR Intents {label} response was unreadable."))?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "NEAR Intents {label} lookup returned HTTP {status}."
        ));
    }
    serde_json::from_str(&text).map_err(|_| format!("NEAR Intents {label} response was not JSON."))
}

pub async fn refresh_policies(env: &Env, arena: &mut ArenaState) -> usize {
    let now = Date::now().as_millis() / 1000;
    let mut due: Vec<(u64, String, String)> = arena
        .live
        .links
        .iter()
        .filter(|link| !link.near_intents_agent_id.is_empty())
        .filter(|link| now.saturating_sub(link.policy_checked_unix) >= POLICY_REFRESH_SECS)
        .map(|link| {
            (
                link.policy_checked_unix,
                link.agent_id.clone(),
                link.near_intents_agent_id.clone(),
            )
        })
        .collect();
    due.sort_by_key(|(checked, _, _)| *checked);
    due.truncate(POLICY_REFRESH_LIMIT);
    let mut revalidated = 0;
    for (_, agent_id, near_intents_agent_id) in due {
        let check = policy_check(env, &near_intents_agent_id).await;
        let Some(link) = arena.live.link_mut(&agent_id) else {
            continue;
        };
        match check {
            PolicyCheck::Active(snapshot) => {
                if snapshot.agent_id != link.near_intents_agent_id {
                    mark_policy_mismatch(link, snapshot.checked_unix);
                    revalidated += 1;
                    continue;
                }
                if snapshot.status != "APPLIED"
                    || !snapshot.provider_policy_synced
                    || snapshot.policy_frozen
                {
                    apply_policy_snapshot(link, &snapshot);
                    revalidated += 1;
                    continue;
                }
                match snapshot.wallet_near_account.as_deref() {
                    Some(near_account) if near_account == link.near_account => {
                        apply_policy_snapshot(link, &snapshot);
                        revalidated += 1;
                    }
                    Some(_) => {
                        mark_policy_mismatch(link, snapshot.checked_unix);
                        revalidated += 1;
                    }
                    None => {}
                }
            }
            PolicyCheck::Inactive(snapshot, _) => {
                apply_policy_snapshot(link, &snapshot);
                revalidated += 1;
            }
            PolicyCheck::Unavailable(_) => {}
        }
    }
    revalidated
}

pub async fn refresh(arena: &mut ArenaState) -> usize {
    let now = Date::now().as_millis() / 1000;
    let start_ms = now.saturating_mul(1000).saturating_sub(HISTORY_MS);
    let mut links: Vec<(String, String, u64)> = arena
        .live
        .links
        .iter()
        .map(|link| {
            (
                link.agent_id.clone(),
                link.hyperliquid_account.clone(),
                link.last_sync_unix,
            )
        })
        .collect();
    links.sort_by_key(|(_, _, last_sync)| *last_sync);
    links.truncate(128);
    let mut synced = 0;
    for (agent_id, address, _) in links {
        match crate::markets::snapshot(&address, start_ms).await {
            Ok((account, portfolio, fills, ledger)) => {
                apply_link_update(
                    &mut arena.live,
                    LinkUpdate {
                        agent_id,
                        account,
                        portfolio,
                        fills,
                        ledger,
                    },
                    now,
                );
                synced += 1;
            }
            Err(_) => mark_link_stale(&mut arena.live, &agent_id),
        }
    }
    refresh_seats(&mut arena.live, now);
    arena.live.last_tick_unix = now;
    synced
}
