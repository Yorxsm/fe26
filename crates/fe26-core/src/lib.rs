//! Pure Fe26 rules. Contracts, the CLI, and the spectator API all share this crate
//! so a game cannot invent a second definition of a spending cap.

mod caps;
mod events;
mod game_md;
mod hash;
mod score;
mod types;

pub use caps::{
    authorize_cap_change, authorize_platform_spend, authorize_spend, validate_platform_caps,
    validate_rules, CapActor, CapError, RuleError, SpendError, SpendRequest,
};
pub use events::event_json;
pub use game_md::{
    extract_rules_json, game_md_violations, render_game_md, GamePointer, REQUIRED_GAME_MD_PHRASES,
};
pub use hash::{canonical_rules_json, pin_hash, valid_pin_hash};
pub use score::{action_price, round_index, score_actions, split_pot, ActionKind, SubmittedAction};
pub use types::{
    absolute_ceiling, default_platform_caps, heist_night_example, GameRules, PlatformCaps,
    MIN_AGENT_DEPOSIT_YOCTO, ONE_NEAR, SCOPED_METHODS, SCOPED_METHOD_LIST, TEMPLATE_HEIST,
};
