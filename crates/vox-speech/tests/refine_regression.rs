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
