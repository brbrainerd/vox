#![cfg_attr(test, allow(unsafe_code))] // test-only std::env::set_var (unsafe on edition 2024)
//! Trusted MCP caller role (human vs agent) for privileged operations.
//!
//! Derived from the launcher's `VOX_MCP_CALLER_ROLE` environment variable — not from
//! in-band tool request bodies — so an agent cannot assert "human".

use std::sync::OnceLock;

/// Trusted caller role for the current MCP server process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallerRole {
    Human,
    Agent,
}

impl CallerRole {
    pub fn from_env() -> Self {
        match std::env::var("VOX_MCP_CALLER_ROLE")
            .ok()
            .as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("human") => CallerRole::Human,
            _ => CallerRole::Agent,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CallerRole::Human => "human",
            CallerRole::Agent => "agent",
        }
    }
}

/// The process-wide trusted role (read once from the launcher's environment).
pub fn trusted_caller_role() -> CallerRole {
    static ROLE: OnceLock<CallerRole> = OnceLock::new();
    *ROLE.get_or_init(CallerRole::from_env)
}

#[cfg(test)]
mod tests {
    use super::{CallerRole, trusted_caller_role};
    use serial_test::serial;

    #[test]
    #[serial]
    fn caller_role_from_env_only_trusts_human_literal() {
        for (val, expect) in [
            (Some("human"), CallerRole::Human),
            (Some("HUMAN"), CallerRole::Human),
            (Some("  Human  "), CallerRole::Human),
            (Some("agent"), CallerRole::Agent),
            (Some("operator"), CallerRole::Agent),
            (Some(""), CallerRole::Agent),
            (None, CallerRole::Agent),
        ] {
            // SAFETY: `#[serial]` — no concurrent env mutation in this crate's tests.
            unsafe {
                match val {
                    Some(v) => std::env::set_var("VOX_MCP_CALLER_ROLE", v),
                    None => std::env::remove_var("VOX_MCP_CALLER_ROLE"),
                }
            }
            assert_eq!(CallerRole::from_env(), expect, "input {val:?}");
        }
    }

    #[test]
    #[serial]
    fn trusted_caller_role_matches_from_env() {
        // SAFETY: `#[serial]` — no concurrent env mutation in this crate's tests.
        unsafe { std::env::remove_var("VOX_MCP_CALLER_ROLE") };
        assert_eq!(trusted_caller_role(), CallerRole::Agent);
    }
}
