---
title: "Detector & Heuristic Rule SSOT — Implementation Plan"
description: "Step-by-step plan to land the foundation, pilot detector, and benchmark tool from the rule-SSOT design. Executable by an autonomous Sonnet 4.6 agent."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; transient artifact."
---

# Detector & Heuristic Rule SSOT — Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land a workspace-internal SSOT system for detector regex/heuristic rules — new `vox-rule-pack` crate (zero heavy deps), a YAML rule file, the pilot migration of `victory_claim`, an authoring-time benchmark tool, and `vox-arch-check` invariants that lock the design in. Wave migrations of remaining detectors and the Scientia heuristics consolidation are explicitly **out of scope for this plan** and will be follow-up plans authored against the pilot's template.

**Architecture:** New tiny crate `vox-rule-pack` (deps: `serde`, `serde_yaml`, `regex`, `thiserror`) loads `contracts/code-audit/rules.v1.yaml` validated against `contracts/code-audit/rules.v1.schema.json`. The `victory_claim` detector is rewritten to consume a `RulePack` instead of inlined regex literals. A new `vox ci detect-rules-bench` CLI command runs each rule against committed fixtures and emits precision/recall reports. `vox-arch-check` gains four dependency invariants forbidding `vox-search`/embedding deps from leaking into `vox-rule-pack`, `vox-code-audit`, or `vox-publisher`.

**Tech Stack:** Rust 2024 edition, `cargo`, `serde`, `serde_yaml`, `regex`, `thiserror`, `vox-jsonschema-util` (already in workspace), `vox-arch-check` (workspace's architectural lint binary), YAML, JSON Schema.

**Spec:** [2026-05-09-detector-rule-ssot-design.md](./2026-05-09-detector-rule-ssot-design.md)

**Authoritative ground truth (do NOT update from memory — re-read before editing):**
- `Cargo.toml` `[workspace]` member list and `[workspace.dependencies]`
- `docs/src/architecture/layers.toml` — fan-in / LoC budgets
- `crates/vox-code-audit/src/detectors/victory_claim.rs` — pilot detector
- `crates/vox-code-audit/src/rules.rs` — `Severity`, `Language`, `Finding`, `DetectionRule` trait
- `crates/vox-code-audit/Cargo.toml` — current dep set
- `crates/vox-cli/src/commands/ci/mod.rs` — CI command catalog wiring

**Project rules to honor (from CLAUDE.md / AGENTS.md):**
- Failing test before implementation for every new `pub fn` (test-first policy enforced by lefthook).
- Every change goes through `cargo run -p vox-arch-check` and `cargo test --workspace`; both must pass before commit.
- Auto-generated docs MUST NOT be hand-edited: regenerate via `cargo run -p vox-cli -- ci command-sync` after adding the new CLI subcommand.
- Project automation (this plan does not need any) MUST be `.vox` files via `vox run` — not `.ps1`/`.sh`/`.py`.
- Use `Edit`/`Write`/`Read`/`Glob`/`Grep` tools, not `cat`/`sed`/`awk`/`echo`.
- No emojis in source, doc, or test files.
- Do not read or modify anything under `archive/` or `docs/src/archive/`.

**Out of scope for this plan (deliberately deferred):**
- Migration of detectors other than `victory_claim`.
- Consolidation of `scientia_heuristics.rs` onto `vox-rule-pack`.
- Authoring-time `--suggest` LLM integration on the bench tool (the bench tool ships with TP/FP/FN scoring only; the LLM-backed suggestion mode is a separate follow-up).
- Runtime config-watch / hot-reload of rules.

---

## Phase 0 — Pre-flight

This phase establishes the green baseline that every subsequent phase compares against.

### Task 0.1: Confirm baseline workspace build is clean

**Files:** none (read-only verification)

- [ ] **Step 1: Run the workspace build.**

  Run: `cargo build --workspace`
  Expected: exit 0, no errors. Warnings are acceptable as long as `cargo build` itself succeeds.

- [ ] **Step 2: Run the architecture check.**

  Run: `cargo run -p vox-arch-check`
  Expected: exit 0, no rule violations. If existing violations are reported, stop and surface them — do not continue.

- [ ] **Step 3: Run the workspace test suite at least once.**

  Run: `cargo test --workspace --no-fail-fast`
  Expected: all tests pass. Flaky tests must be reported, not ignored. If anything fails, stop and surface it.

- [ ] **Step 4: Capture the baseline detector count.**

  Run: `cargo test -p vox-code-audit detectors::tests::all_rules_instantiate -- --nocapture`
  Expected: prints exactly 23 rule IDs and asserts `len == rule_count() == 23`. Record this number; it must remain `23` after the pilot migration (we are not adding or removing detectors).

### Task 0.2: Read the spec end-to-end

**Files:** none (read-only)

- [ ] **Step 1: Read the design document.**

  Read: [`docs/src/architecture/2026-05-09-detector-rule-ssot-design.md`](./2026-05-09-detector-rule-ssot-design.md) in full.

- [ ] **Step 2: Read the pilot detector source.**

  Read: [`crates/vox-code-audit/src/detectors/victory_claim.rs`](../../../crates/vox-code-audit/src/detectors/victory_claim.rs) in full. The pilot must produce identical findings after migration.

- [ ] **Step 3: Read the existing rules surface.**

  Read: [`crates/vox-code-audit/src/rules.rs`](../../../crates/vox-code-audit/src/rules.rs) in full to understand `Severity`, `Language`, `Finding`, `FindingConfidence`, `DetectionRule`, `SourceFile`.

---

# PR1 — `vox-rule-pack` crate (foundation)

PR1 introduces the new crate with its core types, loader, and unit tests. No detectors are touched. The crate is added to `[workspace.dependencies]` and `[workspace]` members but has no consumers yet.

## Task 1.1: Scaffold the new crate

**Files:**
- Create: `crates/vox-rule-pack/Cargo.toml`
- Create: `crates/vox-rule-pack/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add `crates/vox-rule-pack` to `[workspace] members` and add `vox-rule-pack` to `[workspace.dependencies]`.

- [ ] **Step 1: Create the crate manifest.**

  Write `crates/vox-rule-pack/Cargo.toml`:
  ```toml
  [package]
  name = "vox-rule-pack"
  description = "Declarative rule pack loader for code-audit detectors and Scientia heuristics. Zero heavy dependencies."
  version.workspace = true
  edition.workspace = true

  [dependencies]
  serde = { workspace = true }
  serde_yaml = { workspace = true }
  serde_json = { workspace = true }
  regex = { workspace = true }
  thiserror = { workspace = true }
  workspace-hack = { workspace = true }

  [dev-dependencies]
  tempfile = { workspace = true }

  [lints]
  workspace = true
  ```

- [ ] **Step 2: Create a stub lib.rs so the crate compiles.**

  Write `crates/vox-rule-pack/src/lib.rs`:
  ```rust
  //! Declarative rule-pack loader. Loads YAML rule definitions used by
  //! `vox-code-audit` detectors and `vox-publisher` Scientia heuristics.
  //!
  //! See `docs/src/architecture/2026-05-09-detector-rule-ssot-design.md`.

  #![deny(rust_2018_idioms)]
  ```

- [ ] **Step 3: Wire into the workspace.**

  In the workspace root `Cargo.toml`:
  - Under `[workspace] members` add `"crates/vox-rule-pack"` (alphabetical order — slot it between the existing `vox-r…` entries).
  - Under `[workspace.dependencies]` add `vox-rule-pack = { path = "crates/vox-rule-pack" }` (alphabetical order).

- [ ] **Step 4: Verify the workspace still builds.**

  Run: `cargo build -p vox-rule-pack`
  Expected: exit 0.
  Run: `cargo build --workspace`
  Expected: exit 0.

- [ ] **Step 5: Commit.**

  ```
  git add crates/vox-rule-pack/Cargo.toml crates/vox-rule-pack/src/lib.rs Cargo.toml
  git commit -m "feat(rule-pack): scaffold vox-rule-pack crate"
  ```

## Task 1.2: Define core types — `RuleSeverity`, `RuleConfidence`, `RuleLanguage`

**Files:**
- Create: `crates/vox-rule-pack/src/types.rs`
- Modify: `crates/vox-rule-pack/src/lib.rs` (add `mod types;` and re-exports)

These mirror `vox-code-audit`'s `Severity`/`FindingConfidence`/`Language` but live one layer below; consumers convert via `From` impls (added when each detector migrates). Defining them here avoids `vox-rule-pack → vox-code-audit` circularity.

- [ ] **Step 1: Write the failing test.**

  Append to `crates/vox-rule-pack/src/types.rs` (creating it):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn severity_serializes_lowercase() {
          let s = serde_yaml::to_string(&RuleSeverity::Warning).unwrap();
          assert_eq!(s.trim(), "warning");
      }

      #[test]
      fn confidence_round_trips() {
          let original = RuleConfidence::Medium;
          let yaml = serde_yaml::to_string(&original).unwrap();
          let back: RuleConfidence = serde_yaml::from_str(&yaml).unwrap();
          assert_eq!(original, back);
      }

      #[test]
      fn language_parses_from_string() {
          let langs: Vec<RuleLanguage> =
              serde_yaml::from_str("[rust, typescript, python, vox, gdscript]").unwrap();
          assert_eq!(langs.len(), 5);
          assert_eq!(langs[0], RuleLanguage::Rust);
          assert_eq!(langs[4], RuleLanguage::GDScript);
      }
  }
  ```

- [ ] **Step 2: Run the test to verify it fails.**

  Run: `cargo test -p vox-rule-pack types::tests`
  Expected: compile error — `RuleSeverity` / `RuleConfidence` / `RuleLanguage` not defined.

- [ ] **Step 3: Implement the types.**

  Replace `crates/vox-rule-pack/src/types.rs` with the test block at the bottom and prepend:
  ```rust
  //! Public enum types used in the rule SSOT. Mirror vox-code-audit's domain
  //! types so consumers can `From`-convert without circular crate dependencies.

  use serde::{Deserialize, Serialize};

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum RuleSeverity {
      Info,
      Warning,
      Error,
      Critical,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum RuleConfidence {
      High,
      Medium,
      Low,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum RuleLanguage {
      Rust,
      TypeScript,
      Python,
      #[serde(rename = "gdscript")]
      GDScript,
      Vox,
  }
  ```

- [ ] **Step 4: Add the module to lib.rs.**

  Edit `crates/vox-rule-pack/src/lib.rs`. After the doc comment, add:
  ```rust
  pub mod types;
  pub use types::{RuleConfidence, RuleLanguage, RuleSeverity};
  ```

- [ ] **Step 5: Run the tests.**

  Run: `cargo test -p vox-rule-pack types::tests`
  Expected: 3 passed.

- [ ] **Step 6: Commit.**

  ```
  git add crates/vox-rule-pack/src/types.rs crates/vox-rule-pack/src/lib.rs
  git commit -m "feat(rule-pack): RuleSeverity, RuleConfidence, RuleLanguage enums"
  ```

## Task 1.3: Define the YAML schema types — `RuleFile`, `RuleSpec`, `MatchSpec`

**Files:**
- Create: `crates/vox-rule-pack/src/schema.rs`
- Modify: `crates/vox-rule-pack/src/lib.rs` (add `mod schema;`)

These are the **on-disk** YAML shape, deserialized verbatim. Compilation (regex pre-build, interning) happens in Task 1.4.

- [ ] **Step 1: Write the failing test.**

  Create `crates/vox-rule-pack/src/schema.rs`:
  ```rust
  //! Deserialization schema for `contracts/code-audit/rules.v1.yaml`.
  //! Validated separately by JSON Schema; this is the structural binding.

  use crate::types::{RuleConfidence, RuleLanguage, RuleSeverity};
  use serde::{Deserialize, Serialize};

  #[cfg(test)]
  mod tests {
      use super::*;

      const SAMPLE: &str = r#"
  version: 1
  rules:
    - id: victory-claim/premature
      parent_id: victory-claim
      name: "Premature victory claim"
      description: "Detects 'done' / 'complete' claims in comments."
      severity: warning
      confidence: medium
      languages: [rust, typescript, python, vox, gdscript]
      match:
        kind: line-regex
        pattern: "(?i)//.*done"
        skip_in: [rust-doc-comment]
      message: "Premature victory claim"
      suggestion: "Remove the comment or describe what is actually done."
      fixtures:
        positive: []
        negative: []
  "#;

      #[test]
      fn parses_minimal_valid_file() {
          let parsed: RuleFile = serde_yaml::from_str(SAMPLE).unwrap();
          assert_eq!(parsed.version, 1);
          assert_eq!(parsed.rules.len(), 1);
          let r = &parsed.rules[0];
          assert_eq!(r.id, "victory-claim/premature");
          assert_eq!(r.severity, RuleSeverity::Warning);
          assert_eq!(r.confidence, Some(RuleConfidence::Medium));
          assert_eq!(r.languages.len(), 5);
          match &r.match_spec.kind {
              MatchKind::LineRegex => {}
              other => panic!("unexpected match kind: {:?}", other),
          }
      }

      #[test]
      fn rejects_unknown_severity() {
          let bad = SAMPLE.replace("severity: warning", "severity: catastrophic");
          let err = serde_yaml::from_str::<RuleFile>(&bad).unwrap_err();
          assert!(err.to_string().contains("catastrophic") || err.to_string().contains("variant"));
      }

      #[test]
      fn rejects_missing_required_field() {
          let bad = SAMPLE.replace("id: victory-claim/premature", "");
          let err = serde_yaml::from_str::<RuleFile>(&bad).unwrap_err();
          assert!(err.to_string().contains("id"));
      }
  }
  ```

- [ ] **Step 2: Run the test to verify it fails.**

  Run: `cargo test -p vox-rule-pack schema::tests`
  Expected: compile error — types not defined.

- [ ] **Step 3: Implement the schema types.**

  Prepend before the `#[cfg(test)]` block in `crates/vox-rule-pack/src/schema.rs`:
  ```rust
  #[derive(Debug, Clone, Deserialize, Serialize)]
  #[serde(deny_unknown_fields)]
  pub struct RuleFile {
      pub version: u32,
      pub rules: Vec<RuleSpec>,
  }

  #[derive(Debug, Clone, Deserialize, Serialize)]
  #[serde(deny_unknown_fields)]
  pub struct RuleSpec {
      pub id: String,
      #[serde(default)]
      pub parent_id: Option<String>,
      pub name: String,
      pub description: String,
      pub severity: RuleSeverity,
      #[serde(default)]
      pub confidence: Option<RuleConfidence>,
      pub languages: Vec<RuleLanguage>,
      #[serde(rename = "match")]
      pub match_spec: MatchSpec,
      pub message: String,
      #[serde(default)]
      pub suggestion: Option<String>,
      #[serde(default)]
      pub fixtures: FixtureSpec,
  }

  #[derive(Debug, Clone, Deserialize, Serialize)]
  #[serde(deny_unknown_fields)]
  pub struct MatchSpec {
      pub kind: MatchKind,
      pub pattern: String,
      #[serde(default)]
      pub skip_in: Vec<SkipScope>,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
  #[serde(rename_all = "kebab-case")]
  pub enum MatchKind {
      LineRegex,
      MultilineRegex,
      Substring,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
  #[serde(rename_all = "kebab-case")]
  pub enum SkipScope {
      /// Skip lines that begin with `///` or `//!`.
      RustDocComment,
      /// Skip bytes the caller's TokenMap reports as comment+string.
      RustNonCode,
      /// Skip bytes the caller's TokenMap reports as comment only.
      RustComment,
  }

  #[derive(Debug, Clone, Default, Deserialize, Serialize)]
  #[serde(deny_unknown_fields)]
  pub struct FixtureSpec {
      #[serde(default)]
      pub positive: Vec<String>,
      #[serde(default)]
      pub negative: Vec<String>,
  }
  ```

- [ ] **Step 4: Register the module in lib.rs.**

  Edit `crates/vox-rule-pack/src/lib.rs`, add after the existing `pub mod types;`:
  ```rust
  pub mod schema;
  pub use schema::{FixtureSpec, MatchKind, MatchSpec, RuleFile, RuleSpec, SkipScope};
  ```

- [ ] **Step 5: Run the tests.**

  Run: `cargo test -p vox-rule-pack schema::tests`
  Expected: 3 passed.

- [ ] **Step 6: Commit.**

  ```
  git add crates/vox-rule-pack/src/schema.rs crates/vox-rule-pack/src/lib.rs
  git commit -m "feat(rule-pack): YAML schema types (RuleFile, RuleSpec, MatchSpec)"
  ```

## Task 1.4: Implement `RulePack` (compiled, runtime-ready)

**Files:**
- Create: `crates/vox-rule-pack/src/pack.rs`
- Modify: `crates/vox-rule-pack/src/lib.rs`

`RulePack` holds compiled `Regex` instances (one per rule), interned IDs, and an O(1) lookup by ID. It is the type detectors actually consume.

- [ ] **Step 1: Write the failing test.**

  Create `crates/vox-rule-pack/src/pack.rs`:
  ```rust
  //! Compiled, runtime-ready container of rules. Built from a `RuleFile`.

  use crate::error::{RulePackError, RulePackResult};
  use crate::schema::{MatchKind, MatchSpec, RuleFile, RuleSpec, SkipScope};
  use crate::types::{RuleConfidence, RuleLanguage, RuleSeverity};
  use regex::Regex;
  use std::collections::HashMap;
  use std::path::Path;

  #[cfg(test)]
  mod tests {
      use super::*;

      const SAMPLE: &str = r#"
  version: 1
  rules:
    - id: test/foo
      name: "Foo"
      description: "Test rule"
      severity: warning
      confidence: medium
      languages: [rust]
      match: { kind: line-regex, pattern: "foo\\d+" }
      message: "matched"
  "#;

      #[test]
      fn loads_from_str() {
          let pack = RulePack::load_from_str(SAMPLE).unwrap();
          assert_eq!(pack.len(), 1);
          let rule = pack.rule("test/foo").unwrap();
          assert_eq!(rule.severity, RuleSeverity::Warning);
          assert!(rule.matches_line("foo123"));
          assert!(!rule.matches_line("bar123"));
      }

      #[test]
      fn rejects_invalid_regex() {
          let bad = SAMPLE.replace("foo\\\\d+", "(unclosed");
          let err = RulePack::load_from_str(&bad).unwrap_err();
          assert!(matches!(err, RulePackError::InvalidRegex { .. }));
      }

      #[test]
      fn rejects_duplicate_id() {
          let dup = format!("{}\n  - id: test/foo\n    name: dup\n    description: dup\n    severity: warning\n    languages: [rust]\n    match: {{ kind: line-regex, pattern: \"x\" }}\n    message: m\n", SAMPLE);
          let err = RulePack::load_from_str(&dup).unwrap_err();
          assert!(matches!(err, RulePackError::DuplicateId { .. }));
      }

      #[test]
      fn iterates_by_language() {
          let pack = RulePack::load_from_str(SAMPLE).unwrap();
          assert_eq!(pack.rules_for_language(RuleLanguage::Rust).count(), 1);
          assert_eq!(pack.rules_for_language(RuleLanguage::Python).count(), 0);
      }
  }
  ```

- [ ] **Step 2: Run the test to verify it fails.**

  Run: `cargo test -p vox-rule-pack pack::tests`
  Expected: compile error.

- [ ] **Step 3: Implement the error module first.**

  Create `crates/vox-rule-pack/src/error.rs`:
  ```rust
  //! Error type returned by RulePack loaders.

  use thiserror::Error;

  pub type RulePackResult<T> = Result<T, RulePackError>;

  #[derive(Debug, Error)]
  pub enum RulePackError {
      #[error("YAML parse error: {0}")]
      Yaml(#[from] serde_yaml::Error),
      #[error("I/O error reading rule pack at {path}: {source}")]
      Io {
          path: String,
          #[source]
          source: std::io::Error,
      },
      #[error("Invalid regex for rule '{rule_id}': {source}")]
      InvalidRegex {
          rule_id: String,
          #[source]
          source: regex::Error,
      },
      #[error("Duplicate rule id: {0}")]
      DuplicateId(String),
      #[error("Unsupported rule pack version: {0} (this build supports v1)")]
      UnsupportedVersion(u32),
  }
  ```

  Add to `crates/vox-rule-pack/src/lib.rs`:
  ```rust
  pub mod error;
  pub use error::{RulePackError, RulePackResult};
  ```

- [ ] **Step 4: Implement `CompiledRule` and `RulePack`.**

  Replace the prelude of `crates/vox-rule-pack/src/pack.rs` (above the `#[cfg(test)]` block) with:
  ```rust
  /// A rule with its regex pre-compiled and id interned.
  #[derive(Debug)]
  pub struct CompiledRule {
      pub id: String,
      pub parent_id: Option<String>,
      pub name: String,
      pub description: String,
      pub severity: RuleSeverity,
      pub confidence: Option<RuleConfidence>,
      pub languages: Vec<RuleLanguage>,
      pub message: String,
      pub suggestion: Option<String>,
      pub skip_in: Vec<SkipScope>,
      pub kind: MatchKind,
      regex: Regex,
  }

  impl CompiledRule {
      pub fn matches_line(&self, line: &str) -> bool {
          self.regex.is_match(line)
      }

      pub fn regex(&self) -> &Regex {
          &self.regex
      }
  }

  /// Loaded, compiled rule pack. Cheap to share via `Arc`.
  #[derive(Debug)]
  pub struct RulePack {
      rules: Vec<CompiledRule>,
      by_id: HashMap<String, usize>,
  }

  impl RulePack {
      pub fn load_from_str(yaml: &str) -> RulePackResult<Self> {
          let file: RuleFile = serde_yaml::from_str(yaml)?;
          if file.version != 1 {
              return Err(RulePackError::UnsupportedVersion(file.version));
          }
          let mut rules = Vec::with_capacity(file.rules.len());
          let mut by_id: HashMap<String, usize> = HashMap::new();
          for spec in file.rules.into_iter() {
              if by_id.contains_key(&spec.id) {
                  return Err(RulePackError::DuplicateId(spec.id));
              }
              let compiled = compile(spec)?;
              by_id.insert(compiled.id.clone(), rules.len());
              rules.push(compiled);
          }
          Ok(Self { rules, by_id })
      }

      pub fn load_from_path(path: &Path) -> RulePackResult<Self> {
          let yaml = std::fs::read_to_string(path).map_err(|source| RulePackError::Io {
              path: path.display().to_string(),
              source,
          })?;
          Self::load_from_str(&yaml)
      }

      pub fn len(&self) -> usize {
          self.rules.len()
      }

      pub fn is_empty(&self) -> bool {
          self.rules.is_empty()
      }

      pub fn rule(&self, id: &str) -> Option<&CompiledRule> {
          self.by_id.get(id).map(|&i| &self.rules[i])
      }

      pub fn rules(&self) -> &[CompiledRule] {
          &self.rules
      }

      pub fn rules_for_language(
          &self,
          lang: RuleLanguage,
      ) -> impl Iterator<Item = &CompiledRule> {
          self.rules.iter().filter(move |r| r.languages.contains(&lang))
      }
  }

  fn compile(spec: RuleSpec) -> RulePackResult<CompiledRule> {
      let regex = build_regex(&spec.id, &spec.match_spec)?;
      Ok(CompiledRule {
          id: spec.id,
          parent_id: spec.parent_id,
          name: spec.name,
          description: spec.description,
          severity: spec.severity,
          confidence: spec.confidence,
          languages: spec.languages,
          message: spec.message,
          suggestion: spec.suggestion,
          skip_in: spec.match_spec.skip_in.clone(),
          kind: spec.match_spec.kind,
          regex,
      })
  }

  fn build_regex(rule_id: &str, m: &MatchSpec) -> RulePackResult<Regex> {
      let pattern = match m.kind {
          MatchKind::LineRegex | MatchKind::MultilineRegex => m.pattern.clone(),
          MatchKind::Substring => regex::escape(&m.pattern),
      };
      Regex::new(&pattern).map_err(|source| RulePackError::InvalidRegex {
          rule_id: rule_id.to_string(),
          source,
      })
  }
  ```

- [ ] **Step 5: Register `pack` module in lib.rs and re-export.**

  In `crates/vox-rule-pack/src/lib.rs` add:
  ```rust
  pub mod pack;
  pub use pack::{CompiledRule, RulePack};
  ```

- [ ] **Step 6: Run all tests.**

  Run: `cargo test -p vox-rule-pack`
  Expected: all tests in `types`, `schema`, `pack` pass (10 tests total).

- [ ] **Step 7: Commit.**

  ```
  git add crates/vox-rule-pack/src/error.rs crates/vox-rule-pack/src/pack.rs crates/vox-rule-pack/src/lib.rs
  git commit -m "feat(rule-pack): RulePack loader with regex compile, dup detection, lang index"
  ```

## Task 1.5: Add layer policy entry for `vox-rule-pack`

**Files:**
- Modify: `docs/src/architecture/layers.toml`

`vox-rule-pack` is a leaf utility crate. Place it at L1 (foundation utilities) so all other crates may depend on it.

- [ ] **Step 1: Read the existing layers.toml.**

  Read: `docs/src/architecture/layers.toml`. Identify the L1 section (look for an existing entry like `vox-secrets` or similar utility crates) to find the canonical row format.

- [ ] **Step 2: Add the `vox-rule-pack` row.**

  Insert a row in the L1 section using the format you observed in step 1. Keep entries alphabetical. The row must declare:
  - `name = "vox-rule-pack"`
  - `layer = 1` (or whatever the L1 section calls it)
  - `loc_budget` = at minimum `2000` (foundation crate, generous budget)
  - `forbidden_deps = ["vox-search", "tantivy", "qdrant-client"]` (use the field name actually present in the file; if absent, skip this and add it via Task 5.1 instead)

- [ ] **Step 3: Run vox-arch-check.**

  Run: `cargo run -p vox-arch-check`
  Expected: exit 0. If the new entry causes a failure, fix the row format until it passes — do not silence the check.

- [ ] **Step 4: Commit.**

  ```
  git add docs/src/architecture/layers.toml
  git commit -m "chore(arch): register vox-rule-pack at L1"
  ```

## Task 1.6: PR1 verification

- [ ] **Step 1: Full workspace build.**
  Run: `cargo build --workspace`
  Expected: exit 0.

- [ ] **Step 2: Full test run.**
  Run: `cargo test --workspace --no-fail-fast`
  Expected: all green; no detector tests changed.

- [ ] **Step 3: Architecture check.**
  Run: `cargo run -p vox-arch-check`
  Expected: exit 0.

PR1 is complete. The crate exists with no consumers — that's intentional. PR2 wires the first consumer.

---

# PR2 — Rule SSOT file + JSON Schema + fixtures

PR2 creates the canonical rule file, its JSON Schema validator, and the labeled fixtures used by the bench tool. No detector changes yet.

## Task 2.1: Create the rule SSOT directory and JSON Schema

**Files:**
- Create: `contracts/code-audit/rules.v1.schema.json`
- Create: `contracts/code-audit/README.md`

- [ ] **Step 1: Create the JSON Schema.**

  Write `contracts/code-audit/rules.v1.schema.json`:
  ```json
  {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": "https://vox.dev/contracts/code-audit/rules.v1.schema.json",
    "title": "Code-audit rule pack v1",
    "type": "object",
    "additionalProperties": false,
    "required": ["version", "rules"],
    "properties": {
      "version": { "type": "integer", "const": 1 },
      "rules": {
        "type": "array",
        "items": { "$ref": "#/$defs/rule" }
      }
    },
    "$defs": {
      "rule": {
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "name", "description", "severity", "languages", "match", "message"],
        "properties": {
          "id": { "type": "string", "pattern": "^[a-z][a-z0-9-]*(/[a-z0-9-]+)*$" },
          "parent_id": { "type": "string" },
          "name": { "type": "string", "minLength": 1 },
          "description": { "type": "string", "minLength": 1 },
          "severity": { "enum": ["info", "warning", "error", "critical"] },
          "confidence": { "enum": ["high", "medium", "low"] },
          "languages": {
            "type": "array",
            "minItems": 1,
            "items": { "enum": ["rust", "typescript", "python", "vox", "gdscript"] }
          },
          "match": { "$ref": "#/$defs/match" },
          "message": { "type": "string", "minLength": 1 },
          "suggestion": { "type": "string" },
          "fixtures": { "$ref": "#/$defs/fixtures" }
        }
      },
      "match": {
        "type": "object",
        "additionalProperties": false,
        "required": ["kind", "pattern"],
        "properties": {
          "kind": { "enum": ["line-regex", "multiline-regex", "substring"] },
          "pattern": { "type": "string", "minLength": 1 },
          "skip_in": {
            "type": "array",
            "items": { "enum": ["rust-doc-comment", "rust-non-code", "rust-comment"] }
          }
        }
      },
      "fixtures": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "positive": { "type": "array", "items": { "type": "string" } },
          "negative": { "type": "array", "items": { "type": "string" } }
        }
      }
    }
  }
  ```

- [ ] **Step 2: Create a brief README.**

  Write `contracts/code-audit/README.md`:
  ```markdown
  # Code-audit rule pack contracts

  - `rules.v1.yaml` — the rule SSOT consumed by `vox-rule-pack` and `vox-code-audit` detectors.
  - `rules.v1.schema.json` — JSON Schema for `rules.v1.yaml`.
  - `fixtures/` — labeled positive/negative samples per rule, used by `vox ci detect-rules-bench`.

  See [2026-05-09-detector-rule-ssot-design.md](./2026-05-09-detector-rule-ssot-design.md).
  ```

- [ ] **Step 3: Commit.**

  ```
  git add contracts/code-audit/rules.v1.schema.json contracts/code-audit/README.md
  git commit -m "feat(code-audit): rule SSOT JSON Schema and contract README"
  ```

## Task 2.2: Author `rules.v1.yaml` containing only the four `victory_claim` rules

**Files:**
- Create: `contracts/code-audit/rules.v1.yaml`

The patterns below are **byte-for-byte translations** of the regex literals currently in [`crates/vox-code-audit/src/detectors/victory_claim.rs`](../../../crates/vox-code-audit/src/detectors/victory_claim.rs:23-49). Do not "improve" them in this PR — parity is the gate.

- [ ] **Step 1: Author the file.**

  Write `contracts/code-audit/rules.v1.yaml`:
  ```yaml
  # yaml-language-server: $schema=./rules.v1.schema.json
  # SSOT for code-audit detector rules. Loaded by vox-rule-pack at startup.
  # See docs/src/architecture/2026-05-09-detector-rule-ssot-design.md.

  version: 1
  rules:
    - id: victory-claim/premature
      parent_id: victory-claim
      name: "Victory Claim / Leftover Marker Detector"
      description: "Detects past-tense completion claims in comments or panic-class macro literals."
      severity: warning
      languages: [rust, typescript, python, gdscript, vox]
      match:
        kind: line-regex
        pattern: "(?i)(?://|#|/\\*|todo!|panic!|unimplemented!).*?(?:\\bdone\\b|all\\s*set|fully\\s*implemented|implementation\\s+\\bcomplete\\b)"
        skip_in: [rust-doc-comment]
      message: "Premature victory claim — verify the implementation is truly complete"
      suggestion: "Remove the comment if the implementation is complete, or replace with a descriptive comment."
      fixtures:
        positive:
          - "fixtures/victory-claim/premature_pos_done.txt"
          - "fixtures/victory-claim/premature_pos_all_set.txt"
        negative:
          - "fixtures/victory-claim/premature_neg_doc_comment.txt"

    - id: victory-claim/todo-leftover
      parent_id: victory-claim
      name: "Victory Claim / Leftover Marker Detector"
      description: "Detects TODO comment markers paired with action verbs."
      severity: warning
      languages: [rust, typescript, python, gdscript, vox]
      match:
        kind: line-regex
        pattern: "(?i)(?://|#|todo!).*?(?:TODO(?:\\(ai\\))?)\\s*:?\\s*(?:implement|add|finish|complete|wire|fix|later)"
        skip_in: [rust-doc-comment]
      message: "TODO marker left behind — work is not finished"
      suggestion: "Complete the TODO or create a tracked task for it."
      fixtures:
        positive:
          - "fixtures/victory-claim/todo_pos_python.txt"
        negative:
          - "fixtures/victory-claim/todo_neg_unrelated.txt"

    - id: victory-claim/fixme
      parent_id: victory-claim
      name: "Victory Claim / Leftover Marker Detector"
      description: "Detects FIXME comment markers."
      severity: warning
      languages: [rust, typescript, python, gdscript, vox]
      match:
        kind: line-regex
        pattern: "(?i)(?://|#).*?FIXME(?:\\(ai\\))?\\b"
        skip_in: [rust-doc-comment]
      message: "FIXME marker — known issue left unresolved"
      suggestion: "Fix the issue or track it as a task."
      fixtures:
        positive:
          - "fixtures/victory-claim/fixme_pos.txt"
        negative: []

    - id: victory-claim/hack
      parent_id: victory-claim
      name: "Victory Claim / Leftover Marker Detector"
      description: "Detects HACK comment markers (informational)."
      severity: info
      languages: [rust, typescript, python, gdscript, vox]
      match:
        kind: line-regex
        pattern: "(?i)(?://|#).*?HACK\\b"
        skip_in: [rust-doc-comment]
      message: "HACK marker — temporary workaround left in code"
      suggestion: "Replace with a proper solution or document why the hack is necessary."
      fixtures:
        positive:
          - "fixtures/victory-claim/hack_pos.txt"
        negative: []
  ```

- [ ] **Step 2: Verify YAML parses with the schema types from PR1.**

  Run from the workspace root:
  ```
  cargo run -p vox-rule-pack --quiet --example load_rules -- contracts/code-audit/rules.v1.yaml
  ```
  This example does not yet exist; create it as Task 2.3 below before running this step.

  Skip step 2 here and complete it after Task 2.3.

## Task 2.3: Add a tiny example binary that loads the YAML

**Files:**
- Create: `crates/vox-rule-pack/examples/load_rules.rs`

This is a smoke-test entry point used in the previous step and by CI to confirm the YAML parses.

- [ ] **Step 1: Write the example.**

  Create `crates/vox-rule-pack/examples/load_rules.rs`:
  ```rust
  //! `cargo run -p vox-rule-pack --example load_rules -- <path>`
  //!
  //! Loads a rule pack YAML and prints rule count + each rule id.
  //! Used in CI smoke tests; not part of the public surface.

  use std::path::PathBuf;
  use vox_rule_pack::RulePack;

  fn main() -> anyhow::Result<()> {
      let path: PathBuf = std::env::args()
          .nth(1)
          .ok_or_else(|| anyhow::anyhow!("usage: load_rules <path>"))?
          .into();
      let pack = RulePack::load_from_path(&path)?;
      println!("loaded {} rules from {}", pack.len(), path.display());
      for rule in pack.rules() {
          println!("  - {}  [{:?}, {} lang(s)]", rule.id, rule.severity, rule.languages.len());
      }
      Ok(())
  }
  ```

- [ ] **Step 2: Add `anyhow` to dev-dependencies.**

  Edit `crates/vox-rule-pack/Cargo.toml`. In `[dev-dependencies]` add:
  ```toml
  anyhow = { workspace = true }
  ```

- [ ] **Step 3: Run the example.**

  Run: `cargo run -p vox-rule-pack --example load_rules -- contracts/code-audit/rules.v1.yaml`
  Expected: prints `loaded 4 rules from contracts/code-audit/rules.v1.yaml` and lists 4 rule IDs.

- [ ] **Step 4: Commit.**

  ```
  git add contracts/code-audit/rules.v1.yaml crates/vox-rule-pack/examples/load_rules.rs crates/vox-rule-pack/Cargo.toml
  git commit -m "feat(code-audit): rules.v1.yaml with victory-claim translations + smoke example"
  ```

## Task 2.4: Author the labeled fixtures

**Files:**
- Create: `contracts/code-audit/fixtures/victory-claim/premature_pos_done.txt`
- Create: `contracts/code-audit/fixtures/victory-claim/premature_pos_all_set.txt`
- Create: `contracts/code-audit/fixtures/victory-claim/premature_neg_doc_comment.txt`
- Create: `contracts/code-audit/fixtures/victory-claim/todo_pos_python.txt`
- Create: `contracts/code-audit/fixtures/victory-claim/todo_neg_unrelated.txt`
- Create: `contracts/code-audit/fixtures/victory-claim/fixme_pos.txt`
- Create: `contracts/code-audit/fixtures/victory-claim/hack_pos.txt`

Each fixture is a **single line** of source text. The bench tool will run the rule against the line and assert that positives match and negatives do not. Do not include trailing newlines in semantic content; trailing newline at end of file is fine.

The contents below are obfuscated where needed (e.g., split TODO/FIXME/HACK across `concat!`-style markers) **only in source comments**, **not** in fixtures — fixtures contain literal text the rule must match.

- [ ] **Step 1: Write each fixture.**

  Create `contracts/code-audit/fixtures/victory-claim/premature_pos_done.txt`:
  ```
  // Done! Implementation complete
  ```

  Create `contracts/code-audit/fixtures/victory-claim/premature_pos_all_set.txt`:
  ```
  // all set, ready to ship
  ```

  Create `contracts/code-audit/fixtures/victory-claim/premature_neg_doc_comment.txt`:
  ```
  /// Adds two numbers and returns the result.
  ```

  Create `contracts/code-audit/fixtures/victory-claim/todo_pos_python.txt`:
  ```
  # TODO: implement later
  ```

  Create `contracts/code-audit/fixtures/victory-claim/todo_neg_unrelated.txt`:
  ```
  let x = 1; // bumped from 0
  ```

  Create `contracts/code-audit/fixtures/victory-claim/fixme_pos.txt`:
  ```
  // FIXME this is broken
  ```

  Create `contracts/code-audit/fixtures/victory-claim/hack_pos.txt`:
  ```
  // HACK: workaround for upstream bug
  ```

- [ ] **Step 2: Verify each fixture is a single line ending with `\n`.**

  Run (PowerShell): `Get-ChildItem contracts/code-audit/fixtures/victory-claim -File | ForEach-Object { Get-Content -Raw $_.FullName | Select-String -Pattern "`n" -AllMatches | Select-Object -ExpandProperty Matches | Measure-Object | Select-Object -ExpandProperty Count }`
  Expected: each file reports `1` (exactly one newline).

  If any file reports more, rewrite it to a single line.

- [ ] **Step 3: Commit.**

  ```
  git add contracts/code-audit/fixtures/
  git commit -m "feat(code-audit): victory-claim fixtures (positive/negative)"
  ```

## Task 2.5: Schema-validate the YAML in CI

**Files:**
- Modify: `crates/vox-rule-pack/src/lib.rs` (add a path-based smoke test)
- Create: `crates/vox-rule-pack/tests/canonical_rules.rs`

A workspace test guarantees `contracts/code-audit/rules.v1.yaml` always parses with the current schema types.

- [ ] **Step 1: Write the integration test.**

  Create `crates/vox-rule-pack/tests/canonical_rules.rs`:
  ```rust
  //! Locks the canonical rules.v1.yaml against the current vox-rule-pack schema.

  use std::path::PathBuf;
  use vox_rule_pack::RulePack;

  fn workspace_root() -> PathBuf {
      let manifest_dir = env!("CARGO_MANIFEST_DIR");
      PathBuf::from(manifest_dir).join("..").join("..")
  }

  #[test]
  fn canonical_rules_yaml_parses() {
      let path = workspace_root()
          .join("contracts")
          .join("code-audit")
          .join("rules.v1.yaml");
      let pack = RulePack::load_from_path(&path).expect("rules.v1.yaml must parse");
      assert!(pack.len() >= 4, "expected at least the four victory-claim rules");
      for needed in [
          "victory-claim/premature",
          "victory-claim/todo-leftover",
          "victory-claim/fixme",
          "victory-claim/hack",
      ] {
          assert!(pack.rule(needed).is_some(), "rule {} must exist", needed);
      }
  }
  ```

- [ ] **Step 2: Run the test.**

  Run: `cargo test -p vox-rule-pack --test canonical_rules`
  Expected: 1 passed.

- [ ] **Step 3: Commit.**

  ```
  git add crates/vox-rule-pack/tests/canonical_rules.rs
  git commit -m "test(rule-pack): lock canonical rules.v1.yaml against schema"
  ```

## Task 2.6: PR2 verification

- [ ] **Step 1: Full workspace build.**
  Run: `cargo build --workspace`
  Expected: exit 0.

- [ ] **Step 2: Full test run.**
  Run: `cargo test --workspace --no-fail-fast`
  Expected: all green.

- [ ] **Step 3: Architecture check.**
  Run: `cargo run -p vox-arch-check`
  Expected: exit 0.

---

# PR3 — Migrate `victory_claim` (pilot detector)

PR3 rewrites [`crates/vox-code-audit/src/detectors/victory_claim.rs`](../../../crates/vox-code-audit/src/detectors/victory_claim.rs) to consume `RulePack`. A parity test asserts identical findings on a fixed corpus before and after migration.

## Task 3.1: Add the dependency edge

**Files:**
- Modify: `crates/vox-code-audit/Cargo.toml`

- [ ] **Step 1: Add the dependency.**

  Edit `crates/vox-code-audit/Cargo.toml`. Add to `[dependencies]` (alphabetical):
  ```toml
  vox-rule-pack = { workspace = true }
  ```

- [ ] **Step 2: Verify build.**

  Run: `cargo build -p vox-code-audit`
  Expected: exit 0.

- [ ] **Step 3: Commit.**

  ```
  git add crates/vox-code-audit/Cargo.toml
  git commit -m "chore(code-audit): depend on vox-rule-pack"
  ```

## Task 3.2: Define `From` conversions for the shared types

**Files:**
- Create: `crates/vox-code-audit/src/rule_pack_bridge.rs`
- Modify: `crates/vox-code-audit/src/lib.rs`

These conversions map `vox-rule-pack` enums into `vox-code-audit` enums so the detector code can stay idiomatic.

- [ ] **Step 1: Write the failing test.**

  Create `crates/vox-code-audit/src/rule_pack_bridge.rs`:
  ```rust
  //! Conversions between vox-rule-pack and vox-code-audit enums.
  //! Kept in vox-code-audit (not vox-rule-pack) so the lower-layer crate stays domain-free.

  use crate::rules::{FindingConfidence, Language, Severity};
  use vox_rule_pack::{RuleConfidence, RuleLanguage, RuleSeverity};

  impl From<RuleSeverity> for Severity {
      fn from(value: RuleSeverity) -> Self {
          match value {
              RuleSeverity::Info => Severity::Info,
              RuleSeverity::Warning => Severity::Warning,
              RuleSeverity::Error => Severity::Error,
              RuleSeverity::Critical => Severity::Critical,
          }
      }
  }

  impl From<RuleConfidence> for FindingConfidence {
      fn from(value: RuleConfidence) -> Self {
          match value {
              RuleConfidence::High => FindingConfidence::High,
              RuleConfidence::Medium => FindingConfidence::Medium,
              RuleConfidence::Low => FindingConfidence::Low,
          }
      }
  }

  impl From<RuleLanguage> for Language {
      fn from(value: RuleLanguage) -> Self {
          match value {
              RuleLanguage::Rust => Language::Rust,
              RuleLanguage::TypeScript => Language::TypeScript,
              RuleLanguage::Python => Language::Python,
              RuleLanguage::GDScript => Language::GDScript,
              RuleLanguage::Vox => Language::Vox,
          }
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn severity_round_trip_warning() {
          let s: Severity = RuleSeverity::Warning.into();
          assert_eq!(s, Severity::Warning);
      }

      #[test]
      fn confidence_round_trip_medium() {
          let c: FindingConfidence = RuleConfidence::Medium.into();
          assert_eq!(c, FindingConfidence::Medium);
      }

      #[test]
      fn language_round_trip_rust() {
          let l: Language = RuleLanguage::Rust.into();
          assert_eq!(l, Language::Rust);
      }
  }
  ```

- [ ] **Step 2: Register the module.**

  Edit `crates/vox-code-audit/src/lib.rs`. Add (in module declaration order, near the other top-level modules):
  ```rust
  pub(crate) mod rule_pack_bridge;
  ```

- [ ] **Step 3: Run the tests.**

  Run: `cargo test -p vox-code-audit rule_pack_bridge`
  Expected: 3 passed.

- [ ] **Step 4: Commit.**

  ```
  git add crates/vox-code-audit/src/rule_pack_bridge.rs crates/vox-code-audit/src/lib.rs
  git commit -m "feat(code-audit): rule-pack ↔ code-audit enum bridge"
  ```

## Task 3.3: Embed the rules YAML at compile time and expose a singleton

**Files:**
- Modify: `crates/vox-code-audit/src/lib.rs`
- Create: `crates/vox-code-audit/src/embedded_rules.rs`

The pack is loaded once via `OnceLock`. The `victory_claim` detector and (in future PRs) other detectors share this single instance.

- [ ] **Step 1: Write the failing test.**

  Create `crates/vox-code-audit/src/embedded_rules.rs`:
  ```rust
  //! Compile-time-embedded copy of contracts/code-audit/rules.v1.yaml,
  //! exposed as a process-wide singleton RulePack.

  use std::sync::OnceLock;
  use vox_rule_pack::RulePack;

  const EMBEDDED_YAML: &str =
      include_str!("../../../contracts/code-audit/rules.v1.yaml");

  static PACK: OnceLock<RulePack> = OnceLock::new();

  /// Returns the process-wide rule pack.
  ///
  /// Panics on first call if the embedded YAML is malformed; callers should
  /// not catch this — it indicates a build-time invariant violation.
  pub fn embedded_pack() -> &'static RulePack {
      PACK.get_or_init(|| {
          RulePack::load_from_str(EMBEDDED_YAML).expect("embedded rules.v1.yaml must parse")
      })
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn embedded_pack_loads() {
          let pack = embedded_pack();
          assert!(pack.len() >= 4);
          assert!(pack.rule("victory-claim/premature").is_some());
      }
  }
  ```

- [ ] **Step 2: Register the module.**

  In `crates/vox-code-audit/src/lib.rs` add:
  ```rust
  pub(crate) mod embedded_rules;
  ```

- [ ] **Step 3: Run the test.**

  Run: `cargo test -p vox-code-audit embedded_rules`
  Expected: 1 passed.

- [ ] **Step 4: Commit.**

  ```
  git add crates/vox-code-audit/src/embedded_rules.rs crates/vox-code-audit/src/lib.rs
  git commit -m "feat(code-audit): embed rules.v1.yaml as process-wide singleton"
  ```

## Task 3.4: Author the parity test (red bar)

**Files:**
- Create: `crates/vox-code-audit/tests/victory_claim_parity.rs`

This test pins the **current** `victory_claim` output. After Task 3.5 rewrites the detector, this test must still pass.

- [ ] **Step 1: Write the parity test.**

  Create `crates/vox-code-audit/tests/victory_claim_parity.rs`:
  ```rust
  //! Parity harness for the victory_claim detector migration.
  //!
  //! Asserts that the post-migration detector emits findings with identical
  //! (rule_id, line, severity) tuples on a fixed source corpus.

  use std::collections::BTreeSet;
  use std::path::PathBuf;
  use vox_code_audit::detectors::victory_claim::VictoryClaimDetector;
  use vox_code_audit::rules::{DetectionRule, Severity, SourceFile};

  const CORPUS: &str = r#"
  // Done! Implementation complete
  fn alpha() {}

  /// Adds two numbers.
  fn beta(a: i32, b: i32) -> i32 { a + b }

  // FIXME this is broken
  const X: i32 = 1;

  // HACK: workaround for upstream bug
  fn gamma() {}

  // TODO: implement later
  fn delta() {}

  // all set, ready to ship
  fn epsilon() {}
  "#;

  fn ids_lines(src: &str) -> BTreeSet<(String, usize, Severity)> {
      let file = SourceFile::new(PathBuf::from("corpus.rs"), src.to_string());
      let detector = VictoryClaimDetector::new();
      detector
          .detect(&file, None)
          .into_iter()
          .map(|f| (f.rule_id, f.line, f.severity))
          .collect()
  }

  #[test]
  fn parity_findings_match_baseline() {
      let actual = ids_lines(CORPUS);
      let expected: BTreeSet<(String, usize, Severity)> = [
          ("victory-claim/premature".to_string(), 2, Severity::Warning),
          ("victory-claim/fixme".to_string(), 8, Severity::Warning),
          ("victory-claim/hack".to_string(), 11, Severity::Info),
          ("victory-claim/todo-leftover".to_string(), 14, Severity::Warning),
          ("victory-claim/premature".to_string(), 17, Severity::Warning),
      ]
      .into_iter()
      .collect();
      assert_eq!(actual, expected, "victory_claim output must remain stable");
  }
  ```

- [ ] **Step 2: Run the test against the current (regex-inlined) detector.**

  Run: `cargo test -p vox-code-audit --test victory_claim_parity`
  Expected: PASS. If it fails, the expected set is wrong — fix the expected set to reflect actual current output, then re-run. The point of this step is to **lock the current behavior** before editing the detector.

- [ ] **Step 3: Commit.**

  ```
  git add crates/vox-code-audit/tests/victory_claim_parity.rs
  git commit -m "test(code-audit): victory_claim parity baseline (pre-migration)"
  ```

## Task 3.5: Rewrite `victory_claim.rs` to consume `RulePack`

**Files:**
- Modify: `crates/vox-code-audit/src/detectors/victory_claim.rs`

The four hand-rolled `Regex::new` calls are replaced by lookups into the embedded `RulePack`. Existing unit tests in the file (`detects_victory_comment`, `detects_todo_leftover`, `detects_fixme`, `clean_code_no_findings`) must remain green.

- [ ] **Step 1: Replace the file contents.**

  Replace the entire contents of `crates/vox-code-audit/src/detectors/victory_claim.rs` with:
  ```rust
  //! Victory-claim detector. Patterns live in `contracts/code-audit/rules.v1.yaml`
  //! and are loaded via `embedded_rules::embedded_pack()`.

  use crate::embedded_rules::embedded_pack;
  use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
  use vox_rule_pack::CompiledRule;

  /// Loads the four `victory-claim/*` rules from the embedded rule pack.
  pub struct VictoryClaimDetector {
      rules: Vec<&'static CompiledRule>,
  }

  impl Default for VictoryClaimDetector {
      fn default() -> Self {
          Self::new()
      }
  }

  impl VictoryClaimDetector {
      pub fn new() -> Self {
          let pack = embedded_pack();
          let mut rules = Vec::with_capacity(4);
          for id in [
              "victory-claim/premature",
              "victory-claim/todo-leftover",
              "victory-claim/fixme",
              "victory-claim/hack",
          ] {
              rules.push(
                  pack.rule(id)
                      .unwrap_or_else(|| panic!("rule pack missing required rule: {id}")),
              );
          }
          Self { rules }
      }
  }

  impl DetectionRule for VictoryClaimDetector {
      fn id(&self) -> &'static str {
          "victory-claim"
      }
      fn name(&self) -> &'static str {
          "Victory Claim / Leftover Marker Detector"
      }
      fn description(&self) -> &'static str {
          "Detects premature 'Done!' comments, TODO/FIXME/HACK markers left behind"
      }
      fn severity(&self) -> Severity {
          Severity::Warning
      }
      fn languages(&self) -> &[Language] {
          &[
              Language::Rust,
              Language::TypeScript,
              Language::Python,
              Language::GDScript,
              Language::Vox,
          ]
      }

      fn detect(
          &self,
          file: &SourceFile,
          _rust: Option<&crate::analysis::RustFileContext>,
      ) -> Vec<Finding> {
          let mut findings = Vec::new();
          let context_radius_for = |id: &str| -> usize {
              if id == "victory-claim/premature" { 2 } else { 1 }
          };

          for (i, line) in file.lines.iter().enumerate() {
              let line_num = i + 1;
              let tri = line.trim_start();
              if tri.starts_with("///") || tri.starts_with("//!") {
                  continue;
              }

              for rule in &self.rules {
                  if !rule.matches_line(line) {
                      continue;
                  }
                  findings.push(Finding {
                      rule_id: rule.id.clone(),
                      rule_name: rule.name.clone(),
                      severity: rule.severity.into(),
                      file: file.path.clone(),
                      line: line_num,
                      column: 0,
                      message: rule.message.clone(),
                      suggestion: rule.suggestion.clone(),
                      context: file.context_around(line_num, context_radius_for(&rule.id)),
                      confidence: rule.confidence.map(Into::into),
                      evidence: None,
                  });
              }
          }

          findings
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use std::path::PathBuf;

      fn source(ext: &str, code: &str) -> SourceFile {
          SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
      }

      #[test]
      fn detects_victory_comment() {
          let d = VictoryClaimDetector::new();
          let snippet = concat!("// ", "D", "one!", " Implementation complete\nfn foo() {}");
          let f = source("rs", snippet);
          let findings = d.detect(&f, None);
          assert!(
              findings
                  .iter()
                  .any(|f| f.rule_id == "victory-claim/premature"),
              "should detect victory claim"
          );
      }

      #[test]
      fn detects_todo_leftover() {
          let d = VictoryClaimDetector::new();
          let py = concat!("# TO", "DO: implement later", "\ndef foo():\n    pass");
          let f = source("py", py);
          let findings = d.detect(&f, None);
          assert!(
              findings
                  .iter()
                  .any(|f| f.rule_id == "victory-claim/todo-leftover"),
              "should detect TODO leftover"
          );
      }

      #[test]
      fn detects_fixme() {
          let d = VictoryClaimDetector::new();
          let snippet = concat!("// ", "FIX", "ME this is broken\nconst x = 1;");
          let f = source("ts", snippet);
          let findings = d.detect(&f, None);
          assert!(
              findings.iter().any(|f| f.rule_id == "victory-claim/fixme"),
              "should detect FIXME"
          );
      }

      #[test]
      fn clean_code_no_findings() {
          let d = VictoryClaimDetector::new();
          let f = source(
              "rs",
              "/// Adds two numbers.\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}",
          );
          let findings = d.detect(&f, None);
          assert!(findings.is_empty());
      }
  }
  ```

- [ ] **Step 2: Run the per-detector tests.**

  Run: `cargo test -p vox-code-audit detectors::victory_claim`
  Expected: 4 passed.

- [ ] **Step 3: Run the parity test.**

  Run: `cargo test -p vox-code-audit --test victory_claim_parity`
  Expected: PASS. If it fails, **do not** edit the parity test — instead, fix the YAML pattern or the new detector code until findings match the baseline.

- [ ] **Step 4: Run the full vox-code-audit suite.**

  Run: `cargo test -p vox-code-audit --no-fail-fast`
  Expected: all green, including `detectors::tests::all_rules_instantiate` (still 23 rules).

- [ ] **Step 5: Commit.**

  ```
  git add crates/vox-code-audit/src/detectors/victory_claim.rs
  git commit -m "refactor(code-audit): victory_claim consumes vox-rule-pack"
  ```

## Task 3.6: PR3 verification

- [ ] **Step 1: Full workspace build.**
  Run: `cargo build --workspace`
  Expected: exit 0.

- [ ] **Step 2: Full test run.**
  Run: `cargo test --workspace --no-fail-fast`
  Expected: all green.

- [ ] **Step 3: Architecture check.**
  Run: `cargo run -p vox-arch-check`
  Expected: exit 0.

- [ ] **Step 4: Confirm no new heavy deps reached vox-code-audit.**
  Run: `cargo tree -p vox-code-audit --prefix none --no-default-features --edges normal | Select-String -Pattern "tantivy|qdrant|hnsw"`
  Expected: empty output. If any line returns, stop and investigate — `vox-rule-pack` must not transitively pull these in.

---

# PR4 — `vox ci detect-rules-bench` authoring-time tool

PR4 adds a CLI subcommand that runs every rule in the pack against its labeled fixtures, computes precision/recall, and writes a JSON report.

## Task 4.1: Define the report types

**Files:**
- Create: `crates/vox-rule-pack/src/bench.rs`
- Modify: `crates/vox-rule-pack/src/lib.rs`

The bench logic lives in `vox-rule-pack` so other consumers (Scientia heuristics in a follow-up plan) can reuse it.

- [ ] **Step 1: Write the failing test.**

  Create `crates/vox-rule-pack/src/bench.rs`:
  ```rust
  //! Authoring-time bench: runs rules against fixtures, computes precision/recall.
  //!
  //! Pure function over RulePack + filesystem of fixtures. No network, no LLM.

  use crate::pack::RulePack;
  use serde::{Deserialize, Serialize};
  use std::path::Path;

  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct RuleBenchResult {
      pub rule_id: String,
      pub positive_total: u32,
      pub positive_matched: u32,
      pub negative_total: u32,
      pub negative_matched: u32,
      pub precision: f64,
      pub recall: f64,
      pub f1: f64,
  }

  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct BenchReport {
      pub generated_at_unix: u64,
      pub rules: Vec<RuleBenchResult>,
  }

  /// Runs the bench against `pack`, resolving fixture paths relative to `fixtures_root`.
  pub fn run_bench(pack: &RulePack, fixtures_root: &Path) -> BenchReport {
      let now = std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .map(|d| d.as_secs())
          .unwrap_or(0);
      let mut results = Vec::with_capacity(pack.len());
      for rule in pack.rules() {
          results.push(score_rule(rule, fixtures_root));
      }
      BenchReport {
          generated_at_unix: now,
          rules: results,
      }
  }

  fn score_rule(rule: &crate::pack::CompiledRule, fixtures_root: &Path) -> RuleBenchResult {
      // Fixture path lookups: walk fixtures_root for files under <rule_id with '/' replaced by '/'>.
      // For PR4 we read fixtures from disk based on the rule id (segment before '/' = directory).
      let parent = rule.id.split('/').next().unwrap_or(&rule.id);
      let dir = fixtures_root.join(parent);
      let mut pos_total = 0u32;
      let mut pos_match = 0u32;
      let mut neg_total = 0u32;
      let mut neg_match = 0u32;

      let suffix = rule.id.split('/').nth(1).unwrap_or("");
      if let Ok(entries) = std::fs::read_dir(&dir) {
          for entry in entries.flatten() {
              let name = entry.file_name().to_string_lossy().to_string();
              let is_pos = name.starts_with(&format!("{suffix}_pos"))
                  || (suffix.is_empty() && name.contains("_pos"));
              let is_neg = name.starts_with(&format!("{suffix}_neg"))
                  || (suffix.is_empty() && name.contains("_neg"));
              if !is_pos && !is_neg {
                  continue;
              }
              let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
              let line = content.lines().next().unwrap_or("");
              let matched = rule.matches_line(line);
              if is_pos {
                  pos_total += 1;
                  if matched {
                      pos_match += 1;
                  }
              } else {
                  neg_total += 1;
                  if matched {
                      neg_match += 1;
                  }
              }
          }
      }

      let tp = pos_match as f64;
      let fp = neg_match as f64;
      let fn_ = (pos_total - pos_match) as f64;
      let precision = if tp + fp == 0.0 { 1.0 } else { tp / (tp + fp) };
      let recall = if tp + fn_ == 0.0 { 1.0 } else { tp / (tp + fn_) };
      let f1 = if precision + recall == 0.0 {
          0.0
      } else {
          2.0 * precision * recall / (precision + recall)
      };

      RuleBenchResult {
          rule_id: rule.id.clone(),
          positive_total: pos_total,
          positive_matched: pos_match,
          negative_total: neg_total,
          negative_matched: neg_match,
          precision,
          recall,
          f1,
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use std::io::Write;
      use tempfile::TempDir;

      fn write(p: &Path, content: &str) {
          if let Some(parent) = p.parent() {
              std::fs::create_dir_all(parent).unwrap();
          }
          let mut f = std::fs::File::create(p).unwrap();
          writeln!(f, "{content}").unwrap();
      }

      #[test]
      fn perfect_classifier_reports_f1_one() {
          let yaml = r#"
  version: 1
  rules:
    - id: alpha/foo
      name: F
      description: F
      severity: warning
      languages: [rust]
      match: { kind: line-regex, pattern: "^foo$" }
      message: m
  "#;
          let pack = RulePack::load_from_str(yaml).unwrap();
          let dir = TempDir::new().unwrap();
          write(&dir.path().join("alpha").join("foo_pos_a.txt"), "foo");
          write(&dir.path().join("alpha").join("foo_neg_a.txt"), "bar");
          let report = run_bench(&pack, dir.path());
          let r = &report.rules[0];
          assert_eq!(r.positive_total, 1);
          assert_eq!(r.positive_matched, 1);
          assert_eq!(r.negative_total, 1);
          assert_eq!(r.negative_matched, 0);
          assert!((r.f1 - 1.0).abs() < 1e-9);
      }
  }
  ```

- [ ] **Step 2: Run the test.**

  Run: `cargo test -p vox-rule-pack bench::tests`
  Expected: 1 passed.

- [ ] **Step 3: Register `bench` in lib.rs.**

  Edit `crates/vox-rule-pack/src/lib.rs` to add:
  ```rust
  pub mod bench;
  pub use bench::{BenchReport, RuleBenchResult, run_bench};
  ```

- [ ] **Step 4: Commit.**

  ```
  git add crates/vox-rule-pack/src/bench.rs crates/vox-rule-pack/src/lib.rs
  git commit -m "feat(rule-pack): bench scorer (precision/recall/F1)"
  ```

## Task 4.2: Add the `vox ci detect-rules-bench` subcommand

**Files:**
- Create: `crates/vox-cli/src/commands/ci/detect_rules_bench.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs`
- Modify: `crates/vox-cli/Cargo.toml`

- [ ] **Step 1: Add the dependency.**

  Edit `crates/vox-cli/Cargo.toml`. Add to `[dependencies]` (alphabetical):
  ```toml
  vox-rule-pack = { workspace = true }
  ```

- [ ] **Step 2: Read the existing CI command catalog.**

  Read: `crates/vox-cli/src/commands/ci/mod.rs` to understand how subcommands are registered (look for an existing simple subcommand to use as a template — e.g., `command-sync` or `secret-env-guard`).

- [ ] **Step 3: Write the failing test.**

  Create `crates/vox-cli/src/commands/ci/detect_rules_bench.rs`:
  ```rust
  //! `vox ci detect-rules-bench` — runs the rule pack against committed fixtures
  //! and writes contracts/reports/code-audit/rules-bench-latest.json.

  use std::path::{Path, PathBuf};
  use vox_rule_pack::{RulePack, run_bench};

  /// Default workspace-relative paths.
  pub const RULES_YAML_REL: &str = "contracts/code-audit/rules.v1.yaml";
  pub const FIXTURES_REL: &str = "contracts/code-audit/fixtures";
  pub const REPORT_REL: &str = "contracts/reports/code-audit/rules-bench-latest.json";

  pub fn run(workspace_root: &Path, check_only: bool) -> anyhow::Result<()> {
      let rules_path: PathBuf = workspace_root.join(RULES_YAML_REL);
      let fixtures_path: PathBuf = workspace_root.join(FIXTURES_REL);
      let report_path: PathBuf = workspace_root.join(REPORT_REL);

      let pack = RulePack::load_from_path(&rules_path)?;
      let report = run_bench(&pack, &fixtures_path);

      let mut bad: Vec<String> = Vec::new();
      for r in &report.rules {
          if r.f1 < 0.99 {
              bad.push(format!(
                  "{}: f1={:.3} precision={:.3} recall={:.3} (pos {}/{}, neg {}/{})",
                  r.rule_id,
                  r.f1,
                  r.precision,
                  r.recall,
                  r.positive_matched,
                  r.positive_total,
                  r.negative_matched,
                  r.negative_total,
              ));
          }
      }

      // Only the non-check (publish) mode writes the report file. Check mode is read-only
      // so concurrent test invocations do not race on the canonical report path.
      if !check_only {
          if let Some(parent) = report_path.parent() {
              std::fs::create_dir_all(parent)?;
          }
          let json = serde_json::to_string_pretty(&report)?;
          std::fs::write(&report_path, json)?;
      }

      if check_only && !bad.is_empty() {
          for line in &bad {
              eprintln!("FAIL {line}");
          }
          anyhow::bail!("{} rule(s) failed F1 ≥ 0.99 gate", bad.len());
      }
      Ok(())
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use std::path::PathBuf;

      fn workspace_root() -> PathBuf {
          PathBuf::from(env!("CARGO_MANIFEST_DIR"))
              .join("..")
              .join("..")
      }

      #[test]
      fn check_mode_passes_for_canonical_corpus() {
          // check_only = true is read-only; safe to run concurrently with other tests.
          let root = workspace_root();
          run(&root, true).expect("bench check must pass on canonical corpus");
      }
  }
  ```

- [ ] **Step 4: Wire the subcommand into the CI catalog.**

  Edit `crates/vox-cli/src/commands/ci/mod.rs`. Following the template subcommand pattern you observed in step 2:
  - Add `pub mod detect_rules_bench;`.
  - Add the `DetectRulesBench` variant to the `cmd_enums::CiCmd` enum (or whichever enum the catalog uses).
  - Add the dispatch arm so `vox ci detect-rules-bench [--check]` calls `detect_rules_bench::run(&workspace_root, check_flag)`.
  - Use the same flag style as existing subcommands; the flag `--check` should be optional (default false).

  If the catalog generates docs from a manifest file (look for `command_catalog_paths_baseline.txt` or similar), add the new path there as well.

- [ ] **Step 5: Run the unit tests.**

  Run: `cargo test -p vox-cli detect_rules_bench`
  Expected: 1 passed.

- [ ] **Step 6: Run the bench end-to-end.**

  Run: `cargo run -p vox-cli -- ci detect-rules-bench`
  Expected: writes `contracts/reports/code-audit/rules-bench-latest.json`. Open the file and confirm all four `victory-claim/*` rules show F1 ≥ 0.99.

  Run: `cargo run -p vox-cli -- ci detect-rules-bench --check`
  Expected: exit 0.

- [ ] **Step 7: Regenerate CLI surface docs.**

  Run: `cargo run -p vox-cli -- ci command-sync`
  Expected: updates `docs/src/reference/cli-command-surface.generated.md` to include the new subcommand. Stage the regenerated file.

- [ ] **Step 8: Commit.**

  ```
  git add crates/vox-cli/Cargo.toml crates/vox-cli/src/commands/ci/detect_rules_bench.rs crates/vox-cli/src/commands/ci/mod.rs docs/src/reference/cli-command-surface.generated.md contracts/reports/code-audit/rules-bench-latest.json
  git commit -m "feat(cli): vox ci detect-rules-bench (precision/recall report)"
  ```

## Task 4.3: PR4 verification

- [ ] **Step 1: Full workspace build.**
  Run: `cargo build --workspace`
  Expected: exit 0.

- [ ] **Step 2: Full test run.**
  Run: `cargo test --workspace --no-fail-fast`
  Expected: all green.

- [ ] **Step 3: Architecture check.**
  Run: `cargo run -p vox-arch-check`
  Expected: exit 0.

- [ ] **Step 4: Doc-pipeline check.**
  Run: `cargo run -p vox-doc-pipeline -- --check`
  Expected: clean (the regenerated CLI surface doc must match what command-sync produces).

---

# PR5 — Lock in the invariants

PR5 adds the architectural rules that prevent the design from being eroded later: forbid heavy deps from leaking into the rule-pack stack. (A line-scan lint that requires any *new* `Regex::new` literal in detectors to carry an exemption marker is a follow-up — useful but not load-bearing without first migrating the wave-2 detectors. Tracked as future work in the wave-migration plan.)

## Task 5.1: Add `vox-arch-check` dependency invariants

**Files:**
- Modify: `docs/src/architecture/layers.toml` (or the `vox-arch-check` rule source — read it first to see where dependency forbids live)

The exact format depends on the existing `vox-arch-check` schema. Read [`crates/vox-arch-check/`](../../../crates/vox-arch-check/) source to find how forbidden dependencies are declared and where.

- [ ] **Step 1: Read the arch-check source to find the rule format.**

  Run: `Get-ChildItem crates/vox-arch-check -Recurse -File -Filter *.rs`
  Read each file under `crates/vox-arch-check/src/` to identify how dependency restrictions are configured (in `layers.toml`, in code, or both).

- [ ] **Step 2: Add the four invariants.**

  In whichever surface owns dependency policy, add these four rules:
  - `vox-rule-pack` MUST NOT depend on `vox-search`, `tantivy`, `qdrant-client`, `vox-corpus`.
  - `vox-code-audit` MUST NOT depend on `vox-search`.
  - `vox-publisher` MUST NOT depend on `vox-search` (transitively or directly).
  - Any new crate added to L1 MUST NOT depend on `vox-search` family.

  If `layers.toml` already supports a `forbidden_deps` field per crate row, add the entries there. If invariants are encoded in Rust code in `vox-arch-check`, add them as a new check function with a unit test.

- [ ] **Step 3: Write a regression test.**

  Add a test in `crates/vox-arch-check/tests/` (create the directory if needed) that asserts the four invariants by parsing `cargo metadata` output and walking the dep graph from each named crate, failing if any forbidden crate appears.

- [ ] **Step 4: Run the test.**

  Run: `cargo test -p vox-arch-check`
  Expected: all green.

- [ ] **Step 5: Run arch-check.**

  Run: `cargo run -p vox-arch-check`
  Expected: exit 0.

- [ ] **Step 6: Commit.**

  ```
  git add docs/src/architecture/layers.toml crates/vox-arch-check/
  git commit -m "chore(arch): forbid vox-search deps in rule-pack/code-audit/publisher"
  ```

## Task 5.2: PR5 verification

- [ ] **Step 1: Full workspace build.**
  Run: `cargo build --workspace`
  Expected: exit 0.

- [ ] **Step 2: Full test run.**
  Run: `cargo test --workspace --no-fail-fast`
  Expected: all green.

- [ ] **Step 3: Architecture check.**
  Run: `cargo run -p vox-arch-check`
  Expected: exit 0, including the new dep invariants and the advisory regex-literal lint.

- [ ] **Step 4: Bench check.**
  Run: `cargo run -p vox-cli -- ci detect-rules-bench --check`
  Expected: exit 0; all `victory-claim/*` rules at F1 ≥ 0.99.

- [ ] **Step 5: Confirm no heavy deps reach `vox-rule-pack` consumers.**
  Run: `cargo tree -p vox-code-audit --no-default-features --edges normal | Select-String -Pattern "tantivy|qdrant|hnsw"`
  Expected: empty.

  Run: `cargo tree -p vox-publisher --no-default-features --edges normal | Select-String -Pattern "tantivy|qdrant|hnsw"`
  Expected: empty (or unchanged from baseline if `vox-publisher` already has any of these for unrelated reasons; this plan must not increase its set).

---

# Final acceptance gate

Before declaring the plan done:

- [ ] **Build:** `cargo build --workspace` green.
- [ ] **Test:** `cargo test --workspace --no-fail-fast` green; `detectors::tests::all_rules_instantiate` still asserts exactly 23 rules; `victory_claim_parity` green.
- [ ] **Architecture:** `cargo run -p vox-arch-check` green, four new dep invariants in place.
- [ ] **Bench:** `cargo run -p vox-cli -- ci detect-rules-bench --check` green; report committed at `contracts/reports/code-audit/rules-bench-latest.json`.
- [ ] **Docs:** `cargo run -p vox-doc-pipeline -- --check` green; CLI surface doc updated by `vox ci command-sync`.
- [ ] **Dep hygiene:** `cargo tree -p vox-code-audit` shows no new heavy deps vs. baseline.
- [ ] **No emojis** in any new file.
- [ ] **Pre-commit hooks** pass on the merge commit (`vox run scripts/install-hooks.vox` was run once; lefthook `tdd-guard` and other hooks are quiet).

When the gate is green, the foundation is in place. The next plan (`2026-05-09-detector-rule-ssot-wave-migrations-plan.md`, to be authored by the same agent following this plan as a template) covers Wave 2–4 detector migrations and the Scientia heuristics consolidation.
