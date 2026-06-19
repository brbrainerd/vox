use crate::attention::InterruptionDecision;
use crate::feedback::types::Surface;

pub fn surface_for(d: &InterruptionDecision) -> Surface {
    match d {
        InterruptionDecision::InterruptNow { .. }
        | InterruptionDecision::RequireHumanBeforeContinue { .. } => Surface::NeedsYou,
        _ => Surface::Withheld,
    }
}

pub fn scaled_cost_of(d: &InterruptionDecision) -> u64 {
    match d {
        InterruptionDecision::InterruptNow { scaled_cost_ms, .. }
        | InterruptionDecision::RequireHumanBeforeContinue { scaled_cost_ms, .. } => *scaled_cost_ms,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::InterruptionDecision as D;
    use crate::feedback::Surface;

    #[test]
    fn now_and_require_human_are_needs_you() {
        assert_eq!(
            surface_for(&D::InterruptNow {
                reason: "x".into(),
                scaled_cost_ms: 123
            }),
            Surface::NeedsYou
        );
        assert_eq!(
            surface_for(&D::RequireHumanBeforeContinue {
                reason: "x".into(),
                scaled_cost_ms: 456
            }),
            Surface::NeedsYou
        );
    }

    #[test]
    fn defer_batch_proceed_are_withheld() {
        assert_eq!(
            surface_for(&D::DeferUntilCheckpoint { reason: "x".into() }),
            Surface::Withheld
        );
        assert_eq!(
            surface_for(&D::BatchWithExistingPrompt { reason: "x".into() }),
            Surface::Withheld
        );
        assert_eq!(
            surface_for(&D::ProceedAutonomously { reason: "x".into() }),
            Surface::Withheld
        );
    }

    #[test]
    fn gets_scaled_cost_correctly() {
        assert_eq!(
            scaled_cost_of(&D::InterruptNow {
                reason: "x".into(),
                scaled_cost_ms: 123
            }),
            123
        );
        assert_eq!(
            scaled_cost_of(&D::RequireHumanBeforeContinue {
                reason: "x".into(),
                scaled_cost_ms: 456
            }),
            456
        );
        assert_eq!(
            scaled_cost_of(&D::DeferUntilCheckpoint { reason: "x".into() }),
            0
        );
    }
}
