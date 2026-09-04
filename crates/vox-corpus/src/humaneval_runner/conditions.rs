//! Prompt conditions C0-C3.
//!
//! Frontier models have essentially never seen Vox, so a zero-shot prompt
//! measures guessability from Rust/Python surface similarity — a floor near
//! zero that says nothing about the model or the language. The scientifically
//! meaningful measurement is in-context acquisition of an unseen grammar.
//! Reported as separate columns, never averaged: the C0->C3 lift is the
//! headline result, because it is the falsifiable form of "Vox is a good LLM
//! target". `context_hash` is part of row identity — without it a later doc
//! edit silently changes every score and no run is comparable to any other.
//!
//! Uses `vox_compiler::llm_prompt::vox_grammar_prompt()` (which delegates to
//! `vox-grammar-export`'s compact prompt) rather than depending on
//! `vox-grammar-export` directly: `vox-corpus` already depends on
//! `vox-compiler`, so this adds zero new workspace crate edges.
//!
//! See `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md` §S3.

use anyhow::Result;
use std::path::Path;

/// How much Vox reference material the model receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Condition {
    C0ZeroShot,
    C1Grammar,
    C2FewShot,
    C3FullDocs,
}

impl Condition {
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::C0ZeroShot => "C0",
            Self::C1Grammar => "C1",
            Self::C2FewShot => "C2",
            Self::C3FullDocs => "C3",
        }
    }

    /// Parse the CLI-facing condition id (`"C0"`..`"C3"`).
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "C0" => Ok(Self::C0ZeroShot),
            "C1" => Ok(Self::C1Grammar),
            "C2" => Ok(Self::C2FewShot),
            "C3" => Ok(Self::C3FullDocs),
            other => anyhow::bail!("unknown condition `{other}` — expected one of C0, C1, C2, C3"),
        }
    }
}

/// Resolved context for one condition, with the hash that pins row identity.
#[derive(Debug, Clone)]
pub struct PromptContext {
    pub condition: Condition,
    pub context_text: String,
    pub context_hash: String,
}

fn hash_context(condition_id: &str, text: &str) -> String {
    // FNV-1a: adequate for a row-identity fingerprint (collision resistance,
    // not cryptographic secrecy), and avoids adding a new crate dependency.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in condition_id.bytes().chain(text.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Assemble the context for `condition`.
///
/// C1 uses `vox-grammar-export`'s compact LLM prompt (via `vox-compiler`),
/// which is the documented SSOT for this and is ~780 tokens.
pub fn build_context(condition: Condition, repo_root: &Path) -> Result<PromptContext> {
    let text = match condition {
        Condition::C0ZeroShot => String::new(),
        Condition::C1Grammar => vox_compiler::llm_prompt::vox_grammar_prompt(),
        Condition::C2FewShot => format!(
            "{}\n\n## Worked examples\n\n{}",
            vox_compiler::llm_prompt::vox_grammar_prompt(),
            std::fs::read_to_string(repo_root.join("examples/golden/ref_syntax.vox"))
                .unwrap_or_default()
        ),
        Condition::C3FullDocs => format!(
            "{}\n\n## Worked examples\n\n{}\n\n## Syntax reference\n\n{}",
            vox_compiler::llm_prompt::vox_grammar_prompt(),
            std::fs::read_to_string(repo_root.join("examples/golden/ref_syntax.vox"))
                .unwrap_or_default(),
            std::fs::read_to_string(repo_root.join("docs/src/reference/ref-syntax.md"))
                .unwrap_or_default()
        ),
    };
    let context_hash = hash_context(condition.id(), &text);
    Ok(PromptContext {
        condition,
        context_text: text,
        context_hash,
    })
}

/// The single-turn prompt. Identical across conditions except for the context
/// block, so the condition is the only manipulated variable.
#[must_use]
pub fn build_prompt(ctx: &PromptContext, signature: &str, task: &str) -> String {
    let preamble = if ctx.context_text.is_empty() {
        String::new()
    } else {
        format!(
            "Here is a reference for the Vox language:\n\n{}\n\n---\n\n",
            ctx.context_text
        )
    };
    format!(
        "{preamble}Write a Vox function with EXACTLY this signature:\n\n    {signature}\n\n\
         Task: {task}\n\n\
         Rules:\n\
         - Reply with ONLY Vox source code. No prose, no explanation, no markdown fences.\n\
         - Do NOT write a `fn main()`; only the function above plus any helpers it needs.\n\
         - Do NOT define or rebind `assert`, `main`, or `print`.\n\
         - Do not read files or access the network."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn c0_supplies_no_language_context() {
        let ctx = build_context(Condition::C0ZeroShot, &root()).unwrap();
        assert!(ctx.context_text.is_empty(), "C0 is the no-context control");
    }

    #[test]
    fn c1_includes_the_compact_grammar_and_is_small() {
        let ctx = build_context(Condition::C1Grammar, &root()).unwrap();
        assert!(!ctx.context_text.is_empty(), "C1 must carry the grammar");
        assert!(
            ctx.context_text.contains("fn"),
            "grammar mentions declarations"
        );
        assert!(
            ctx.context_text.len() < 8_000,
            "compact grammar must stay compact: {} chars",
            ctx.context_text.len()
        );
    }

    #[test]
    fn conditions_have_distinct_stable_hashes() {
        let a = build_context(Condition::C0ZeroShot, &root()).unwrap();
        let b = build_context(Condition::C1Grammar, &root()).unwrap();
        assert_ne!(
            a.context_hash, b.context_hash,
            "different context -> different row identity"
        );
        let a2 = build_context(Condition::C0ZeroShot, &root()).unwrap();
        assert_eq!(
            a.context_hash, a2.context_hash,
            "same context -> stable hash"
        );
    }

    #[test]
    fn prompt_pins_the_signature_and_forbids_a_main() {
        let ctx = build_context(Condition::C0ZeroShot, &root()).unwrap();
        let p = build_prompt(&ctx, "fn nth_prime(n: int) to int", "Return the nth prime.");
        assert!(p.contains("fn nth_prime(n: int) to int"));
        assert!(p.contains("Return the nth prime."));
        let lower = p.to_lowercase();
        assert!(
            lower.contains("do not") && lower.contains("main"),
            "must forbid a candidate main"
        );
        assert!(
            lower.contains("assert"),
            "must forbid redefining assert (the C1 exploit)"
        );
    }

    #[test]
    fn parse_round_trips_every_id() {
        for c in [
            Condition::C0ZeroShot,
            Condition::C1Grammar,
            Condition::C2FewShot,
            Condition::C3FullDocs,
        ] {
            assert_eq!(Condition::parse(c.id()).unwrap(), c);
        }
        assert!(Condition::parse("C9").is_err());
    }
}
