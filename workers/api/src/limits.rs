//! Every abuse and lobby limit lives here. Route handlers and wrangler.toml
//! both follow these constants. The Workers Rate Limiting binding only accepts
//! a 10 or 60 second period, so the per-minute caps are bindings and the
//! hourly registration cap is a counter keyed by IP.

use sha2::{Digest, Sha256};

pub const REGISTER_PER_HOUR: u32 = 5;
pub const REGISTER_WINDOW_SECS: u64 = 60 * 60;
/// Burst cap enforced by the binding. A minute cannot exceed the hourly cap.
pub const REGISTER_BURST: u32 = 5;
pub const REGISTER_BURST_PERIOD_SECS: u64 = 60;
pub const REGISTER_BINDING: &str = "REGISTER_LIMIT";

pub const ACTION_PER_MINUTE: u32 = 120;
pub const ACTION_PERIOD_SECS: u64 = 60;
pub const ACTION_BINDING: &str = "ACTION_LIMIT";

pub const READ_PER_MINUTE: u32 = 300;
pub const READ_PERIOD_SECS: u64 = 60;
pub const READ_BINDING: &str = "READ_LIMIT";

pub const BINDINGS: &[(&str, u32, u64)] = &[
    (REGISTER_BINDING, REGISTER_BURST, REGISTER_BURST_PERIOD_SECS),
    (ACTION_BINDING, ACTION_PER_MINUTE, ACTION_PERIOD_SECS),
    (READ_BINDING, READ_PER_MINUTE, READ_PERIOD_SECS),
];

pub const LOBBY_MAX_POSTS: usize = 500;
pub const LOBBY_COOLDOWN_SECS: u64 = 10;
pub const LOBBY_REPORT_HIDE: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LimitPlan {
    Exempt,
    Register {
        ip: String,
    },
    Binding {
        name: &'static str,
        key: String,
        retry_after: u64,
    },
}

pub fn classify(method: &str, path: &str, authorization: Option<&str>, ip: &str) -> LimitPlan {
    if method == "OPTIONS" {
        return LimitPlan::Exempt;
    }
    let path = path.split('?').next().unwrap_or(path);
    let path = path.trim_end_matches('/');
    if method == "POST" && path.ends_with("/v1/agents/register") {
        return LimitPlan::Register {
            ip: normalize_ip(ip),
        };
    }
    if matches!(method, "POST" | "DELETE" | "PUT" | "PATCH") {
        if let Some(token) = bearer_token(authorization) {
            return LimitPlan::Binding {
                name: ACTION_BINDING,
                key: token_key(token),
                retry_after: ACTION_PERIOD_SECS,
            };
        }
    }
    LimitPlan::Binding {
        name: READ_BINDING,
        key: normalize_ip(ip),
        retry_after: READ_PERIOD_SECS,
    }
}

pub fn register_retry_after(now: u64) -> u64 {
    let elapsed = now % REGISTER_WINDOW_SECS;
    if elapsed == 0 {
        REGISTER_WINDOW_SECS
    } else {
        REGISTER_WINDOW_SECS - elapsed
    }
}

fn normalize_ip(ip: &str) -> String {
    let trimmed = ip.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn bearer_token(authorization: Option<&str>) -> Option<&str> {
    let header = authorization?.trim();
    let rest = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))?;
    let token = rest.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Stable per-token key. The raw token never leaves the worker as a limiter key.
pub fn token_key(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_match_the_published_limits() {
        assert!(matches!(
            classify("OPTIONS", "/v1/agents/register", None, "1.1.1.1"),
            LimitPlan::Exempt
        ));
        match classify("POST", "/v1/agents/register?now=1", None, " 203.0.113.4 ") {
            LimitPlan::Register { ip } => assert_eq!(ip, "203.0.113.4"),
            other => panic!("{other:?}"),
        }
        let token = "sample-token";
        match classify(
            "POST",
            "/v1/matches/mt_1/actions",
            Some(&format!("Bearer {token}")),
            "203.0.113.4",
        ) {
            LimitPlan::Binding {
                name,
                key,
                retry_after,
            } => {
                assert_eq!(name, ACTION_BINDING);
                assert_eq!(retry_after, ACTION_PERIOD_SECS);
                assert_eq!(key.len(), 64);
                assert!(!key.contains(token));
                assert_eq!(key, token_key(token));
            }
            other => panic!("{other:?}"),
        }
        match classify(
            "DELETE",
            "/v1/arenas/vault/queue",
            Some("bearer fe2_abc"),
            "203.0.113.4",
        ) {
            LimitPlan::Binding { name, .. } => assert_eq!(name, ACTION_BINDING),
            other => panic!("{other:?}"),
        }
        match classify(
            "GET",
            "/v1/leaderboard",
            Some(&format!("Bearer {token}")),
            "203.0.113.4",
        ) {
            LimitPlan::Binding {
                name,
                key,
                retry_after,
            } => {
                assert_eq!(name, READ_BINDING);
                assert_eq!(key, "203.0.113.4");
                assert_eq!(retry_after, READ_PERIOD_SECS);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            register_retry_after(REGISTER_WINDOW_SECS + 15),
            REGISTER_WINDOW_SECS - 15
        );
        assert_eq!(ACTION_PER_MINUTE, 120);
        assert_eq!(READ_PER_MINUTE, 300);
        assert_eq!(REGISTER_PER_HOUR, 5);
    }

    #[test]
    fn wrangler_bindings_use_these_numbers() {
        let toml = include_str!("../wrangler.toml");
        assert!(toml.contains(&format!(
            "name = \"{REGISTER_BINDING}\"\nnamespace_id = \"1001\"\nsimple = {{ limit = {REGISTER_BURST}, period = {REGISTER_BURST_PERIOD_SECS} }}"
        )));
        assert!(toml.contains(&format!(
            "name = \"{ACTION_BINDING}\"\nnamespace_id = \"1002\"\nsimple = {{ limit = {ACTION_PER_MINUTE}, period = {ACTION_PERIOD_SECS} }}"
        )));
        assert!(toml.contains(&format!(
            "name = \"{READ_BINDING}\"\nnamespace_id = \"1003\"\nsimple = {{ limit = {READ_PER_MINUTE}, period = {READ_PERIOD_SECS} }}"
        )));
    }
}
