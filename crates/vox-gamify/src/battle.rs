//! Bug battle system seeded from TOESTUB findings.

use crate::util::now_unix;
use serde::{Deserialize, Serialize};

// ─── Bug Type ────────────────────────────────────────────

/// Bug categories with associated reward tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BugType {
    Syntax,
    Logic,
    Performance,
    Security,
}

impl BugType {
    /// Crystal reward for defeating this bug type.
    pub const fn crystal_reward(&self) -> u64 {
        match self {
            BugType::Syntax => 10,
            BugType::Logic => 25,
            BugType::Performance => 40,
            BugType::Security => 50,
        }
    }

    /// XP reward for defeating this bug type.
    pub const fn xp_reward(&self) -> u64 {
        match self {
            BugType::Syntax => 15,
            BugType::Logic => 30,
            BugType::Performance => 50,
            BugType::Security => 60,
        }
    }

    /// Emoji representation.
    pub const fn emoji(&self) -> &str {
        match self {
            BugType::Syntax => "🐛",
            BugType::Logic => "🧩",
            BugType::Performance => "🐢",
            BugType::Security => "🔒",
        }
    }

    /// Display name.
    pub const fn display(&self) -> &str {
        match self {
            BugType::Syntax => "Syntax Bug",
            BugType::Logic => "Logic Bug",
            BugType::Performance => "Performance Bug",
            BugType::Security => "Security Bug",
        }
    }

    /// Slug for DB storage.
    pub const fn as_str(&self) -> &str {
        match self {
            BugType::Syntax => "syntax",
            BugType::Logic => "logic",
            BugType::Performance => "performance",
            BugType::Security => "security",
        }
    }

    /// Map a TOESTUB rule_id prefix to a bug type.
    pub fn from_rule_id(rule_id: &str) -> Self {
        if rule_id.starts_with("stub/") || rule_id.starts_with("empty/") {
            BugType::Syntax
        } else if rule_id.starts_with("magic/") || rule_id.starts_with("dry/") {
            BugType::Logic
        } else if rule_id.starts_with("victory/") || rule_id.starts_with("unwired/") || rule_id.starts_with("clone/") {
            BugType::Performance
        } else if rule_id.starts_with("unresolved/")
            || rule_id.starts_with("ai/")
            || rule_id.starts_with("E04")
        {
            BugType::Security
        } else if rule_id.starts_with("E03") || rule_id.starts_with("vox-typeck") {
            BugType::Logic
        } else {
            BugType::Syntax // default
        }
    }
}

impl std::fmt::Display for BugType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

// ─── Battle ──────────────────────────────────────────────

/// A bug battle instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Battle {
    pub id: String,
    pub user_id: String,
    pub companion_id: String,
    pub bug_type: BugType,
    pub bug_description: String,
    pub bug_code: Option<String>,
    pub submitted_code: Option<String>,
    pub success: bool,
    pub crystals_earned: u64,
    pub xp_earned: u64,
    pub duration_secs: u64,
    pub created_at: i64,
}

impl Battle {
    /// Create a new battle from a TOESTUB-style finding.
    pub fn from_finding(
        id: impl Into<String>,
        user_id: impl Into<String>,
        companion_id: impl Into<String>,
        rule_id: &str,
        description: impl Into<String>,
        code_context: Option<String>,
    ) -> Self {
        let now = now_unix();
        let bug_type = BugType::from_rule_id(rule_id);
        Self {
            id: id.into(),
            user_id: user_id.into(),
            companion_id: companion_id.into(),
            bug_type,
            bug_description: description.into(),
            bug_code: code_context,
            submitted_code: None,
            success: false,
            crystals_earned: 0,
            xp_earned: 0,
            duration_secs: 0,
            created_at: now,
        }
    }

    /// Record a battle result.
    pub fn record_result(&mut self, success: bool, duration_secs: u64) {
        self.success = success;
        self.duration_secs = duration_secs;
        if success {
            self.crystals_earned = self.bug_type.crystal_reward();
            self.xp_earned = self.bug_type.xp_reward();
        } else {
            self.crystals_earned = 0;
            self.xp_earned = 0;
        }
    }

    /// Check if a battle has been completed (code has been submitted).
    pub fn is_completed(&self) -> bool {
        self.submitted_code.is_some()
    }
}

// ─── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bug_type_rewards() {
        assert_eq!(BugType::Syntax.crystal_reward(), 10);
        assert_eq!(BugType::Security.crystal_reward(), 50);
        assert_eq!(BugType::Logic.xp_reward(), 30);
        assert_eq!(BugType::Performance.xp_reward(), 50);
    }

    #[test]
    fn bug_type_from_rule_id() {
        assert_eq!(BugType::from_rule_id("stub/todo"), BugType::Syntax);
        assert_eq!(BugType::from_rule_id("empty/body"), BugType::Syntax);
        assert_eq!(BugType::from_rule_id("magic/value"), BugType::Logic);
        assert_eq!(BugType::from_rule_id("dry/violation"), BugType::Logic);
        assert_eq!(BugType::from_rule_id("victory/claim"), BugType::Performance);
        assert_eq!(
            BugType::from_rule_id("unwired/module"),
            BugType::Performance
        );
        assert_eq!(BugType::from_rule_id("clone/abuse"), BugType::Performance);
        assert_eq!(BugType::from_rule_id("unresolved/ref"), BugType::Security);
        assert_eq!(BugType::from_rule_id("ai/dead-code"), BugType::Security);
        assert_eq!(BugType::from_rule_id("unknown/rule"), BugType::Syntax); // default
    }

    #[test]
    fn battle_from_finding() {
        let b = Battle::from_finding(
            "b-1",
            "u-1",
            "c-1",
            "stub/todo",
            "Found todo!() macro",
            Some("fn foo() { todo!() }".to_string()),
        );
        assert_eq!(b.bug_type, BugType::Syntax);
        assert!(!b.success);
        assert_eq!(b.crystals_earned, 0);
    }

    #[test]
    fn battle_record_success() {
        let mut b = Battle::from_finding("b-1", "u-1", "c-1", "magic/value", "Hardcoded 42", None);
        b.record_result(true, 30);
        assert!(b.success);
        assert_eq!(b.crystals_earned, 25); // Logic bug reward
        assert_eq!(b.xp_earned, 30);
        assert_eq!(b.duration_secs, 30);
    }

    #[test]
    fn battle_record_failure() {
        let mut b = Battle::from_finding(
            "b-1",
            "u-1",
            "c-1",
            "unresolved/ref",
            "Missing import",
            None,
        );
        b.record_result(false, 60);
        assert!(!b.success);
        assert_eq!(b.crystals_earned, 0);
        assert_eq!(b.xp_earned, 0);
    }

    #[test]
    fn battle_completion_check() {
        let mut b = Battle::from_finding("b-1", "u-1", "c-1", "stub/todo", "Test", None);
        assert!(!b.is_completed());
        b.submitted_code = Some("fn foo() { 42 }".to_string());
        assert!(b.is_completed());
    }
}
