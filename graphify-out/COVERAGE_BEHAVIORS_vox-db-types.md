# Semantic Behavior Map — vox-db-types

Five extracted Behavior claims resolve to **2 distinct symbols** in two files (`exec_time.rs`, `ids.rs`). Both are serialization-surface types. Coverage is dominated by happy-path serialization: every proven behavior asserts correct output for well-formed input, and only one claim (`DbAgentId` round-trip) rises to an invariant. No symbol has an error-path proof for malformed, empty, or unknown input — so every type here is proven only on inputs that already conform to its contract.

## ExecOutcome
- **File:** `crates/vox-db-types/src/exec_time.rs`
- **Proven behaviors (happy path):**
  - `ExecOutcome::Success.as_str()` returns `"success"`
  - `ExecOutcome::Timeout.as_str()` returns `"timeout"`
  - `ExecOutcome::Error.as_str()` returns `"error"`
- **Error-path proof:** none
- **Edge/invariant proof:** none (all 3 enumerate the variant→string mapping; no round-trip, no parse-back, no unknown-input case)
- **Notes:** All three claims share test `exec_outcome_serialization` and are the full variant set. Distinct, not dedup-able, but they form a single "string encoding is total over variants" assertion. The *decode* direction is entirely unproven.

## DbAgentId
- **File:** `crates/vox-db-types/src/ids.rs`
- **Proven behaviors:**
  - (happy) Serializes value `"agent-42"` to JSON string `"\"agent-42\""`
  - (invariant) Deserializes that JSON string back to an equal instance — round-trip identity holds
- **Error-path proof:** none
- **Edge/invariant proof:** has the round-trip invariant; no edge cases (empty, malformed, non-string JSON, wrong-type) proven
- **Notes:** Both claims share test `round_trips_through_serde_json`. The round-trip is a genuine invariant but is exercised on a single well-formed sample only.

## Semantic gaps

Symbols proven only on the happy path whose contract has an obvious failure/unknown mode:

1. **`ExecOutcome` decode direction (`exec_time.rs`)** — *most actionable.* The encode side is total over all three variants, but there is no test for parsing/deserializing an outcome *from* a string. An unknown or malformed outcome value (e.g. `"cancelled"`, `""`) has no proven behavior — the rejection or fallback mode is unverified. This is an enum with a clear "unknown variant" failure mode and zero negative coverage.

2. **`DbAgentId` malformed-input rejection (`ids.rs`)** — an ID newtype that round-trips a well-formed string but has no proof for empty, non-string, or otherwise malformed JSON. ID types are classic validator surfaces; the absence of any negative/edge test means the deserialize path's behavior on bad input is unspecified by the suite.

No mutators, integrity, or security surfaces appear in this claim set — both gaps are on serialization/deserialization validators lacking a rejection test.