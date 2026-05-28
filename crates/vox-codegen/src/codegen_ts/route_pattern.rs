//! Route-pattern parser and segment-aware overlap detection.
//!
//! Phase C of the Svelte-mineable features implementation plan upgrades the
//! existing exact-string-match conflict detection at
//! `super::routes` (a `HashSet<(Method, String)>` of literal paths) to
//! segment-aware overlap detection that catches `/users/:id` vs `/users/me`
//! ambiguity at compile time. This module is the pure utility layer; the
//! integration into the route emitter is a separate change.
//!
//! Grammar accepted by `RoutePattern::parse`:
//! - Empty path `""` or `"/"` — root.
//! - Slash-separated segments.
//! - A segment beginning with `:` is a parameter (e.g. `:id`); the remainder is the
//!   parameter name.
//! - A `*` segment is a wildcard absorbing the rest of the path.
//! - Any other segment is a literal.
//!
//! Two patterns *overlap* when there exists a concrete path that both could match.
//! See `Overlap` for the precedence resolution rule.

use std::fmt;

/// One segment in a parsed [`RoutePattern`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// A literal path segment, e.g. `"users"` in `/users/:id`.
    Literal(String),
    /// A typed parameter segment, e.g. `id` in `/users/:id`.
    Param(String),
    /// A `*` wildcard absorbing zero or more trailing segments.
    Wildcard,
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Segment::Literal(s) => f.write_str(s),
            Segment::Param(name) => write!(f, ":{name}"),
            Segment::Wildcard => f.write_str("*"),
        }
    }
}

/// A parsed route path, decomposed into ordered [`Segment`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePattern {
    /// Ordered segments. Empty for the root path.
    pub segments: Vec<Segment>,
}

impl RoutePattern {
    /// Parse a slash-separated path string into a [`RoutePattern`]. Leading and trailing
    /// slashes are tolerated; empty segments are skipped (so `"//foo//"` parses as a
    /// single literal `foo`).
    #[must_use]
    pub fn parse(path: &str) -> Self {
        let segments = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s == "*" {
                    Segment::Wildcard
                } else if let Some(name) = s.strip_prefix(':') {
                    Segment::Param(name.to_string())
                } else {
                    Segment::Literal(s.to_string())
                }
            })
            .collect();
        RoutePattern { segments }
    }

    /// Decide whether this pattern overlaps with `other` (i.e. some concrete path matches both).
    ///
    /// Precedence model:
    /// - `Literal` vs same `Literal`: matches.
    /// - `Literal` vs different `Literal`: cannot overlap (anywhere along the path).
    /// - `Literal` vs `Param`: overlaps; literal is more specific (caller resolves precedence
    ///   by source order or by the more-specific-wins rule).
    /// - `Param` vs `Param`: overlaps and is **ambiguous** (no specificity tiebreaker).
    /// - `Wildcard` absorbs all remaining segments on either side.
    /// - Mismatched lengths without a wildcard cannot overlap.
    #[must_use]
    pub fn overlap_with(&self, other: &RoutePattern) -> Overlap {
        overlap_segments(&self.segments, &other.segments)
    }
}

impl fmt::Display for RoutePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.segments.is_empty() {
            return f.write_str("/");
        }
        for seg in &self.segments {
            f.write_str("/")?;
            seg.fmt(f)?;
        }
        Ok(())
    }
}

/// Result of overlap analysis between two patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlap {
    /// No concrete path matches both patterns.
    None,
    /// Both patterns match a shared concrete path; one is strictly more specific
    /// (i.e. has more `Literal` segments at the conflicting positions). Callers should
    /// resolve by source order, with a `routes.overlap.shadowed` info diagnostic.
    Shadowed,
    /// Both patterns match a shared concrete path with no specificity tiebreaker
    /// (e.g. `/:a/:b` vs `/:x/:y`). Callers should emit a
    /// `routes.overlap.unresolvable_precedence` error diagnostic.
    Ambiguous,
}

fn overlap_segments(a: &[Segment], b: &[Segment]) -> Overlap {
    use Segment::*;

    match (a.first(), b.first()) {
        (None, None) => Overlap::Ambiguous, // identical empty paths
        (Some(Wildcard), _) | (_, Some(Wildcard)) => {
            // Wildcard absorbs the remainder on either side; the rest cannot disambiguate.
            // Whether the wildcard is alone, or partnered with literals/params, both patterns
            // ultimately match a shared concrete path. Treat one-side-wildcard as Shadowed
            // (the literal/param side is more specific) and both-sides-wildcard as Ambiguous.
            match (a.first(), b.first()) {
                (Some(Wildcard), Some(Wildcard)) => Overlap::Ambiguous,
                _ => Overlap::Shadowed,
            }
        }
        (None, _) | (_, None) => Overlap::None, // different lengths, no wildcard to absorb
        (Some(seg_a), Some(seg_b)) => match (seg_a, seg_b) {
            (Literal(la), Literal(lb)) => {
                if la != lb {
                    return Overlap::None;
                }
                overlap_segments(&a[1..], &b[1..])
            }
            (Literal(_), Param(_)) | (Param(_), Literal(_)) => {
                match overlap_segments(&a[1..], &b[1..]) {
                    Overlap::None => Overlap::None,
                    // The literal side is strictly more specific at this position.
                    Overlap::Ambiguous | Overlap::Shadowed => Overlap::Shadowed,
                }
            }
            (Param(_), Param(_)) => overlap_segments(&a[1..], &b[1..]),
            // Wildcards are handled by the outer match arm above; this branch is unreachable.
            (Wildcard, _) | (_, Wildcard) => unreachable!("wildcard handled in outer arm"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> RoutePattern {
        RoutePattern::parse(s)
    }

    #[test]
    fn parse_root_yields_empty_segments() {
        assert!(p("/").segments.is_empty());
        assert!(p("").segments.is_empty());
    }

    #[test]
    fn parse_literal_segments() {
        assert_eq!(
            p("/users/me").segments,
            vec![
                Segment::Literal("users".to_string()),
                Segment::Literal("me".to_string()),
            ]
        );
    }

    #[test]
    fn parse_param_segment() {
        assert_eq!(
            p("/users/:id").segments,
            vec![
                Segment::Literal("users".to_string()),
                Segment::Param("id".to_string()),
            ]
        );
    }

    #[test]
    fn parse_wildcard_segment() {
        assert_eq!(
            p("/files/*").segments,
            vec![Segment::Literal("files".to_string()), Segment::Wildcard,]
        );
    }

    #[test]
    fn parse_normalizes_redundant_slashes() {
        assert_eq!(p("//users//:id//").segments.len(), 2);
    }

    #[test]
    fn display_round_trips_simple_path() {
        assert_eq!(p("/users/:id").to_string(), "/users/:id");
        assert_eq!(p("/").to_string(), "/");
    }

    #[test]
    fn overlap_identical_literals_is_ambiguous() {
        // Same path declared twice — the most direct kind of conflict.
        assert_eq!(
            p("/users/me").overlap_with(&p("/users/me")),
            Overlap::Ambiguous
        );
    }

    #[test]
    fn overlap_disjoint_literals_is_none() {
        assert_eq!(p("/users").overlap_with(&p("/posts")), Overlap::None);
        assert_eq!(p("/users/me").overlap_with(&p("/users/all")), Overlap::None);
    }

    #[test]
    fn overlap_literal_shadows_param() {
        // /users/me is more specific than /users/:id at position 1.
        assert_eq!(
            p("/users/me").overlap_with(&p("/users/:id")),
            Overlap::Shadowed
        );
        // Symmetric.
        assert_eq!(
            p("/users/:id").overlap_with(&p("/users/me")),
            Overlap::Shadowed
        );
    }

    #[test]
    fn overlap_two_param_routes_is_ambiguous() {
        assert_eq!(p("/:a/:b").overlap_with(&p("/:x/:y")), Overlap::Ambiguous);
    }

    #[test]
    fn overlap_param_in_different_position_does_not_save_disjoint_literal() {
        // /users/:id vs /posts/:id share zero concrete paths because users != posts.
        assert_eq!(
            p("/users/:id").overlap_with(&p("/posts/:id")),
            Overlap::None
        );
    }

    #[test]
    fn overlap_length_mismatch_without_wildcard_is_none() {
        assert_eq!(p("/users").overlap_with(&p("/users/me")), Overlap::None);
        assert_eq!(p("/").overlap_with(&p("/users")), Overlap::None);
    }

    #[test]
    fn overlap_wildcard_absorbs_trailing_segments() {
        // /files/* shadows /files/readme.md
        assert_eq!(
            p("/files/*").overlap_with(&p("/files/readme.md")),
            Overlap::Shadowed
        );
        // /files/* shadows /files/:name
        assert_eq!(
            p("/files/*").overlap_with(&p("/files/:name")),
            Overlap::Shadowed
        );
    }

    #[test]
    fn overlap_two_wildcards_is_ambiguous() {
        assert_eq!(p("/*").overlap_with(&p("/*")), Overlap::Ambiguous);
    }

    #[test]
    fn overlap_root_with_root_is_ambiguous() {
        assert_eq!(p("/").overlap_with(&p("")), Overlap::Ambiguous);
    }
}
