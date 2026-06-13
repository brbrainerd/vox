# Semantic Behavior Map — `vox-constrained-gen`

Deterministically synthesized from 29 distinct proven-behavior claims (of 29 extracted) across 19 symbols. 1 symbols have an explicit error-path proof; **17 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `build_sampler()`  (happy; EXTRACTED)
- [happy] build_sampler() returns None when passed GrammarMode::None  (crates/vox-constrained-gen/src/factory.rs)
- [happy] build_sampler() returns None when passed GrammarMode::Json  (crates/vox-constrained-gen/src/factory.rs)
- [happy] build_sampler() returns Some when passed GrammarMode::Vox  (crates/vox-constrained-gen/src/factory.rs)
- [happy] build_sampler(GrammarMode::Vox) returns a sampler with name 'deadlock-watchdog'  (crates/vox-constrained-gen/src/factory.rs)
- [happy] build_sampler() returns Some when passed GrammarMode::VoxPda  (crates/vox-constrained-gen/src/factory.rs)
- [happy] build_sampler(GrammarMode::VoxPda) returns a sampler with name 'deadlock-watchdog'  (crates/vox-constrained-gen/src/factory.rs)

### `is_backtrack_token()`  (edge, happy; EXTRACTED)
- [happy] is_backtrack_token() returns false for non-sentinel token indices  (crates/vox-constrained-gen/src/revision.rs)
- [happy] is_backtrack_token() returns true when token at index matches BACKTRACK_TOKEN  (crates/vox-constrained-gen/src/revision.rs)
- [edge] is_backtrack_token() returns false for out-of-bounds indices  (crates/vox-constrained-gen/src/revision.rs)

### `EarleySampler::mask_logits()`  (error, happy; EXTRACTED)
- [happy] EarleySampler mask_logits() keeps valid tokens (fn) unmasked (greater than NEG_INFINITY)  (crates/vox-constrained-gen/src/earley.rs)
- [error] EarleySampler mask_logits() masks invalid tokens (???) to NEG_INFINITY  (crates/vox-constrained-gen/src/earley.rs)

### `PdaSampler::mask_logits()`  (happy; EXTRACTED)
- [happy] PdaSampler::mask_logits() returns Ok for valid grammar state  (crates/vox-constrained-gen/src/pda.rs)
- [happy] PdaSampler::mask_logits() masks invalid tokens with f32::NEG_INFINITY  (crates/vox-constrained-gen/src/pda.rs)

### `PdaState::new()`  (happy; EXTRACTED)
- [happy] PdaState::new() initializes stack with length 1  (crates/vox-constrained-gen/src/pda.rs)
- [happy] PdaState::new() initializes position to 0  (crates/vox-constrained-gen/src/pda.rs)

### `DeadlockWatchdog`  (happy; EXTRACTED)
- [happy] DeadlockWatchdog wraps an EarleySampler and exposes its initial state as a SamplerState::Earley variant  (crates/vox-constrained-gen/src/deadlock.rs)

### `DeadlockWatchdog::mask_logits()`  (happy; EXTRACTED)
- [happy] DeadlockWatchdog mask_logits() succeeds with valid input logits and tokens  (crates/vox-constrained-gen/src/deadlock.rs)

### `DeadlockWatchdog::name()`  (happy; EXTRACTED)
- [happy] DeadlockWatchdog name() returns 'deadlock-watchdog'  (crates/vox-constrained-gen/src/deadlock.rs)

### `EarleySampler::from_vox_grammar()`  (happy; EXTRACTED)
- [happy] EarleySampler::from_vox_grammar() successfully builds an EarleySampler  (crates/vox-constrained-gen/src/earley.rs)

### `EarleySampler::initial_state()`  (happy; EXTRACTED)
- [happy] EarleySampler initial_state() returns a SamplerState::Earley variant  (crates/vox-constrained-gen/src/earley.rs)

### `EarleySampler::name()`  (happy; EXTRACTED)
- [happy] EarleySampler name() returns 'earley'  (crates/vox-constrained-gen/src/earley.rs)

### `Grammar::from_ebnf()`  (happy; EXTRACTED)
- [happy] Grammar::from_ebnf() parses Vox EBNF grammar with over 20 productions  (crates/vox-constrained-gen/src/earley.rs)

### `Grammar::start`  (happy; EXTRACTED)
- [happy] Parsed Grammar has start symbol 'module'  (crates/vox-constrained-gen/src/earley.rs)

### `GrammarMode::default()`  (happy; EXTRACTED)
- [happy] GrammarMode default value is GrammarMode::None  (crates/vox-constrained-gen/src/factory.rs)

### `PdaSampler::from_vox_grammar()`  (happy; EXTRACTED)
- [happy] PdaSampler::from_vox_grammar() successfully builds a PdaSampler  (crates/vox-constrained-gen/src/pda.rs)

### `PdaSampler::initial_state()`  (happy; EXTRACTED)
- [happy] PdaSampler initial_state() returns a SamplerState::Pda variant  (crates/vox-constrained-gen/src/pda.rs)

### `PdaSampler::name()`  (happy; EXTRACTED)
- [happy] PdaSampler name() returns 'pda'  (crates/vox-constrained-gen/src/pda.rs)

### `RevisionSampler::mask_logits()`  (happy; EXTRACTED)
- [happy] RevisionSampler::mask_logits() returns Ok for normal tokens  (crates/vox-constrained-gen/src/revision.rs)

### `RevisionSampler::name()`  (happy; EXTRACTED)
- [happy] RevisionSampler::name() returns 'revision'  (crates/vox-constrained-gen/src/revision.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`DeadlockWatchdog`** — only: _DeadlockWatchdog wraps an EarleySampler and exposes its initial state as a SamplerState::Earley variant_
- **`DeadlockWatchdog::mask_logits()`** — only: _DeadlockWatchdog mask_logits() succeeds with valid input logits and tokens_
- **`DeadlockWatchdog::name()`** — only: _DeadlockWatchdog name() returns 'deadlock-watchdog'_
- **`EarleySampler::from_vox_grammar()`** — only: _EarleySampler::from_vox_grammar() successfully builds an EarleySampler_
- **`EarleySampler::initial_state()`** — only: _EarleySampler initial_state() returns a SamplerState::Earley variant_
- **`EarleySampler::name()`** — only: _EarleySampler name() returns 'earley'_
- **`Grammar::from_ebnf()`** — only: _Grammar::from_ebnf() parses Vox EBNF grammar with over 20 productions_
- **`Grammar::start`** — only: _Parsed Grammar has start symbol 'module'_
- **`GrammarMode::default()`** — only: _GrammarMode default value is GrammarMode::None_
- **`PdaSampler::from_vox_grammar()`** — only: _PdaSampler::from_vox_grammar() successfully builds a PdaSampler_
- **`PdaSampler::initial_state()`** — only: _PdaSampler initial_state() returns a SamplerState::Pda variant_
- **`PdaSampler::mask_logits()`** — only: _PdaSampler::mask_logits() returns Ok for valid grammar state_
- **`PdaSampler::name()`** — only: _PdaSampler name() returns 'pda'_
- **`PdaState::new()`** — only: _PdaState::new() initializes stack with length 1_
- **`RevisionSampler::mask_logits()`** — only: _RevisionSampler::mask_logits() returns Ok for normal tokens_
- **`RevisionSampler::name()`** — only: _RevisionSampler::name() returns 'revision'_
- **`build_sampler()`** — only: _build_sampler() returns None when passed GrammarMode::None_
