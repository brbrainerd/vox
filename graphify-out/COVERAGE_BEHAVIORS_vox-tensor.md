# vox-tensor — Semantic Behavior Map

Synthesized from 29 extracted Behavior claims (deduped to ~21 distinct behaviors across 12 symbols). The crate's proven surface is the tokenizer and training-tensor preparation in `crates/vox-tensor/src/data.rs`, plus a JSONL data-loading layer and a replay buffer (`replay.rs`). Tokenization is well-covered with roundtrip and bound invariants; data loading and replay are almost entirely happy-path.

## VoxTokenizer::encode
`crates/vox-tensor/src/data.rs`, `tests/tokenizer_chatml_smoke.rs`
- Non-empty ASCII input yields a non-empty token sequence (at least one token). *(happy)*
- Multi-character compound tokens are matched as a single token, not split into chars. *(happy)*
- Non-ASCII UTF-8 bytes fall back to one `UNK_ID` token per byte. *(happy / edge-ish fallback)*
- All emitted token IDs stay within `VOCAB_SIZE`. *(invariant)*
- Error/edge: UNK fallback is the only failure-ish path; **no empty-input or vocab-overflow proof**.

## VoxTokenizer::decode
`crates/vox-tensor/src/data.rs`, `tests/tokenizer_chatml_smoke.rs`
- Faithfully reconstructs original text from encoded IDs (exact roundtrip). *(invariant)*
- Roundtrip holds for multi-role ChatML token streams (all role messages recovered). *(invariant)*
- Strong roundtrip coverage; no malformed/out-of-range token-ID handling proof.

## VoxTokenizer::encode_chatml
`crates/vox-tensor/src/data.rs`
- Encodes multi-role ChatML with all role contents preserved (verified via decode roundtrip). *(happy + invariant)*
- No edge proof for empty/unknown roles or malformed ChatML.

## VoxTokenizer::tokenize_for_training
`crates/vox-tensor/src/data.rs`, `tests/tokenizer_chatml_smoke.rs`
- Pads `input_ids` to exactly `max_len`. *(invariant)*
- Pads `labels` to exactly `max_len`. *(invariant)*
- Masks the prompt region with `-100` in labels. *(invariant)*
- Response region carries real (non-negative, non-PAD) positive supervision tokens. *(happy)*
- Well-covered for length and masking invariants; **no truncation/over-length proof** (input longer than `max_len`).

## COMPOUND_BASE
`crates/vox-tensor/src/data.rs`
- Compound token IDs are allocated sequentially starting after `COMPOUND_BASE`. *(invariant)*

## UNK_ID
`crates/vox-tensor/src/data.rs`
- Non-ASCII bytes encode to the `UNK_ID` token. *(happy)*

## JsonlDataLoader
`crates/vox-tensor/src/data.rs`
- Reads/parses JSONL with the correct record count. *(happy)*
- Extracts the `prompt` field from records. *(happy)*
- Filters out records with rating below threshold; retains records at/above threshold. *(happy)*
- Error/edge: **none** — no empty-file, missing-field, or malformed-record behavior proven.

## TrainingPair
`crates/vox-tensor/src/data.rs`
- Deserializes `instruction`/`output` JSON fields as `prompt`/`response`. *(happy)*
- Error/edge: **none** — no missing-field or alias-conflict proof.

## load_all
`crates/vox-tensor/src/data.rs`
- Reads and parses a JSONL file using instruction/response aliases. *(happy)*
- Error/edge: **none** despite delegating to policy-bearing loading.

## load_all_with_policy
`crates/vox-tensor/src/data.rs`
- Returns an `InvalidData` error on malformed JSON under the `FailFast` policy. *(error — the crate's only proven error path)*
- Edge: **no lenient/Skip-policy branch proof**.

## count_jsonl_records
`crates/vox-tensor/src/data.rs`
- Accurately counts lines in a JSONL file. *(happy)*
- Edge: **none** — no empty-file, blank-line, or trailing-newline proof.

## ReplayBuffer::select_replay_indices / get_pair
`crates/vox-tensor/src/replay.rs`
- At-risk samples (loss increase > threshold) are prioritized in selection. *(happy)*
- Returned indices are valid for `get_pair` lookup. *(invariant)*
- Edge: **no empty-buffer, all-below-threshold, ties, or capacity-bound proof**.

## Semantic gaps

Symbols whose contract clearly has a failure/empty/conflict mode but are proven only on the happy path:

1. **`JsonlDataLoader` (rating filter + field extraction)** — A validating/filtering loader with no rejection test. Empty files, records missing `prompt`/`rating`, and malformed lines are unproven. Most actionable: add a missing-field and empty-file test.
2. **`load_all_with_policy` (Skip/lenient branch)** — Only `FailFast` is exercised. The reason the policy enum exists — gracefully skipping bad rows — is entirely unproven. High-value: assert a malformed row is skipped (not fatal) under the lenient policy and counts decrement accordingly.
3. **`count_jsonl_records`** — A pure counter with no boundary proof: empty file should be 0, and blank/trailing-newline handling is undefined by the tests.
4. **`ReplayBuffer::select_replay_indices`** — A selection/priority surface with no empty-buffer or all-below-threshold behavior; conflict (ties at threshold) and capacity bounds unproven.
5. **`TrainingPair`** — Alias deserialization is proven, but the conflict case (both `prompt` and `instruction` present) and missing-required-field rejection are not, leaving the precedence contract unverified.

The tokenizer surface (`encode`/`decode`/`tokenize_for_training`) is the strongest part of the crate; the data-ingestion and replay layers are where the semantic holes concentrate.