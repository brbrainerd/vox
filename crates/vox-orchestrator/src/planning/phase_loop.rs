//! Pure Plan/Act/Verify phase machine. Planning is entered only when the clutch
//! warrants plan-first; Verifying is forced by Low risk and skippable under High.
//!
//! NOTE: `TaskPhase` already exists in `types/tasks.rs` for the OOPAV debug
//! phases (`Inspect/Localize/…`). This module uses **`PavPhase`** for the
//! high-level Plan/Act/Verify loop to avoid any name collision.

use crate::mode::{ClutchProfile, RiskPosture};
use serde::{Deserialize, Serialize};

/// High-level PAV loop phase. Distinct from `crate::types::TaskPhase` (OOPAV).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PavPhase {
    /// Generating a structured plan before execution.
    Planning,
    /// Executing the plan (or reacting directly).
    Acting,
    /// Verifying the results.
    Verifying,
    /// All phases complete.
    Done,
}

/// Serializable snapshot of a task's PAV loop, stored on `AgentTask`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PavLoopState {
    pub phase: PavPhase,
    pub verify_required: bool,
    pub verify_skipped: bool,
}

/// Logic wrapper over `PavLoopState`. Constructed at task enqueue time and
/// stored (as `pav_loop: Option<PavLoopState>`) on the task.
pub struct PhaseLoop {
    state: PavLoopState,
}

impl PhaseLoop {
    /// Build a new loop from the user's clutch + risk settings.
    ///
    /// - Plan-first for `Balanced`/`Genius`; act-first for `Free`/`Efficiency`.
    /// - `verify_required` is `true` when `Low` risk forces Socrates enforcement
    ///   OR when `Genius` clutch is selected (always validate premium work).
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: PavLoopState {
                phase: PavPhase::Planning,
                verify_required: true,
                verify_skipped: false,
            },
        }
    }

    /// Build a new loop driven by clutch + risk.
    #[must_use]
    pub fn start(clutch: ClutchProfile, risk: RiskPosture) -> Self {
        let plan_first = matches!(clutch, ClutchProfile::Balanced | ClutchProfile::Genius);
        let verify_required = risk.resolve().socrates_enforce
            || matches!(clutch, ClutchProfile::Genius);
        Self {
            state: PavLoopState {
                phase: if plan_first {
                    PavPhase::Planning
                } else {
                    PavPhase::Acting
                },
                verify_required,
                verify_skipped: false,
            },
        }
    }

    /// Reconstruct a `PhaseLoop` from a persisted `PavLoopState`.
    pub fn from_state(state: PavLoopState) -> Self {
        Self { state }
    }

    /// Current state snapshot (borrow).
    pub fn state(&self) -> &PavLoopState {
        &self.state
    }

    /// Consume and return the owned state (for storing back on the task).
    pub fn into_state(self) -> PavLoopState {
        self.state
    }

    /// Current phase.
    #[must_use]
    pub fn phase(&self) -> PavPhase {
        self.state.phase
    }

    /// Shortcut: advance Planning → Acting. No-op from any other phase (so a
    /// stray APPROVE_PLAN after acting/verifying cannot rewind the loop).
    pub fn advance_to_acting(&mut self) {
        if self.state.phase == PavPhase::Planning {
            self.state.phase = PavPhase::Acting;
        }
    }

    /// Shortcut: advance Acting → Verifying. No-op from any other phase.
    pub fn advance_to_verifying(&mut self) {
        if self.state.phase == PavPhase::Acting {
            self.state.phase = PavPhase::Verifying;
        }
    }

    /// Shortcut: complete (any → Done).
    pub fn complete(&mut self) {
        self.state.phase = PavPhase::Done;
    }

    /// User intervention: skip verification. Records the skip and completes the
    /// loop (→ Done) from either Acting or Verifying — the two phases from which
    /// "skip verify" is meaningful — so a one-shot intervention never strands the
    /// task mid-loop. No-op from Planning/Done.
    pub fn skip_verify(&mut self) {
        self.state.verify_skipped = true;
        if matches!(self.state.phase, PavPhase::Acting | PavPhase::Verifying) {
            self.state.phase = PavPhase::Done;
        }
    }

    /// Force verification (override any skip) and advance Acting → Verifying.
    pub fn force_verify(&mut self) {
        self.state.verify_required = true;
        self.state.verify_skipped = false;
        if self.state.phase == PavPhase::Acting {
            self.state.phase = PavPhase::Verifying;
        }
    }

    /// Advance to the next logical phase following the rules encoded at construction.
    pub fn advance(&mut self) {
        self.state.phase = match self.state.phase {
            PavPhase::Planning => PavPhase::Acting,
            PavPhase::Acting => {
                if self.state.verify_required && !self.state.verify_skipped {
                    PavPhase::Verifying
                } else {
                    PavPhase::Done
                }
            }
            PavPhase::Verifying => PavPhase::Done,
            PavPhase::Done => PavPhase::Done,
        };
    }
}

impl Default for PhaseLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::{ClutchProfile, RiskPosture};

    #[test]
    fn phase_loop_transitions_planning_to_acting() {
        let mut p = PhaseLoop::new();
        assert_eq!(p.phase(), PavPhase::Planning);
        p.advance_to_acting();
        assert_eq!(p.phase(), PavPhase::Acting);
    }

    #[test]
    fn genius_plans_then_acts_then_verifies() {
        let mut p = PhaseLoop::start(ClutchProfile::Genius, RiskPosture::Moderate);
        assert_eq!(p.phase(), PavPhase::Planning);
        p.advance();
        assert_eq!(p.phase(), PavPhase::Acting);
        p.advance();
        assert_eq!(p.phase(), PavPhase::Verifying);
        p.advance();
        assert_eq!(p.phase(), PavPhase::Done);
    }

    #[test]
    fn efficiency_reacts_skips_planning() {
        let p = PhaseLoop::start(ClutchProfile::Efficiency, RiskPosture::High);
        assert_eq!(p.phase(), PavPhase::Acting); // React: no upfront plan
    }

    #[test]
    fn low_risk_forces_verify_even_when_high_clutch_would_skip() {
        let mut p = PhaseLoop::start(ClutchProfile::Efficiency, RiskPosture::Low);
        // React → Acting, then Low risk forces Verifying (not Done)
        assert_eq!(p.phase(), PavPhase::Acting);
        p.advance();
        assert_eq!(p.phase(), PavPhase::Verifying);
    }

    #[test]
    fn high_risk_allows_skip_verify() {
        let mut p = PhaseLoop::start(ClutchProfile::Genius, RiskPosture::High);
        p.advance(); // Planning -> Acting
        p.skip_verify();
        p.advance();
        assert_eq!(p.phase(), PavPhase::Done);
    }

    #[test]
    fn skip_verify_from_acting_completes_without_a_following_advance() {
        // A one-shot SKIP_VERIFY intervention while Acting must complete the loop,
        // not strand the task in Acting waiting for a later advance().
        let mut p = PhaseLoop::start(ClutchProfile::Genius, RiskPosture::Moderate);
        p.advance(); // Planning -> Acting
        assert_eq!(p.phase(), PavPhase::Acting);
        p.skip_verify();
        assert_eq!(p.phase(), PavPhase::Done);
    }

    #[test]
    fn skip_verify_is_noop_from_planning() {
        let mut p = PhaseLoop::start(ClutchProfile::Genius, RiskPosture::Moderate);
        assert_eq!(p.phase(), PavPhase::Planning);
        p.skip_verify();
        assert_eq!(p.phase(), PavPhase::Planning); // cannot skip what hasn't started
    }

    #[test]
    fn force_verify_while_acting_goes_to_verifying() {
        let mut p = PhaseLoop::start(ClutchProfile::Free, RiskPosture::High);
        // Free/High: Acting, no verify required
        assert_eq!(p.phase(), PavPhase::Acting);
        p.force_verify();
        assert_eq!(p.phase(), PavPhase::Verifying);
    }

    #[test]
    fn pav_loop_state_roundtrips_serde() {
        let state = PavLoopState {
            phase: PavPhase::Acting,
            verify_required: true,
            verify_skipped: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: PavLoopState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.phase, PavPhase::Acting);
        assert!(back.verify_required);
    }
}
