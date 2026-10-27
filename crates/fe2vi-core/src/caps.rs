use thiserror::Error;

use crate::types::{GameRules, PlatformCaps, TEMPLATE_CUP, TEMPLATE_HEIST};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapActor {
    /// The single platform owner account.
    PlatformOwner,
    /// Any game contract, creator, or agent.
    Game,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapError {
    #[error("game cannot raise platform caps")]
    GameCannotRaise,
    #[error("caps exceed the absolute ceiling")]
    AboveCeiling,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("unknown template")]
    UnknownTemplate,
    #[error("players outside platform cap")]
    Players,
    #[error("duration outside platform cap")]
    Duration,
    #[error("rounds outside platform cap")]
    Rounds,
    #[error("pot outside platform cap")]
    Pot,
    #[error("per-agent cap exceeds platform cap")]
    AgentCap,
    #[error("per-action cap exceeds platform cap")]
    ActionCap,
    #[error("per-action cap exceeds per-agent cap")]
    ActionExceedsAgent,
    #[error("entry exceeds per-agent cap")]
    Entry,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpendError {
    #[error("revoked")]
    Revoked,
    #[error("over cap")]
    OverCap,
    #[error("game caps exceed platform caps")]
    CapsAbovePlatform,
    #[error("amount is zero")]
    ZeroAmount,
}

#[derive(Clone, Copy, Debug)]
pub struct SpendRequest {
    pub amount: u128,
    pub already_spent: u128,
    pub per_action_cap: u128,
    pub per_agent_cap: u128,
    pub platform_per_action: u128,
    pub platform_per_agent: u128,
    pub revoked: bool,
}

pub fn validate_platform_caps(
    proposed: &PlatformCaps,
    ceiling: &PlatformCaps,
) -> Result<(), CapError> {
    let within = proposed.max_players > 0
        && proposed.max_players <= ceiling.max_players
        && proposed.max_duration_seconds > 0
        && proposed.max_duration_seconds <= ceiling.max_duration_seconds
        && proposed.max_round_count > 0
        && proposed.max_round_count <= ceiling.max_round_count
        && proposed.max_pot_yocto > 0
        && proposed.max_pot_yocto <= ceiling.max_pot_yocto
        && proposed.max_per_agent_yocto > 0
        && proposed.max_per_agent_yocto <= ceiling.max_per_agent_yocto
        && proposed.max_per_action_yocto > 0
        && proposed.max_per_action_yocto <= ceiling.max_per_action_yocto
        && proposed.max_per_action_yocto <= proposed.max_per_agent_yocto;
    if within {
        Ok(())
    } else {
        Err(CapError::AboveCeiling)
    }
}

/// Games have no path that returns new caps. Only the platform owner can, and
/// only inside the absolute ceiling.
pub fn authorize_cap_change(
    actor: CapActor,
    proposed: &PlatformCaps,
    ceiling: &PlatformCaps,
) -> Result<PlatformCaps, CapError> {
    match actor {
        CapActor::Game => Err(CapError::GameCannotRaise),
        CapActor::PlatformOwner => {
            validate_platform_caps(proposed, ceiling)?;
            Ok(proposed.clone())
        }
    }
}

pub fn validate_rules(rules: &GameRules, caps: &PlatformCaps) -> Result<(), RuleError> {
    if rules.template_id != TEMPLATE_HEIST && rules.template_id != TEMPLATE_CUP {
        return Err(RuleError::UnknownTemplate);
    }
    if rules.max_players == 0 || rules.max_players > caps.max_players {
        return Err(RuleError::Players);
    }
    if rules.duration_seconds == 0 || rules.duration_seconds > caps.max_duration_seconds {
        return Err(RuleError::Duration);
    }
    if rules.round_count == 0 || rules.round_count > caps.max_round_count {
        return Err(RuleError::Rounds);
    }
    if rules.pot_yocto == 0 || rules.pot_yocto > caps.max_pot_yocto {
        return Err(RuleError::Pot);
    }
    if rules.per_agent_cap_yocto == 0 || rules.per_agent_cap_yocto > caps.max_per_agent_yocto {
        return Err(RuleError::AgentCap);
    }
    if rules.per_action_cap_yocto == 0 || rules.per_action_cap_yocto > caps.max_per_action_yocto {
        return Err(RuleError::ActionCap);
    }
    if rules.per_action_cap_yocto > rules.per_agent_cap_yocto {
        return Err(RuleError::ActionExceedsAgent);
    }
    if rules.entry_yocto > rules.per_agent_cap_yocto {
        return Err(RuleError::Entry);
    }
    Ok(())
}

pub fn authorize_spend(request: SpendRequest) -> Result<u128, SpendError> {
    if request.revoked {
        return Err(SpendError::Revoked);
    }
    if request.per_action_cap > request.platform_per_action
        || request.per_agent_cap > request.platform_per_agent
    {
        return Err(SpendError::CapsAbovePlatform);
    }
    authorize_platform_spend(
        request.amount,
        request.already_spent,
        request.platform_per_action.min(request.per_action_cap),
        request.platform_per_agent.min(request.per_agent_cap),
        false,
    )
}

/// Registry-side check: revocation plus the platform ceilings only.
pub fn authorize_platform_spend(
    amount: u128,
    already_spent: u128,
    per_action_cap: u128,
    per_agent_cap: u128,
    revoked: bool,
) -> Result<u128, SpendError> {
    if revoked {
        return Err(SpendError::Revoked);
    }
    if amount == 0 {
        return Err(SpendError::ZeroAmount);
    }
    if amount > per_action_cap {
        return Err(SpendError::OverCap);
    }
    let next = already_spent
        .checked_add(amount)
        .ok_or(SpendError::OverCap)?;
    if next > per_agent_cap {
        return Err(SpendError::OverCap);
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{absolute_ceiling, default_platform_caps, heist_night_example, ONE_NEAR};

    #[test]
    fn game_cannot_raise_platform_caps() {
        let ceiling = absolute_ceiling();
        let mut raised = default_platform_caps();
        raised.max_per_agent_yocto = 10 * ONE_NEAR;
        let err = authorize_cap_change(CapActor::Game, &raised, &ceiling).unwrap_err();
        assert_eq!(err, CapError::GameCannotRaise);

        let owner = authorize_cap_change(CapActor::PlatformOwner, &raised, &ceiling).unwrap();
        assert_eq!(owner.max_per_agent_yocto, 10 * ONE_NEAR);

        let mut absurd = ceiling.clone();
        absurd.max_pot_yocto = ceiling.max_pot_yocto + 1;
        assert_eq!(
            authorize_cap_change(CapActor::PlatformOwner, &absurd, &ceiling).unwrap_err(),
            CapError::AboveCeiling
        );

        let caps = default_platform_caps();
        let mut rules = heist_night_example();
        rules.per_agent_cap_yocto = caps.max_per_agent_yocto + 1;
        assert_eq!(
            validate_rules(&rules, &caps).unwrap_err(),
            RuleError::AgentCap
        );
        rules.per_agent_cap_yocto = caps.max_per_agent_yocto;
        rules.template_id = "auction-night".to_string();
        assert_eq!(
            validate_rules(&rules, &caps).unwrap_err(),
            RuleError::UnknownTemplate
        );
    }

    #[test]
    fn spend_over_cap_rejected() {
        let caps = default_platform_caps();
        let err = authorize_spend(SpendRequest {
            amount: caps.max_per_action_yocto + 1,
            already_spent: 0,
            per_action_cap: caps.max_per_action_yocto,
            per_agent_cap: caps.max_per_agent_yocto,
            platform_per_action: caps.max_per_action_yocto,
            platform_per_agent: caps.max_per_agent_yocto,
            revoked: false,
        })
        .unwrap_err();
        assert_eq!(err, SpendError::OverCap);

        let err = authorize_spend(SpendRequest {
            amount: 1,
            already_spent: caps.max_per_agent_yocto,
            per_action_cap: caps.max_per_action_yocto,
            per_agent_cap: caps.max_per_agent_yocto,
            platform_per_action: caps.max_per_action_yocto,
            platform_per_agent: caps.max_per_agent_yocto,
            revoked: false,
        })
        .unwrap_err();
        assert_eq!(err, SpendError::OverCap);
    }

    #[test]
    fn revoked_agent_cannot_act() {
        let caps = default_platform_caps();
        let err = authorize_spend(SpendRequest {
            amount: 1,
            already_spent: 0,
            per_action_cap: caps.max_per_action_yocto,
            per_agent_cap: caps.max_per_agent_yocto,
            platform_per_action: caps.max_per_action_yocto,
            platform_per_agent: caps.max_per_agent_yocto,
            revoked: true,
        })
        .unwrap_err();
        assert_eq!(err, SpendError::Revoked);
    }

    #[test]
    fn spend_rejects_game_caps_above_platform() {
        let caps = default_platform_caps();
        let err = authorize_spend(SpendRequest {
            amount: 1,
            already_spent: 0,
            per_action_cap: caps.max_per_action_yocto + 1,
            per_agent_cap: caps.max_per_agent_yocto + 1,
            platform_per_action: caps.max_per_action_yocto,
            platform_per_agent: caps.max_per_agent_yocto,
            revoked: false,
        })
        .unwrap_err();
        assert_eq!(err, SpendError::CapsAbovePlatform);
    }
}
