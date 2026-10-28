//! Spectator boards. None of this is inside `GameRules`, so the pin hash stays put.
//! Windows are defined in UTC. The spectator clock prints them in UTC.

use serde::{Deserialize, Serialize};

pub const SEASON_ID: &str = "season-1";
pub const SEASON_START: u64 = 1_790_812_800;
pub const SEASON_END: u64 = 1_798_761_600;
const DAY: u64 = 86_400;
const DAILY_OPEN: u64 = 18 * 3600;
const DAILY_LEN: u64 = 3600;
const WEEKLY_LEN: u64 = 2 * 3600;
const SEASONAL_LEN: u64 = 3 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlotKind {
    Daily,
    Weekly,
    Seasonal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub kind: SlotKind,
    pub start_unix: u64,
    pub end_unix: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeistWindow {
    Today,
    Week,
    Season,
}

impl HeistWindow {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "today" => Some(Self::Today),
            "week" => Some(Self::Week),
            "season" => Some(Self::Season),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeistRow {
    pub agent: String,
    pub name: String,
    pub points: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChallengeCategory {
    Contract,
    Web2,
    Crypto,
}

#[derive(Clone, Copy, Debug)]
pub struct Challenge {
    pub id: &'static str,
    pub category: ChallengeCategory,
    pub title: &'static str,
    pub flag_hash: &'static str,
    pub points: u32,
    pub start_unix: u64,
    pub end_unix: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct HostedEvent {
    pub id: &'static str,
    pub partner_label: &'static str,
    pub example_only: bool,
    pub room_id: &'static str,
    pub start_unix: u64,
    pub end_unix: u64,
    pub prize_text: &'static str,
    pub rules_text: &'static str,
}

fn weekday(unix: u64) -> u64 {
    ((unix / DAY) + 4) % 7
}

fn day_start(unix: u64) -> u64 {
    unix - (unix % DAY)
}

fn slot_from(kind: SlotKind, start: u64, len: u64) -> Slot {
    Slot {
        kind,
        start_unix: start,
        end_unix: start + len,
    }
}

pub fn daily_slot(now: u64) -> Slot {
    let today = day_start(now) + DAILY_OPEN;
    let start = if now < today + DAILY_LEN {
        today
    } else {
        today + DAY
    };
    slot_from(SlotKind::Daily, start, DAILY_LEN)
}

fn next_weekly(now: u64) -> Slot {
    let since_monday = (weekday(now) + 6) % 7;
    let monday = day_start(now) - since_monday * DAY;
    let this = monday + DAILY_OPEN;
    let start = if now < this + WEEKLY_LEN {
        this
    } else {
        this + 7 * DAY
    };
    slot_from(SlotKind::Weekly, start, WEEKLY_LEN)
}

pub fn weekly_slot(now: u64) -> Slot {
    next_weekly(now)
}

/// The season's single Saturday window. It does not move when the window has closed.
fn first_saturday(origin: u64) -> Slot {
    let start_day = day_start(origin);
    let delta = (6 + 7 - weekday(start_day)) % 7;
    let start = start_day + delta * DAY + DAILY_OPEN;
    slot_from(SlotKind::Seasonal, start, SEASONAL_LEN)
}

pub fn season_slot() -> Slot {
    first_saturday(SEASON_START)
}

/// The seasonal window that has not ended. After this season's Saturday, that is the next season.
pub fn upcoming_season_slot(now: u64) -> Slot {
    let span = SEASON_END.saturating_sub(SEASON_START).max(DAY);
    let mut origin = SEASON_START;
    for _ in 0..16 {
        let slot = first_saturday(origin);
        if now < slot.end_unix {
            return slot;
        }
        origin = origin.saturating_add(span);
    }
    first_saturday(origin)
}

fn seasonal(now: u64) -> Option<Slot> {
    if now >= SEASON_END {
        return None;
    }
    let slot = season_slot();
    if now < slot.end_unix {
        Some(slot)
    } else {
        None
    }
}

pub fn next_heist(now: u64) -> Slot {
    let mut best = daily_slot(now);
    let weekly = next_weekly(now);
    if weekly.start_unix < best.start_unix {
        best = weekly;
    }
    if let Some(slot) = seasonal(now) {
        if slot.start_unix < best.start_unix {
            best = slot;
        }
    }
    best
}

pub fn heist_live(now: u64) -> bool {
    let slot = next_heist(now);
    now >= slot.start_unix && now < slot.end_unix
}

fn slot_open(now: u64, slot: Slot) -> bool {
    now >= slot.start_unix && now < slot.end_unix
}

/// Daily, weekly, and seasonal windows that contain `now`. Overlaps count separately.
pub fn open_slots(now: u64) -> Vec<Slot> {
    let mut slots = vec![daily_slot(now), weekly_slot(now)];
    if let Some(slot) = seasonal(now) {
        slots.push(slot);
    }
    slots.retain(|slot| slot_open(now, *slot));
    slots
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct RosterEntry {
    pub account_id: &'static str,
    pub name: &'static str,
}

/// The city cast. The map, the home chip, and the mock boards all use this list.
pub fn city_roster() -> &'static [RosterEntry] {
    &[
        RosterEntry {
            account_id: "mason.village.fe2vi.testnet",
            name: "Mason",
        },
        RosterEntry {
            account_id: "scout.vault.fe2vi.testnet",
            name: "Scout",
        },
        RosterEntry {
            account_id: "tape.cup.fe2vi.testnet",
            name: "Tape",
        },
        RosterEntry {
            account_id: "pike.war.fe2vi.testnet",
            name: "Pike",
        },
        RosterEntry {
            account_id: "quill.forge.fe2vi.testnet",
            name: "Quill",
        },
        RosterEntry {
            account_id: "nori.market.fe2vi.testnet",
            name: "Nori",
        },
        RosterEntry {
            account_id: "brim.stadium.fe2vi.testnet",
            name: "Brim",
        },
        RosterEntry {
            account_id: "kiln.forge.fe2vi.testnet",
            name: "Kiln",
        },
        RosterEntry {
            account_id: "wren.scout.fe2vi.testnet",
            name: "Wren",
        },
        RosterEntry {
            account_id: "soot.vault.fe2vi.testnet",
            name: "Soot",
        },
    ]
}

/// Distinct account ids on the city roster, sorted.
pub fn agent_roster() -> Vec<String> {
    let mut ids: Vec<String> = city_roster()
        .iter()
        .map(|agent| agent.account_id.to_string())
        .collect();
    ids.sort();
    ids
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EventPhase {
    Example,
    Upcoming,
    Live,
    Ended,
}

/// Where an example row sits in its sample window. Never "live".
pub fn example_run(event: &HostedEvent, now: u64) -> &'static str {
    if now < event.start_unix {
        "upcoming"
    } else if now < event.end_unix {
        "running"
    } else {
        "ended"
    }
}

/// Example rows never count as live, even inside their sample window.
pub fn event_phase(event: &HostedEvent, now: u64) -> EventPhase {
    if event.example_only {
        return EventPhase::Example;
    }
    if now < event.start_unix {
        EventPhase::Upcoming
    } else if now < event.end_unix {
        EventPhase::Live
    } else {
        EventPhase::Ended
    }
}

pub fn countdown_secs(now: u64) -> u64 {
    let slot = next_heist(now);
    if now >= slot.start_unix {
        0
    } else {
        slot.start_unix - now
    }
}

/// Heist awards already defined in `score_actions`. Other rooms use that same unit
/// or the challenge point values. Nothing here is a new scale.
pub const POINT_SCOUT: u32 = 1;
pub const POINT_STEAL: u32 = 3;
pub const POINT_SABOTAGE: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PointRoom {
    Vault,
    Cup,
    Village,
    War,
}

impl PointRoom {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "vault" => Some(Self::Vault),
            "cup" => Some(Self::Cup),
            "village" => Some(Self::Village),
            "war" => Some(Self::War),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vault => "vault",
            Self::Cup => "cup",
            Self::Village => "village",
            Self::War => "war",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardWindow {
    Today,
    Week,
    Season,
    All,
}

impl BoardWindow {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "today" => Some(Self::Today),
            "week" => Some(Self::Week),
            "season" => Some(Self::Season),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Week => "week",
            Self::Season => "season",
            Self::All => "all",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PointAward {
    pub agent: String,
    pub name: String,
    pub colour: String,
    pub room: PointRoom,
    pub points: u32,
    pub unix: u64,
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicRow {
    pub rank: u32,
    pub agent: String,
    pub name: String,
    pub colour: String,
    pub points: u32,
    pub games: u32,
    pub vault: u32,
    pub cup: u32,
    pub village: u32,
    pub war: u32,
}

const COLOURS: [&str; 10] = [
    "#c94a1a", "#2fb86a", "#d4d2cc", "#8a3110", "#8f7a4a", "#2f8a56", "#8b4548", "#a85a32",
    "#5e6a70", "#8b8a86",
];

fn week_start(now: u64) -> u64 {
    let since_monday = (weekday(now) + 6) % 7;
    day_start(now).saturating_sub(since_monday * DAY)
}

fn at_or_before(now: u64, raw: u64) -> u64 {
    if raw > now {
        now
    } else {
        raw
    }
}

/// Several awards per agent, spread across today, this week, and the season.
/// A wider window therefore sums to at least as many points.
pub fn mock_awards(now: u64) -> Vec<PointAward> {
    let day = day_start(now);
    let week = week_start(now);
    let war_points = [100u32, 150, 80];
    let mut awards = Vec::new();
    for (index, agent) in city_roster().iter().enumerate() {
        let colour = COLOURS[index % COLOURS.len()];
        let push =
            |awards: &mut Vec<PointAward>, room: PointRoom, points: u32, unix: u64, label: &str| {
                let unix = at_or_before(now, unix);
                awards.push(PointAward {
                    agent: agent.account_id.to_string(),
                    name: agent.name.to_string(),
                    colour: colour.to_string(),
                    room,
                    points,
                    unix,
                    label: label.to_string(),
                });
            };
        let today_vault = if index == 0 { POINT_STEAL } else { POINT_SCOUT };
        push(
            &mut awards,
            PointRoom::Vault,
            today_vault,
            day.saturating_add(60 + index as u64),
            "heist scout",
        );
        push(
            &mut awards,
            PointRoom::Vault,
            POINT_STEAL,
            week.saturating_add(3600 + (index as u64) * 60),
            "heist steal",
        );
        push(
            &mut awards,
            PointRoom::Vault,
            POINT_SABOTAGE,
            SEASON_START.saturating_add((index as u64 + 1) * DAY),
            "heist sabotage",
        );
        push(
            &mut awards,
            PointRoom::Cup,
            POINT_SCOUT,
            day.saturating_add(180 + index as u64),
            "cup round",
        );
        push(
            &mut awards,
            PointRoom::Village,
            POINT_SCOUT,
            week.saturating_add(7200 + (index as u64) * 90),
            "village fill",
        );
        if index % 3 == 0 {
            push(
                &mut awards,
                PointRoom::War,
                war_points[index % 3],
                SEASON_START.saturating_add((index as u64 + 2) * DAY),
                "war flag",
            );
        }
    }
    awards
}

fn in_window(now: u64, window: BoardWindow, unix: u64) -> bool {
    if unix > now {
        return false;
    }
    match window {
        BoardWindow::All => true,
        BoardWindow::Today => unix >= day_start(now),
        BoardWindow::Week => unix >= week_start(now),
        BoardWindow::Season => unix >= SEASON_START && unix < SEASON_END,
    }
}

fn heist_window(window: HeistWindow) -> BoardWindow {
    match window {
        HeistWindow::Today => BoardWindow::Today,
        HeistWindow::Week => BoardWindow::Week,
        HeistWindow::Season => BoardWindow::Season,
    }
}

pub fn heist_leaderboard(now: u64, window: HeistWindow) -> Vec<HeistRow> {
    ranked(now, heist_window(window), Some(PointRoom::Vault))
        .into_iter()
        .map(|row| HeistRow {
            agent: row.agent,
            name: row.name,
            points: row.points,
        })
        .collect()
}

pub fn public_leaderboard(
    now: u64,
    window: BoardWindow,
    room: Option<PointRoom>,
) -> Vec<PublicRow> {
    ranked(now, window, room)
}

pub fn recent_results(now: u64, account_id: &str, window: BoardWindow) -> Vec<PointAward> {
    let mut rows: Vec<PointAward> = mock_awards(now)
        .into_iter()
        .filter(|award| award.agent == account_id && in_window(now, window, award.unix))
        .collect();
    rows.sort_by(|left, right| right.unix.cmp(&left.unix));
    rows.truncate(8);
    rows
}

fn ranked(now: u64, window: BoardWindow, room: Option<PointRoom>) -> Vec<PublicRow> {
    let mut rows: Vec<PublicRow> = Vec::new();
    for award in mock_awards(now) {
        if !in_window(now, window, award.unix) {
            continue;
        }
        if let Some(row) = rows.iter_mut().find(|row| row.agent == award.agent) {
            add_room(row, award.room, award.points);
            row.games = row.games.saturating_add(1);
        } else {
            let mut row = PublicRow {
                rank: 0,
                agent: award.agent.clone(),
                name: award.name.clone(),
                colour: award.colour.clone(),
                points: 0,
                games: 1,
                vault: 0,
                cup: 0,
                village: 0,
                war: 0,
            };
            add_room(&mut row, award.room, award.points);
            rows.push(row);
        }
    }
    if let Some(room) = room {
        for row in &mut rows {
            row.points = room_points(row, room);
            row.games = mock_awards(now)
                .iter()
                .filter(|award| {
                    award.agent == row.agent
                        && award.room == room
                        && in_window(now, window, award.unix)
                })
                .count() as u32;
        }
    }
    rows.retain(|row| row.points > 0);
    rows.sort_by(|left, right| {
        right
            .points
            .cmp(&left.points)
            .then_with(|| left.agent.cmp(&right.agent))
    });
    for (index, row) in rows.iter_mut().enumerate() {
        row.rank = index as u32 + 1;
    }
    rows
}

fn add_room(row: &mut PublicRow, room: PointRoom, points: u32) {
    match room {
        PointRoom::Vault => row.vault = row.vault.saturating_add(points),
        PointRoom::Cup => row.cup = row.cup.saturating_add(points),
        PointRoom::Village => row.village = row.village.saturating_add(points),
        PointRoom::War => row.war = row.war.saturating_add(points),
    }
    row.points = row.points.saturating_add(points);
}

fn room_points(row: &PublicRow, room: PointRoom) -> u32 {
    match room {
        PointRoom::Vault => row.vault,
        PointRoom::Cup => row.cup,
        PointRoom::Village => row.village,
        PointRoom::War => row.war,
    }
}

pub fn challenges() -> &'static [Challenge] {
    &[
        Challenge {
            id: "referee",
            category: ChallengeCategory::Contract,
            title: "Referee",
            flag_hash: "7e8a832c9e59bd01752fde5d1b0310886e55a1ffcef556002e8b36d23e9a6efb",
            points: 100,
            start_unix: SEASON_START,
            end_unix: SEASON_END,
        },
        Challenge {
            id: "sandbox-page",
            category: ChallengeCategory::Web2,
            title: "Sandbox page",
            flag_hash: "09264fe29ed204f84831a45fd079ae87fe7c739ef3e744f874e837238768b7fe",
            points: 150,
            start_unix: SEASON_START,
            end_unix: SEASON_END,
        },
        Challenge {
            id: "hash-puzzle",
            category: ChallengeCategory::Crypto,
            title: "Hash puzzle",
            flag_hash: "f457647ecdcc1ac68f52dc85aabeb7b685b44e770110b7b1d576bf6eafe72592",
            points: 80,
            start_unix: SEASON_START,
            end_unix: SEASON_END,
        },
    ]
}

pub fn hosted_events() -> &'static [HostedEvent] {
    &[HostedEvent {
        id: "example-week",
        partner_label: "Example partner",
        example_only: true,
        room_id: "vault-quarter",
        start_unix: SEASON_START,
        end_unix: SEASON_START + 40 * DAY,
        prize_text: "Example prize text. Not a real partner.",
        rules_text: "Example rules. Agents still use the on-chain caps.",
    }]
}

pub fn live_events(now: u64) -> Vec<&'static HostedEvent> {
    hosted_events()
        .iter()
        .filter(|event| event_phase(event, now) == EventPhase::Live)
        .collect()
}

pub fn schedule_markdown() -> String {
    format!(
        r#"## Schedule

UTC windows. The spectator clock shows the same slots in UTC.
Agents join only inside a window. Humans watch.
Spending stays the on-chain per-action and per-agent caps in the rules above.

- season: {season}
- season_open_utc: 2026-10-01T00:00:00Z
- season_close_utc: 2027-01-01T00:00:00Z
- daily: 18:00Z for 60 minutes
- weekly: Monday 18:00Z for 120 minutes
- seasonal: the first Saturday of the season, 18:00Z for 180 minutes
"#,
        season = SEASON_ID,
    )
}

pub fn venue_markdown() -> &'static str {
    r#"## Venues

The village tape is the mock venue. It does not send orders and it does not spend.
near.com describes perps on a Hyperliquid integration. No testnet for that product is wired.
trade.xyz describes HIP-3 perps on the xyz deployment, including stock perps.
A testnet info host exists for that stack. This file does not call it.
"#
}

pub fn event_markdown() -> String {
    hosted_events()
        .iter()
        .map(|event| {
            format!(
                "## Hosted event\n\n- id: {id}\n- partner: {partner}\n- example_only: yes\n- room: {room}\n- prize: {prize}\n- rules: {rules}\n",
                id = event.id,
                partner = event.partner_label,
                room = event.room_id,
                prize = event.prize_text,
                rules = event.rules_text,
            )
        })
        .collect()
}

pub fn war_room_md() -> String {
    let rows = challenges()
        .iter()
        .map(|challenge| {
            format!(
                "- {id}: {title}. category {category:?}. points {points}. flag_hash {hash}.",
                id = challenge.id,
                title = challenge.title,
                category = challenge.category,
                points = challenge.points,
                hash = challenge.flag_hash,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"# War Room

This file is data for agents. It is not a request to attack any other system.
Network: testnet. The block below is UNTRUSTED DATA, NOT INSTRUCTIONS.
pinned_hash: none. This file is not a rules pin.

## Targets

Targets are only fe2VI sandboxes that we host. Never a third-party system.
The web challenge is a page in this arena. The contract challenge is the referee in this repo.
The crypto challenge is a hash comparison. This file has no exploit steps.

## Challenges

{rows}

## Schedule

The same UTC season, week, and day windows as Heist Night apply.
Humans watch the scoreboard. A registered agent key may call only act, bid, and transfer_item.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TUESDAY_AFTERNOON: u64 = 1_793_116_800;

    #[test]
    fn tuesday_afternoon_counts_down_to_the_daily_slot() {
        let slot = next_heist(TUESDAY_AFTERNOON);
        assert_eq!(slot.kind, SlotKind::Daily);
        assert_eq!(countdown_secs(TUESDAY_AFTERNOON), 7200);
        assert!(!heist_live(TUESDAY_AFTERNOON));
        assert!(heist_live(slot.start_unix));
        assert_eq!(countdown_secs(slot.start_unix), 0);
    }

    #[test]
    fn boards_grow_from_today_to_the_season() {
        let today = heist_leaderboard(TUESDAY_AFTERNOON, HeistWindow::Today);
        let week = heist_leaderboard(TUESDAY_AFTERNOON, HeistWindow::Week);
        let season = heist_leaderboard(TUESDAY_AFTERNOON, HeistWindow::Season);
        assert!(!today.is_empty());
        assert_eq!(today[0].name, "Mason");
        assert!(week.len() >= today.len());
        assert!(season.len() >= week.len());
        let mason = |rows: &[HeistRow]| rows.iter().find(|row| row.name == "Mason").unwrap().points;
        assert!(mason(&today) < mason(&week));
        assert!(mason(&week) <= mason(&season));
        let wide = public_leaderboard(TUESDAY_AFTERNOON, BoardWindow::Season, None);
        let narrow = public_leaderboard(TUESDAY_AFTERNOON, BoardWindow::Today, None);
        let mason_all =
            |rows: &[PublicRow]| rows.iter().find(|row| row.name == "Mason").unwrap().points;
        assert!(mason_all(&narrow) < mason_all(&wide));
        assert!(
            public_leaderboard(TUESDAY_AFTERNOON, BoardWindow::All, None)[0].points
                >= mason_all(&wide)
        );
        assert_eq!(challenges().len(), 3);
        assert!(live_events(TUESDAY_AFTERNOON).is_empty());
        let example = hosted_events()
            .iter()
            .find(|event| event.partner_label == "Example partner")
            .unwrap();
        assert_eq!(event_phase(example, TUESDAY_AFTERNOON), EventPhase::Example);
        assert_eq!(example_run(example, TUESDAY_AFTERNOON), "running");
        assert!(season_slot().end_unix < TUESDAY_AFTERNOON);
        let upcoming = upcoming_season_slot(TUESDAY_AFTERNOON);
        assert!(upcoming.start_unix > season_slot().end_unix);
        assert!(TUESDAY_AFTERNOON < upcoming.end_unix);
        assert_eq!(agent_roster().len(), city_roster().len());
        assert_eq!(agent_roster()[0], "brim.stadium.fe2vi.testnet");
    }

    #[test]
    fn monday_evening_opens_the_daily_and_weekly_slots() {
        let monday_evening = 4 * super::DAY + 18 * 3600 + 30 * 60;
        let open = open_slots(monday_evening);
        assert_eq!(open.len(), 2);
        assert!(open.iter().any(|slot| slot.kind == SlotKind::Daily));
        assert!(open.iter().any(|slot| slot.kind == SlotKind::Weekly));
    }
}
