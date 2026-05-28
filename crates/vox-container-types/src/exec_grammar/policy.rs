//! Policy evaluation: maps an [`ExecAst`] against `exec-policy.v1.yaml` rules.

use serde::{Deserialize, Serialize};

use crate::exec_grammar::ExecAst;

/// A loaded exec-policy — mirrors `contracts/terminal/exec-policy.v1.yaml`.
///
/// Callers deserialise from YAML (e.g. via `serde_yaml`) and pass the result
/// to [`ExecPolicy::evaluate`].  This crate stays `serde_yaml`-free so it
/// doesn't pull in heavy YAML machinery for consumers that don't need it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecPolicy {
    /// Shell cmdlets that may be invoked (case-insensitive).
    #[serde(default)]
    pub allowed_cmdlets: Vec<String>,
    /// Binary executables that may be invoked (case-insensitive).
    #[serde(default)]
    pub allowed_binaries: Vec<String>,
    /// Parameters blocked per command (key `"*"` applies to all commands).
    #[serde(default)]
    pub blocked_parameters: std::collections::HashMap<String, Vec<String>>,
    /// Commands that perform network I/O (subject to `network_fetch_domains` enforcement).
    #[serde(default)]
    pub network_fetch_commands: Vec<String>,
    /// Domains that network-fetch commands are allowed to contact.
    #[serde(default)]
    pub network_fetch_domains: Vec<String>,
}

/// A single policy violation found by [`ExecPolicy::evaluate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub kind: ViolationKind,
    pub detail: String,
}

/// The kind of policy violation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationKind {
    /// The command binary / cmdlet is not in the allow-list.
    UnknownCommand,
    /// A flag or parameter is blocked for this command.
    BlockedParameter,
}

impl ExecPolicy {
    /// Evaluate `ast` against this policy. Returns all violations found.
    ///
    /// An empty `Vec` means the command passes policy. The caller should also
    /// consult `ast.risk` (after [`crate::exec_grammar::risk::classify`]) for commands that
    /// are allowed but warrant a confirmation prompt.
    pub fn evaluate(&self, ast: &ExecAst) -> Vec<PolicyViolation> {
        let mut violations = Vec::new();

        // Allow-list check (only enforced when allow-list is non-empty)
        if !self.allowed_binaries.is_empty() || !self.allowed_cmdlets.is_empty() {
            let allowed = self
                .allowed_binaries
                .iter()
                .chain(self.allowed_cmdlets.iter())
                .any(|a| a.trim().eq_ignore_ascii_case(ast.command.trim()));

            if !allowed {
                violations.push(PolicyViolation {
                    kind: ViolationKind::UnknownCommand,
                    detail: format!(
                        "`{}` is not in allowed_cmdlets or allowed_binaries",
                        ast.command
                    ),
                });
            }
        }

        // Blocked-parameter check (wildcard `"*"` + case-insensitive command scope)
        for (scope, blocked) in &self.blocked_parameters {
            let applies = scope == "*" || scope.trim().eq_ignore_ascii_case(ast.command.trim());
            if !applies {
                continue;
            }
            for flag in &ast.flags {
                if blocked
                    .iter()
                    .any(|b| b.trim().eq_ignore_ascii_case(flag.name.trim()))
                {
                    violations.push(PolicyViolation {
                        kind: ViolationKind::BlockedParameter,
                        detail: format!(
                            "parameter `{}` is blocked for `{}`",
                            flag.name, ast.command
                        ),
                    });
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec_grammar::parse;

    #[test]
    fn unknown_command_violation() {
        let policy = ExecPolicy {
            allowed_binaries: vec!["cargo".into()],
            ..Default::default()
        };
        let ast = parse("rm -rf /").unwrap();
        let v = policy.evaluate(&ast);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::UnknownCommand);
    }

    #[test]
    fn blocked_parameter_wildcard() {
        let mut blocked = std::collections::HashMap::new();
        blocked.insert("*".into(), vec!["Recurse".into()]);
        let policy = ExecPolicy {
            blocked_parameters: blocked,
            ..Default::default()
        };
        let ast = parse("Get-ChildItem -Recurse").unwrap();
        let v = policy.evaluate(&ast);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::BlockedParameter);
    }

    #[test]
    fn empty_policy_allows_everything() {
        let policy = ExecPolicy::default();
        let ast = parse("anything --goes").unwrap();
        assert!(policy.evaluate(&ast).is_empty());
    }

    #[test]
    fn allowed_command_passes() {
        let policy = ExecPolicy {
            allowed_binaries: vec!["git".into()],
            ..Default::default()
        };
        let ast = parse("git status").unwrap();
        assert!(policy.evaluate(&ast).is_empty());
    }
}
