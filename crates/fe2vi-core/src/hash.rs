use serde::{Deserialize, Deserializer, Serializer};
use sha2::{Digest, Sha256};

use crate::types::GameRules;

/// Decimal-string codec for yoctoNEAR amounts so JSON integers never lose precision.
pub mod yocto_as_str {
    use super::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &u128, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u128, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse::<u128>().map_err(serde::de::Error::custom)
    }
}

/// Canonical UTF-8 JSON for `rules`. Field order matches [`GameRules`].
/// This byte string is what `pin_hash` commits to. Prose in GAME.md is not included.
pub fn canonical_rules_json(rules: &GameRules) -> String {
    serde_json::to_string(rules).expect("GameRules serialization is infallible")
}

pub fn pin_hash(rules: &GameRules) -> String {
    let digest = Sha256::digest(canonical_rules_json(rules).as_bytes());
    hex::encode(digest)
}

pub fn valid_pin_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heist_night_example;

    #[test]
    fn example_rules_hash_is_stable() {
        let rules = heist_night_example();
        let json = canonical_rules_json(&rules);
        assert_eq!(
            json,
            concat!(
                r#"{"template_id":"heist-night","max_players":6,"duration_seconds":3600,"#,
                r#""round_count":4,"pot_yocto":"10000000000000000000000000","#,
                r#""per_agent_cap_yocto":"2000000000000000000000000","#,
                r#""per_action_cap_yocto":"500000000000000000000000","entry_yocto":"0"}"#
            )
        );
        assert_eq!(
            pin_hash(&rules),
            "320421571d7cf9e178b0b5c0aa1dce8d3ea3f2dbf0bdc507849aaf70a121651d"
        );
        assert!(valid_pin_hash(&pin_hash(&rules)));
        assert!(!valid_pin_hash("ABC"));
        assert!(!valid_pin_hash(&"A".repeat(64)));
    }
}
