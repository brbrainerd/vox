# Semantic Behavior Map — `vox-plugin-speech`

Synthesized from 8 extracted Behavior claims across 4 source files. After dedup the claims map to 8 distinct symbols, each with exactly one proven behavior. The coverage profile is striking: every proof is either **happy-path** (6) or a source **invariant** (1, plus 1 INFERRED happy-path). No claim exercises an error path, a rejection, an empty/degenerate input, or a conflict-resolution branch — even though several symbols (the two config mergers, the logit processors, the audio normalizer) have obvious failure or edge modes in their contracts.

## Backends — `candle_whisper.rs`

### `merge_adjacent_transcripts()`
- **Proven (happy):** merges two transcript strings with an overlapping word boundary, removing the duplicate boundary word.
- Error path: none. Edge/invariant: none.
- Untested edges: empty `prev`/`next`, and the no-overlap (identity concat) case.

### single-window Whisper inference (production source)
- **Proven (invariant):** the single-window branch contains no simulated/test-only OOM error code in production source (`single_window_branch_does_not_force_simulated_oom`).
- This is a source-hygiene invariant, not a behavioral proof of the inference path itself.

### silent-PCM audio handling
- **Proven (happy):** a 5-second silent PCM array has the correct length (80000 samples @ 16 kHz).
- This proves only buffer sizing, not the hallucination-prevention *behavior* the test name (`test_silence_hallucination_prevention`) implies — there is no assertion that silence yields empty/suppressed output.

## Backends — `logit_processors.rs`

### `ForbiddenTokenMaskProcessor::apply()`
- **Proven (happy):** applies `-inf` to forbidden token positions in the logit tensor.
- Returns `Result<Tensor>` but no error-path proof. Edge/invariant: none.
- Untested: empty forbidden set, out-of-bounds token id, mismatched tensor shape.

### `TokenTrieConstraintProcessor` state transitions
- **Proven (happy, INFERRED):** advances through trie nodes and recovers to root on an invalid token sequence.
- Confidence is INFERRED, single-path. No proof of empty trie, leaf/exhausted node, or a repeated-reset invariant.

## Oratio internals — `acoustic_preprocess.rs`

### peak normalization in `preprocess_audio_pcm_f32_reported()`
- **Proven (happy):** increases peak amplitude, scaling a quiet signal to ≈0.95.
- No edge proof. The contract has a clear degenerate mode: an all-zero/silent input means `peak == 0` (potential divide-by-zero / NaN), and already-clipping (>1.0) input is unverified.

## Oratio internals — `runtime_config.rs`

### `OratioRuntimeConfig::merge_env()`
- **Proven (happy):** `VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE` overrides the default `tool_route_min_confidence` (precedence env > default).
- No rejection path for unparseable or out-of-range env values.

### `OratioRuntimeConfig::merge_file()`
- **Proven (happy):** parses a TOML file and merges routing, timing, refine, and HF-retry settings (roundtrip).
- Returns `Result<()>` but is proven only on valid input. No proof of malformed TOML, missing file, or invalid/out-of-range value rejection.

## Semantic gaps

These symbols are proven **only** on the happy path (or as a source invariant) yet have a clear failure, empty, or conflict mode in their contract. Ordered by actionability.

1. **`OratioRuntimeConfig::merge_file()` — validator/mutator with no rejection test.** It returns `Result<()>` and parses external TOML, the textbook place for malformed-input and out-of-range-value handling. Only the valid roundtrip is proven. Add tests for malformed TOML, missing file, and an out-of-range value (e.g. `tool_route_min_confidence = 5.0`).
2. **`OratioRuntimeConfig::merge_env()` — override path with no invalid-input proof.** Only a clean override is proven. Add a non-numeric and an out-of-range env value test to pin down whether it rejects, clamps, or silently ignores.
3. **`ForbiddenTokenMaskProcessor::apply()` — integrity surface (token masking) with no failure path.** This is a security/correctness-relevant masker returning `Result<Tensor>`; only the well-formed case is proven. Add empty-mask, out-of-bounds-token-id, and shape-mismatch cases.
4. **`TokenTrieConstraintProcessor` — INFERRED single-path only.** Promote from INFERRED by proving empty-trie and leaf/exhausted-node behavior, and the reset invariant under repeated stuck tokens.
5. **`preprocess_audio_pcm_f32_reported()` peak normalization — div-by-zero edge unproven.** The silent (`peak == 0`) and already-clipping inputs are exactly the degenerate cases the happy-path quiet-signal test skips.
6. **`merge_adjacent_transcripts()` — boundary edges unproven.** Empty inputs and the no-overlap identity case are untested.

Note also that `test_silence_hallucination_prevention` proves only buffer length, not the hallucination-suppression behavior its name promises — a coverage label that overstates what is actually asserted.