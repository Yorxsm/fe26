use serde::{Deserialize, Serialize};

/// 1 NEAR, in yoctoNEAR.
pub const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;

/// Balance forwarded to a new agent subaccount so its function-call key can pay gas.
pub const MIN_AGENT_DEPOSIT_YOCTO: u128 = 100_000_000_000_000_000_000_000;

/// The only template this foundation will instantiate.
pub const TEMPLATE_HEIST: &str = "heist-night";

/// Methods a registered agent function-call key is allowed to invoke.
/// The NEAR protocol enforces this list on the key. Empty would mean "all methods",
/// so this list is never empty.
pub const SCOPED_METHODS: [&str; 3] = ["act", "bid", "transfer_item"];

/// Comma-separated form required by `Promise::add_access_key_allowance`.
pub const SCOPED_METHOD_LIST: &str = "act,bid,transfer_item";

/// Platform-wide ceilings. A game stores its own limits, which must be less than
/// or equal to these. Nothing in a game contract can raise them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct PlatformCaps {
    pub max_players: u32,
    pub max_duration_seconds: u64,
    pub max_round_count: u32,
    #[serde(with = "crate::hash::yocto_as_str")]
    pub max_pot_yocto: u128,
    #[serde(with = "crate::hash::yocto_as_str")]
    pub max_per_agent_yocto: u128,
    #[serde(with = "crate::hash::yocto_as_str")]
    pub max_per_action_yocto: u128,
}

/// Structured rules for one game. There is no free-text rule field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct GameRules {
    pub template_id: String,
    pub max_players: u32,
    pub duration_seconds: u64,
    pub round_count: u32,
    #[serde(with = "crate::hash::yocto_as_str")]
    pub pot_yocto: u128,
    #[serde(with = "crate::hash::yocto_as_str")]
    pub per_agent_cap_yocto: u128,
    #[serde(with = "crate::hash::yocto_as_str")]
    pub per_action_cap_yocto: u128,
    #[serde(with = "crate::hash::yocto_as_str")]
    pub entry_yocto: u128,
}

/// Compiled-in absolute maximum. Even the platform owner cannot exceed this.
pub fn absolute_ceiling() -> PlatformCaps {
    PlatformCaps {
        max_players: 32,
        max_duration_seconds: 7 * 24 * 60 * 60,
        max_round_count: 16,
        max_pot_yocto: 1_000 * ONE_NEAR,
        max_per_agent_yocto: 20 * ONE_NEAR,
        max_per_action_yocto: 5 * ONE_NEAR,
    }
}

/// Caps installed at init. The owner may lower them, or raise them only up to
/// [`absolute_ceiling`]. A game cannot change them at all.
pub fn default_platform_caps() -> PlatformCaps {
    PlatformCaps {
        max_players: 8,
        max_duration_seconds: 24 * 60 * 60,
        max_round_count: 8,
        max_pot_yocto: 50 * ONE_NEAR,
        max_per_agent_yocto: 5 * ONE_NEAR,
        max_per_action_yocto: ONE_NEAR,
    }
}

/// The checked-in Heist Night example. Every field is within the default caps.
pub fn heist_night_example() -> GameRules {
    GameRules {
        template_id: TEMPLATE_HEIST.to_string(),
        max_players: 6,
        duration_seconds: 3_600,
        round_count: 4,
        pot_yocto: 10 * ONE_NEAR,
        per_agent_cap_yocto: 2 * ONE_NEAR,
        per_action_cap_yocto: ONE_NEAR / 2,
        entry_yocto: 0,
    }
}
