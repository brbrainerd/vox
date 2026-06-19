---
title: "Subsystem B — Local Pre-Publish Skill Review Gate (vox-skill-review)"
description: "Antigravity/Gemini-3.5-Flash-executable TDD plan for vox-skill-review (L3): a local, advisory, pre-publish review gate that parses a candidate SKILL.md, runs a deterministic floor (frontmatter completeness, stub/placeholder detection, MCP-tool SSOT validation, dedup-vs-installed), proposes auto-tags, and emits a severity-graded verdict (gate-before-listing). Reuses vox-skill-discovery's validate_ssot + dedup_skills and vox-plugin-host's skill parser. Local-first differentiator; no server, no network. The vox-code-audit LLM pass is a documented optional follow-up."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Subsystem B — Local Pre-Publish Skill Review Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** A local, advisory gate that reviews a candidate skill before it could ever be published — catching stubs, incomplete frontmatter, drifted/phantom MCP tools, and duplicates of installed skills — and proposes auto-tags + a severity-graded verdict.

**Architecture:** One new crate `vox-skill-review` (L3) with a pure-ish library (`review_skill`) + a standalone `vox-skill-review` binary. It reuses the skill parser (`vox_plugin_host::skill_parser::parse_skill_md`) and the discovery engine (`vox_skill_discovery::validate_ssot`, `::dedup_skills`). The deterministic floor runs offline; **no install/execute/publish path** and **no network** (the LLM review is a documented optional follow-up, not in v1). Gate-before-listing: any `Error`/`Critical` finding ⇒ verdict `NeedsHuman`.

**Tech Stack:** Rust; `vox-plugin-host`, `vox-plugin-types`, `vox-skill-discovery`, `serde`/`serde_json`, `anyhow`, `clap`.

**Execution target:** Gemini 3.5 Flash in Antigravity. Basis: `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`. Grounding: `docs/src/architecture/skill-code-marketplace-research-and-audit-2026-06-18.md` §2.A/§3 (deterministic-floor-first; gate-before-listing; tiered trust — automation is a floor, not the sole gate).

## Operating rules (every task — includes AGH-0001 hardenings)
1. Atomic + green + committed. 2. Verify-before-use. 3. Self-contained. 4. Two-strike circuit breaker. 5. `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags. 6. **No unplanned shared-config edits** beyond this crate's own registration (AGH-0001 §B-2) — if `vox-arch-check` is red for an unrelated reason, STOP and report. 7. **Branch isolation + delivery manifest** (AGH-0001 §B-3/4) — fresh branch off `origin/main`; list every file changed. 8. House rules: no `cargo fmt --all`; no-stub via `rg`; no `vox stub-check`; new crate needs a `where-things-live.md` row + `orphan_exempt` (binary/leaf, no in-tree dependents) — these are `error`-level arch rules.

Per-task ritual: `cargo test -p vox-skill-review` → `cargo clippy -p vox-skill-review -- -D warnings` → stub `rg` → `cargo fmt -p vox-skill-review`.

## Pre-flight (run once)
- [ ] `git fetch origin main && git switch -c claude/skill-review-gate origin/main`.
- [ ] `rg -n 'pub fn parse_skill_md|pub struct ParseSkillError' crates/vox-plugin-host/src/skill_parser.rs` — confirm `parse_skill_md(&str) -> Result<VoxSkillBundle, ParseSkillError>`.
- [ ] `rg -n 'pub struct VoxSkillBundle|pub manifest|pub skill_md|pub fn new' crates/vox-plugin-host/src/skill_bundle.rs` — confirm `VoxSkillBundle { manifest: SkillManifest, skill_md: String, .. }`. The body is the **public field `skill_md`** (full SKILL.md text), accessed as `&bundle.skill_md` — there is NO `body()` method.
- [ ] `rg -n 'pub fn validate_ssot|pub fn dedup_skills' crates/vox-skill-discovery/src/lib.rs` — confirm both are re-exported at the crate root.
- [ ] `rg -n 'where_things_live|orphan' docs/src/architecture/layers.toml` — confirm both rules are `error` (so this crate needs a WTL row + `orphan_exempt`).
- [ ] `cargo run -p vox-arch-check` — baseline must pass on the fresh branch.

---

## Task 1: Scaffold `vox-skill-review` + register [SEQUENTIAL]

**Files:**
- Create: `crates/vox-skill-review/Cargo.toml`, `crates/vox-skill-review/src/lib.rs`
- Modify: `Cargo.toml` (`[workspace.dependencies]`), `docs/src/architecture/layers.toml`, `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Cargo.toml.**
```toml
[package]
name = "vox-skill-review"
version = "0.1.0"
edition = "2021"
description = "Local advisory pre-publish review gate for VoxSkills: frontmatter, stub, MCP-SSOT, and dedup checks with severity-graded verdict."

[dependencies]
vox-plugin-host = { workspace = true }
vox-plugin-types = { workspace = true }
vox-skill-discovery = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
clap = { workspace = true }

[[bin]]
name = "vox-skill-review"
path = "src/bin/vox_skill_review.rs"
```

- [ ] **Step 2: lib.rs (doc comment only for now — modules added by later tasks).**
```rust
//! Local advisory pre-publish review gate for VoxSkills. Deterministic floor only
//! (no network, no execution); the LLM review pass is an optional follow-up.
```

- [ ] **Step 3: Create the bin stub** `crates/vox-skill-review/src/bin/vox_skill_review.rs`:
```rust
fn main() {
    println!("vox-skill-review: not yet wired");
}
```

- [ ] **Step 4: Register.** Root `Cargo.toml` `[workspace.dependencies]` (near `vox-skill-discovery`): `vox-skill-review = { path = "crates/vox-skill-review" }`. `layers.toml` (L3 group): `vox-skill-review = { layer = 3, orphan_exempt = true }`. `where-things-live.md` (crate table, mirror the format): a row pointing at `crates/vox-skill-review/` described as "Local advisory pre-publish skill review gate (frontmatter/stub/SSOT/dedup)."

- [ ] **Step 5: Verify + commit.** `cargo check -p vox-skill-review`; `cargo run -p vox-arch-check` (exits 0);
```bash
git add crates/vox-skill-review Cargo.toml docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "feat(vox-skill-review): scaffold local skill review gate + register"
```

---

## Task 2: Finding + Verdict + Report model [SEQUENTIAL]

**Files:**
- Create: `crates/vox-skill-review/src/model.rs`
- Modify: `crates/vox-skill-review/src/lib.rs`

- [ ] **Step 1: Write `model.rs` with a test.**
```rust
//! Severity-graded review findings + the overall verdict.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub severity: Severity,
    pub rule: String,    // e.g. "frontmatter/missing-description"
    pub message: String,
}

/// Gate-before-listing verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// No Error/Critical findings — safe to auto-list at the community tier.
    Pass,
    /// At least one Error/Critical — must escalate to a human reviewer.
    NeedsHuman,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewReport {
    pub skill_id: String,
    pub items: Vec<ReviewItem>,
    pub suggested_tags: Vec<String>,
    pub verdict: Verdict,
}

impl ReviewReport {
    /// Verdict from the highest-severity item (gate-before-listing).
    pub fn verdict_for(items: &[ReviewItem]) -> Verdict {
        if items.iter().any(|i| i.severity >= Severity::Error) {
            Verdict::NeedsHuman
        } else {
            Verdict::Pass
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_forces_needs_human() {
        let items = vec![ReviewItem { severity: Severity::Error, rule: "x".into(), message: "m".into() }];
        assert_eq!(ReviewReport::verdict_for(&items), Verdict::NeedsHuman);
    }

    #[test]
    fn warnings_only_pass() {
        let items = vec![ReviewItem { severity: Severity::Warn, rule: "x".into(), message: "m".into() }];
        assert_eq!(ReviewReport::verdict_for(&items), Verdict::Pass);
    }
}
```

- [ ] **Step 2: lib.rs.**
```rust
//! Local advisory pre-publish review gate for VoxSkills. Deterministic floor only
//! (no network, no execution); the LLM review pass is an optional follow-up.

pub mod model;
pub use model::{ReviewItem, ReviewReport, Severity, Verdict};
```

- [ ] **Step 3: Test, clippy, fmt, commit.** `cargo test -p vox-skill-review model`;
```bash
cargo clippy -p vox-skill-review -- -D warnings
cargo fmt -p vox-skill-review
git add crates/vox-skill-review/src
git commit -m "feat(vox-skill-review): review model (Severity/ReviewItem/Verdict/Report)"
```

---

## Task 3: Deterministic checks — frontmatter + stub/placeholder [SEQUENTIAL]

**Files:**
- Create: `crates/vox-skill-review/src/checks.rs`
- Modify: `crates/vox-skill-review/src/lib.rs`

- [ ] **Step 1 (verify-before-use):** From Pre-flight, confirm the `VoxSkillBundle` body accessor name. This task references the manifest only; Task 5 uses the body — note it now.

- [ ] **Step 2: Write `checks.rs` with tests.**
```rust
//! Deterministic (offline) review checks over a parsed skill manifest + body.

use vox_plugin_types::skill_manifest::SkillManifest;

use crate::model::{ReviewItem, Severity};

/// Frontmatter completeness: name + description required and non-trivial.
pub fn check_frontmatter(m: &SkillManifest, out: &mut Vec<ReviewItem>) {
    if m.name.trim().is_empty() {
        out.push(ReviewItem { severity: Severity::Error, rule: "frontmatter/missing-name".into(), message: "skill has no name".into() });
    }
    if m.description.trim().len() < 16 {
        out.push(ReviewItem { severity: Severity::Error, rule: "frontmatter/weak-description".into(), message: "description is missing or too short (< 16 chars) to be discoverable".into() });
    }
}

/// Stub / placeholder detection over the skill body text.
pub fn check_stub(body: &str, out: &mut Vec<ReviewItem>) {
    const MARKERS: &[&str] = &["TODO", "FIXME", "PLACEHOLDER", "coming soon", "fill in", "lorem ipsum", "<your "];
    let lower = body.to_lowercase();
    for marker in MARKERS {
        if lower.contains(&marker.to_lowercase()) {
            out.push(ReviewItem {
                severity: Severity::Error,
                rule: "stub/placeholder".into(),
                message: format!("body contains placeholder marker `{marker}` — finish the skill before publishing"),
            });
        }
    }
    if body.trim().len() < 80 {
        out.push(ReviewItem { severity: Severity::Warn, rule: "stub/thin-body".into(), message: "skill body is very short (< 80 chars); likely incomplete".into() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_plugin_types::skill_manifest::SkillCategory;

    fn manifest(name: &str, desc: &str) -> SkillManifest {
        SkillManifest::new("x.test", name, "0.1.0", "tester", desc, SkillCategory::Unknown)
    }

    #[test]
    fn flags_weak_description() {
        let mut v = Vec::new();
        check_frontmatter(&manifest("good name", "short"), &mut v);
        assert!(v.iter().any(|i| i.rule == "frontmatter/weak-description"));
    }

    #[test]
    fn flags_placeholder_body() {
        let mut v = Vec::new();
        check_stub("This skill will TODO: implement the thing later.", &mut v);
        assert!(v.iter().any(|i| i.rule == "stub/placeholder"));
    }

    #[test]
    fn clean_skill_has_no_findings() {
        let mut v = Vec::new();
        check_frontmatter(&manifest("Format Vox", "Formats Vox source files using the standard style and reports diffs."), &mut v);
        check_stub("A complete, well-described skill body that explains exactly what to do and how, with enough detail to be useful.", &mut v);
        assert!(v.is_empty(), "{v:?}");
    }
}
```

- [ ] **Step 3: lib.rs — add `pub mod checks;`.**
- [ ] **Step 4: Test, clippy, fmt, commit.** `cargo test -p vox-skill-review checks`;
```bash
cargo clippy -p vox-skill-review -- -D warnings
cargo fmt -p vox-skill-review
git add crates/vox-skill-review/src
git commit -m "feat(vox-skill-review): frontmatter + stub/placeholder checks"
```

---

## Task 4: SSOT + dedup checks (reuse the discovery engine) [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-review/src/checks.rs`

- [ ] **Step 1 (verify-before-use):** `rg -n 'pub fn validate_ssot|pub fn dedup_skills|pub struct DiscoverOptions|pub enum CandidateKind' crates/vox-skill-discovery/src/lib.rs crates/vox-skill-discovery/src/candidate.rs` — confirm signatures: `validate_ssot(&[SkillManifest]) -> Vec<Candidate>`, `dedup_skills(&[SkillManifest], &DiscoverOptions) -> Vec<Candidate>`, `CandidateKind::{DuplicatesInstalled, SsotDrift}`, and `Candidate { members: Vec<String>, .. }`.

- [ ] **Step 2: Append SSOT + dedup checks to `checks.rs`.**
```rust
use vox_skill_discovery::{dedup_skills, validate_ssot, DiscoverOptions};

/// Flag declared MCP tools that don't exist in the registry (reuses the discovery engine).
pub fn check_ssot(candidate: &SkillManifest, out: &mut Vec<ReviewItem>) {
    for c in validate_ssot(std::slice::from_ref(candidate)) {
        out.push(ReviewItem {
            severity: Severity::Error,
            rule: "ssot/unknown-tool".into(),
            message: c.suggested_action,
        });
    }
}

/// Flag the candidate as a near-duplicate of an already-installed skill.
pub fn check_dedup(candidate: &SkillManifest, installed: &[SkillManifest], out: &mut Vec<ReviewItem>) {
    if installed.is_empty() {
        return;
    }
    let mut all: Vec<SkillManifest> = installed.to_vec();
    all.push(candidate.clone());
    let opts = DiscoverOptions { shingle_k: 2, ..DiscoverOptions::default() };
    for c in dedup_skills(&all, &opts) {
        if c.members.iter().any(|m| m == &candidate.id) {
            let others: Vec<&String> = c.members.iter().filter(|m| *m != &candidate.id).collect();
            out.push(ReviewItem {
                severity: Severity::Warn,
                rule: "dedup/duplicates-installed".into(),
                message: format!("near-duplicate of installed skill(s): {others:?} — consider reusing instead of publishing"),
            });
        }
    }
}
```

- [ ] **Step 3: Add tests** to the `checks.rs` test module:
```rust
    #[test]
    fn ssot_flags_phantom_tool() {
        let mut m = manifest("tool skill", "Declares a tool that does not exist in the registry at all.");
        m.tools = vec!["vox_totally_made_up_tool".to_string()];
        let mut v = Vec::new();
        check_ssot(&m, &mut v);
        assert!(v.iter().any(|i| i.rule == "ssot/unknown-tool"));
    }

    #[test]
    fn dedup_flags_duplicate_of_installed() {
        let installed = vec![{
            let mut m = manifest("format vox", "Formats vox source files with the standard style and reports diffs.");
            m.id = "installed.fmt".into(); m
        }];
        let mut cand = manifest("format vox", "Formats vox source files with the standard style and reports diffs.");
        cand.id = "candidate.fmt".into();
        let mut v = Vec::new();
        check_dedup(&cand, &installed, &mut v);
        assert!(v.iter().any(|i| i.rule == "dedup/duplicates-installed"));
    }
```
(Note: `manifest()` sets id `"x.test"`; the two tests override `.id` so members are distinguishable.)

- [ ] **Step 4: Test, clippy, fmt, commit.** `cargo test -p vox-skill-review checks`;
```bash
cargo clippy -p vox-skill-review -- -D warnings
cargo fmt -p vox-skill-review
git add crates/vox-skill-review/src/checks.rs
git commit -m "feat(vox-skill-review): SSOT + dedup-vs-installed checks (reuse discovery engine)"
```

---

## Task 5: Auto-tagging + `review_skill` orchestrator [SEQUENTIAL]

**Files:**
- Create: `crates/vox-skill-review/src/review.rs`
- Modify: `crates/vox-skill-review/src/lib.rs`

- [ ] **Step 1 (verify-before-use):** Confirm from Pre-flight that the body is the public field `bundle.skill_md` (NOT a `body()` method). The code below uses `&bundle.skill_md`.

- [ ] **Step 2: Write `review.rs`.**
```rust
//! Orchestrates the deterministic review of a candidate SKILL.md.

use vox_plugin_host::skill_parser::parse_skill_md;
use vox_plugin_types::skill_manifest::SkillManifest;

use crate::checks::{check_dedup, check_frontmatter, check_ssot, check_stub};
use crate::model::{ReviewItem, ReviewReport, Severity, Verdict};

/// Propose advisory tags from the manifest category + salient body keywords.
fn suggest_tags(m: &SkillManifest, body: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let cat = m.category.to_string();
    if cat != "Unknown" {
        tags.push(cat.to_lowercase());
    }
    const KEYWORDS: &[&str] = &["test", "git", "deploy", "format", "compile", "search", "review", "doc", "security"];
    let lower = body.to_lowercase();
    for kw in KEYWORDS {
        if lower.contains(kw) && !tags.iter().any(|t| t == kw) {
            tags.push((*kw).to_string());
        }
    }
    tags
}

/// Review a candidate SKILL.md against the installed set. Deterministic + offline.
pub fn review_skill(skill_md: &str, installed: &[SkillManifest]) -> ReviewReport {
    let bundle = match parse_skill_md(skill_md) {
        Ok(b) => b,
        Err(e) => {
            return ReviewReport {
                skill_id: "(unparseable)".into(),
                items: vec![ReviewItem {
                    severity: Severity::Critical,
                    rule: "parse/invalid-skill-md".into(),
                    message: format!("SKILL.md failed to parse: {e}"),
                }],
                suggested_tags: Vec::new(),
                verdict: Verdict::NeedsHuman,
            };
        }
    };
    let m = &bundle.manifest;
    // `skill_md` is a PUBLIC FIELD on VoxSkillBundle (the full SKILL.md text:
    // frontmatter + body). It is NOT a method — `bundle.body()` does not exist.
    let body: &str = &bundle.skill_md;

    let mut items = Vec::new();
    check_frontmatter(m, &mut items);
    check_stub(body, &mut items);
    check_ssot(m, &mut items);
    check_dedup(m, installed, &mut items);

    let verdict = ReviewReport::verdict_for(&items);
    ReviewReport {
        skill_id: m.id.clone(),
        items,
        suggested_tags: suggest_tags(m, body),
        verdict,
    }
}
```

- [ ] **Step 3: lib.rs — add `pub mod review;` and `pub use review::review_skill;`.**

- [ ] **Step 4: Add an end-to-end test.** Append a `#[cfg(test)] mod tests` to `review.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "---\nname = \"format-vox\"\ndescription = \"Formats Vox source files using the standard style and reports the diff.\"\n\n[metadata]\n\"vox-id\" = \"x.fmt\"\n\"vox-version\" = \"0.1.0\"\n\"vox-category\" = \"refactor\"\n---\nThis skill formats Vox source. It runs the formatter, shows a diff, and explains any changes in detail so the user can review them before applying.\n";

    #[test]
    fn good_skill_passes() {
        let r = review_skill(GOOD, &[]);
        assert_eq!(r.verdict, Verdict::Pass, "{:?}", r.items);
        assert!(r.suggested_tags.iter().any(|t| t == "refactor" || t == "format"));
    }

    #[test]
    fn placeholder_skill_needs_human() {
        let bad = GOOD.replace("This skill formats Vox source.", "TODO: write this skill.");
        let r = review_skill(&bad, &[]);
        assert_eq!(r.verdict, Verdict::NeedsHuman);
    }
}
```
(If the TOML/YAML frontmatter shape in `GOOD` doesn't parse, run the Pre-flight parser-format check and mirror an example from `crates/vox-plugin-host/src/skill_parser.rs` tests — use the exact accepted frontmatter format.)

- [ ] **Step 5: Test, clippy, fmt, commit.** `cargo test -p vox-skill-review`;
```bash
cargo clippy -p vox-skill-review -- -D warnings
cargo fmt -p vox-skill-review
git add crates/vox-skill-review/src
git commit -m "feat(vox-skill-review): auto-tagging + review_skill orchestrator + verdict"
```

---

## Task 6: `vox-skill-review` binary [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-review/src/bin/vox_skill_review.rs`

- [ ] **Step 1: Write the CLI.**
```rust
//! `vox-skill-review` — local advisory pre-publish review of a candidate SKILL.md.

use std::path::PathBuf;

use clap::Parser;
use vox_plugin_types::skill_manifest::SkillManifest;
use vox_skill_review::{review_skill, ReviewReport};

#[derive(Parser, Debug)]
#[command(name = "vox-skill-review", about = "Local advisory pre-publish skill review (deterministic)")]
struct Args {
    /// Path to the candidate SKILL.md.
    #[arg(long)]
    skill: PathBuf,
    /// Optional JSON file: [SkillManifest, ...] of installed skills (for dedup).
    #[arg(long)]
    installed: Option<PathBuf>,
    /// Output: terminal | json
    #[arg(long, default_value = "terminal")]
    format: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let skill_md = std::fs::read_to_string(&args.skill)?;
    let installed: Vec<SkillManifest> = match &args.installed {
        Some(p) => serde_json::from_str(&std::fs::read_to_string(p)?)?,
        None => Vec::new(),
    };
    let report = review_skill(&skill_md, &installed);
    match args.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => print_terminal(&report),
    }
    // Advisory: exit 0 always (gate-before-listing is the caller's policy decision).
    Ok(())
}

fn print_terminal(r: &ReviewReport) {
    println!("skill: {}  verdict: {:?}", r.skill_id, r.verdict);
    if !r.suggested_tags.is_empty() {
        println!("suggested tags: {}", r.suggested_tags.join(", "));
    }
    if r.items.is_empty() {
        println!("no findings.");
    }
    for it in &r.items {
        println!("  [{:?}] {} — {}", it.severity, it.rule, it.message);
    }
}
```

- [ ] **Step 2: Build + smoke.** `cargo build -p vox-skill-review --bin vox-skill-review`. Create a tiny good SKILL.md in a temp path and run `cargo run -p vox-skill-review --bin vox-skill-review -- --skill <that-file>` — prints a `Pass` verdict.

- [ ] **Step 3: Clippy, fmt, commit.**
```bash
cargo clippy -p vox-skill-review -- -D warnings
cargo fmt -p vox-skill-review
git add crates/vox-skill-review/src/bin/vox_skill_review.rs
git commit -m "feat(vox-skill-review): vox-skill-review CLI binary"
```

---

## Task 7: Final verification [SEQUENTIAL]

- [ ] **Step 1:** `cargo test -p vox-skill-review` — paste counts (≥ 9 tests).
- [ ] **Step 2:** `cargo clippy -p vox-skill-review -- -D warnings` — clean.
- [ ] **Step 3:** `cargo run -p vox-arch-check` — exits 0.
- [ ] **Step 4: Delivery manifest** (AGH-0001 §B-4): list every file changed; confirm the only shared-config edits are this crate's registration rows.

## Deferred follow-ups (NOT in this plan)
- **LLM advisory pass:** reuse `vox_code_audit::review::ReviewClient` (network, API key) as a SECOND, optional, default-off pass behind a `--llm` flag, mapping its findings into `ReviewItem`s. Keep it advisory (never the sole gate) per research §2.A.
- **Tiered-trust wiring:** feed the `Verdict` into the existing `TrustLevel` (Untrusted/Community/Trusted) approval gate in `vox-skills/src/sandbox/policy.rs`.
- **Marketplace backend / publish pipeline** (subsystem C): hold-then-publish using this verdict as the floor.

## Self-Review (author)
- **Coverage:** model + gate-before-listing verdict (T2); deterministic floor — frontmatter, stub (T3), SSOT + dedup reuse (T4); auto-tagging + orchestrator (T5); CLI (T6). Matches the research's "deterministic-floor-first, gate-before-listing, automation-is-a-floor" constraints. LLM pass + trust wiring explicitly deferred.
- **Type consistency:** `review_skill(&str, &[SkillManifest]) -> ReviewReport`; `check_*` all take `(&_, .., &mut Vec<ReviewItem>)`; `Severity` ordering drives `verdict_for` (`>= Severity::Error`). `Candidate.members: Vec<String>` + `CandidateKind` reused from vox-skill-discovery.
- **Offline:** no network in the core; LLM is a documented follow-up.
