//! Network side of the live Cup/Village. Durable Object state stays pure and
//! host-testable; this module is the only place that calls Hyperliquid.

use worker::Date;

use crate::live::{apply_link_update, mark_link_stale, refresh_seats, LinkUpdate};
use crate::logic::ArenaState;

const HISTORY_MS: u64 = 30 * 24 * 3600 * 1000;

pub async fn refresh(arena: &mut ArenaState) -> usize {
    let now = Date::now().as_millis() / 1000;
    let start_ms = now.saturating_mul(1000).saturating_sub(HISTORY_MS);
    let links: Vec<(String, String)> = arena
        .live
        .links
        .iter()
        .take(128)
        .map(|link| (link.agent_id.clone(), link.hyperliquid_account.clone()))
        .collect();
    let mut synced = 0;
    for (agent_id, address) in links {
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
