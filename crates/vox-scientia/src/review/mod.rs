//! Human-gated discovery review — pure logic layer (P2 Task 2).
//!
//! # Security contract
//!
//! [`ApprovalToken`] is the **type-level gate**: no SCIENTIA artifact may be
//! emitted without one, and the *only* way to obtain a token is via
//! [`mint_from_decision`] with a decision whose status is `"approved"`.
//!
//! `ApprovalToken` has **all-private fields** — there is no public constructor
//! and no `Default` impl.  Attempts to construct one outside this module are a
//! **compile error** (`compile-fence:`).  This is intentional and
//! security-critical; do not add a `pub` constructor.
//!
//! # State machine
//!
//! ```text
//! Surfaced ──StartReview──► UnderReview ──Approve──► Approved
//!                                       ──Reject───► Rejected
//!                                       ──Defer────► Deferred ──(re-surface)─► Surfaced
//!                                       ──Edit─────► Edited ───(re-surface)──► Surfaced
//! ```
//!
//! Invalid transitions return the current state unchanged (see [`next_state`]).

use vox_db::store::ReviewDecisionRow;

// ── Trait for input decoupling ─────────────────────────────────────────────

/// Sealed so only types in this crate can implement [`ReviewDecisionLike`].
/// Without this, an external crate could implement the trait on a fabricated
/// type returning `"approved"` and mint an [`ApprovalToken`] out of thin air,
/// bypassing the audited DB-decision intent. Only the real persisted decision
/// row (and the in-crate test fake) may carry approval semantics.
mod sealed {
    pub trait Sealed {}
    impl Sealed for super::ReviewDecisionRow {}
}

/// Abstraction over any type that carries enough data to mint an
/// [`ApprovalToken`].  Implemented here for [`vox_db::store::ReviewDecisionRow`];
/// unit tests use a tiny in-module fake. Sealed (see `sealed`) — cannot be
/// implemented outside this crate.
pub trait ReviewDecisionLike: sealed::Sealed {
    fn claim_id(&self) -> i64;
    fn publication_id(&self) -> &str;
    fn bound_digest(&self) -> &str;
    /// One of: `approved | rejected | deferred | edited`.
    fn decision(&self) -> &str;
}

impl ReviewDecisionLike for ReviewDecisionRow {
    fn claim_id(&self) -> i64 {
        self.claim_id
    }

    fn publication_id(&self) -> &str {
        &self.publication_id
    }

    fn bound_digest(&self) -> &str {
        &self.bound_digest
    }

    fn decision(&self) -> &str {
        &self.decision
    }
}

// ── ApprovalToken ──────────────────────────────────────────────────────────

/// A cryptographic proof that a specific claim was **approved** by a human
/// reviewer.
///
/// # compile-fence:
///
/// All fields are private; there is no `pub` constructor and no `Default`.
/// Struct-literal construction outside this module is a compile error:
///
/// ```compile_fail
/// let _ = vox_scientia::review::ApprovalToken { claim_id: 1, bound_digest: String::new() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalToken {
    // `i64` (not the spec's `u64`) is deliberate: it matches
    // `vox_db::store::ReviewDecisionRow::claim_id` and the libSQL `INTEGER`
    // column it is minted from (libSQL integers are always signed). Keeping the
    // token's type identical to the row's avoids a cast at every mint/compare site.
    claim_id: i64,
    publication_id: String,
    bound_digest: String,
}

impl ApprovalToken {
    /// Private constructor — only callable from within this module.
    fn new(claim_id: i64, publication_id: String, bound_digest: String) -> Self {
        Self {
            claim_id,
            publication_id,
            bound_digest,
        }
    }

    /// The claim that was approved.
    pub fn claim_id(&self) -> i64 {
        self.claim_id
    }

    /// The publication the approval is scoped to; `nanopub_build` rejects a token
    /// whose `publication_id` differs from the build's, preventing
    /// cross-publication replay.
    pub fn publication_id(&self) -> &str {
        &self.publication_id
    }

    /// The digest bound at approval time (expected to be the publication's
    /// SHA3-256 `content_sha3_256`, but **not validated by this module** — the
    /// token trusts the caller). Binds the token to the exact artifact version
    /// reviewed by the human; Task 3 rejects a token whose digest no longer
    /// matches the current manifest, so an edit invalidates a stale approval.
    pub fn bound_digest(&self) -> &str {
        &self.bound_digest
    }
}

// ── mint_from_decision ─────────────────────────────────────────────────────

/// Returns `Some(token)` **only** when `d.decision() == "approved"`;
/// returns `None` for `rejected`, `deferred`, `edited`, or any other value.
///
/// This is the **sole construction path** for [`ApprovalToken`].
pub fn mint_from_decision<D: ReviewDecisionLike>(d: &D) -> Option<ApprovalToken> {
    if d.decision() == "approved" {
        Some(ApprovalToken::new(
            d.claim_id(),
            d.publication_id().to_string(),
            d.bound_digest().to_string(),
        ))
    } else {
        None
    }
}

// ── State machine ──────────────────────────────────────────────────────────

/// The lifecycle state of a nanopublication claim in the review pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    /// Claim has been extracted and queued; awaiting a reviewer.
    Surfaced,
    /// A reviewer has opened the claim but has not yet decided.
    UnderReview,
    /// A human has approved the claim for publication.
    Approved,
    /// A human has rejected the claim; it will not be published.
    Rejected,
    /// Decision deferred; the claim remains in the queue for later review.
    Deferred,
    /// The claim content was edited; it will be re-surfaced for fresh review.
    Edited,
}

/// Actions that drive transitions in the review state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
    /// A reviewer opens/claims the item.
    StartReview,
    /// The reviewer approves the claim.
    Approve,
    /// The reviewer rejects the claim.
    Reject,
    /// The reviewer defers the decision.
    Defer,
    /// The claim content is edited (triggers re-surfacing on the *next* call
    /// via the `Edited → Surfaced` transition).
    Edit,
    /// An edited claim is re-queued for fresh review.
    ReSurface,
}

/// Pure state-machine transition function.
///
/// Invalid transitions (e.g. `Surfaced + Approve`) return `current` unchanged.
///
/// Valid transitions:
/// ```text
/// Surfaced    + StartReview → UnderReview
/// UnderReview + Approve     → Approved
/// UnderReview + Reject      → Rejected
/// UnderReview + Defer       → Deferred
/// UnderReview + Edit        → Edited
/// Edited      + ReSurface   → Surfaced
/// Deferred    + ReSurface   → Surfaced
/// ```
///
/// `Approved` and `Rejected` are terminal (no outgoing transition). `Deferred`
/// and `Edited` are *recoverable* — both re-enter the queue via `ReSurface`.
pub fn next_state(current: ReviewState, action: ReviewAction) -> ReviewState {
    match (current, action) {
        (ReviewState::Surfaced, ReviewAction::StartReview) => ReviewState::UnderReview,
        (ReviewState::UnderReview, ReviewAction::Approve) => ReviewState::Approved,
        (ReviewState::UnderReview, ReviewAction::Reject) => ReviewState::Rejected,
        (ReviewState::UnderReview, ReviewAction::Defer) => ReviewState::Deferred,
        (ReviewState::UnderReview, ReviewAction::Edit) => ReviewState::Edited,
        // Recoverable states re-enter the queue for a fresh pass.
        (ReviewState::Edited, ReviewAction::ReSurface) => ReviewState::Surfaced,
        (ReviewState::Deferred, ReviewAction::ReSurface) => ReviewState::Surfaced,
        // All other (state, action) pairs are invalid; return current unchanged.
        (state, _) => state,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Fake decision for unit tests (no DB required) ──────────────────────

    struct FakeDecision {
        claim_id: i64,
        publication_id: &'static str,
        bound_digest: &'static str,
        decision: &'static str,
    }

    // The in-crate test fake is allowed to carry the sealed trait.
    impl super::sealed::Sealed for FakeDecision {}
    impl ReviewDecisionLike for FakeDecision {
        fn claim_id(&self) -> i64 {
            self.claim_id
        }
        fn publication_id(&self) -> &str {
            self.publication_id
        }
        fn bound_digest(&self) -> &str {
            self.bound_digest
        }
        fn decision(&self) -> &str {
            self.decision
        }
    }

    // ── mint_from_decision ─────────────────────────────────────────────────

    #[test]
    fn mint_returns_some_for_approved() {
        let d = FakeDecision {
            claim_id: 7,
            publication_id: "pub-test",
            bound_digest: "sha3abc",
            decision: "approved",
        };
        let token = mint_from_decision(&d).expect("approved must yield Some");
        assert_eq!(token.claim_id(), 7);
        assert_eq!(token.publication_id(), "pub-test");
        assert_eq!(token.bound_digest(), "sha3abc");
    }

    #[test]
    fn mint_carries_publication_id_into_token() {
        let d = FakeDecision {
            claim_id: 5,
            publication_id: "pub-carry",
            bound_digest: "dig",
            decision: "approved",
        };
        let token = mint_from_decision(&d).expect("approved must yield Some");
        assert_eq!(
            token.publication_id(),
            "pub-carry",
            "the decision's publication_id must flow into the token"
        );
    }

    #[test]
    fn mint_returns_none_for_rejected() {
        let d = FakeDecision {
            claim_id: 1,
            publication_id: "pub-test",
            bound_digest: "d",
            decision: "rejected",
        };
        assert!(mint_from_decision(&d).is_none(), "rejected must yield None");
    }

    #[test]
    fn mint_returns_none_for_deferred() {
        let d = FakeDecision {
            claim_id: 1,
            publication_id: "pub-test",
            bound_digest: "d",
            decision: "deferred",
        };
        assert!(mint_from_decision(&d).is_none(), "deferred must yield None");
    }

    #[test]
    fn mint_returns_none_for_edited() {
        let d = FakeDecision {
            claim_id: 1,
            publication_id: "pub-test",
            bound_digest: "d",
            decision: "edited",
        };
        assert!(mint_from_decision(&d).is_none(), "edited must yield None");
    }

    #[test]
    fn mint_returns_none_for_unknown_decision() {
        let d = FakeDecision {
            claim_id: 1,
            publication_id: "pub-test",
            bound_digest: "d",
            decision: "maybe",
        };
        assert!(
            mint_from_decision(&d).is_none(),
            "unknown decision must yield None"
        );
    }

    #[test]
    fn token_accessors_carry_decision_values() {
        let d = FakeDecision {
            claim_id: 42,
            publication_id: "pub-test",
            bound_digest: "deadbeef",
            decision: "approved",
        };
        let token = mint_from_decision(&d).unwrap();
        assert_eq!(token.claim_id(), 42);
        assert_eq!(token.bound_digest(), "deadbeef");
    }

    // ── ReviewDecisionLike impl for ReviewDecisionRow (smoke-test) ─────────

    #[test]
    fn review_decision_row_impl_reviewdecisionlike() {
        let row = vox_db::store::ReviewDecisionRow {
            claim_id: 99,
            publication_id: "pub-smoke".into(),
            bound_digest: "row-digest".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1_000,
        };
        // Verify the trait impl routes correctly.
        assert_eq!(ReviewDecisionLike::claim_id(&row), 99);
        assert_eq!(ReviewDecisionLike::bound_digest(&row), "row-digest");
        assert_eq!(ReviewDecisionLike::decision(&row), "approved");

        let token = mint_from_decision(&row).expect("approved row must mint token");
        assert_eq!(token.claim_id(), 99);
        assert_eq!(token.bound_digest(), "row-digest");
    }

    // ── State machine — valid transitions ──────────────────────────────────

    #[test]
    fn surfaced_plus_start_review_gives_under_review() {
        assert_eq!(
            next_state(ReviewState::Surfaced, ReviewAction::StartReview),
            ReviewState::UnderReview
        );
    }

    #[test]
    fn under_review_plus_approve_gives_approved() {
        assert_eq!(
            next_state(ReviewState::UnderReview, ReviewAction::Approve),
            ReviewState::Approved
        );
    }

    #[test]
    fn under_review_plus_reject_gives_rejected() {
        assert_eq!(
            next_state(ReviewState::UnderReview, ReviewAction::Reject),
            ReviewState::Rejected
        );
    }

    #[test]
    fn under_review_plus_defer_gives_deferred() {
        assert_eq!(
            next_state(ReviewState::UnderReview, ReviewAction::Defer),
            ReviewState::Deferred
        );
    }

    #[test]
    fn under_review_plus_edit_gives_edited() {
        assert_eq!(
            next_state(ReviewState::UnderReview, ReviewAction::Edit),
            ReviewState::Edited
        );
    }

    #[test]
    fn edited_plus_resurface_gives_surfaced() {
        assert_eq!(
            next_state(ReviewState::Edited, ReviewAction::ReSurface),
            ReviewState::Surfaced
        );
    }

    #[test]
    fn deferred_plus_resurface_gives_surfaced() {
        // Deferred is recoverable: an explicit ReSurface re-enters the queue.
        assert_eq!(
            next_state(ReviewState::Deferred, ReviewAction::ReSurface),
            ReviewState::Surfaced
        );
    }

    // ── State machine — invalid transitions (return current unchanged) ─────

    #[test]
    fn surfaced_cannot_be_approved_directly() {
        // Must go through UnderReview first.
        assert_eq!(
            next_state(ReviewState::Surfaced, ReviewAction::Approve),
            ReviewState::Surfaced,
            "Surfaced + Approve must be a no-op"
        );
    }

    #[test]
    fn surfaced_cannot_be_rejected_directly() {
        assert_eq!(
            next_state(ReviewState::Surfaced, ReviewAction::Reject),
            ReviewState::Surfaced
        );
    }

    #[test]
    fn surfaced_cannot_be_deferred_directly() {
        assert_eq!(
            next_state(ReviewState::Surfaced, ReviewAction::Defer),
            ReviewState::Surfaced
        );
    }

    #[test]
    fn approved_is_terminal_start_review_is_noop() {
        assert_eq!(
            next_state(ReviewState::Approved, ReviewAction::StartReview),
            ReviewState::Approved
        );
    }

    #[test]
    fn rejected_is_terminal_approve_is_noop() {
        assert_eq!(
            next_state(ReviewState::Rejected, ReviewAction::Approve),
            ReviewState::Rejected
        );
    }

    #[test]
    fn deferred_does_not_transition_on_approve() {
        // Deferred items must be re-surfaced by an explicit workflow step,
        // not approved directly.
        assert_eq!(
            next_state(ReviewState::Deferred, ReviewAction::Approve),
            ReviewState::Deferred
        );
    }

    #[test]
    fn edited_cannot_be_approved_without_resurface_first() {
        assert_eq!(
            next_state(ReviewState::Edited, ReviewAction::Approve),
            ReviewState::Edited
        );
    }
}
