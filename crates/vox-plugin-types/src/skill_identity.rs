//! Skill namespace/identity scheme (Task 3.4, harness parity plan).
//!
//! Prior to this module a skill's [`crate::skill_manifest::SkillManifest::id`]
//! was a free-text string with no collision handling: two independently
//! mined or externally-sourced skills could claim the same id and the last
//! one to call `SkillRegistry::install_bundle` (which does `INSERT OR
//! REPLACE` in `vox-db`) would silently overwrite the other.
//!
//! ## Investigation findings (do not re-derive; read this first)
//!
//! * No existing crate in this workspace does reverse-DNS or
//!   `io.github.<user>/<name>`-style namespacing for anything (plugins, MCP
//!   tool registry entries, etc.) — greenfield. Grepping for `io.github`
//!   across `crates/` and `contracts/` (excluding `node_modules`) turns up
//!   nothing but incidental README matches in vendored JS packages.
//! * No GitHub OAuth/OIDC integration or any other "prove you own this
//!   namespace" mechanism exists anywhere in this codebase. Building one
//!   (web callback flow, stored credentials, token verification) is out of
//!   scope here — see the module docs on [`SkillNamespace::GitHub`] for the
//!   documented deferred gap.
//!
//! ## Scheme
//!
//! An identity is `<namespace>/<name>`:
//!
//! * `local/<name>` — first-party / locally-mined skills (this vox
//!   instance's own stuff; mirrors the existing `source` field on
//!   `skill_candidates` rows, where the code/op miners write `source =
//!   "local"`-shaped values).
//! * `io.github.<user>/<name>` — externally-sourced/imported skills,
//!   reverse-DNS style, anchored on the GitHub user/org that published the
//!   skill (the only external identity anchor available without new
//!   infrastructure).
//!
//! Uniqueness (first-come-first-served, crates.io-style) is enforced at the
//! DB layer by `vox-db`'s `skill_identities` table + `claim_skill_identity`
//! op, not by this module — this module only parses and validates the
//! *format*.

use std::fmt;

/// Lowercase ASCII alphanumeric plus `-` and `_`, non-empty. Used for both
/// the GitHub user segment and the skill name segment.
fn is_valid_segment(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// The namespace half of a [`SkillIdentity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillNamespace {
    /// `local` — this vox instance's own first-party / locally-mined skills.
    /// No ownership proof needed or possible; anti-squatting still applies
    /// (see [`SkillIdentity`] docs) but "owner" for local skills is always
    /// the literal string `"local"`.
    Local,
    /// `io.github.<user>` — an externally-sourced skill anchored on a
    /// GitHub user/org name. **Format only**: nothing in this codebase
    /// today verifies the registrant actually controls that GitHub
    /// account. Documented deferred gap — see module docs.
    GitHub {
        /// The GitHub user/org segment, already lowercased.
        user: String,
    },
}

impl fmt::Display for SkillNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillNamespace::Local => write!(f, "local"),
            SkillNamespace::GitHub { user } => write!(f, "io.github.{user}"),
        }
    }
}

impl SkillNamespace {
    /// True for anything that isn't [`SkillNamespace::Local`] — the
    /// distinction the stricter externally-sourced install gate (Task 3.4
    /// design point 3) keys off of.
    pub fn is_external(&self) -> bool {
        !matches!(self, SkillNamespace::Local)
    }
}

/// A parsed, validated skill identity: `<namespace>/<name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillIdentity {
    pub namespace: SkillNamespace,
    pub name: String,
}

/// Error returned by [`SkillIdentity::parse`].
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SkillIdentityError {
    #[error(
        "skill identity {0:?}: namespace must be \"local\" or \"io.github.<user>\" (reverse-DNS GitHub form)"
    )]
    UnknownNamespace(String),
    #[error(
        "skill identity {0:?}: GitHub user segment must be non-empty lowercase alphanumeric/-/_"
    )]
    InvalidGitHubUser(String),
    #[error("skill identity {0:?}: name must be non-empty lowercase alphanumeric/-/_")]
    InvalidName(String),
}

impl SkillIdentity {
    /// Parse `id` (e.g. a [`crate::skill_manifest::SkillManifest::id`]) into
    /// a validated namespace + name pair.
    ///
    /// Backward compatibility: ids predating this scheme (e.g.
    /// `"vox.compiler"`, `"vox.docs"` — bundled first-party skills with no
    /// `/` at all) are treated as implicitly [`SkillNamespace::Local`] with
    /// the whole id as the name, un-validated against the character set
    /// below. Only ids that opt into the explicit `<namespace>/<name>` form
    /// are format-checked. This lets existing bundled skill manifests keep
    /// their ids as-is while new/external skills are required to namespace
    /// themselves.
    pub fn parse(id: &str) -> Result<Self, SkillIdentityError> {
        let Some((namespace_part, name)) = id.split_once('/') else {
            return Ok(SkillIdentity {
                namespace: SkillNamespace::Local,
                name: id.to_string(),
            });
        };

        if !is_valid_segment(name) {
            return Err(SkillIdentityError::InvalidName(id.to_string()));
        }

        let namespace = if namespace_part == "local" {
            SkillNamespace::Local
        } else if let Some(user) = namespace_part.strip_prefix("io.github.") {
            if !is_valid_segment(user) {
                return Err(SkillIdentityError::InvalidGitHubUser(id.to_string()));
            }
            SkillNamespace::GitHub {
                user: user.to_string(),
            }
        } else {
            return Err(SkillIdentityError::UnknownNamespace(id.to_string()));
        };

        Ok(SkillIdentity {
            namespace,
            name: name.to_string(),
        })
    }

    /// True when this identity requires the stricter externally-sourced
    /// install gate (namespace present + minimum reliability signal).
    pub fn is_external(&self) -> bool {
        self.namespace.is_external()
    }
}

impl fmt::Display for SkillIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_identity() {
        let id = SkillIdentity::parse("local/my-skill").unwrap();
        assert_eq!(id.namespace, SkillNamespace::Local);
        assert_eq!(id.name, "my-skill");
        assert!(!id.is_external());
        assert_eq!(id.to_string(), "local/my-skill");
    }

    #[test]
    fn parses_github_identity() {
        let id = SkillIdentity::parse("io.github.alice/cool_skill").unwrap();
        assert_eq!(
            id.namespace,
            SkillNamespace::GitHub {
                user: "alice".to_string()
            }
        );
        assert_eq!(id.name, "cool_skill");
        assert!(id.is_external());
        assert_eq!(id.to_string(), "io.github.alice/cool_skill");
    }

    #[test]
    fn legacy_dotted_id_without_slash_is_implicitly_local() {
        let id = SkillIdentity::parse("vox.compiler").unwrap();
        assert_eq!(id.namespace, SkillNamespace::Local);
        assert_eq!(id.name, "vox.compiler");
        assert!(!id.is_external());
    }

    #[test]
    fn rejects_unknown_namespace() {
        assert!(matches!(
            SkillIdentity::parse("npm.someuser/foo"),
            Err(SkillIdentityError::UnknownNamespace(_))
        ));
    }

    #[test]
    fn rejects_empty_github_user() {
        assert!(matches!(
            SkillIdentity::parse("io.github./foo"),
            Err(SkillIdentityError::InvalidGitHubUser(_))
        ));
    }

    #[test]
    fn rejects_uppercase_or_invalid_chars() {
        assert!(matches!(
            SkillIdentity::parse("local/My Skill"),
            Err(SkillIdentityError::InvalidName(_))
        ));
        assert!(matches!(
            SkillIdentity::parse("io.github.Alice/foo"),
            Err(SkillIdentityError::InvalidGitHubUser(_))
        ));
    }

    #[test]
    fn rejects_empty_name() {
        assert!(matches!(
            SkillIdentity::parse("local/"),
            Err(SkillIdentityError::InvalidName(_))
        ));
    }
}
