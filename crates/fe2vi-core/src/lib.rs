//! Pure fe2VI rules. Contracts, the CLI, and the spectator API all share this crate
//! so a game cannot invent a second definition of a spending cap.

mod board;
mod caps;
mod events;
mod game_md;
mod hash;
mod price;
mod score;
mod skin;
mod types;
mod venue;
mod village;

pub use board::{
    agent_roster, award_in_window, challenges, city_roster, countdown_secs, daily_slot,
    event_markdown, event_phase, example_run, heist_leaderboard, heist_live, hosted_events,
    live_events, mock_awards, next_heist, open_slots, public_leaderboard, recent_results,
    room_windows, schedule_markdown, season_slot, upcoming_season_slot, venue_markdown,
    war_room_md, weekly_slot, BoardWindow, Challenge, ChallengeCategory, EventPhase, HeistRow,
    HeistWindow, HostedEvent, PointAward, PointRoom, PublicRow, RoomWindow, RosterEntry, Slot,
    SlotKind, POINT_SABOTAGE, POINT_SCOUT, POINT_STEAL, SEASON_END, SEASON_ID, SEASON_START,
};
pub use caps::{
    authorize_cap_change, authorize_platform_spend, authorize_spend, validate_platform_caps,
    validate_rules, CapActor, CapError, RuleError, SpendError, SpendRequest,
};
pub use events::event_json;
pub use game_md::{
    extract_rules_json, game_md_violations, render_game_md, GamePointer, REQUIRED_GAME_MD_PHRASES,
};
pub use hash::{canonical_rules_json, pin_hash, valid_pin_hash};
pub use price::{select_price, simulated_price_micros, PriceSource, WRAP_NEAR};
pub use score::{
    action_price, round_index, score_actions, split_pot, tape_winner, ActionKind, SubmittedAction,
};
pub use skin::{
    default_skin, earned_progress, earned_trait, unknown_trait, EarnedTrait, SkinTraits,
    BACK_ITEMS, EARNED_TRAITS, EMBLEMS, EYE_STYLES, FACE_ITEMS, HEADWEAR, MINT_PRICE_YOCTO, PAINTS,
    ROLE_OUTFITS, SCREENS, TRAILS,
};
pub use types::{
    absolute_ceiling, default_platform_caps, heist_night_example, trading_cup_example, GameRules,
    PlatformCaps, MIN_AGENT_DEPOSIT_YOCTO, ONE_NEAR, SCOPED_METHODS, SCOPED_METHOD_LIST,
    TEMPLATE_CUP, TEMPLATE_HEIST,
};
pub use venue::{
    default_venue, refuse_mainnet, venue_notes, MockVenue, Quote, Venue, VenueNote,
    HYPERLIQUID_MAINNET_INFO, HYPERLIQUID_TESTNET_INFO,
};
pub use village::{
    demo_tape, demo_tape_at, districts, follow, leaderboard, unfollow, District, Fill, FollowEdge,
    Side, Standing, Window, DEMO_EPOCH,
};
