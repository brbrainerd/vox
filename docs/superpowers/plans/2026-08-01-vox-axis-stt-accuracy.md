# Vox Axis STT Accuracy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix voice-dictation accuracy in the Vox Axis GUI: build a real (previously nonexistent) automated regression signal for the refine/symbol-expansion pipeline, fix broken correction rules, switch the default ASR backend from Candle-Whisper-tiny to NeMo Parakeet-TDT via the existing sherpa-onnx backend, unblock code-dictation symbol expansion in the shipped GUI, and expose the two highest-value STT knobs in Settings.

**Architecture:** Five sequential slices in `crates/vox-speech` (deterministic refine rules, ASR backend selection) and `crates/vox-gui` (Cargo features, Tauri commands, Settings UI). A new text-only regression test (no audio required) gates the rule fixes and the symbol-expansion change; the backend swap is gated by enabling a previously-uncompiled feature plus a manual cross-platform packaging check documented inline.

**Tech Stack:** Rust (`vox-speech`, `vox-gui`), the `sherpa-onnx` crate's `OfflineTransducerModelConfig` (confirmed via docs.rs to have `encoder`/`decoder`/`joiner: Option<String>` fields, nesting into `OfflineModelConfig.transducer`), React/TypeScript (`SettingsView.tsx`).

**Deviations from the design spec** (`docs/src/architecture/vox-axis-stt-accuracy-design-2026-08-01.md`), disclosed per project convention rather than silently applied:
- The design's Phase 0 proposed running the fixture manifests through the *full audio pipeline*. On inspection, `vox_code_corpus_v1.jsonl` has no audio and no spoken-form transcript field — only `expected` (final post-refinement text) and `description`. Synthesizing matching audio would require inventing a spoken-form script per entry and hoping a TTS voice reads jargon the way a developer would, which is a shaky proxy for real ASR accuracy. Task 1 below instead builds a **text-only regression harness**: hand-authored, realistic raw-ASR-hypothesis strings (approximating actual Whisper/Parakeet mis-transcriptions) run through the real `refine_transcript` + `normalize_spoken_code_phrase` code, scored with the existing (previously-unused) `eval.rs` functions. This is deterministic, fast, CI-safe, and exercises exactly the code Tasks 2, 3, and 7 change. A full audio-based ASR eval against real recorded speech remains valuable but needs an actual speech corpus — out of scope for this plan; flagged for follow-up rather than faked.
- The design's Phase 4 proposed exposing backend choice, correction aggressiveness, and custom lexicon entries. This plan scopes Task 8 to **two** knobs only — ASR backend and domain mode (general/code) — because they map directly to plain `VOX_ORATIO_*` env vars with existing enum semantics and reuse the codebase's existing flat-config persistence helper with no new registry. Correction-aggressiveness exposure and lexicon-entry CRUD UI are real, separable follow-up work, not silently dropped.
- The design noted `crates/vox-gui/src/commands/oratio.rs` as a finding but explicitly deferred touching it (see the design doc's corrected finding #6). This plan does not touch it either — no task here modifies `oratio.rs` or `oratioVoiceInput.ts`.
- **The design's Phase 2 gate is only half-implemented here, and that is a deliberate, disclosed narrowing, not a silent drop.** The design requires *two* conditions before Parakeet-via-sherpa-onnx ships as the default: (a) the Phase-0 eval harness shows Parakeet's WER/CER beating Candle-tiny.en on all three fixture manifests, AND (b) the Tauri build packages and runs on all three OSes. Task 1's harness is text-only (see the first Deviations bullet above) and never invokes any ASR backend, so it is structurally unable to measure condition (a) — there is no audio-based Candle-vs-Parakeet comparison anywhere in this plan. Task 6 Step 6 below implements only condition (b) (the manual cross-platform packaging smoke test). Shipping Parakeet as the default on the strength of (b) alone is a real scope reduction from the design's stated gate; it is called out explicitly here and in the Follow-ups section rather than left for a reader to notice only after Task 6 flips the default.

---

## Execution parallelism

Read this before dispatching tasks to subagents (e.g. via `superpowers:subagent-driven-development`). It summarizes which tasks touch disjoint files with no compile-time or verification coupling (safe to run concurrently) versus which have a real ordering requirement.

**Safe to run as parallel subagents (pick one grouping):**
- **Task 1 + Task 3 + Task 4 + Task 8** — four mutually disjoint file sets (new `tests/refine_regression.rs`; `refine/rules.rs`'s `default_domain_lexicon` function; `backends/sherpa_model_config.rs`; the new `commands/stt_config.rs` + `commands/mod.rs` + `main.rs` + `SettingsView.tsx`). None imports from, reads output of, or has a verification step depending on the others.
- **Task 2 + Task 4 + Task 8** — alternative to the above if Task 2 (instead of Task 3) is preferred: Task 2 only touches `refine/rules.rs`'s `code_confusion_map`/`apply_phrase_confusions` and is disjoint from Task 4 and Task 8's files. Do **not** combine this with Task 3 in the same batch (see file conflict below), and prefer running it after Task 1 has landed, since Task 2 Step 6 (formerly Step 5) re-runs Task 1's harness file as part of its own verification.

**Hard sequential dependencies (must run in order, not parallel):**
- **Task 4 before Task 5.** Task 5's rewritten `SherpaOnnxBackend::new()` imports and calls `resolve_sherpa_transducer_model_paths()` / `SherpaTransducerModelPaths`, both created by Task 4. Disjoint files, but a real compile-time dependency.
- **Task 6 before Task 7 (file conflict, not just ordering).** Both edit the identical line of `crates/vox-gui/Cargo.toml` (the `vox-speech` feature list) — Task 7 Step 2 appends `compiler-rerank` onto the exact list Task 6 Step 2 sets. Running these concurrently risks a lost edit; this is the plan's own worked example of a same-line collision.
- **Task 1 before Task 2 (soft — verification-only).** Task 2's harness re-run step runs `cargo test ... --test refine_regression`, the file Task 1 creates. The code edits themselves never collide (`tests/refine_regression.rs` vs `refine/rules.rs`), but if dispatched as fully independent parallel subagents (separate worktrees), Task 2's verification step fails until Task 1's file exists.
- **Task 1 and Task 2 before Task 7 (soft — verification-only).** Task 7's harness-passes verification step depends on Task 1's file existing AND Task 2's `code_confusion_map`/phrase-matching fix being applied (that fix, not Task 7's own feature flip, is what the WER improvement traces to), in addition to Task 6's Cargo.toml line already having landed.
- **Task 2 and Task 3 must not run concurrently (file conflict, not a logical dependency).** Both edit `crates/vox-speech/src/refine/rules.rs` — different functions (`code_confusion_map`/`apply_phrase_confusions` vs. `default_domain_lexicon`) but the same file and the same `mod tests` block. This mirrors the Task 6/Task 7 Cargo.toml collision exactly: conceptually independent fixes that collide on a shared file. Either order is fine; simultaneous dispatch is not.

**Ambiguous — use judgment or ask before batching:**
- **Task 5 → Task 6 ordering** is not a hard compile/file dependency (Task 6 only calls the pre-existing `SherpaOnnxBackend::new()` signature), but Task 6's stated purpose and commit message ("default to Parakeet-via-sherpa-onnx... fall back to Candle Whisper") is only accurate once Task 5's transducer-default logic actually lives in `sherpa_onnx.rs`. Decide whether "safe to parallelize" means "won't merge-conflict" (true) or "won't produce a semantically-misleading intermediate commit" (false) before batching these together.
- **Task 8 relative to the Task 4–7 chain** never overlaps in files or compile-time dependencies (its `set_stt_config` just validates against a fixed option list and persists/env-sets via existing helpers). But it exposes a `sherpa` backend option and a `code` domain-mode option in Settings that are only functionally meaningful once Tasks 4–7 actually ship the Parakeet-default behavior and symbol-expansion reachability. Consider whether Task 8 should be gated behind the rest landing, for product-correctness reasons, even though nothing forces that at the file/code level.

---

### Task 1: Text-only refine/symbol-expansion regression harness

**Files:**
- Create: `crates/vox-speech/tests/refine_regression.rs`
- Read (no changes): `crates/vox-speech/src/eval.rs`, `crates/vox-speech/src/refine/mod.rs`, `crates/vox-speech/src/speech_normalize.rs`, `crates/vox-speech/tests/fixtures/eval_manifests/vox_code_corpus_v1.jsonl`

This is an integration test (`tests/` directory, not `#[cfg(test)]` inline), so it only needs `vox-speech`'s public API. Check `crates/vox-speech/src/lib.rs` exports `refine::{CorrectionContext, DomainMode, OratioCorrectionProfile, refine_transcript}`, `speech_normalize::normalize_spoken_code_phrase`, and `eval::{word_error_rate, symbol_error_rate}` as `pub` — all four are already `pub` per the files read during planning, so no export changes needed.

- [ ] **Step 1: Write the failing test**

Five hand-authored hypotheses below approximate what Candle-Whisper `tiny.en` actually outputs for each of the 5 entries in `vox_code_corpus_v1.jsonl` (based on the corpus's `description` field and the known confusion-map bugs from Task 2 — e.g. "box dine" is the literal ASR mishearing of "Box dyn" fixed by the `code_confusion_map`). This couples the fixture to realistic ASR noise instead of clean text, so the test actually exercises the refine + symbol-expansion pipeline.

**Scope note (audit-driven):** the original draft of cases `vox_code_001` and `vox_code_005` spelled out "zero", bare "equals", "two hundred", and a letter-by-letter "h t t p" acronym in the raw hypothesis. None of those conversions exist anywhere in this codebase today — `speech_normalize.rs`'s `expand_spoken_symbols` only has `"double equals" -> "=="` and `"not equals" -> "!="`, nothing for bare "equals", spoken numerals, or letter-spelled acronyms — and no task in this plan adds them. Left as originally drafted, this harness could never pass regardless of how correctly Tasks 2/3/7 are implemented, which would make Task 7 Step 5's "confirm it now passes" gate permanently unsatisfiable. The two hypotheses below are revised to only exercise conversions that already exist (or that Tasks 2/7 add): digits and the `=` sign are given to the pipeline pre-formed rather than spelled out, which is itself a realistic ASR behavior for short numerals/symbols. Implementing spoken-numeral and letter-spelled-acronym expansion is tracked as a follow-up (see "Follow-ups" at the end of this plan), not silently assumed.

**Second scope note (audit-driven, cases `vox_code_002`–`vox_code_004`):** these three cases' raw hypotheses originally spoke "open curly"/"close curly", "comma", and (for `vox_code_003`) two consecutive "colon" tokens for `UserState::Active` and a bare "dot" for `self.user_state` — but `expand_spoken_symbols` had no "curly" alias for brace, no comma mapping, and no bare-"dot" mapping at all (only "dot dot"/"dot dot dot"), and `vox_code_002`'s hypothesis never spoke "open angle"/"close angle" tokens in the first place even though its expected output needs `Result<(), Error>`. Left as originally drafted, none of these three cases could ever reach their expected text regardless of Tasks 2/3/7, for reasons distinct from the numeral/acronym gap above. Fixed two ways, both disclosed rather than silently patched over:
1. Task 7 Step 3 (below) adds the missing phrase mappings (`"open curly"`/`"close curly"` as brace aliases, `"comma"`, `"open angle"`/`"close angle"`, and a bare `"dot"`) to `expand_spoken_symbols`, and `vox_code_002`'s raw hypothesis now speaks "open angle"/"close angle" around the `Result` generic. `vox_code_003`'s raw hypothesis also changes its final "arrow" to "fat arrow" — the original wording asked for `->` where the expected match-arm text needs `=>`, an authoring slip unrelated to any of Tasks 2/3/7.
2. Even with those mappings, the harness still cannot reproduce a real Rust formatter's exact idiomatic spacing (e.g. `"count : i32"` vs. `"count: i32"`) or Rust's PascalCase-vs-snake_case type/field-name capitalization (`user state` needs to become `UserState` in one spot and `user_state` in another, spoken identically) — neither is implemented anywhere in this codebase, and building either is out of scope for this plan (see "Follow-ups"). Word-level `word_error_rate` scores exact spacing/casing differences as full token substitutions, which would fail every symbol-expansion case regardless of whether the underlying conversion was otherwise correct. The assertion below therefore gates on `char_error_rate` (case-folded, and already whitespace-insensitive by construction — see `eval.rs`'s own `cer_ignores_space` test) instead of `word_error_rate`, which is scoped to validate "did the pipeline produce the right words and symbols in the right order," not "did it reproduce a Rust formatter's exact byte-for-byte output." `word_error_rate`/`symbol_error_rate` are still computed and printed on failure as extra diagnostics.

```rust
//! Text-only regression harness for the deterministic refine + symbol-expansion
//! pipeline, scored against `tests/fixtures/eval_manifests/vox_code_corpus_v1.jsonl`.
//!
//! No audio or ASR model is involved: the corpus only carries `expected` (final
//! post-refinement text), not a spoken-form transcript, so there is nothing to
//! synthesize audio from. Instead this feeds realistic raw-ASR-hypothesis strings
//! (approximating actual Whisper/Parakeet mis-transcriptions) through the real
//! `refine_transcript` + `normalize_spoken_code_phrase` code and scores the result
//! against the corpus's `expected` field with the existing CER/WER/SER functions
//! (gated on CER; WER/SER are diagnostic-only — see the second Scope note above
//! this block for why).

use vox_speech::eval::{char_error_rate, symbol_error_rate, word_error_rate};
use vox_speech::refine::{CorrectionContext, DomainMode, refine_transcript};
use vox_speech::speech_normalize::normalize_spoken_code_phrase;

/// One regression case: a raw ASR-shaped hypothesis paired with the corpus's
/// expected final text. `id` matches the `id` field in `vox_code_corpus_v1.jsonl`.
struct Case {
    id: &'static str,
    raw_hypothesis: &'static str,
    expected: &'static str,
}

const CASES: &[Case] = &[
    Case {
        id: "vox_code_001",
        // "zero" and bare "equals" are not implemented anywhere in this
        // codebase (only "double equals"/"not equals" exist) — see the
        // Scope note above. The digit and symbol are given pre-formed here
        // instead, which still exercises "let mute" -> "let mut " (Task 2)
        // and "colon"/"semicolon" symbol expansion (Task 7).
        raw_hypothesis: "let mute count colon i32 = 0 semicolon",
        expected: "let mut count: i32 = 0;",
    },
    Case {
        id: "vox_code_002",
        // "open angle"/"close angle" added around the `Result` generic — the
        // original draft never spoke these tokens at all, so no mapping-table
        // fix could ever produce the `<...>` in `expected` (see the second
        // Scope note above). Task 7 Step 3 adds "open angle"/"close angle"
        // (and "open curly"/"close curly", "comma") to `expand_spoken_symbols`.
        raw_hypothesis: "pub fun handle message msg colon string arrow result open angle open paren close paren comma error close angle open curly",
        expected: "fn handle_message(msg: String) -> Result<(), Error> {",
    },
    Case {
        id: "vox_code_003",
        // Changed trailing "arrow" -> "fat arrow": the original wording
        // produces `->` (thin arrow) where the expected match-arm text needs
        // `=>` (fat arrow) — an authoring slip in the original draft, not a
        // pipeline gap (see the second Scope note above).
        raw_hypothesis: "match self dot user state open curly user state colon colon active fat arrow open curly close curly",
        expected: "match self.user_state { UserState::Active => {}",
    },
    Case {
        id: "vox_code_004",
        raw_hypothesis: "table user open curly name colon string comma limit colon u32 close curly",
        expected: "table User { name: String, limit: u32 }",
    },
    Case {
        id: "vox_code_005",
        // Letter-spelled acronyms ("h t t p" -> "HTTP"), "two hundred", and
        // bare "equals" are not implemented anywhere in this codebase — see
        // the Scope note above. This case now exercises "underscore" joining
        // and pass-through casing/digit handling instead of inventing those
        // conversions.
        raw_hypothesis: "let HTTP underscore status underscore ok = 200",
        expected: "let HTTP_STATUS_OK = 200;",
    },
];

/// Ceiling on mean character-error-rate across the corpus after refine +
/// symbol expansion. This is the Task-1 baseline: it MUST fail before Tasks
/// 2/7 land (broken rules + gated symbol expansion mean today's pipeline does
/// not clear this bar) and MUST pass after them.
///
/// Gates on CER, not WER (see the second Scope note above): `char_error_rate`
/// is case-folded here and is whitespace-insensitive by construction (see
/// `eval.rs`'s `cer_ignores_space` test), so it validates that the pipeline
/// produced the right characters/symbols in the right order without also
/// requiring a Rust-formatter-exact spacing pass or a type-name
/// capitalization pass this plan does not implement.
const MAX_MEAN_CER: f64 = 0.35;

#[test]
fn refine_pipeline_meets_code_corpus_baseline() {
    let ctx = CorrectionContext {
        domain: DomainMode::Code,
        ..Default::default()
    };

    let mut total_cer = 0.0;
    let mut failures = Vec::new();
    for case in CASES {
        let refined = refine_transcript(case.raw_hypothesis, &ctx).text;
        let final_text = normalize_spoken_code_phrase(&refined);
        let expected_lower = case.expected.to_lowercase();
        let got_lower = final_text.to_lowercase();
        let cer = char_error_rate(&expected_lower, &got_lower);
        // Diagnostics only (not gated on): word-level WER penalizes
        // idiomatic-spacing/casing differences the pipeline doesn't attempt
        // to reproduce exactly, and SER ignores punctuation entirely — both
        // are still useful context in a failure message.
        let wer = word_error_rate(case.expected, &final_text);
        let ser = symbol_error_rate(case.expected, &final_text);
        total_cer += cer;
        if cer > 0.3 {
            failures.push(format!(
                "{}: cer={cer:.2} wer={wer:.2} ser={ser:.2}\n  expected: {}\n  got:      {final_text}",
                case.id, case.expected
            ));
        }
    }
    let mean_cer = total_cer / CASES.len() as f64;
    assert!(
        mean_cer <= MAX_MEAN_CER,
        "mean CER {mean_cer:.3} exceeds baseline {MAX_MEAN_CER}; per-case failures:\n{}",
        failures.join("\n")
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-speech --features stt-candle --test refine_regression -- --nocapture`
Expected: FAIL — `mean CER ... exceeds baseline 0.35`. This is the correct failing state: today `code_confusion_map`'s multi-word entries (e.g. `"box dine"`, `"let mute"`) never match at all, because `refine_transcript`'s matching loop only looks up single whitespace-split tokens — Task 2 fixes both the matching algorithm and the map's bad values — `expand_spoken_symbols` doesn't yet have the curly/comma/angle-bracket/dot mappings Task 7 Step 3 adds, so cases `vox_code_002`–`004`'s literal spoken words for those symbols survive unconverted — and `normalize_spoken_code_phrase` is never actually reached in the shipped GUI per finding #1, but here we're calling it directly so this run measures the refine+normalize layer in isolation, pre-fix.

- [ ] **Step 3: Commit the harness (still red)**

```bash
git add crates/vox-speech/tests/refine_regression.rs
git commit -m "test: add text-only refine/symbol-expansion regression harness"
```

---

### Task 2: Fix broken correction rules

**Files:**
- Modify: `crates/vox-speech/src/refine/rules.rs:26-45` (and its `refine_transcript` matching loop — see Step 3 below)
- Test: inline `#[cfg(test)] mod tests` in the same file

**Audit finding (blocker), read before starting this task:** `code_confusion_map()`'s keys are all multi-word phrases (`"box dine"`, `"let mute"`, `"if let some"`, ...), but `refine_transcript`'s matching loop only ever looks up a single whitespace-split token (`for token in current.split_whitespace() { ... confusion.get(lower.as_str()) ... }`, rules.rs:115-153). A multi-word key can never equal one token, so **every entry in `code_confusion_map` is dead code today, and fixing only the map's values (as originally drafted for this task) changes nothing at runtime** — confirmed by direct testing: adding this task's `box_dyn_confusion_closes_angle_bracket` test with only the map-value fix applied still fails with `left: "box dine error" / right: "Box<dyn error"`. Step 3 below therefore fixes the matching algorithm first, then the map's values.

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` block in `crates/vox-speech/src/refine/rules.rs` (after the existing `protected_tokens_not_rewritten` test):

```rust
    #[test]
    fn box_dyn_confusion_closes_angle_bracket() {
        let ctx = CorrectionContext {
            domain: crate::refine::DomainMode::Code,
            ..Default::default()
        };
        let out = refine_transcript("box dine error", &ctx);
        assert_eq!(out.text, "Box<dyn error");
        // The full-phrase closing-bracket case (with a following type token) is
        // handled by the phrase_canonicalization pass, not the token map alone —
        // this test only asserts the map no longer emits an unbalanced `<`
        // followed by a bare trailing space with nothing to close it.
        assert!(!out.text.ends_with("Box<dyn "), "must not leave a dangling space with no type");
    }

    #[test]
    fn mut_self_is_not_a_confusion_entry() {
        // `mut self` is valid Rust as spoken; it must not appear in the code
        // confusion map (it was a no-op identity mapping doing nothing).
        let ctx = CorrectionContext {
            domain: crate::refine::DomainMode::Code,
            ..Default::default()
        };
        assert!(!super::code_confusion_map().contains_key("mut self"));
        let out = refine_transcript("fn foo mut self", &ctx);
        assert_eq!(out.text, "fn foo mut self");
    }

    #[test]
    fn guessy_print_phrases_removed_from_confusion_map() {
        // "print len" / "print el in" were unvalidated phonetic guesses that can
        // misfire on unrelated speech (e.g. "the print length was wrong").
        assert!(!super::code_confusion_map().contains_key("print len"));
        assert!(!super::code_confusion_map().contains_key("print el in"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-speech --lib refine::rules::tests -- --nocapture`
Expected: FAIL on `box_dyn_confusion_closes_angle_bracket` — with the *current* matching loop and the *current* (unfixed) map, `"box dine"` is a two-word key that a single-token lookup can never find, so the input `"box dine error"` passes through completely unchanged as `"box dine error"`, not `"Box<dyn error"` as asserted. (If you fix only the map's values without first fixing the matching loop, this test still fails identically — the map stays unreachable.) `mut_self_is_not_a_confusion_entry` and `guessy_print_phrases_removed_from_confusion_map` fail because those keys are still present in the map, independent of the matching-loop bug.

- [ ] **Step 3: Add multi-word phrase matching to `refine_transcript`**

This is the root fix: `code_confusion_map`'s phrase-shaped entries need to be matched as phrases, not as single tokens. Add a phrase pass that runs before the existing single-token loop:

```rust
/// Replace multi-word `code_confusion_map` phrases (e.g. "box dine",
/// "let mute") with their canonical form BEFORE the single-token loop below
/// runs. `code_confusion_map()`'s keys are phrases, but a whitespace-split
/// single-token lookup can never equal a multi-word key — without this pass,
/// every phrase entry in the map is permanently dead code (confirmed by
/// direct testing; see the audit finding above this task).
fn apply_phrase_confusions(text: &str) -> String {
    let mut phrases: Vec<(&'static str, &'static str)> = code_confusion_map().into_iter().collect();
    // Longest phrases first, so a 3-word key can't be shadowed by a 2-word
    // key that happens to be one of its prefixes.
    phrases.sort_by_key(|(k, _)| std::cmp::Reverse(k.split_whitespace().count()));

    let mut result = text.to_string();
    for (phrase, replacement) in phrases {
        let lower = result.to_lowercase();
        if let Some(pos) = lower.find(phrase) {
            result = format!("{}{}{}", &result[..pos], replacement, &result[pos + phrase.len()..]);
        }
    }
    result
}
```

Wire it in at the top of `refine_transcript`, on the same string the existing single-token loop reads from, before that loop runs (rules.rs:~115) — **read the current top of the function first** to confirm the exact working-string variable name (traced as `current` during the audit) and whether anything upstream of the token loop (e.g. protected-token detection) needs to run before or after this pass; adjust the insertion point accordingly rather than assuming line 115 is still exactly the right spot once earlier tasks/edits have landed.

After wiring this in, run the *full* `refine::rules::tests` module (not just this task's 3 new tests) once, to confirm no existing single-word-confusion test (e.g. `confusion_token_rewrite`) regresses now that a phrase pass runs ahead of it.

- [ ] **Step 4: Fix `code_confusion_map`**

In `crates/vox-speech/src/refine/rules.rs`, replace the `code_confusion_map` function body:

```rust
fn code_confusion_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("unwrap or else", "unwrap_or_else"),
        ("unwrap or default", "unwrap_or_default"),
        ("hash map", "HashMap"),
        ("box dine", "Box<dyn"),
        ("to string", "to_string"),
        ("pub fun", "pub fn"),
        ("pub function", "pub fn"),
        ("let mute", "let mut "),
        ("a sync", "async"),
        ("vec bang", "vec!"),
        ("debug bang", "dbg!"),
        ("if let some", "if let Some"),
    ])
}
```

Removed: `"impl for" -> "impl for "` (trailing-space-only no-op), `"mut self" -> "mut self"` (identity no-op), `"print len" -> "println!"` and `"print el in" -> "println!"` (unvalidated phonetic guesses). Changed `"box dine" -> "Box<dyn"` (dropped the trailing `<` + space so it no longer leaves a dangling unclosed generic — with phrase-level matching now in place via Step 3, the replacement is a straight substring swap, so it must not itself emit an unclosed bracket with trailing artifacts).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-speech --lib refine::rules::tests -- --nocapture`
Expected: PASS (all tests in the module, including the 3 new ones and the pre-existing `confusion_token_rewrite`/`protected_tokens_not_rewritten`) — this now requires both Step 3 (phrase matching) and Step 4 (map values) to be in place; Step 4 alone does not make these tests pass.

- [ ] **Step 6: Re-run Task 1's harness to confirm improvement (not yet passing — Task 7 still pending)**

Run: `cargo test -p vox-speech --features stt-candle --test refine_regression -- --nocapture`
Expected: still FAIL, but with a lower `mean_cer` printed in the assertion message than Task 1 Step 2's run (the `box dine`/`let mute`/`mut self` fixes now actually fire, thanks to Step 3's phrase matching, and remove some error contribution; the remaining gap is symbol expansion, fixed in Task 7).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-speech/src/refine/rules.rs
git commit -m "fix: add phrase-level matching so code_confusion_map's multi-word entries actually fire, and remove broken/no-op entries"
```

---

### Task 3: Remove common-English-word collisions from the domain lexicon

**Files:**
- Modify: `crates/vox-speech/src/refine/rules.rs:47-64`
- Test: inline `#[cfg(test)] mod tests` in the same file

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `crates/vox-speech/src/refine/rules.rs`:

```rust
    #[test]
    fn generic_english_words_not_force_lowercased() {
        // "status" is a common English word; forcing it lowercase mid-sentence
        // corrupts normal capitalized usage. Use a bare, punctuation-free
        // token (no trailing colon) — `refine_transcript`'s matching loop
        // splits on whitespace only, so a colon-attached token like "Status:"
        // never equals the plain-word lexicon entry "status" regardless of
        // whether "status" is in the lexicon, which would make this test
        // pass trivially in both the broken and fixed states and prove
        // nothing (audit finding: the original draft used "Status: complete").
        let out = refine_transcript("Status complete", &CorrectionContext::default());
        assert_eq!(out.text, "Status complete");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-speech --lib refine::rules::tests::generic_english_words_not_force_lowercased -- --nocapture`
Expected: FAIL — current output is `"status complete"` (the bare `"Status"` token matches the lexicon's `"status"` entry and gets force-lowercased by `domain_lexicon_case`).

- [ ] **Step 3: Fix `default_domain_lexicon`**

In `crates/vox-speech/src/refine/rules.rs`, replace the `default_domain_lexicon` function body:

```rust
fn default_domain_lexicon() -> HashSet<String> {
    [
        "vox",
        "mens",
        "oratio",
        "schola",
        "candle",
        "whisper",
        "transcribe",
        "orchestrator",
        "tool-call",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}
```

Removed `"workflow"` and `"status"` — both are common English words with legitimate capitalized uses outside the Vox domain-term context this lexicon is meant to canonicalize.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-speech --lib refine::rules::tests -- --nocapture`
Expected: PASS (full module, including the pre-existing `confusion_token_rewrite` test — re-check it: that test's input was `"vox mends oration status"` asserting output `"vox oratio status"`. Since `"status"` was already lowercase in that test's input, removing it from the lexicon changes it from an explicit "canonicalize known token" pass to a no-op passthrough for that word, which still yields the same lowercase `"status"` in the output — the test continues to pass unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-speech/src/refine/rules.rs
git commit -m "fix: remove generic English words from domain lexicon"
```

---

### Task 4: Resolve Parakeet transducer model paths in `sherpa_model_config.rs`

**Files:**
- Modify: `crates/vox-speech/src/backends/sherpa_model_config.rs`
- Modify: `crates/vox-speech/src/lib.rs` (add a tiny `#[cfg(test)]` env-mutation lock, shared with Task 6 Step 3's test — see Step 1 below)
- Test: inline `#[cfg(test)]` in the same file (new)

`OfflineTransducerModelConfig` (confirmed via docs.rs) needs `encoder`, `decoder`, and `joiner` ONNX files, vs. Whisper's `encoder`/`decoder` only. This task adds joiner resolution and a transducer-shaped default model, without removing the existing Whisper-shaped resolution (both paths coexist; Task 5 picks between them based on which files resolve).

- [ ] **Step 1: Write the failing test**

`VOX_ORATIO_SHERPA_MODEL_DIR` is also mutated by Task 6 Step 3's test in `backend_dispatch.rs`, and both land in the same `vox-speech` test binary that `cargo test` runs with multiple threads by default — two tests racing on the same global env var is a real, not hypothetical, flakiness source (this crate's `runtime_config.rs` precedent gets away with a comment-only "not parallelized" convention only because it's a single test per var; a second file mutating the same var needs an actual lock, not just a comment). Add a tiny shared lock in `crates/vox-speech/src/lib.rs`:

```rust
/// Serializes tests across the crate that mutate shared global env vars
/// (e.g. `VOX_ORATIO_SHERPA_MODEL_DIR`, read by both
/// `backends::sherpa_model_config` and `backend_dispatch` tests). `cargo
/// test` runs test functions concurrently by default; a comment-only
/// "don't run this in parallel" convention is not enough once more than one
/// file touches the same var.
#[cfg(test)]
pub(crate) mod env_test_lock {
    pub static SHERPA_MODEL_DIR_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
```

Then add to `crates/vox-speech/src/backends/sherpa_model_config.rs` (new `#[cfg(test)]` block at the end of the file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transducer_paths_resolve_from_local_dir() {
        // Held for the duration of the test: see `crate::env_test_lock` doc
        // comment — this env var is also mutated by backend_dispatch's test.
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("encoder.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("decoder.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("joiner.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("tokens.txt"), b"stub").unwrap();

        // SAFETY: test-only env mutation; serialized against other tests
        // touching the same var by the lock above.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("VOX_ORATIO_SHERPA_MODEL_DIR", dir.path());
        }
        let paths = resolve_sherpa_transducer_model_paths().expect("resolve");
        assert_eq!(paths.encoder, dir.path().join("encoder.onnx"));
        assert_eq!(paths.decoder, dir.path().join("decoder.onnx"));
        assert_eq!(paths.joiner, dir.path().join("joiner.onnx"));
        assert_eq!(paths.tokens, dir.path().join("tokens.txt"));
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
        }
    }
}
```

Add `tempfile = { workspace = true }` is already present in `[dev-dependencies]` per `crates/vox-speech/Cargo.toml:79` — no Cargo.toml change needed for this test. Note: this crate's workspace lints set `unsafe_code = "warn"` (`Cargo.toml:38`, inherited via `[lints] workspace = true`); every existing unsafe-env-mutation test in this crate pairs the block with `#[allow(unsafe_code)]` (see `runtime_config.rs:509,514,520`, `backends/acoustic_preprocess.rs:157,168,175,183,187`) — omitting it here would trip `-D warnings` under the mandatory pre-push/CI clippy gate.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-speech --features stt-sherpa --lib backends::sherpa_model_config::tests -- --nocapture`
Expected: FAIL with a compile error — `resolve_sherpa_transducer_model_paths` and `SherpaTransducerModelPaths` (used in Step 3 below) don't exist yet.

- [ ] **Step 3: Add transducer path resolution**

In `crates/vox-speech/src/backends/sherpa_model_config.rs`, add after the existing `SherpaModelPaths` struct and before `resolve_sherpa_model_paths`:

```rust
/// Resolved paths to Sherpa-ONNX NeMo transducer model artifacts (e.g. Parakeet-TDT).
pub struct SherpaTransducerModelPaths {
    /// Path to the ONNX encoder model.
    pub encoder: PathBuf,
    /// Path to the ONNX decoder model.
    pub decoder: PathBuf,
    /// Path to the ONNX joiner model (transducer-specific; Whisper has no joiner).
    pub joiner: PathBuf,
    /// Path to the BPE/token vocabulary file.
    pub tokens: PathBuf,
}

/// Default HF model ID for the Parakeet-TDT transducer download.
///
/// VERIFY BEFORE RELYING ON THIS IN PRODUCTION: this repo ID is the best candidate
/// identified during research (see
/// docs/src/architecture/vox-axis-stt-accuracy-design-2026-08-01.md,
/// "External research" section, and
/// https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html)
/// but was not confirmed to exist byte-for-byte at design time. Task 4 Step 4's
/// manual download check is the actual verification gate.
///
/// KNOWN RISK (minor, disclosed not silently accepted): this mirrors the
/// pre-existing `resolve_sherpa_model_paths`' pattern of fetching from an
/// env-configurable HF repo id with no checksum/signature verification of
/// the downloaded files. That was an opt-in, never-shipped surface before
/// this plan; Task 6 makes it the GUI's default, tried-first backend path,
/// which raises the stakes of an unverified download without adding any new
/// integrity control. A real fix (pinned content hash + verification before
/// `OfflineRecognizer::create` loads the files) is out of scope for this
/// plan — see "Follow-ups".
pub const DEFAULT_SHERPA_TRANSDUCER_HF_MODEL: &str =
    "csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";

/// Resolve transducer model paths: env-set local dir OR HF Hub download.
/// Mirrors [`resolve_sherpa_model_paths`] but also resolves a `joiner` file.
pub fn resolve_sherpa_transducer_model_paths() -> Result<SherpaTransducerModelPaths> {
    if let Ok(dir) = std::env::var("VOX_ORATIO_SHERPA_MODEL_DIR") {
        let dir = PathBuf::from(dir.trim());
        return Ok(SherpaTransducerModelPaths {
            encoder: dir.join("encoder.onnx"),
            decoder: dir.join("decoder.onnx"),
            joiner: dir.join("joiner.onnx"),
            tokens: dir.join("tokens.txt"),
        });
    }

    let model_id = std::env::var("VOX_ORATIO_SHERPA_HF_MODEL")
        .unwrap_or_else(|_| DEFAULT_SHERPA_TRANSDUCER_HF_MODEL.to_string());
    let revision = "main";
    let api = hf_hub::api::sync::Api::new().context("HF API init")?;
    let repo = api.repo(hf_hub::Repo::with_revision(
        model_id.clone(),
        hf_hub::RepoType::Model,
        revision.to_string(),
    ));

    let encoder = repo
        .get("encoder.int8.onnx")
        .or_else(|_| repo.get("encoder.onnx"))
        .with_context(|| format!("fetch encoder from {model_id}"))?;
    let decoder = repo
        .get("decoder.int8.onnx")
        .or_else(|_| repo.get("decoder.onnx"))
        .with_context(|| format!("fetch decoder from {model_id}"))?;
    let joiner = repo
        .get("joiner.int8.onnx")
        .or_else(|_| repo.get("joiner.onnx"))
        .with_context(|| format!("fetch joiner from {model_id}"))?;
    let tokens = repo
        .get("tokens.txt")
        .with_context(|| format!("fetch tokens.txt from {model_id}"))?;
    Ok(SherpaTransducerModelPaths {
        encoder,
        decoder,
        joiner,
        tokens,
    })
}
```

- [ ] **Step 4: Run test to verify it passes, then manually verify the HF repo ID**

Run: `cargo test -p vox-speech --features stt-sherpa --lib backends::sherpa_model_config::tests -- --nocapture`
Expected: PASS.

**Known risk (major, disclosed not silently dropped):** neither the design nor this plan bundle the model with the app or add a download timeout/progress indicator. `tauri.conf.json`'s `bundle.resources` has no entry for a pre-bundled model, and `resolve_sherpa_transducer_model_paths` falls back to a live, unbounded HF Hub call whenever `VOX_ORATIO_SHERPA_MODEL_DIR` isn't set. Once Task 6 makes this the default "auto" path, a fresh install's first dictation attempt has a live network dependency and, on a cold cache, blocks synchronously on a ~671MB download with no progress UI. Task 6's fallback-to-Candle logic (Step 3 there) means a failed download does not crash the app, but it does mean the *first* utterance on a fresh, offline install silently blocks for as long as the HF client's own timeout allows before falling back — there is no explicit timeout configured here. Actually bundling the model or adding a bounded timeout + progress callback is tracked as a follow-up (see "Follow-ups"), not implemented in this task.

Then, separately (not part of the automated test — this hits the network), verify `DEFAULT_SHERPA_TRANSDUCER_HF_MODEL` actually resolves:

Run: `cargo run -p vox-speech --features stt-sherpa --example verify_sherpa_model 2>&1 || echo "no example yet — verify via a scratch main.rs calling resolve_sherpa_transducer_model_paths() and printing the result, or check https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8 directly in a browser for file listing"`

If the repo ID is wrong, browse `https://huggingface.co/models?search=sherpa-onnx-nemo-parakeet` to find the correct one and update `DEFAULT_SHERPA_TRANSDUCER_HF_MODEL` before proceeding to Task 5.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-speech/src/backends/sherpa_model_config.rs crates/vox-speech/src/lib.rs
git commit -m "feat: resolve NeMo transducer (Parakeet) model paths for sherpa-onnx"
```

---

### Task 5: Support transducer models in the Sherpa-ONNX backend

**Files:**
- Modify: `crates/vox-speech/src/backends/sherpa_onnx.rs`
- Test: inline `#[cfg(test)]` in the same file (new)

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-speech/src/backends/sherpa_onnx.rs` (new `#[cfg(test)]` block at the end):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transducer_config_variant_builds_recognizer_config() {
        // Construction-only test (no real ONNX files) — asserts the config
        // struct wiring is correct, not that a real model loads.
        let mut config = OfflineRecognizerConfig::default();
        config.model_config.transducer = sherpa_onnx::OfflineTransducerModelConfig {
            encoder: Some("encoder.onnx".to_string()),
            decoder: Some("decoder.onnx".to_string()),
            joiner: Some("joiner.onnx".to_string()),
            ..Default::default()
        };
        config.model_config.tokens = Some("tokens.txt".to_string());
        assert_eq!(
            config.model_config.transducer.encoder.as_deref(),
            Some("encoder.onnx")
        );
        assert_eq!(
            config.model_config.transducer.joiner.as_deref(),
            Some("joiner.onnx")
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-speech --features stt-sherpa --lib backends::sherpa_onnx::tests -- --nocapture`
Expected: FAIL to compile if `OfflineTransducerModelConfig` isn't yet imported in this file (it currently only imports `OfflineWhisperModelConfig`, per the file read during planning) — this is expected; Step 3 adds the import as part of the real implementation, not just the test.

- [ ] **Step 3: Add transducer support to `SherpaOnnxBackend::new`**

In `crates/vox-speech/src/backends/sherpa_onnx.rs`, change the imports and `new()`:

```rust
use super::asr_backend::{AsrBackend, AsrOutput};
use super::sherpa_model_config::{resolve_sherpa_model_paths, resolve_sherpa_transducer_model_paths};
use anyhow::Result;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig,
    OfflineWhisperModelConfig,
};
use std::sync::Mutex;
```

Replace the body of `impl SherpaOnnxBackend { pub fn new() -> Result<Self> { ... } }`:

```rust
impl SherpaOnnxBackend {
    /// Initialize the backend (downloads model if needed).
    ///
    /// Tries the NeMo transducer (Parakeet) path first — it is the default,
    /// faster, more accurate engine (see the STT accuracy design doc). Falls
    /// back to the Whisper-shaped config only when `VOX_ORATIO_SHERPA_KIND=whisper`
    /// is explicitly set, so existing local Whisper-model setups keep working.
    pub fn new() -> Result<Self> {
        let kind = std::env::var("VOX_ORATIO_SHERPA_KIND").unwrap_or_default();
        let mut config = OfflineRecognizerConfig::default();

        if kind.eq_ignore_ascii_case("whisper") {
            let paths = resolve_sherpa_model_paths()?;
            config.model_config.whisper = OfflineWhisperModelConfig {
                encoder: Some(paths.encoder.to_string_lossy().to_string()),
                decoder: Some(paths.decoder.to_string_lossy().to_string()),
                ..Default::default()
            };
            config.model_config.tokens = Some(paths.tokens.to_string_lossy().to_string());
        } else {
            let paths = resolve_sherpa_transducer_model_paths()?;
            config.model_config.transducer = OfflineTransducerModelConfig {
                encoder: Some(paths.encoder.to_string_lossy().to_string()),
                decoder: Some(paths.decoder.to_string_lossy().to_string()),
                joiner: Some(paths.joiner.to_string_lossy().to_string()),
                ..Default::default()
            };
            config.model_config.tokens = Some(paths.tokens.to_string_lossy().to_string());
        }
        config.model_config.num_threads = SHERPA_DEFAULT_THREADS as i32;
        config.model_config.debug = false;

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            anyhow::anyhow!("Sherpa-ONNX init failed (kind={kind:?}, check model paths)")
        })?;

        tracing::info!(
            target: "vox_oratio_sherpa",
            event = "sherpa_backend_init",
            kind = if kind.is_empty() { "transducer" } else { kind.as_str() },
            "Sherpa-ONNX backend initialized"
        );
        Ok(Self {
            inner: Mutex::new(recognizer),
        })
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-speech --features stt-sherpa --lib backends::sherpa_onnx::tests -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-speech/src/backends/sherpa_onnx.rs
git commit -m "feat: default sherpa-onnx backend to NeMo transducer (Parakeet), keep Whisper as opt-in"
```

---

### Task 6: Make Parakeet-via-sherpa the default GUI backend, with automatic fallback

**Files:**
- Modify: `crates/vox-gui/Cargo.toml:40`
- Modify: `crates/vox-speech/src/backend_dispatch.rs`
- Modify: `crates/vox-speech/src/traits.rs` (route the per-transcription call through the new cache — see Step 1)
- Test: inline `#[cfg(test)]` addition to `backend_dispatch.rs`

**Audit finding (blocker), read before starting this task:** `crate::backend_dispatch::create_backend()` is called fresh inside the per-file transcription path in `traits.rs` (line ~243) with no caching anywhere in `vox-speech`, and `vox-gui`'s mic-stop handler calls that path on every dictation stop. Once this task makes Sherpa-ONNX/Parakeet — a ~671MB 3-file int8 model plus a freshly-constructed ONNX Runtime session — the tried-first "auto" choice, every single utterance would pay full model-resolution + session-construction cost, not once at startup. Step 1 below fixes this before the default flips; do not skip it.

- [ ] **Step 1: Cache the ASR backend instance so it is created once, not per utterance**

1. Add a process-lifetime cache in `backend_dispatch.rs`. **Do not use `OnceLock<Mutex<Box<dyn AsrBackend>>>`**: `create_backend()` is fallible (`anyhow::Result<Box<dyn AsrBackend>>`), but `OnceLock::get_or_init` only accepts an infallible closure and stable Rust has no `get_or_try_init` — that type forces either panicking on first failure (losing retry forever) or abandoning `OnceLock` entirely, and there is no clean way to support retry-on-failure with it. Use a plain lazily-populated `Mutex<Option<Box<dyn AsrBackend>>>` instead, which supports retry naturally (a failed attempt just leaves the slot `None` for the next call to try again):
   ```rust
   static BACKEND: std::sync::Mutex<Option<Box<dyn AsrBackend>>> = std::sync::Mutex::new(None);

   /// Returns the cached backend, constructing it on first successful call.
   /// A construction failure leaves the cache empty so the *next* call
   /// retries `create_backend()` from scratch (e.g. a model download that
   /// failed once due to a transient network error can succeed later)
   /// instead of latching into a permanent "no backend" state until restart.
   pub fn cached_backend() -> anyhow::Result<std::sync::MappedMutexGuard<'static, Box<dyn AsrBackend>>> {
       // Illustrative signature only — `MappedMutexGuard` requires
       // `std::sync::MutexGuard::map` (stable) or an equivalent accessor
       // shape; pick whichever the actual call sites in `traits.rs` need
       // (e.g. returning a cloneable handle, or restructuring `AsrBackend`
       // calls to run while holding the guard) once you've read them.
       let mut guard = BACKEND.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
       if guard.is_none() {
           *guard = Some(create_backend()?);
       }
       Ok(std::sync::MutexGuard::map(guard, |b| b.as_mut().expect("just populated")))
   }
   ```
   (Check whether this crate already has a `once_cell`/`arc-swap` convention in `Cargo.toml` before hand-rolling further — if `arc_swap::ArcSwapOption` is already a dependency, it is a cleaner fit than a `Mutex<Option<_>>` guard for a value read on every transcription; otherwise the `Mutex<Option<_>>` shape above is sufficient and needs no new dependency.)
2. Update the call site in `traits.rs` (and any other direct `create_backend()` call in the per-transcription path) to go through `cached_backend()` instead.
3. Write a test in `backend_dispatch.rs`'s test module that exercises the accessor twice and asserts construction work happens only once (e.g. a `#[cfg(test)]`-visible atomic counter incremented inside `create_backend()`'s body, checked before/after two calls to `cached_backend()`). Confirm this test FAILS against the current un-cached baseline (each call re-runs full construction, so the counter would read 2, not 1) before adding the cache; confirm it PASSES after.
4. Resolved by the `Mutex<Option<_>>` shape above (not left open, per the earlier audit finding here): if the *first* `cached_backend()` call fails (e.g. no models, no network), the cache slot stays `None` and the *next* call retries `create_backend()` from scratch — the process never latches into a permanent "no backend" state until restart. Task 6's Sherpa→Candle fallback (Step 4 below) still runs on every retry attempt, inside `create_backend()`, before a result is ever cached. Add a test asserting this: force one failing construction (e.g. via an env var that makes `create_backend()` return `Err`), confirm `cached_backend()` returns `Err` but a subsequent call with the env var fixed succeeds.
5. Commit this step on its own (`perf: cache the ASR backend instance instead of reconstructing it per utterance`) so it can be reviewed/reverted independently of the default-backend switch below.

- [ ] **Step 2: Enable `stt-sherpa` alongside `stt-candle` in vox-gui**

In `crates/vox-gui/Cargo.toml`, change line 40:

```toml
vox-speech = { workspace = true, features = ["stt-candle", "stt-sherpa"] }
```

- [ ] **Step 3: Write a genuinely failing test for the fallback behavior**

The original draft of this test only asserted "`create_backend()` must not panic," which was already true before any change here (`create_backend` already returns `Result`, never panics) — that is a regression guard, not a red/green test, and the plan should not claim otherwise. Assert the actual *behavior* this step adds instead: today, "auto" mode returns Sherpa's `Err` directly (via `?`) and never tries Candle; after this step it must fall back. Use an empty (real, offline) model directory so the test is deterministic and never touches the network — see the audit note below.

**Audit finding (major), also fixed by this version of the test:** the original draft removed `VOX_ORATIO_BACKEND` but left `VOX_ORATIO_SHERPA_MODEL_DIR` unset, which falls through to Task 4's live Hugging Face Hub download branch — turning a nominally offline unit test into one that performs uncontrolled network I/O for a ~671MB model on every `cargo test` run. It also mutated the same `VOX_ORATIO_SHERPA_MODEL_DIR` env var as Task 4's test in a different file of the same test binary, with no lock — a real cross-file race. Both are fixed below: this test always sets `VOX_ORATIO_SHERPA_MODEL_DIR` to an empty temp dir (so `resolve_sherpa_transducer_model_paths` never reaches the network branch and `OfflineRecognizer::create` fails fast and deterministically), and it takes the shared lock added in Task 4 Step 1.

In `crates/vox-speech/src/backend_dispatch.rs`, add this test to a new `#[cfg(test)] mod tests` block at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_backend_auto_falls_back_to_candle_when_sherpa_init_fails() {
        // Held for the duration of the test: see `crate::env_test_lock` (Task
        // 4 Step 1) — this env var is also mutated by sherpa_model_config's test.
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let dir = tempfile::tempdir().expect("tempdir");
        // Deliberately empty: no real ONNX model files, so Sherpa-ONNX init
        // fails fast and offline. VOX_ORATIO_SHERPA_MODEL_DIR being set means
        // `resolve_sherpa_transducer_model_paths` short-circuits before the
        // HF Hub network branch entirely (see Task 4) — no network I/O here.
        // SAFETY: test-only env mutation, serialized by the lock above.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_BACKEND");
            std::env::set_var("VOX_ORATIO_SHERPA_MODEL_DIR", dir.path());
        }
        let result = create_backend();
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
        }
        assert!(
            result.is_ok(),
            "auto mode must fall back to Candle Whisper when Sherpa-ONNX init \
             fails (empty model dir), not propagate the Sherpa error: {:?}",
            result.err()
        );
    }
}
```

- [ ] **Step 4: Run test to verify it fails, then add fallback logging**

Run: `cargo test -p vox-speech --features "stt-candle stt-sherpa" --lib backend_dispatch::tests -- --nocapture`
Expected: FAIL — today's `"auto"` arm returns Sherpa's `Err` directly (see the audit finding above this task), so `result.is_ok()` is false with an empty model dir. This is the genuine red state; proceed to add the fallback-with-logging behavior below.

In `crates/vox-speech/src/backend_dispatch.rs`, replace the `"auto" | ""` match arm:

```rust
        "auto" | "" => {
            #[cfg(feature = "stt-sherpa")]
            {
                match crate::backends::sherpa_onnx::SherpaOnnxBackend::new() {
                    Ok(backend) => return Ok(Box::new(backend)),
                    Err(e) => {
                        tracing::warn!(
                            target: "vox_oratio_backend",
                            event = "sherpa_init_failed_falling_back",
                            error = %e,
                            "Sherpa-ONNX (Parakeet) init failed; falling back to Candle Whisper"
                        );
                        #[cfg(feature = "stt-candle")]
                        return Ok(Box::new(CandleWhisperBackend));
                        #[cfg(not(feature = "stt-candle"))]
                        return Err(e);
                    }
                }
            }
            #[cfg(all(feature = "stt-candle", not(feature = "stt-sherpa")))]
            {
                Ok(Box::new(CandleWhisperBackend))
            }
            #[cfg(not(any(feature = "stt-candle", feature = "stt-sherpa")))]
            anyhow::bail!(
                "No STT backend compiled in. Enable `stt-candle` or `stt-sherpa` feature."
            );
        }
```

This changes "auto" from "sherpa-or-nothing" (the old code returned sherpa's `Err` directly with `?`, never trying Candle) to "sherpa, falling back to Candle on any init failure" — the behavior the design's Phase 2 gate requires (native ONNX Runtime missing on some platform must not take down dictation entirely).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-speech --features "stt-candle stt-sherpa" --lib backend_dispatch::tests -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Manual cross-platform packaging verification — covers only the *packaging* half of the design's Phase 2 gate (not automatable — do by hand before merging)**

**Scope note (audit-driven disclosure):** the design's Phase 2 gate has two parts: (a) the eval harness shows Parakeet's WER/CER beating Candle-tiny.en on all three fixture manifests, and (b) the Tauri build packages and runs on all three OSes. This step implements only (b). Part (a) cannot be implemented by this plan — Task 1's harness is text-only and never invokes any ASR backend, so it structurally cannot produce a Candle-vs-Parakeet accuracy comparison (see "Deviations" at the top of this plan and "Follow-ups" at the end). Passing this step is *not* confirmation that Parakeet is more accurate than Candle on real audio — only that it packages and runs.

Build the Tauri app with `stt-sherpa` enabled and confirm it packages and launches on Windows, macOS, and Linux, since `sherpa-onnx` has never been linked into a shipped binary in this project before (it links the native ONNX Runtime shared library; Candle does not).

Run on each target platform: `cd crates/vox-gui/ui && pnpm tauri build`
Expected: build succeeds, and the packaged app launches without a missing-shared-library error on startup (check the OS-native crash/error dialog, or run the built binary from a terminal to see stderr if it fails silently).

**Windows-specific check, in addition to the above (major risk, previously undocumented):** `sherpa-onnx-sys` (pinned at 1.13.3 in `Cargo.lock`) links ONNX Runtime statically by default using the MSVC static CRT (`/MT`), which can conflict with the dynamic CRT (`/MD`) typically used by production Rust/MSVC builds — this is a documented issue in the upstream `k2-fsa/sherpa-onnx` project (see discussion #1202, "Linking and search path"), not something specific to Candle, which never exercised this path. On Windows specifically, also check the build/link output for CRT-mismatch linker errors (e.g. `LNK2038` mismatched `_ITERATOR_DEBUG_LEVEL` or duplicate-CRT warnings) in addition to the missing-shared-library check above. If this surfaces, consult `sherpa-onnx-sys`'s build-time env vars (e.g. a dynamic-CRT or `SHERPA_ONNX_LIB_DIR`-style override, check the crate's current docs/build.rs for the exact flag) before assuming the packaging gate has failed outright.

If packaging fails on any platform, do not proceed to Task 7. The originally-drafted remediation here ("revert Step 2's Cargo.toml change to keep `stt-sherpa` compiled in for the platforms where it works, defaulting the env var rather than the feature") is **not actually executable as written**: `crates/vox-gui/Cargo.toml`'s `vox-speech` dependency has a single, non-platform-conditional `features = [...]` list, so flipping it either enables `stt-sherpa` on every platform or none — there is no way to disable it "for just the platforms where it works" without introducing a real per-target dependency table (e.g. `[target.'cfg(windows)'.dependencies]`, following the existing precedent in `crates/vox-actor-runtime/Cargo.toml:54-55`), which is a larger change than this plan scopes. The actually-executable remediation is simpler: keep `stt-sherpa` compiled in for all platforms (Step 2's Cargo.toml change stands), but set the runtime default to `VOX_ORATIO_BACKEND=whisper` everywhere (not per-platform) until the packaging gap is fixed, and file the packaging gap as a separate follow-up with the specific failing platform(s) named.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/Cargo.toml crates/vox-speech/src/backend_dispatch.rs crates/vox-speech/src/traits.rs
git commit -m "feat: default to Parakeet-via-sherpa-onnx in the GUI, fall back to Candle Whisper on init failure, cache the backend instance"
```

---

### Task 7: Make code-dictation symbol expansion reachable in the GUI build

**Files:**
- Modify: `crates/vox-speech/Cargo.toml:16` (default features)
- Modify: `crates/vox-speech/src/speech_normalize.rs` (add missing spoken-symbol phrase mappings — see Step 3)
- Modify: `crates/vox-speech/src/transcript_rerank.rs` and/or `crates/vox-speech/src/traits.rs` (domain-gate the compiler-frontend penalty — see Step 4; the domain check may land in either file depending on where `ctx` is already in scope)

Finding #1: `pick_best_transcript_index_with_raw` is a no-op returning index 0 unless `compiler-rerank` is compiled in, and `compiler-rerank` is neither enabled by `vox-gui` nor a default feature. `compiler-rerank` pulls in `vox-compiler` (see `Cargo.toml:34`, `compiler-rerank = ["dep:vox-compiler"]`) to typecheck candidate hypotheses as Vox source — a real, non-trivial dependency, which is presumably why it was opt-in rather than default. **Corrected citation (an earlier draft of this paragraph cited the wrong crates):** `vox-orchestrator` and `vox-actor-runtime` do **not** depend on `vox-compiler` at all — neither `crates/vox-orchestrator/Cargo.toml` nor `crates/vox-actor-runtime/Cargo.toml` references it. The transitive edge that actually already links `vox-compiler` into the shipped `vox-gui` binary today runs through `vox-cli` (`crates/vox-cli/Cargo.toml:141`, a real non-dev, non-optional dependency) and `vox-orchestrator-mcp` (`crates/vox-orchestrator-mcp/Cargo.toml:66`, same) — both of which `vox-gui` depends on unconditionally (`crates/vox-gui/Cargo.toml:18,24`). (`vox-db` also references `vox-compiler`, but only under `[dev-dependencies]`, which never links into the shipped lib, so it does not support this claim either.) Given `vox-compiler` is already compiled into the shipped binary via that vox-cli/vox-orchestrator-mcp path, enabling it here does not introduce a new external dependency — it reuses an already-compiled-in crate.

- [ ] **Step 1: Enable `compiler-rerank` as a default feature**

In `crates/vox-speech/Cargo.toml`, change line 16:

```toml
default = ["compiler-rerank"]
```

- [ ] **Step 2: Enable it explicitly for vox-gui too (default features can be disabled by consumers; make the dependency explicit)**

In `crates/vox-gui/Cargo.toml` (the same line touched in Task 6 Step 2):

```toml
vox-speech = { workspace = true, features = ["stt-candle", "stt-sherpa", "compiler-rerank"] }
```

- [ ] **Step 3: Add missing spoken-symbol phrase mappings to `expand_spoken_symbols`**

**Audit finding (blocker, test-design), read before starting this step:** Task 1's harness cases `vox_code_002`–`vox_code_004` spoke "open curly"/"close curly", "comma", "colon colon" (for a double-colon), and (in a fix applied above) "open angle"/"close angle", and `vox_code_003` also spoke a bare "dot" for `self.user_state` — but `speech_normalize.rs`'s `expand_spoken_symbols` had no curly-brace alias, no comma mapping, no angle-bracket mapping, and no bare-`"dot"` mapping (only `"dot dot"`/`"dot dot dot"`) anywhere in the crate. Left unfixed, those literal English words would survive untouched in the pipeline's output no matter how correctly the rest of this plan is implemented — see the second Scope note in Task 1. Fix this before Step 4's domain gate and before Step 5 re-runs the Task-1 harness.

In `crates/vox-speech/src/speech_normalize.rs`, replace `expand_spoken_symbols`'s `pairs` list:

```rust
    let pairs: &[(&str, &str)] = &[
        ("open brace", "{"),
        ("close brace", "}"),
        ("open curly", "{"),
        ("close curly", "}"),
        ("open bracket", "["),
        ("close bracket", "]"),
        ("open angle", "<"),
        ("close angle", ">"),
        ("open paren", "("),
        ("close paren", ")"),
        ("fat arrow", "=>"),
        ("arrow", "->"),
        ("semicolon", ";"),
        ("colon colon", "::"),
        ("colon", ":"),
        ("comma", ","),
        ("new line", "\n"),
        ("underscore", "_"),
        ("double equals", "=="),
        ("not equals", "!="),
        ("dot dot dot", "..."),
        ("dot dot", ".."),
        ("dot", "."),
        ("bang", "!"),
        ("ampersand", "&"),
        ("pipe", "|"),
        ("asterisk", "*"),
        ("backslash", "\\"),
    ];
```

Ordering matters here and is preserved from the original list plus new entries placed so a longer phrase is always tried by its own pair before a shorter phrase that is one of its substrings could partially consume it: `"colon colon"` sits before the bare `"colon"` entry (so `"user state colon colon active"` becomes `"user state :: active"` in one shot, not two independently-placed colons), and `"dot"` sits after `"dot dot dot"`/`"dot dot"` (so a spoken ellipsis is never partially eaten by the new bare-dot mapping). `"open curly"`/`"close curly"` are added as aliases alongside the existing `"open brace"`/`"close brace"` rather than replacing them, since both phrasings are realistic ASR outputs for the same symbol.

Add a test to `speech_normalize.rs`'s existing `mod tests`:

```rust
    #[test]
    fn curly_comma_angle_and_bare_dot_expand() {
        assert_eq!(expand_spoken_symbols("open curly close curly"), "{ }");
        assert_eq!(expand_spoken_symbols("a comma b"), "a , b");
        assert_eq!(expand_spoken_symbols("open angle close angle"), "< >");
        assert_eq!(expand_spoken_symbols("self dot user"), "self . user");
        assert_eq!(expand_spoken_symbols("user state colon colon active"), "user state :: active");
        // Existing ellipsis mappings must still win over the new bare "dot".
        assert_eq!(expand_spoken_symbols("wait dot dot dot done"), "wait ... done");
    }
```

Run: `cargo test -p vox-speech --lib speech_normalize::tests -- --nocapture`
Expected: PASS (this is additive — no existing test asserts on the old, narrower `pairs` list's exact contents, so nothing regresses).

Commit this addition on its own:

```bash
git add crates/vox-speech/src/speech_normalize.rs
git commit -m "feat: add curly-brace/comma/angle-bracket/dot/colon-colon spoken-symbol mappings"
```

- [ ] **Step 4: Gate the compiler-frontend rerank pass to `DomainMode::Code` only, before enabling it by default**

**Audit finding (major):** `rerank_candidates_best_first_with_context` (`traits.rs:75-79`) invokes the compiler-rerank path unconditionally, with no check on `ctx.domain`, and `vox_frontend_penalty` (`transcript_rerank.rs:51-83`, under `#[cfg(feature = "compiler-rerank")]`) runs the full `vox_compiler` lex → parse → typecheck_ast_module → lower_module → validate_module pipeline per candidate (up to ~4 candidates per utterance). Making `compiler-rerank` a default feature (Step 1 above) without a domain gate means *every* dictation utterance — including plain-English chat dictation, not just code dictation — now pays a full compiler-frontend pass, with no latency measurement anywhere in this plan or the design doc. Fix this before Step 1 ships as default:

1. Read `traits.rs:60-90` and `transcript_rerank.rs:40-90` to find the exact call site and confirm where `ctx`/`ctx.domain` is already in scope relative to the `#[cfg(feature = "compiler-rerank")]` branch.
2. Add a test to `transcript_rerank.rs`'s existing test module: with `CorrectionContext { domain: DomainMode::General, .. }`, rerank a candidate set where the compiler-frontend penalty *would* change the chosen index (e.g. a plain-English candidate that fails to parse/typecheck as Vox source, versus a syntactically Vox-shaped candidate that is clearly the wrong hypothesis for general dictation) and assert the compiler penalty was **not** applied — pick the concrete assertion shape (return-value comparison, or a call-counter on the penalty function) that fits the actual function signatures once you've read them in step 1.
   Run: `cargo test -p vox-speech --features "compiler-rerank" --lib transcript_rerank::tests -- --nocapture`
   Expected: FAIL (today the compiler penalty runs regardless of domain).
3. Implement the gate: skip (or neutralize) the compiler-frontend penalty branch whenever `ctx.domain != DomainMode::Code`, falling back to the existing non-compiler heuristic scoring for all non-code dictation.
4. Re-run the test from step 2. Expected: PASS.
5. Re-run the pre-existing `rerank_prefers_parseable_vox_when_compiler_rerank_enabled` / `rerank_context_prefers_hotwords_without_compiler` tests (both written for `DomainMode::Code` per the file read during planning) to confirm the new gate doesn't affect them:
   Run: `cargo test -p vox-speech --features "stt-candle compiler-rerank" --lib transcript_rerank::tests -- --nocapture`
   Expected: PASS.
6. Commit this gate on its own (`fix: scope compiler-rerank's frontend penalty to code-domain dictation only`) before continuing to Step 5 below, so the latency-scoping fix is reviewable independently of the default-feature flip.

- [ ] **Step 5: Run the Task-1 harness to confirm it now passes**

Run: `cargo test -p vox-speech --features "stt-candle compiler-rerank" --test refine_regression -- --nocapture`
Expected: PASS — with `compiler-rerank` enabled, `normalize_spoken_code_phrase`'s symbol/casing output can now be selected by the reranker in the real pipeline (Task 1's harness calls `normalize_spoken_code_phrase` directly, so it was already exercising this code regardless of the feature flag; what actually changes for the SHIPPED GUI is that `pick_best_transcript_index_with_raw` stops being a hardcoded no-op, so the full `transcribe_path_detailed` → `build_transcript_candidates` → rerank pipeline now surfaces the symbol-expanded candidate instead of discarding it. Re-run this to confirm no regression from Task 2's rule changes plus this feature flip). This depends on Task 1's file existing, Step 3's mapping additions above, and Task 2's `code_confusion_map`/phrase-matching fix already being applied — the CER improvement traces to Task 2 and Step 3, not to this feature flip alone (see "Execution parallelism" at the top of this plan).

- [ ] **Step 6: Run vox-speech's existing `transcript_rerank` tests to confirm the feature-gated tests now exercise the enabled path**

Run: `cargo test -p vox-speech --features "stt-candle compiler-rerank" --lib transcript_rerank::tests -- --nocapture`
Expected: PASS, specifically `rerank_prefers_parseable_vox_when_compiler_rerank_enabled` and `rerank_context_prefers_hotwords_without_compiler` — both already have `#[cfg(feature = "compiler-rerank")]`-gated assertions written for exactly this state (per the file read during planning), so this is confirming existing test coverage activates correctly, not writing new tests.

- [ ] **Step 7: Full vox-gui build check**

Run: `cargo build -p vox-gui`
Expected: succeeds. This is the first time `vox-gui` compiles with `vox-compiler` linked in via `vox-speech`'s `compiler-rerank` — confirm no symbol conflicts or feature-unification surprises (Cargo unifies features across the dependency graph, so if any other crate in the tree disables a feature `vox-compiler` needs, this would surface here).

- [ ] **Step 8: Commit**

```bash
git add crates/vox-speech/Cargo.toml crates/vox-gui/Cargo.toml
git commit -m "feat: enable compiler-rerank by default so code-dictation symbol expansion reaches the GUI"
```

---

### Task 8: Expose ASR backend and domain-mode settings in the GUI

**Files:**
- Create: `crates/vox-gui/src/commands/stt_config.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs` (register the new module + commands — check this file's existing pattern for how `mic`/`oratio`/`user_config` are registered before editing, since the exact registration call site depends on how the Tauri `invoke_handler![...]` macro list is structured there)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`

**Audit finding (blocker), read before starting this task:** the original draft of this task persisted `VOX_ORATIO_BACKEND` purely through `vox_config::toml_config`'s flat file, reading it back via a raw `std::env::var`. But `VOX_ORATIO_BACKEND` is a *registered secret* (`vox_secrets::SecretId::VoxOratioBackend`), and `backend_dispatch.rs`'s `create_backend()` — the actual runtime consumer — resolves it via `vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioBackend).expose()`, which sources from env/SecureStore/Clavis/auth.json/populi-env, never from the flat config file. A Settings write that only touches the flat file, and a read that only checks raw env, would (a) violate this codebase's "no new direct secret env reads in consumers" convention and (b) silently disconnect the Settings UI from the value `create_backend()` actually uses — a user's backend choice in Settings would have no effect on real dictation behavior, and none of the originally-drafted tests would catch this since they only unit-test the DTO/validation logic in isolation. Fixed below: reads go through `vox_secrets::resolve_secret` for the backend key, and writes both persist to the flat config (for the next launch) **and** call `std::env::set_var` (for the current process), mirroring the established live-effect pattern already used in `commands/models.rs` (`:165,256,558`) — persistence alone does nothing until something re-reads it into `env`, and nothing in `main.rs` re-hydrates the flat file into process env at startup. `VOX_ORATIO_DOMAIN_MODE` is *not* a registered secret — `runtime_config.rs` reads it via a direct `std::env::var` call, so that key's read path is left as a plain env/flat-config lookup, but its write path gets the same `std::env::set_var` live-effect treatment for the same reason.

- [ ] **Step 1: Write the failing tests for the new command module**

Create `crates/vox-gui/src/commands/stt_config.rs`:

```rust
//! Tauri commands for the two highest-value STT/voice knobs in Settings:
//! ASR backend selection and dictation domain mode (general vs. code).
//!
//! `VOX_ORATIO_BACKEND` is a registered secret (`vox_secrets::SecretId::VoxOratioBackend`)
//! that `backend_dispatch::create_backend()` resolves via
//! `vox_secrets::resolve_secret(...).expose()` — reads here go through the
//! same resolver so the Settings UI shows the value actually in effect, not
//! a value from a different source. `VOX_ORATIO_DOMAIN_MODE` is a plain env
//! var read directly by `runtime_config.rs` (not a registered secret), so it
//! keeps the simpler env/flat-config lookup. Both writes persist to
//! `vox_config::toml_config` (so the choice survives restart, the same
//! mechanism `user_config.rs`'s `FlatToml`-tier keys use) *and* call
//! `std::env::set_var` so the change takes effect in the running process —
//! mirroring `commands/models.rs`'s established live-effect pattern
//! (`:165,256,558`). These are not added to the LLM/AI-scoped
//! `vox-llm-config` registry: that registry's own header comment scopes it
//! to "Band A" provider/model/tuning/budget keys and explicitly excludes
//! other config domains — STT is not Band A.

use serde::Serialize;
use tauri::command;

/// One STT setting field, mirroring the shape of `UserConfigFieldDto` in
/// `user_config.rs` closely enough that `SettingsView.tsx` can reuse its
/// existing `Row` + enum-button rendering pattern.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttConfigFieldDto {
    pub key: String,
    pub label: String,
    pub hint: String,
    pub options: Vec<String>,
    pub current_value: String,
}

const BACKEND_KEY: &str = "VOX_ORATIO_BACKEND";
const BACKEND_OPTIONS: [&str; 3] = ["auto", "whisper", "sherpa"];
const DOMAIN_KEY: &str = "VOX_ORATIO_DOMAIN_MODE";
const DOMAIN_OPTIONS: [&str; 2] = ["general", "code"];

fn flat_config_fallback(key: &str, default: &str) -> String {
    let cfg = vox_config::toml_config::load_user_config();
    cfg.values
        .get(key)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| default.to_string())
}

/// Current effective backend value — resolved the same way
/// `backend_dispatch::create_backend()` resolves it, not from a separate
/// source the Settings UI would drift from. `vox_secrets::resolve_secret`
/// returns a `ResolvedSecret` directly (not a `Result`), and its `expose()`
/// returns `Option<&str>` — mirror the chained pattern already used in
/// `backend_dispatch.rs:17-20` and `traits.rs`
/// (`resolve_secret(...).expose().map(|s| s.to_string()).unwrap_or_else(...)`),
/// not an `if let Ok(...) = ...` match, which does not type-check against
/// `ResolvedSecret`.
fn current_backend_value() -> String {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioBackend)
        .expose()
        .map(str::to_string)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| flat_config_fallback(BACKEND_KEY, "auto"))
}

/// Current effective domain-mode value. `VOX_ORATIO_DOMAIN_MODE` is not a
/// registered secret — mirror `runtime_config.rs`'s own direct-env-var read
/// rather than routing through `vox_secrets`.
fn current_domain_value() -> String {
    if let Ok(v) = std::env::var(DOMAIN_KEY)
        && !v.is_empty()
    {
        return v;
    }
    flat_config_fallback(DOMAIN_KEY, "general")
}

/// Read the two STT settings for the Settings UI.
#[command]
pub fn get_stt_config() -> Vec<SttConfigFieldDto> {
    vec![
        SttConfigFieldDto {
            key: BACKEND_KEY.to_string(),
            label: "Voice dictation engine".to_string(),
            hint: "auto picks Parakeet (sherpa-onnx) when available, falling back to Whisper".to_string(),
            options: BACKEND_OPTIONS.iter().map(|s| s.to_string()).collect(),
            current_value: current_backend_value(),
        },
        SttConfigFieldDto {
            key: DOMAIN_KEY.to_string(),
            label: "Dictation domain".to_string(),
            hint: "code enables symbol/casing expansion (\"open paren\" -> \"(\")".to_string(),
            options: DOMAIN_OPTIONS.iter().map(|s| s.to_string()).collect(),
            current_value: current_domain_value(),
        },
    ]
}

/// Persist one STT setting AND make it take effect immediately in the
/// running process. Both keys are plain enums validated against a fixed
/// option list.
#[command]
pub fn set_stt_config(key: String, value: String) -> Result<(), String> {
    let valid = match key.as_str() {
        BACKEND_KEY => BACKEND_OPTIONS.contains(&value.as_str()),
        DOMAIN_KEY => DOMAIN_OPTIONS.contains(&value.as_str()),
        _ => return Err(format!("unknown STT config key: {key}")),
    };
    if !valid {
        return Err(format!("{value} is not a valid value for {key}"));
    }
    // Persist for the next launch...
    vox_config::toml_config::set_user_config_value(&key, &value)?;
    // ...and take effect immediately: config persistence alone does nothing
    // until something re-reads it into env, and nothing re-hydrates the flat
    // file into process env at startup. Mirrors commands/models.rs's
    // established live-effect pattern (:165,256,558).
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(&key, &value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_stt_config_returns_both_keys_with_defaults() {
        let fields = get_stt_config();
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.key == BACKEND_KEY));
        assert!(fields.iter().any(|f| f.key == DOMAIN_KEY));
    }

    #[test]
    fn set_stt_config_rejects_unknown_key() {
        assert!(set_stt_config("NOT_A_KEY".to_string(), "x".to_string()).is_err());
    }

    #[test]
    fn set_stt_config_rejects_invalid_value() {
        assert!(set_stt_config(BACKEND_KEY.to_string(), "not_a_backend".to_string()).is_err());
    }

    #[test]
    fn set_stt_config_takes_effect_immediately_in_process_env() {
        // Regression test for the audit finding above this task: a Settings
        // write must be visible to the same process's runtime resolvers
        // without a restart, not just persisted to the flat config file.
        // NOTE: mutates a real env var read by other tests/processes in this
        // binary; if this proves flaky under parallel test execution, adopt
        // this crate's `env_test_lock`-style pattern (see vox-speech's Task 4
        // Step 1) to serialize it against any other test touching this key.
        let original = std::env::var(DOMAIN_KEY).ok();
        set_stt_config(DOMAIN_KEY.to_string(), "code".to_string()).expect("valid set");
        assert_eq!(std::env::var(DOMAIN_KEY).as_deref(), Ok("code"));
        #[allow(unsafe_code)]
        unsafe {
            match &original {
                Some(v) => std::env::set_var(DOMAIN_KEY, v),
                None => std::env::remove_var(DOMAIN_KEY),
            }
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (module not yet registered — expect a compile scope issue if you add a `mod` line before the file exists, or run standalone)**

Run: `cargo test -p vox-gui --lib commands::stt_config::tests -- --nocapture`
Expected: fails to find the module until Step 3 registers it in `mod.rs`.

- [ ] **Step 3: Register the module and its two commands**

In `crates/vox-gui/src/commands/mod.rs`, add next to the existing `pub mod user_config;` (line 46):

```rust
pub mod stt_config;
```

In `crates/vox-gui/src/main.rs`, the `tauri::generate_handler![...]` list registers `user_config`'s commands at lines 228-231:

```rust
            commands::user_config::get_user_config,
            commands::user_config::set_user_config,
            commands::user_config::reset_user_config,
            commands::user_config::get_llm_spend,
```

Add immediately after line 231:

```rust
            commands::stt_config::get_stt_config,
            commands::stt_config::set_stt_config,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-gui --lib commands::stt_config::tests -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Wire the Settings UI reference first, before the component exists (red state)**

The original draft of this task added the `SttSettingsSection` component and wired it into `SettingsView.tsx` in one shot, with no failing check beforehand and only a manual "run the dev app and look" verification afterward — inconsistent with the red/green sequencing used for every Rust-side step in this plan. TypeScript's compiler gives a real, automatable red/green pair here: wire the *reference* to the component first, before the component is defined.

In `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`, first add just the `SECTIONS` entry and the render-body branch (not the component definition yet):

```tsx
  { id: 'runtime',      icon: 'flow',    label: 'Runtime' },
  { id: 'voice',        icon: 'bolt',    label: 'Voice & dictation' },
```

```tsx
        {section === 'runtime' && <RuntimeConfigSection pushToast={pushToast} />}

        {section === 'voice' && <SttSettingsSection pushToast={pushToast} />}

        {section === 'mesh' && <MeshPeersSection pushToast={pushToast} />}
```

Run this repo's TypeScript check for the GUI's UI package (confirm the exact script name in `crates/vox-gui/ui/package.json` — likely `pnpm --dir crates/vox-gui/ui exec tsc --noEmit` or a `pnpm --dir crates/vox-gui/ui run typecheck` script):
Expected: FAIL — `Cannot find name 'SttSettingsSection'` (or equivalent "used before defined" / "no such module member" error), since the component doesn't exist yet. This is the genuine red state.

- [ ] **Step 6: Define the component (green state)**

Add the component itself, near `LlmSettingsSection` in the same file, following its exact pattern — enum buttons via inline-styled buttons, not a new shared component, matching how `RuntimeConfigSection`'s `control()` renders `kind === 'enum'` fields:

```tsx
interface SttConfigFieldDto {
  key: string;
  label: string;
  hint: string;
  options: string[];
  currentValue: string;
}

function SttSettingsSection({ pushToast }: { pushToast: (t: Toast) => void }) {
  const [fields, setFields] = useState<SttConfigFieldDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setFields(await invoke<SttConfigFieldDto[]>('get_stt_config'));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Could not load voice settings', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { reload(); }, [reload]);

  const save = async (key: string, value: string) => {
    setBusy(key);
    try {
      await invoke('set_stt_config', { key, value });
      await reload();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">Voice &amp; dictation</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">ASR engine and dictation domain for the mic button in chat.</p>
      {loading ? (
        <div className="mt-4 text-[12px] text-text-muted">Loading…</div>
      ) : (
        <div className="mt-4 space-y-2">
          {fields.map(f => (
            <Row key={f.key} label={f.label} hint={f.hint}>
              <div className="inline-flex flex-wrap items-center rounded-md border border-border-subtle bg-black/30 p-0.5">
                {f.options.map(opt => (
                  <button
                    key={opt}
                    type="button"
                    disabled={busy === f.key}
                    onClick={() => save(f.key, opt)}
                    className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.12em] transition disabled:opacity-40 ${
                      f.currentValue === opt ? 'bg-overlay-subtle text-text-primary' : 'text-text-muted hover:text-text-secondary'
                    }`}
                  >{opt}</button>
                ))}
              </div>
            </Row>
          ))}
        </div>
      )}
    </>
  );
}
```

(The `SECTIONS` entry and render-body branch referencing this component were already added in Step 5 — that's the point of the red/green split: the reference came first and failed to compile, this definition makes it resolve.)

Re-run the same TypeScript check from Step 5:
Expected: PASS — `SttSettingsSection` now resolves and the file type-checks cleanly. This is the automated green state; it confirms the wiring compiles, though it does not exercise the Tauri IPC round-trip (Step 7 below covers that by hand).

- [ ] **Step 7: Manual verification in the running app**

The compile-time check in Steps 5–6 confirms the component wiring is well-typed, but STT settings still can't be verified via the automated browser preview (no real Tauri IPC, no mic) — this step is a supplementary manual check of actual runtime behavior, not the sole verification gate for this task (that gate is Step 6's passing type-check plus Step 4's passing Rust tests).

Run: `cd crates/vox-gui/ui && pnpm tauri dev`
Then: open Settings, click "Voice & dictation", change the backend and domain enum buttons, confirm the selection persists across a Settings-panel remount (switch to another section and back).

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/src/commands/stt_config.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
git commit -m "feat: expose ASR backend and dictation domain in GUI Settings"
```

---

## Follow-ups explicitly out of scope (tracked, not silently dropped)

- Wiring Loquela's push-to-talk mic button to the `oratio` plugin-hosted path instead of the direct-Candle `mic.rs` path — tracked as a spawned follow-up task (`task_f971226b`), since it needs its own design (interaction-model mismatch between fixed-duration and push-to-talk capture).
- Full audio-based ASR accuracy comparison between Candle-Whisper-tiny and Parakeet on real recorded speech, i.e. the accuracy half of the design's Phase 2 gate (this plan's Task 1 harness is text-only by necessity and Task 6 Step 6 implements only the packaging half of that gate — see "Deviations" above and the Scope note in Task 6 Step 6).
- Correction-aggressiveness (`Conservative`/`Balanced`/`Aggressive`) and custom-lexicon-entry exposure in Settings (Task 8 scoped to 2 of the design's 3 proposed knobs).
- Extending the eval regression harness to the other two fixture manifests (`librispeech_test_clean.jsonl`, `blender_films.jsonl`) — Task 1 covers only the code-dictation corpus, the highest-value one for this product.
- Spoken-numeral (`"zero"` → `"0"`, `"two hundred"` → `"200"`), bare-`"equals"` → `"="`, and letter-spelled-acronym (`"h t t p"` → `"HTTP"`) expansion in `speech_normalize.rs` — needed for full coverage of the `vox_code_001`/`vox_code_005` corpus entries; Task 1's harness was rescoped (see its "Scope note") to avoid depending on these until they're implemented, since none of Tasks 2–7 add them and adding them is a distinct feature, not a bugfix.
- Idiomatic Rust-formatter spacing (e.g. `"count : i32"` vs. `"count: i32"`) and identifier-casing inference (PascalCase type/variant names vs. snake_case field/function names generated from the same spoken phrase, e.g. `"user state"` needing to become `UserState` in one spot and `user_state` in another) — neither is implemented anywhere in this codebase. Task 1's harness (see its second "Scope note") gates on case-folded, whitespace-insensitive `char_error_rate` specifically to avoid depending on either until they're implemented; a real fix needs a code-aware formatting/casing pass, which is a distinct feature, not a bugfix in scope for this plan.
- Model-artifact integrity verification (checksum/signature pinning) for the Sherpa-ONNX/Parakeet download path added in Task 4 and made the default, tried-first path in Task 6 — today's download has no integrity check, same as the pre-existing (never-shipped) `resolve_sherpa_model_paths`.
- Bundling the Parakeet model with the app (or adding a bounded download timeout + progress UI) instead of relying on an unbounded live Hugging Face Hub fetch on first use — see the "Known risk" note in Task 4 Step 4.
- CC-BY-4.0 attribution surface (NOTICE file, About/Licenses panel, or README credit) for the Parakeet-TDT model this plan makes the default — the design doc's licensing note ("commercial use is unrestricted") does not mention the attribution condition CC-BY-4.0 actually carries, and no such surface exists in the product today to extend; out of scope for this plan since it touches licensing/legal copy, not code.
