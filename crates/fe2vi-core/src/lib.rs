//! Pure fe2VI rules. Contracts, the CLI, and the spectator API all share this crate
//! so a game cannot invent a second definition of a spending cap.

mod caps;
mod events;
mod game_md;
mod hash;
mod price;
mod score;
mod skin;
mod types;
mod village;

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
    default_skin, unknown_trait, SkinTraits, ACCESSORIES, BODY_STYLES, FACE_STYLES, HAT_STYLES,
    MINT_PRICE_YOCTO, ROLE_OUTFITS, TRAILS,
};
pub use types::{
    absolute_ceiling, default_platform_caps, heist_night_example, trading_cup_example, GameRules,
    PlatformCaps, MIN_AGENT_DEPOSIT_YOCTO, ONE_NEAR, SCOPED_METHODS, SCOPED_METHOD_LIST,
    TEMPLATE_CUP, TEMPLATE_HEIST,
};
pub use village::{
    demo_tape, districts, follow, leaderboard, unfollow, District, Fill, FollowEdge, Side,
    Standing, Window, DEMO_EPOCH,
};
