//! Risk level classification for parsed commands.
//!
//! Classifies an [`ExecAst`] against a loaded [`ExecPolicy`] to assign a
//! [`RiskLevel`].  This is the pure-Rust equivalent of the PowerShell-AST
//! risk path in `check_terminal.rs`.

use serde::{Deserialize, Serialize};

use crate::exec_grammar::{ExecAst, ExecPolicy};

/// Assessed risk of executing a command as understood by the exec-policy layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Not yet classified (parser stub or unknown command).
    #[default]
    Unknown,
    /// Safe to run without confirmation — command is in the allow-list and no
    /// elevated indicators are present.
    Safe,
    /// Requires user confirmation or elevated permissions — e.g. a network
    /// fetch command, a recursive operation, or an external process spawn with
    /// broad scope.
    Elevated,
    /// Blocked by policy; must not execute.
    Blocked,
}

/// Commands known to perform network I/O even when not in `network_fetch_commands`.
const IMPLICIT_NETWORK_COMMANDS: &[&str] = &[
    "curl",
    "wget",
    "fetch",
    "Invoke-WebRequest",
    "Invoke-RestMethod",
];

/// Classify `ast` using the given policy and update `ast.risk` in place.
///
/// This is intentionally conservative: when in doubt, prefer `Elevated` over
/// `Safe`.  Callers should treat `Blocked` as a hard stop.
pub fn classify(ast: &mut ExecAst, policy: &ExecPolicy) {
    let violations = policy.evaluate(ast);

    if !violations.is_empty() {
        ast.risk = RiskLevel::Blocked;
        return;
    }

    // Check for implicit network commands
    let cmd_lower = ast.command.to_ascii_lowercase();
    let is_net = IMPLICIT_NETWORK_COMMANDS
        .iter()
        .any(|n| n.to_ascii_lowercase() == cmd_lower)
        || policy
            .network_fetch_commands
            .iter()
            .any(|n| n.to_ascii_lowercase() == cmd_lower);

    if is_net {
        ast.risk = RiskLevel::Elevated;
        return;
    }

    // Flags that indicate elevated scope regardless of command
    const ELEVATED_FLAGS: &[&str] = &[
        "recurse",
        "r",
        "rf",
        "force",
        "f",
        "no-preserve-root",
        "delete",
        "rm",
        "sudo",
        "admin",
    ];
    let has_elevated_flag = ast.flags.iter().any(|fl| {
        ELEVATED_FLAGS
            .iter()
            .any(|ef| ef.eq_ignore_ascii_case(&fl.name))
    });

    if has_elevated_flag {
        ast.risk = RiskLevel::Elevated;
        return;
    }

    ast.risk = RiskLevel::Safe;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec_grammar::{ExecPolicy, parse};

    fn empty_policy() -> ExecPolicy {
        ExecPolicy::default()
    }

    fn permissive_policy(commands: &[&str]) -> ExecPolicy {
        ExecPolicy {
            allowed_binaries: commands.iter().map(|s| s.to_string()).collect(),
            allowed_cmdlets: vec![],
            blocked_parameters: Default::default(),
            network_fetch_commands: vec![],
            network_fetch_domains: vec![],
        }
    }

    #[test]
    fn safe_command() {
        let mut ast = parse("cargo build --release").unwrap();
        classify(&mut ast, &permissive_policy(&["cargo"]));
        assert_eq!(ast.risk, RiskLevel::Safe);
    }

    #[test]
    fn blocked_unknown_command() {
        let mut ast = parse("rm -rf /").unwrap();
        let policy = permissive_policy(&["cargo"]); // rm not allowed
        classify(&mut ast, &policy);
        assert_eq!(ast.risk, RiskLevel::Blocked);
    }

    #[test]
    fn elevated_recurse_flag() {
        let mut ast = parse("Get-ChildItem -Recurse").unwrap();
        classify(&mut ast, &empty_policy()); // empty policy = no allow list enforcement
        assert_eq!(ast.risk, RiskLevel::Elevated);
    }

    #[test]
    fn elevated_network_command() {
        let mut ast = parse("curl https://example.com").unwrap();
        classify(&mut ast, &empty_policy());
        assert_eq!(ast.risk, RiskLevel::Elevated);
    }
}
