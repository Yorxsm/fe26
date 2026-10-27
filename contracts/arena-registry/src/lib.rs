//! Agent registry for Fe26.
//!
//! Registering an agent creates a subaccount of this contract and adds exactly
//! one function-call access key. The key's receiver is the heist contract and
//! its method list is `act`, `bid`, and `transfer_item`. There is no full-access
//! key on that subaccount. Platform caps live here. Games cannot write them.

use borsh::{BorshDeserialize, BorshSerialize};
use fe26_core::{
    absolute_ceiling, authorize_cap_change, authorize_platform_spend, default_platform_caps,
    event_json, CapActor, PlatformCaps, MIN_AGENT_DEPOSIT_YOCTO, SCOPED_METHODS,
    SCOPED_METHOD_LIST,
};
use near_sdk::json_types::U128;
use near_sdk::serde::Serialize;
use near_sdk::store::LookupMap;
use near_sdk::{
    env, near, require, AccountId, Allowance, Gas, NearToken, PanicOnDefault, Promise, PublicKey,
};

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct AgentRecord {
    pub registrant: AccountId,
    pub public_key: String,
    pub revoked: bool,
    pub allowed_methods: Vec<String>,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AgentView {
    pub account_id: AccountId,
    pub registrant: AccountId,
    pub public_key: String,
    pub revoked: bool,
    pub allowed_methods: Vec<String>,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct ArenaRegistry {
    owner: AccountId,
    heist: AccountId,
    factory: AccountId,
    caps: PlatformCaps,
    agents: LookupMap<AccountId, AgentRecord>,
}

#[near]
impl ArenaRegistry {
    #[init]
    pub fn new(owner: AccountId, heist: AccountId, factory: AccountId) -> Self {
        Self {
            owner,
            heist,
            factory,
            caps: default_platform_caps(),
            agents: LookupMap::new(b"a"),
        }
    }

    /// Permissionless. The caller pays the deposit and becomes the registrant,
    /// who can later revoke this agent. The subaccount receives only a
    /// function-call key.
    #[payable]
    pub fn register_agent(&mut self, name: String, public_key: PublicKey) -> AccountId {
        require!(
            valid_agent_name(&name),
            "agent name must be 2-32 lowercase characters"
        );
        let deposit = env::attached_deposit().as_yoctonear();
        require!(
            deposit >= MIN_AGENT_DEPOSIT_YOCTO,
            "deposit below the minimum agent balance"
        );
        let subaccount: AccountId = format!("{name}.{}", env::current_account_id())
            .parse()
            .unwrap_or_else(|_| env::panic_str("agent subaccount id is invalid"));
        require!(
            self.agents.get(&subaccount).is_none(),
            "agent already registered"
        );

        let allowed: Vec<String> = SCOPED_METHODS.iter().map(|m| (*m).to_string()).collect();
        let record = AgentRecord {
            registrant: env::predecessor_account_id(),
            public_key: String::from(&public_key),
            revoked: false,
            allowed_methods: allowed.clone(),
        };
        self.agents.insert(subaccount.clone(), record);

        let allowance = Allowance::limited(NearToken::from_yoctonear(deposit))
            .unwrap_or_else(|| env::panic_str("deposit must be non-zero"));
        Promise::new(subaccount.clone())
            .create_account()
            .transfer(NearToken::from_yoctonear(deposit))
            .add_access_key_allowance(
                public_key,
                allowance,
                self.heist.clone(),
                SCOPED_METHOD_LIST.to_string(),
            );

        let heist = self.heist.clone();
        Promise::new(heist).function_call(
            "import_agent".to_string(),
            near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
                "agent": subaccount,
                "revoked": false,
            }))
            .unwrap_or_else(|_| env::panic_str("import args")),
            NearToken::from_yoctonear(0),
            Gas::from_tgas(30),
        );

        env::log_str(&event_json(
            "agent_registered",
            near_sdk::serde_json::json!({
                "agent": subaccount,
                "allowed_methods": allowed,
                "registrant": env::predecessor_account_id(),
            }),
        ));
        subaccount
    }

    /// Owner or the original registrant. A scoped agent key cannot call this:
    /// the key's receiver is the heist contract, not the registry.
    pub fn revoke(&mut self, agent: AccountId) {
        let caller = env::predecessor_account_id();
        let mut record = self
            .agents
            .get(&agent)
            .cloned()
            .unwrap_or_else(|| env::panic_str("agent not registered"));
        require!(
            caller == self.owner || caller == record.registrant,
            "only the owner or registrant can revoke"
        );
        require!(!record.revoked, "already revoked");
        record.revoked = true;
        self.agents.insert(agent.clone(), record);

        Promise::new(self.heist.clone()).function_call(
            "import_agent".to_string(),
            near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
                "agent": agent,
                "revoked": true,
            }))
            .unwrap_or_else(|_| env::panic_str("import args")),
            NearToken::from_yoctonear(0),
            Gas::from_tgas(30),
        );
        env::log_str(&event_json(
            "agent_revoked",
            near_sdk::serde_json::json!({ "agent": agent }),
        ));
    }

    /// Called by the heist contract before it commits a spend. Panics when the
    /// agent is revoked or the amount breaks a platform cap.
    pub fn check_spend(&mut self, agent: AccountId, amount: U128, already_spent: U128) -> bool {
        let revoked = self
            .agents
            .get(&agent)
            .map(|record| record.revoked)
            .unwrap_or(true);
        authorize_platform_spend(
            amount.0,
            already_spent.0,
            self.caps.max_per_action_yocto,
            self.caps.max_per_agent_yocto,
            revoked,
        )
        .unwrap_or_else(|err| env::panic_str(&err.to_string()));
        true
    }

    /// The only writer of platform caps. A game predecessor hits the require
    /// and cannot raise them. The owner cannot pass the absolute ceiling.
    pub fn set_platform_caps(&mut self, caps: PlatformCaps) {
        require!(
            env::predecessor_account_id() == self.owner,
            "game cannot raise platform caps"
        );
        self.caps = authorize_cap_change(CapActor::PlatformOwner, &caps, &absolute_ceiling())
            .unwrap_or_else(|err| env::panic_str(&err.to_string()));
        let args =
            near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({ "caps": self.caps }))
                .unwrap_or_else(|_| env::panic_str("caps args"));
        Promise::new(self.factory.clone()).function_call(
            "apply_platform_caps".to_string(),
            args.clone(),
            NearToken::from_yoctonear(0),
            Gas::from_tgas(20),
        );
        Promise::new(self.heist.clone()).function_call(
            "apply_platform_caps".to_string(),
            args,
            NearToken::from_yoctonear(0),
            Gas::from_tgas(20),
        );
        env::log_str(&event_json(
            "caps_updated",
            near_sdk::serde_json::to_value(&self.caps).unwrap_or_default(),
        ));
    }

    pub fn get_platform_caps(&self) -> PlatformCaps {
        self.caps.clone()
    }

    pub fn get_agent(&self, agent: AccountId) -> Option<AgentView> {
        self.agents.get(&agent).map(|record| AgentView {
            account_id: agent,
            registrant: record.registrant.clone(),
            public_key: record.public_key.clone(),
            revoked: record.revoked,
            allowed_methods: record.allowed_methods.clone(),
        })
    }

    pub fn is_revoked(&self, agent: AccountId) -> bool {
        self.agents
            .get(&agent)
            .map(|record| record.revoked)
            .unwrap_or(false)
    }

    /// Protocol-shaped helper: a function-call key permits a method only when
    /// that method was written onto the key. Unknown agents permit nothing.
    pub fn key_permits(&self, agent: AccountId, method: String) -> bool {
        self.agents
            .get(&agent)
            .map(|record| {
                record
                    .allowed_methods
                    .iter()
                    .any(|allowed| allowed == &method)
            })
            .unwrap_or(false)
    }

    pub fn allowed_methods(&self) -> Vec<String> {
        SCOPED_METHODS
            .iter()
            .map(|method| (*method).to_string())
            .collect()
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }
}

fn valid_agent_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (2..=32).contains(&bytes.len())
        && !name.starts_with('-')
        && !name.ends_with('-')
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{get_created_receipts, get_logs, VMContextBuilder};
    use near_sdk::{testing_env, CurveType, NearToken};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn use_pred(predecessor: &str, deposit: u128) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("registry.fe26.testnet".parse().unwrap())
            .predecessor_account_id(predecessor.parse().unwrap())
            .signer_account_id(predecessor.parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .account_balance(NearToken::from_near(10_000))
            .prepaid_gas(Gas::from_tgas(300));
        testing_env!(builder.build());
    }

    fn test_key() -> PublicKey {
        PublicKey::from_parts(CurveType::ED25519, vec![9u8; 32]).unwrap()
    }

    fn setup() -> ArenaRegistry {
        use_pred("owner.fe26.testnet", 0);
        ArenaRegistry::new(
            "owner.fe26.testnet".parse().unwrap(),
            "heist.fe26.testnet".parse().unwrap(),
            "factory.fe26.testnet".parse().unwrap(),
        )
    }

    fn panic_text(err: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = err.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = err.downcast_ref::<&str>() {
            (*message).to_string()
        } else {
            String::new()
        }
    }

    #[test]
    fn game_cannot_raise_platform_caps() {
        let mut registry = setup();
        let original = registry.get_platform_caps();
        use_pred("heist.fe26.testnet", 0);
        let raised = {
            let mut caps = original.clone();
            caps.max_per_agent_yocto += 1;
            caps
        };
        let err = catch_unwind(AssertUnwindSafe(|| {
            registry.set_platform_caps(raised);
        }))
        .unwrap_err();
        assert!(
            panic_text(err).contains("game cannot raise platform caps"),
            "panic should name the cap rule"
        );
        assert_eq!(registry.get_platform_caps(), original);

        use_pred("owner.fe26.testnet", 0);
        let mut above = absolute_ceiling();
        above.max_pot_yocto += 1;
        let err = catch_unwind(AssertUnwindSafe(|| {
            registry.set_platform_caps(above);
        }))
        .unwrap_err();
        let text = panic_text(err);
        assert!(text.contains("absolute ceiling") || text.contains("caps exceed"));
        assert_eq!(registry.get_platform_caps(), original);

        let mut lowered = original.clone();
        lowered.max_players = 4;
        registry.set_platform_caps(lowered.clone());
        assert_eq!(registry.get_platform_caps().max_players, 4);
    }

    #[test]
    fn spend_over_cap_rejected() {
        let mut registry = setup();
        use_pred("alice.fe26.testnet", MIN_AGENT_DEPOSIT_YOCTO);
        let agent = registry.register_agent("alpha".into(), test_key());
        let caps = registry.get_platform_caps();
        let err = catch_unwind(AssertUnwindSafe(|| {
            registry.check_spend(agent.clone(), U128(caps.max_per_action_yocto + 1), U128(0));
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("over cap"));

        let err = catch_unwind(AssertUnwindSafe(|| {
            registry.check_spend(agent.clone(), U128(1), U128(caps.max_per_agent_yocto));
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("over cap"));

        assert!(registry.check_spend(agent, U128(1), U128(0)));
    }

    #[test]
    fn revoked_agent_cannot_act() {
        let mut registry = setup();
        use_pred("alice.fe26.testnet", MIN_AGENT_DEPOSIT_YOCTO);
        let agent = registry.register_agent("alpha".into(), test_key());
        use_pred("alice.fe26.testnet", 0);
        registry.revoke(agent.clone());
        assert!(registry.is_revoked(agent.clone()));
        let err = catch_unwind(AssertUnwindSafe(|| {
            registry.check_spend(agent, U128(1), U128(0));
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("revoked"));
    }

    #[test]
    fn function_call_key_cannot_call_methods_outside_its_scope() {
        let mut registry = setup();
        use_pred("alice.fe26.testnet", MIN_AGENT_DEPOSIT_YOCTO);
        let agent = registry.register_agent("alpha".into(), test_key());

        let mut scoped: Option<(Vec<String>, AccountId, Option<NearToken>)> = None;
        let mut full_access = false;
        for receipt in get_created_receipts() {
            for action in receipt.actions {
                match action {
                    near_sdk::mock::MockAction::AddKeyWithFunctionCall {
                        method_names,
                        receiver_id,
                        allowance,
                        ..
                    } => {
                        scoped = Some((method_names, receiver_id, allowance));
                    }
                    near_sdk::mock::MockAction::AddKeyWithFullAccess { .. } => {
                        full_access = true;
                    }
                    _ => {}
                }
            }
        }
        let (methods, receiver, allowance) = scoped.expect("function-call key");
        assert!(
            !full_access,
            "agent subaccount must not receive a full-access key"
        );
        assert_eq!(methods, vec!["act", "bid", "transfer_item"]);
        let expected: AccountId = "heist.fe26.testnet".parse().unwrap();
        assert_eq!(receiver, expected);
        assert!(
            allowance.is_some(),
            "gas allowance is limited to the deposit"
        );

        let forbidden = [
            "resolve",
            "delist_game",
            "set_platform_caps",
            "apply_platform_caps",
            "init_game",
            "check_spend",
            "register_agent",
            "revoke",
            "mt_transfer",
            "withdraw",
            "create_game",
            "import_agent",
            "mark_delisted",
        ];
        for method in forbidden {
            assert!(
                !methods.iter().any(|allowed| allowed == method),
                "{method} must not be on the key"
            );
            assert!(!registry.key_permits(agent.clone(), method.to_string()));
        }
        assert!(registry.key_permits(agent.clone(), "act".into()));
        assert!(registry.key_permits(agent.clone(), "bid".into()));
        assert!(registry.key_permits(agent, "transfer_item".into()));

        let registered = get_logs()
            .iter()
            .any(|line| line.contains("agent_registered"));
        assert!(registered);
    }
}
