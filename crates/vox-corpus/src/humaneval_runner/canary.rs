//! Detection of candidates that neutralize the scoring oracle.
//!
//! A candidate can rebind `assert` so every fixture assertion becomes a no-op,
//! scoring 100% with a wrong answer. Empirically verified (2026-09-01, vox
//! 0.6.0): a top-level `fn assert(...)` does NOT neutralize — the builtin
//! wins — but `let assert = fn(c: bool) to bool { return true }` DOES, as do
//! arity and return-type variants. Enumerating forms is therefore a losing
//! game.
//!
//! Instead we detect the effect: run the candidate against `assert(false)`.
//! An honest candidate fails it; a neutralized one passes. This is exact,
//! form-independent, and costs one short subprocess per candidate.
//!
//! See `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md` §C1.

use anyhow::Result;
use std::path::Path;
use std::time::Duration;

/// Identifiers a candidate may never bind: the scoring oracle and the entry
/// point the fixture supplies.
const RESERVED: &[&str] = &["assert", "main", "print"];

/// Cheap first-line rejection of a candidate that rebinds a reserved name.
///
/// Returns `Some(reason)` when the candidate must be refused before it is ever
/// run. Word-boundary matched so `asserted` / `assertion_count` / `mainline`
/// are unaffected.
#[must_use]
pub fn rejects_at_ingest(candidate: &str) -> Option<String> {
    for line in candidate.lines() {
        let t = line.trim_start();
        for kw in ["let ", "fn "] {
            if let Some(rest) = t.strip_prefix(kw) {
                let rest = rest.trim_start().trim_start_matches("mut ");
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if RESERVED.contains(&name.as_str()) {
                    return Some(format!("candidate rebinds reserved name `{name}`"));
                }
            }
        }
    }
    None
}

/// The candidate plus a `main` whose single assertion must fail.
#[must_use]
pub fn canary_program(candidate: &str) -> String {
    format!(
        "{}\n\nfn main() to str {{\n    assert(false)\n    return \"ok\"\n}}\n",
        candidate.trim_end()
    )
}

/// True when the candidate neutralized the oracle: `assert(false)` passed.
///
/// A compile failure is NOT neutralization — it returns `false`, and the
/// normal verification path records the compile error. Only a clean exit 0 on
/// a must-fail assertion indicates cheating.
pub fn is_oracle_neutralized(
    vox_bin: &Path,
    candidate: &str,
    workdir: &Path,
    timeout: Duration,
) -> Result<bool> {
    let outcome = super::verify::run_program(
        vox_bin,
        &canary_program(candidate),
        workdir,
        "canary",
        timeout,
    )?;
    Ok(outcome.compiled && outcome.ran_ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canary_program_appends_a_must_fail_assertion() {
        let p = canary_program("fn f() to int { return 1 }");
        assert!(p.contains("fn f() to int"), "candidate preserved");
        assert!(p.contains("assert(false)"), "canary assertion present");
        assert_eq!(p.matches("fn main").count(), 1, "exactly one main");
    }

    #[test]
    fn ingest_rejects_rebinding_the_oracle() {
        assert!(rejects_at_ingest("let assert = fn(c: bool) to bool { return true }").is_some());
        assert!(
            rejects_at_ingest("  let  assert  =  fn(c: bool) to bool { return true }").is_some()
        );
        assert!(rejects_at_ingest("fn assert(c: bool) to bool { return true }").is_some());
        assert!(rejects_at_ingest("fn main() to str { return \"x\" }").is_some());
        assert!(rejects_at_ingest("let print = fn(s: str) to bool { return true }").is_some());
    }

    #[test]
    fn ingest_allows_honest_solutions_including_similar_names() {
        assert!(rejects_at_ingest("fn nth_prime(n: int) to int { return 2 }").is_none());
        // Must not false-positive on identifiers that merely contain the words.
        assert!(rejects_at_ingest("let asserted = 1").is_none());
        assert!(rejects_at_ingest("fn assertion_count() to int { return 0 }").is_none());
        assert!(rejects_at_ingest("let mainline = 3").is_none());
    }
}
