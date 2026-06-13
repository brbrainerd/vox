# Semantic Behavior Map — `vox-scaling-policy`

Deterministically synthesized from 13 distinct proven-behavior claims (of 13 extracted) across 3 symbols. 1 symbols have an explicit error-path proof; **0 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `CostCircuitBreaker::check_before_task`  (edge, error, happy; EXTRACTED)
- [happy] When a tenant has spent USD at or above their configured daily budget cap, check_before_task returns a CostDefenseRejection::TenantBudgetExhausted rejection.  (crates/vox-scaling-policy/src/cost_defense.rs)
- [happy] Rejects tasks with estimated_duration_secs exceeding per_task_timeout_secs limit with TaskTimeout rejection  (crates/vox-scaling-policy/src/cost_defense.rs)
- [happy] Does not reject tasks with estimated_duration_secs within per_task_timeout_secs limit  (crates/vox-scaling-policy/src/cost_defense.rs)
- [error] Rejects tasks with retry attempts >= max_retries_per_task_day with RetryLimitExceeded rejection  (crates/vox-scaling-policy/src/cost_defense.rs)
- [error] Rejects tasks when projected daily_spent_usd plus estimated_cost_usd exceeds daily_budget_usd with DailyBudgetExhausted rejection  (crates/vox-scaling-policy/src/cost_defense.rs)
- [happy] Does not reject tasks when projected daily spend remains within daily_budget_usd limit  (crates/vox-scaling-policy/src/cost_defense.rs)
- [error] Rejects tasks with requested_model_tier not in allowed_model_tiers when model_pinning_enabled is true with ModelNotPinned rejection  (crates/vox-scaling-policy/src/cost_defense.rs)
- [happy] Does not reject tasks with requested_model_tier in allowed_model_tiers when model_pinning_enabled is true  (crates/vox-scaling-policy/src/cost_defense.rs)
- [edge] Includes MonthlyPacingWarning rejection when projected monthly_spent_usd exceeds monthly_budget_usd multiplied by monthly_pacing_warn_pct  (crates/vox-scaling-policy/src/cost_defense.rs)
- [happy] Returns empty rejection list for tasks within timeout, budget, retry, model pinning, and monthly pacing constraints  (crates/vox-scaling-policy/src/cost_defense.rs)

### `CostCircuitBreaker::has_hard_block`  (invariant; EXTRACTED)
- [invariant] Returns false when rejection list contains only MonthlyPacingWarning  (crates/vox-scaling-policy/src/cost_defense.rs)
- [invariant] Returns true when rejection list contains DailyBudgetExhausted  (crates/vox-scaling-policy/src/cost_defense.rs)

### `CostDefenseConfig::default`  (invariant; EXTRACTED)
- [invariant] Initializes per_task_timeout_secs to 300, max_retries_per_task_day to 3, daily_budget_usd to 25.0, model_pinning_enabled to true, monthly_pacing_warn_pct to 0.80, and allowed_model_tiers with 3 entries  (crates/vox-scaling-policy/src/cost_defense.rs)

## Semantic gaps (proven happy-path only)

_None — every proven symbol has at least one error/edge/invariant claim._
