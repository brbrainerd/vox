# Semantic Behavior Map — vox-skill-runtime

Synthesized from 16 extracted Behavior claims (3 test files: `src/detect.rs`, `tests/microvm_tier.rs`). After deduplication the claims cover 5 distinct symbols. Coverage is strongest on the data-model layer (Tier ordering invariants, FromStr parsing with a rejection path) and weakest on the selection/planning layer: both `detect_choice()` and `plan_for_min_tier()` are exercised only on inputs that resolve to the always-available Wasm runtime, so no test proves alternate-runtime selection or the documented failure/unsatisfiable modes. The result is a runtime selector whose decision logic is verified only where it trivially succeeds.

## RuntimePreference (`src/detect.rs`)
Proven behaviors (test: `runtime_preference_from_str`):
- FromStr parses `"wasm"` → `RuntimePreference::Wasm` (happy)
- FromStr parses `"auto"` → `RuntimePreference::Auto` (happy)
- FromStr parses `"docker"` → `RuntimePreference::Docker` (happy)
- FromStr parses `"podman"` → `RuntimePreference::Podman` (happy)
- FromStr rejects `"invalid"` with an error (error path)

Error path: YES. Edge/invariant: partial (one rejection case). This is the best-covered symbol — it is a validator and it has a rejection test.

## detect_choice() (`src/detect.rs`)
Proven behaviors:
- `Wasm` preference → `Ok(RuntimeChoice::Wasm)` (happy; `wasm_choice_always_available`)
- `Auto` preference → `Ok(RuntimeChoice::Wasm)` (happy; `auto_prefers_wasm`)

Error path: NO. Edge/invariant: NO. Only the two inputs that resolve to the always-available Wasm runtime are tested.

## plan_for_min_tier() (`tests/microvm_tier.rs`)
Proven behaviors:
- `Tier::Wasm` → `Ok`, unwraps to `RuntimeChoice::Wasm` (happy; `plan_for_min_tier_wasm_always_works`)
- `Tier::BareMetal` → `Ok`, unwraps to `RuntimeChoice::Wasm` (happy; `plan_for_min_tier_baremetal_returns_wasm`)

Error path: NO. Edge/invariant: NO. Both proven inputs are min-tiers satisfiable by Wasm. Higher tiers (Container, MicroVm) and the unsatisfiable case are untested despite the `Result` return type implying a failure mode.

## RuntimeChoice (`src/detect.rs`, `tests/microvm_tier.rs`)
Proven behaviors:
- `RuntimeChoice::Wasm.name()` returns `"wasm"` (happy)
- Used as the unwrapped result of `plan_for_min_tier(Wasm)` and `plan_for_min_tier(BareMetal)` (happy)

Error path: N/A. Edge/invariant: NO. Only the Wasm variant is observed; Docker/Podman/MicroVm names and construction are unproven.

## Tier (`tests/microvm_tier.rs`)
Proven behaviors:
- `MicroVmRuntime` with name `"kata"` → `tier()` == `Tier::MicroVm` (happy; `microvm_runtime_tier`)
- Ordering invariants (`tier_ordering`): `BareMetal < Wasm < Container < MicroVm`

Error path: N/A. Edge/invariant: YES (transitive ordering chain proven). Strongest invariant coverage in the crate; the only gap is unknown/empty runtime-name → tier mapping.

## Semantic gaps

Symbols proven only on the happy path whose contract clearly has a failure, empty, or conflict mode:

1. **`plan_for_min_tier()` — planner with no rejection test.** Returns `Result`, implying an unsatisfiable case (no available runtime meets the requested minimum tier). Only `Wasm` and `BareMetal` are tested, both trivially satisfied by the always-available Wasm runtime. No test forces `Tier::Container` or `Tier::MicroVm`, and none drives the `Err` path. This is the most actionable gap: a tier-selection function whose entire reason for returning `Result` is unverified.

2. **`detect_choice()` — selector with no alternate-runtime or unavailable path.** Proven only to return Wasm for `Wasm` and `Auto`. Docker/Podman/MicroVm selection and the failure-when-preferred-runtime-unavailable path (e.g. `Docker` preference with no docker present) are untested. A selection mutator whose non-default branches and conflict handling are entirely unproven.

3. **`RuntimeChoice` — only the Wasm variant observed.** `name()` and construction for Docker/Podman/MicroVm have no coverage; correctness of their string identity (a surface used for matching/routing) is unverified.

4. **`Tier` mapping — no edge proof for unknown runtime names.** `tier()` is proven only for the `"kata"` MicroVm case; behavior for unknown/empty runtime names (default tier? error?) is untested.