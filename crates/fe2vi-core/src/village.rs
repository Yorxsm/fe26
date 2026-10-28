//! Trading Village: an always-on simulated tape, rolling boards, and follows.
//! It is not a time-boxed factory game, so platform duration caps do not apply.

use serde::{Deserialize, Serialize};

use crate::price::{simulated_price_micros, WRAP_NEAR};

/// Demo clock used when a caller does not pass a time. The tape is anchored here.
pub const DEMO_EPOCH: u64 = 1_720_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Long,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Hours24,
    Days7,
    Days30,
    All,
}

impl Window {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "24h" => Some(Self::Hours24),
            "7d" => Some(Self::Days7),
            "30d" => Some(Self::Days30),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    fn cutoff(self, now: u64) -> u64 {
        match self {
            Self::Hours24 => now.saturating_sub(24 * 60 * 60),
            Self::Days7 => now.saturating_sub(7 * 24 * 60 * 60),
            Self::Days30 => now.saturating_sub(30 * 24 * 60 * 60),
            Self::All => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    pub agent: String,
    pub name: String,
    pub asset: String,
    pub side: Side,
    pub qty: u32,
    pub unix_secs: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    pub agent: String,
    pub name: String,
    pub pnl_micros: i64,
    pub trades: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowEdge {
    pub follower: String,
    pub target: String,
}

/// Mock tape pinned to [`DEMO_EPOCH`]. Tests use this. The API uses [`demo_tape_at`].
pub fn demo_tape() -> Vec<Fill> {
    demo_tape_at(DEMO_EPOCH)
}

/// The same fills, aged backwards from `now`, so a live clock still has a today board.
pub fn demo_tape_at(now: u64) -> Vec<Fill> {
    vec![
        fill_at(
            now,
            "mason.village.fe2vi.testnet",
            "Mason",
            Side::Long,
            4,
            60 * 60,
        ),
        fill_at(
            now,
            "scout.vault.fe2vi.testnet",
            "Scout",
            Side::Short,
            2,
            3 * 24 * 60 * 60,
        ),
        fill_at(
            now,
            "tape.cup.fe2vi.testnet",
            "Tape",
            Side::Long,
            3,
            20 * 24 * 60 * 60,
        ),
        fill_at(
            now,
            "pike.war.fe2vi.testnet",
            "Pike",
            Side::Short,
            1,
            400 * 24 * 60 * 60,
        ),
    ]
}

fn fill_at(now: u64, agent: &str, name: &str, side: Side, qty: u32, age_secs: u64) -> Fill {
    Fill {
        agent: agent.to_string(),
        name: name.to_string(),
        asset: WRAP_NEAR.to_string(),
        side,
        qty,
        unix_secs: now.saturating_sub(age_secs),
    }
}

pub fn leaderboard(fills: &[Fill], now: u64, window: Window) -> Vec<Standing> {
    let cutoff = window.cutoff(now);
    let mark = simulated_price_micros(WRAP_NEAR, now);
    let mut rows: Vec<Standing> = Vec::new();
    for fill in fills.iter().filter(|fill| fill.unix_secs >= cutoff) {
        let entry = simulated_price_micros(&fill.asset, fill.unix_secs);
        let mark_i = i64::try_from(mark).unwrap_or(i64::MAX);
        let entry_i = i64::try_from(entry).unwrap_or(i64::MAX);
        let diff = match fill.side {
            Side::Long => mark_i.saturating_sub(entry_i),
            Side::Short => entry_i.saturating_sub(mark_i),
        };
        let pnl = diff.saturating_mul(i64::from(fill.qty));
        if let Some(row) = rows.iter_mut().find(|row| row.agent == fill.agent) {
            row.pnl_micros = row.pnl_micros.saturating_add(pnl);
            row.trades = row.trades.saturating_add(1);
        } else {
            rows.push(Standing {
                agent: fill.agent.clone(),
                name: fill.name.clone(),
                pnl_micros: pnl,
                trades: 1,
            });
        }
    }
    rows.sort_by(|left, right| {
        right
            .pnl_micros
            .cmp(&left.pnl_micros)
            .then_with(|| left.agent.cmp(&right.agent))
    });
    rows
}

pub fn follow(
    book: &mut Vec<FollowEdge>,
    follower: &str,
    target: &str,
) -> Result<(), &'static str> {
    if follower.is_empty() || target.is_empty() {
        return Err("account required");
    }
    if follower == target {
        return Err("an agent cannot follow itself");
    }
    if book
        .iter()
        .any(|edge| edge.follower == follower && edge.target == target)
    {
        return Ok(());
    }
    book.push(FollowEdge {
        follower: follower.to_string(),
        target: target.to_string(),
    });
    Ok(())
}

pub fn unfollow(book: &mut Vec<FollowEdge>, follower: &str, target: &str) {
    book.retain(|edge| !(edge.follower == follower && edge.target == target));
}

/// Districts on the city map. Coming-soon buildings have no game behind them.
pub struct District {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub status: &'static str,
    pub href: &'static str,
    pub model: &'static str,
}

pub fn districts() -> &'static [District] {
    &[
        District {
            id: "vault-quarter",
            name: "Vault Quarter",
            summary: "Heist Night. Timed vault rounds inside platform caps.",
            status: "live",
            href: "/games/heist-night",
            model: "building-a.glb",
        },
        District {
            id: "cup-stadium",
            name: "Cup Stadium",
            summary: "Trading Cup. A short public tape. Even rounds settle long.",
            status: "live",
            href: "/games/trading-cup",
            model: "building-e.glb",
        },
        District {
            id: "trading-village",
            name: "Trading Village",
            summary: "Open tape, rolling boards, and agent follow. Prices are simulated.",
            status: "live",
            href: "/village",
            model: "building-b.glb",
        },
        District {
            id: "war-room",
            name: "War Room",
            summary: "A testnet challenge contract. The first valid claim takes the bounty.",
            status: "live",
            href: "/war-room",
            model: "building-c.glb",
        },
        District {
            id: "skin-forge",
            name: "Skin Forge",
            summary: "Cosmetic skins. One NEAR. No effect on the game.",
            status: "live",
            href: "",
            model: "station.glb",
        },
        District {
            id: "oracle-plaza",
            name: "Oracle Plaza",
            summary: "Coming soon.",
            status: "soon",
            href: "",
            model: "building-f.glb",
        },
        District {
            id: "launch-pier",
            name: "Launch Pier",
            summary: "Coming soon.",
            status: "soon",
            href: "",
            model: "building-g.glb",
        },
        District {
            id: "card-club",
            name: "Card Club",
            summary: "Coming soon.",
            status: "soon",
            href: "",
            model: "building-h.glb",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_keep_only_the_fills_inside_them() {
        let tape = demo_tape();
        let day = leaderboard(&tape, DEMO_EPOCH, Window::Hours24);
        assert_eq!(day.len(), 1);
        assert_eq!(day[0].name, "Mason");
        let week = leaderboard(&tape, DEMO_EPOCH, Window::Days7);
        assert_eq!(week.len(), 2);
        let month = leaderboard(&tape, DEMO_EPOCH, Window::Days30);
        assert_eq!(month.len(), 3);
        let all = leaderboard(&tape, DEMO_EPOCH, Window::All);
        assert_eq!(all.len(), 4);
        assert!(all.iter().any(|row| row.name == "Pike"));
    }

    #[test]
    fn follow_rejects_self_and_is_idempotent() {
        let mut book = Vec::new();
        assert_eq!(
            follow(
                &mut book,
                "mason.village.fe2vi.testnet",
                "mason.village.fe2vi.testnet"
            ),
            Err("an agent cannot follow itself")
        );
        follow(
            &mut book,
            "mason.village.fe2vi.testnet",
            "scout.vault.fe2vi.testnet",
        )
        .unwrap();
        follow(
            &mut book,
            "mason.village.fe2vi.testnet",
            "scout.vault.fe2vi.testnet",
        )
        .unwrap();
        assert_eq!(book.len(), 1);
        unfollow(
            &mut book,
            "mason.village.fe2vi.testnet",
            "scout.vault.fe2vi.testnet",
        );
        assert!(book.is_empty());
    }

    #[test]
    fn eight_districts_and_no_brand_names() {
        let names: Vec<&str> = districts().iter().map(|d| d.name).collect();
        assert_eq!(names.len(), 8);
        let banned = ["fomo", "pump", "polymarket", "ethglobal"];
        for district in districts() {
            let blob = format!("{} {}", district.name, district.summary).to_ascii_lowercase();
            for word in banned {
                assert!(!blob.contains(word), "{word} in {}", district.name);
            }
        }
    }
}
