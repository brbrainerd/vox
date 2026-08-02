//! Budget enforcement guard, run before LLM dispatch. Warn-then-block on
//! `VoxConfig`'s `daily_budget_usd`/`per_session_budget_usd` caps, using the
//! recorded-spend SSOT (`VoxDb::llm_spend_summary`) — not a new spend tracker.

use vox_db::LlmSpendSummary;

/// Which cap tripped, for user-facing messaging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetScope {
    Daily,
    Session,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum BudgetGuardError {
    #[error("{scope:?} budget of ${cap_usd:.2} exceeded (spent ${spent_usd:.2})")]
    Exceeded {
        scope: BudgetScope,
        cap_usd: f64,
        spent_usd: f64,
    },
}

/// Non-blocking warning surfaced at the configured threshold, before the cap itself blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetWarning {
    pub scope: BudgetScope,
    pub cap_usd: f64,
    pub spent_usd: f64,
}

/// Check `spend` against `daily_budget_usd`/`per_session_budget_usd` and
/// `budget_warn_threshold_pct`. Returns `Ok(Some(warning))` at the warn
/// threshold, `Ok(None)` under it, `Err(Exceeded)` at or over either cap.
/// Daily is checked before session (arbitrary but deterministic ordering —
/// callers only need to know *that* a cap tripped, not which one first, since
/// both block dispatch identically).
pub fn check(
    spend: &LlmSpendSummary,
    daily_budget_usd: f64,
    per_session_budget_usd: f64,
    warn_threshold_pct: f32,
) -> Result<Option<BudgetWarning>, BudgetGuardError> {
    if spend.day_usd >= daily_budget_usd {
        return Err(BudgetGuardError::Exceeded {
            scope: BudgetScope::Daily,
            cap_usd: daily_budget_usd,
            spent_usd: spend.day_usd,
        });
    }
    if spend.session_usd >= per_session_budget_usd {
        return Err(BudgetGuardError::Exceeded {
            scope: BudgetScope::Session,
            cap_usd: per_session_budget_usd,
            spent_usd: spend.session_usd,
        });
    }

    // `warn_threshold_pct` round-trips through f32, which cannot represent most decimal
    // fractions (e.g. 0.8) exactly — `f64::from(0.8f32)` is ~0.800000012. Multiplied
    // against the cap, that lands a few ULPs above the "true" threshold and can miss an
    // exact-cent spend value that should trip the warning. Tolerate drift proportional to
    // the cap itself (worst case for an f32 fraction is ~1.2e-7 relative; 1e-6 leaves two
    // orders of magnitude of margin) rather than comparing bit-for-bit.
    let warn_threshold = f64::from(warn_threshold_pct);
    let warn_at_daily = daily_budget_usd * warn_threshold;
    if spend.day_usd >= warn_at_daily - daily_budget_usd.abs() * 1e-6 {
        return Ok(Some(BudgetWarning {
            scope: BudgetScope::Daily,
            cap_usd: daily_budget_usd,
            spent_usd: spend.day_usd,
        }));
    }
    let warn_at_session = per_session_budget_usd * warn_threshold;
    if spend.session_usd >= warn_at_session - per_session_budget_usd.abs() * 1e-6 {
        return Ok(Some(BudgetWarning {
            scope: BudgetScope::Session,
            cap_usd: per_session_budget_usd,
            spent_usd: spend.session_usd,
        }));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spend(day: f64, session: f64) -> LlmSpendSummary {
        LlmSpendSummary {
            total_usd: day,
            day_usd: day,
            session_usd: session,
        }
    }

    #[test]
    fn under_threshold_returns_none() {
        let result = check(&spend(1.0, 0.2), 5.0, 1.0, 0.8);
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn at_warn_threshold_returns_warning() {
        let result = check(&spend(4.0, 0.2), 5.0, 1.0, 0.8);
        assert_eq!(
            result,
            Ok(Some(BudgetWarning {
                scope: BudgetScope::Daily,
                cap_usd: 5.0,
                spent_usd: 4.0,
            }))
        );
    }

    #[test]
    fn at_daily_cap_returns_exceeded() {
        let result = check(&spend(5.0, 0.2), 5.0, 1.0, 0.8);
        assert_eq!(
            result,
            Err(BudgetGuardError::Exceeded {
                scope: BudgetScope::Daily,
                cap_usd: 5.0,
                spent_usd: 5.0,
            })
        );
    }

    #[test]
    fn at_session_cap_returns_exceeded_even_if_daily_ok() {
        let result = check(&spend(1.0, 1.0), 5.0, 1.0, 0.8);
        assert_eq!(
            result,
            Err(BudgetGuardError::Exceeded {
                scope: BudgetScope::Session,
                cap_usd: 1.0,
                spent_usd: 1.0,
            })
        );
    }

    #[test]
    fn warn_threshold_of_one_disables_warning() {
        // At 1.0, "warn at" == the cap itself, so Exceeded fires first (cap check runs before warn check).
        let result = check(&spend(5.0, 0.2), 5.0, 1.0, 1.0);
        assert!(matches!(
            result,
            Err(BudgetGuardError::Exceeded {
                scope: BudgetScope::Daily,
                ..
            })
        ));
    }
}
