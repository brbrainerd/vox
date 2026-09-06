---
title: "Vox Language Rules — Phase 1: SSOT Collapse (2026-05-09)"
description: "Step-by-step plan to collapse hand-mirrored Rust↔Vox surfaces into single-source-of-truth + xtask-generated outputs. Generates the typechecker builtin manifest, LSP completions, system prompt sections, mdbook reference pages, decorator catalog, diagnostic catalog scaffolding, and TS codegen headers from one Rust source each. Every generated file gets a blake3 provenance header and a CI drift check."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 1 child plan of vox-language-rules-and-enforcement-plan-2026.md. Generation patterns shown here are reusable for any future Rust↔Vox seam."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-actor-runtime/builtins/builtin_registry.rs: extended with #[derive] + xtask-readable schema"
  - "vox-grammar-export: gains tree-sitter, LSP, mdbook, system-prompt emitters"
  - "vox-codegen: every TS/Rust output gains @generated-hash header"
  - "vox-lsp: completion table loaded from generated manifest"
  - "mens/config/system_prompt.txt: split into hand-edited narrative + interpolated sections"
  - "xtask: new gen-builtins, gen-decorators, gen-grammar-tables, gen-system-prompt subcommands"
---

# Phase 1 — SSOT Collapse

> **Parent plan:** [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md)
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the largest hand-mirrored surfaces in the Rust↔Vox seam. After this phase, the builtin registry, the decorator catalog, the grammar exports, the LSP completions, the docs reference pages, the Mens system-prompt construct list, and the diagnostic catalog all derive from a single Rust source per surface, with a blake3 provenance header on every emission and a CI drift check that fails on hand-edits.

**Architecture:** A new `xtask` binary group (`crates/xtask/src/bin/gen_*.rs`) reads typed Rust sources (the existing `BuiltinRegistryEntry` table, a new `#[vox_decorator]` attribute applied to decorator implementations, the existing `vox-grammar-export` IR) and emits derived files. Each emitted file starts with a two-line header:

```
// @generated from <source>:<line> at commit <git-hash>
// @generated-hash <blake3 of bytes after this header>
```

A new `vox-arch-check` rule (`generated-file-drift`) recomputes the hash on every committed file matching `*.generated.*` or `**/generated/*` and fails CI if the header doesn't match. This is the structural enforcement of [A.36] and pairs with the existing `cli-command-surface.generated.md` pattern in [AGENTS.md:47–53](../../../AGENTS.md).

**Tech stack:** Rust 2021, `blake3` (already in workspace), `serde_json` for intermediate manifests, `handlebars-rust` for the system-prompt templating (new dep, audited; alternative is `tinytemplate` already in workspace — Task 6 picks one).

**Out of scope for Phase 1:**
- Any change to the *contents* of the builtin set, decorator set, or grammar (all source surfaces stay byte-identical post-migration; only the *consumer* files change).
- New diagnostic IDs (Phase 2 ships those; Phase 1 only scaffolds the catalog enum).
- LSP completion *behavior* changes (only the data source changes).
- Any change to `mens/config/system_prompt.txt`'s narrative sections; only the interpolated construct list changes.

---

## Verification setup

- `cargo test -p vox-actor-runtime --lib builtins::` — builtin schema serializability tests.
- `cargo run -p xtask -- gen-builtins --check` — must report zero drift after each task that touches the registry.
- `cargo run -p xtask -- gen-decorators --check`
- `cargo run -p xtask -- gen-grammar-tables --check`
- `cargo run -p xtask -- gen-system-prompt --check`
- `cargo run -p vox-arch-check` — must pass; `generated-file-drift` rule lands in Task 9.
- `cargo run -p vox-doc-pipeline -- --check` — must pass after Task 5 (mdbook integration).

---

## Task 1: Diagnostic-catalog enum scaffolding (no IDs yet)

**Files:**
- Create: `crates/vox-code-audit/src/diagnostics/catalog.rs`
- Create: `crates/vox-code-audit/src/diagnostics/mod.rs`
- Modify: `crates/vox-code-audit/src/lib.rs` — `pub mod diagnostics;`

**Why this first:** Every later phase emits diagnostics via this catalog. Scaffolding it here (empty enum + the `#[diagnostic]` attribute macro) lets Phase 2 add IDs without designing the scaffolding under deadline pressure.

**Code (skeleton only — no real diagnostics yet):**

```rust
// crates/vox-code-audit/src/diagnostics/catalog.rs
//! Single-source diagnostic catalog. Every Vox lint, type rule, and runtime
//! trap is registered here as an enum variant. Generated outputs (docs page,
//! LSP code-action map, --explain pages) read this catalog at build time.
//!
//! Variants are *append-only*. To rename a diagnostic, add a new variant and
//! mark the old one #[diagnostic(deprecated_alias_of = "...", since = "...")].

use crate::diagnostics::Severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticId {
    // Phase 2 will populate. Empty for Phase 1.
}

/// Metadata attached to each variant via the proc-macro in Task 2.
pub struct DiagnosticMeta {
    pub id: &'static str,
    pub severity: Severity,
    pub since: &'static str,
    pub adr: Option<&'static str>,
    pub explain_path: &'static str,
}

impl DiagnosticId {
    pub fn meta(&self) -> DiagnosticMeta {
        match *self {
            // populated by proc-macro in Task 2
        }
    }
}
```

**Verify:** `cargo build -p vox-code-audit` passes. Add a `#[test] fn catalog_module_compiles() {}` to lock the module path.

---

## Task 2: `#[vox_diagnostic]` proc-macro

**Files:**
- Create: `crates/vox-code-audit-macros/Cargo.toml` (new crate; `proc-macro = true`)
- Create: `crates/vox-code-audit-macros/src/lib.rs`
- Modify: `crates/vox-code-audit/Cargo.toml` — depend on the new macro crate
- Modify: `crates/vox-code-audit/src/diagnostics/catalog.rs` — re-export the macro, replace the `meta()` skeleton with macro-generated impl

**Why:** Diagnostic IDs need stability metadata (`since`, `adr`, severity, explain page). A proc-macro keeps the metadata declared inline with each variant rather than in a parallel table that drifts.

**Macro shape:**

```rust
#[proc_macro_attribute]
pub fn vox_diagnostic(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parses #[vox_diagnostic(id = "vox/effect/unjustified-net",
    //                         severity = "warning",
    //                         since = "0.6.0",
    //                         adr = "ADR-024",
    //                         explain = "diagnostics/effect/unjustified-net.md")]
    // Generates the meta() arm for the variant.
}
```

**Verify:** A round-trip test in `crates/vox-code-audit-macros/tests/` that declares two dummy variants, asserts the generated `meta()` returns the right values, and asserts compile-fail on duplicate IDs.

---

## Task 3: Lift `BuiltinRegistryEntry` to a serializable IR

**Files:**
- Modify: `crates/vox-actor-runtime/src/builtins/builtin_registry.rs`
- Create: `crates/vox-actor-runtime/src/builtins/manifest.rs`

**Why:** Today the registry is hand-coded plain Rust. The typechecker, LSP, system prompt, and docs each maintain a parallel mirror. This task extracts a *serializable* manifest from the registry without changing the registry's API.

**Approach:** Add `#[derive(Serialize)]` to `BuiltinRegistryEntry`'s fields (or a `BuiltinManifestEntry` mirror that the registry projects into). Add `pub fn to_manifest() -> Vec<BuiltinManifestEntry>` that walks the registry array and emits a `Vec` suitable for `serde_json` serialization.

The `BuiltinManifestEntry` carries: name, namespace, signature (typed, not stringly), effect set (`@uses(...)` shape — empty for now; populated in Phase 5), docstring, since-version, deprecation status, and example snippet.

**Verify:** `cargo test -p vox-actor-runtime --lib builtins::manifest::round_trip` — serialize the manifest to JSON, deserialize, assert equality with the in-memory form. Snapshot the JSON output to `crates/vox-actor-runtime/tests/snapshots/builtin_manifest.snap.json` (rebaselined whenever the registry intentionally changes).

---

## Task 4: `xtask gen-builtins` — emit typechecker, LSP, docs, system-prompt section

**Files:**
- Create: `crates/xtask/src/bin/gen_builtins.rs`
- Create (emitted): `crates/vox-compiler/src/typeck/generated/builtin_signatures.rs`
- Create (emitted): `crates/vox-lsp/src/completions/generated/builtins.json`
- Create (emitted): `docs/src/reference/builtins.generated.md`
- Create (emitted): `mens/config/generated/builtins-section.txt`

**Why:** This is the largest single SSOT-collapse win. Today these four files are maintained by hand; after this task they all derive from `to_manifest()` in Task 3.

**Flags:**
- `cargo run -p xtask -- gen-builtins` — regenerate all four targets.
- `cargo run -p xtask -- gen-builtins --check` — recompute and diff; nonzero exit on drift. CI uses this.
- `cargo run -p xtask -- gen-builtins --target typeck` — regenerate one target only.

**Header format** (constant across all generated files in this phase):

```
// @generated from crates/vox-actor-runtime/src/builtins/builtin_registry.rs:<line> at commit <hash>
// @generated-hash <blake3>
// DO NOT EDIT. Regenerate with: cargo run -p xtask -- gen-builtins
```

For markdown / text files, use `<!--` comments instead of `//`.

**Verify:** Add `crates/xtask/tests/gen_builtins_smoke.rs` that runs the generator, asserts all four files exist, asserts each starts with the header, asserts hash recompute passes.

---

## Task 5: Wire generated builtin docs into mdbook

**Files:**
- Modify: `docs/src/SUMMARY.md` is auto-generated — add appropriate frontmatter to `docs/src/reference/builtins.generated.md` (`title`, `category: "Language Reference"`, `sort_order`, `training_eligible: true`)
- Modify: `crates/vox-doc-pipeline/src/lib.rs` (or its test set) — add a smoke test that the builtins reference page renders without errors

**Why:** [AGENTS.md:204–210 (Markdown Hygiene and Code Snippets)](../../../AGENTS.md) requires every `vox` block in docs to compile via `vox-doc-pipeline`. The generator must emit `vox` blocks that round-trip; this task wires the test.

**Verify:** `cargo run -p vox-doc-pipeline -- --check` passes; the rendered reference page lists every builtin with signature, doc, and example.

---

## Task 6: `xtask gen-system-prompt` — split narrative + interpolated

**Files:**
- Modify: `mens/config/system_prompt.txt` — extract narrative-only sections; replace construct lists with template directives like `{{> generated/builtins-section.txt}}` and `{{> generated/decorators-section.txt}}` and `{{> generated/grammar-section.txt}}`
- Create: `mens/config/system_prompt.template.txt` — the new master file (rename of the above)
- Create (emitted): `mens/config/generated/builtins-section.txt`, `decorators-section.txt`, `grammar-section.txt`
- Create (emitted): `mens/config/system_prompt.txt` — composed output (now derived, hashed)
- Create: `crates/xtask/src/bin/gen_system_prompt.rs`

**Template engine choice:** `tinytemplate` (already in workspace, ~3KB). Rejected `handlebars-rust` (new dep, ~200KB compiled).

**Migration:** Hand-author the partition once: identify which sections of the current `system_prompt.txt` are narrative (kept hand-edited in `.template.txt`) vs construct lists (replaced with `{{> ...}}` directives). Keep a diff in the PR showing the partition is faithful to the current prompt.

**Verify:** Round-trip test: regenerate, diff against pre-migration snapshot of `system_prompt.txt`, expect zero diff in narrative sections, expect construct sections to match the new generator output.

---

## Task 7: `#[vox_decorator]` attribute + decorator catalog

**Files:**
- Create: `crates/vox-compiler/src/decorators/registry.rs`
- Create: `crates/vox-compiler-macros/Cargo.toml` (extend existing macros crate, or new)
- Modify: existing decorator implementations in `crates/vox-compiler/src/lower/decorators/*.rs` to wear `#[vox_decorator(name = "@table", category = "type-modifier", since = "0.3.0")]`
- Create: `crates/xtask/src/bin/gen_decorators.rs`
- Create (emitted): `docs/src/reference/decorators.generated.md`, `mens/config/generated/decorators-section.txt`, `crates/vox-lsp/src/completions/generated/decorators.json`

**Why:** Decorator catalog is the same SSOT-collapse pattern as builtins. The audit's [A.33] is the structural enforcement of [AGENTS.md:154–156] (no new bare keyword for decorator-shaped behavior): to add a decorator you add a Rust impl with `#[vox_decorator(...)]`; to add a bare keyword you must touch the (closed in Task 12) lexer table and produce an ADR.

**Verify:** Same generator-smoke pattern as Task 4. Plus: a `vox-arch-check` rule that fails CI if a `crates/vox-compiler/src/lower/decorators/*.rs` file declares a public decorator-shaped fn without the `#[vox_decorator]` attribute (Task 11).

---

## Task 8: `xtask gen-grammar-tables` — tree-sitter, LSP keywords, mdbook grammar page

**Files:**
- Modify: `crates/vox-grammar-export/src/lib.rs` — re-expose the grammar IR through a stable public surface
- Create: `crates/xtask/src/bin/gen_grammar_tables.rs`
- Create (emitted): `tools/tree-sitter-vox/grammar.js` (overwritten on each gen)
- Create (emitted): `crates/vox-lsp/src/completions/generated/keywords.json`
- Create (emitted): `docs/src/reference/grammar.generated.md`
- Create (emitted): `mens/config/generated/grammar-section.txt`

**Why:** `vox-grammar-export` exists. Today it has consumers but no enforced single-emission contract. This task makes it the SSOT for the four downstream surfaces.

**Out of scope for this task:** Tree-sitter grammar correctness against editor parsing — that's a separate `tools/tree-sitter-vox/` ownership question. This task only commits to *deriving* `grammar.js` from the IR; quality of the editor parse comes later.

**Verify:** Same generator-smoke pattern as Task 4.

---

## Task 9: `vox-arch-check` rule `generated-file-drift`

**Files:**
- Modify: `crates/vox-arch-check/src/main.rs` (or appropriate module)
- Modify: `crates/vox-arch-check/src/rules/` — add `generated_file_drift.rs`
- Modify: `docs/src/architecture/layers.toml` — declare which paths are "generated" (`*.generated.*`, `**/generated/*`, files matching the header pattern)

**Why:** This is the structural enforcement that locks every downstream generator. Without it, hand-edits silently bypass the SSOT.

**Detection algorithm:**

1. Walk the workspace.
2. For each file matching the configured "generated" globs OR whose first 5 lines contain `@generated-hash`:
3. Parse the `@generated-hash <H>` line.
4. Compute `blake3(file_bytes_after_header_block)`.
5. Fail if the values differ. Error message includes the regen command (parsed from the header's `Regenerate with:` line, when present).

**Severity ramp:**
- Land as `warning` in CI for one minor version.
- Escalate to `error` in the next minor version.
- An override mechanism (`contracts/codegen/drift-allowlist.v1.json`) for transitional cases, with a max-30-day expiry per entry.

**Verify:** Integration test in `crates/vox-arch-check/tests/` — synthesize a file with a wrong hash, run the rule, expect a structured error pointing at the file and the regen command.

---

## Task 10: TypeScript codegen header

**Files:**
- Modify: `crates/vox-codegen/src/typescript/emitter.rs` (or wherever the TS file head is written)
- Modify: existing TS test fixtures to expect the new header
- Modify: `crates/vox-codegen/tests/snapshots/*.ts` — rebaseline

**Why:** [A.36] applied to the largest existing codegen surface. Today TS output has no provenance header; the file looks hand-authored.

**Header for TypeScript:**

```typescript
// @generated from <vox-source>:<line> at commit <git-hash>
// @generated-hash <blake3>
// DO NOT EDIT. Regenerate with: vox build --target typescript
```

**Verify:** Existing codegen snapshot tests rebaseline; add a new test that asserts every emitted `.ts` file starts with the header.

---

## Task 11: `vox-arch-check` rule `decorator-without-attribute`

**Files:**
- Modify: `crates/vox-arch-check/src/rules/` — add `decorator_without_attribute.rs`

**Why:** Closes the loop on Task 7. If a contributor adds a decorator-shaped public Rust fn under `crates/vox-compiler/src/lower/decorators/` without `#[vox_decorator]`, the SSOT is silently bypassed. This rule fails CI in that case.

**Verify:** Synthetic test crate with a public decorator-shaped fn missing the attr → rule fires.

---

## Task 12: Closed bare-keyword table

**Files:**
- Modify: `crates/vox-compiler/src/lexer/keywords.rs` — convert the keyword set from `Vec<&str>` (or whatever shape) to a `const KEYWORDS: &[&str] = &[...]` with a `#[non_exhaustive]`-style discipline: a separate `xtask add-keyword` is required to mutate it.
- Create: `crates/xtask/src/bin/add_keyword.rs` — interactive helper that:
  1. Validates the new keyword is referenced from an ADR with `keyword:` frontmatter.
  2. Updates the lexer keyword table.
  3. Updates the grammar export.
  4. Triggers `gen-grammar-tables`, `gen-system-prompt`.
  5. Adds a TASK entry to the appropriate phase plan.
- Modify: `AGENTS.md` §131–164 — add a clause: "New bare keywords require `xtask add-keyword`; manual edits to `crates/vox-compiler/src/lexer/keywords.rs::KEYWORDS` are rejected by `vox-arch-check::closed-keyword-table`."
- Modify: `crates/vox-arch-check/src/rules/` — add `closed_keyword_table.rs` that detects manual edits to the keyword constant by scanning `git diff` against the marker.

**Why:** Structural enforcement of [AGENTS.md:154–156] and audit [A.7]. Without this, the bare-keyword vs decorator policy depends on reviewer attention.

**Verify:** Synthetic PR that adds a keyword without the ADR/xtask flow → CI fails.

---

## Task 13: Provenance ledger for codegen outputs

**Files:**
- Create: `contracts/reports/codegen-ledger.v1.json` (initial empty file)
- Modify: every `xtask gen-*` binary to append a row on each generation: `{ generator, source_file, source_commit, output_path, output_hash, generated_at, generator_version }`
- Modify: `crates/xtask/src/lib.rs` (new shared helper module) to encapsulate the ledger append + concurrency safety (file lock).

**Why:** [A.50]. Tamper detection across releases. Lets a future agent ask "was this output produced by an authorized generator at a known commit?" and answer in O(1).

**Verify:** Run all `gen-*` tasks; assert the ledger now contains N+M rows where N was the prior count and M is the count of generated outputs in the workspace.

---

## Task 14: Documentation, AGENTS.md backlinks, where-things-live update

**Files:**
- Modify: `AGENTS.md` — add §"Generated File Discipline" near §40–53 with the new rules, link to this phase plan
- Modify: `docs/src/architecture/where-things-live.md` — add rows for `vox-code-audit-macros`, `crates/vox-compiler-macros` (if new), `tools/tree-sitter-vox`, `crates/xtask/src/bin/gen_*`
- Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.
- Modify: `docs/src/architecture/cli-command-surface.generated.md` — auto-regenerate; expect new `xtask` subcommands to appear

**Verify:** All three doc updates pass `cargo run -p vox-doc-pipeline -- --check`. `where-things-live.md` change shows in same PR.

---

## Risks specific to this phase

| Risk | Mitigation |
|---|---|
| `gen-system-prompt` partition (Task 6) misses a hand-edited construct list, drifting the prompt | Diff-snapshot the pre-migration prompt; reviewer must compare against the new composed output line-by-line. |
| Tree-sitter grammar emission (Task 8) produces a non-functional `grammar.js` | Treat `tools/tree-sitter-vox/grammar.js` as best-effort initially; add `tools/tree-sitter-vox/SMOKE.md` documenting "grammar shapes are emitted; editor-quality parse is a follow-up." |
| Generated-hash header (Task 9) trips on unrelated CI noise (line-ending normalization, etc.) | Generator writes with explicit `\n` line endings; `.gitattributes` forces LF on all `*.generated.*` paths. Hash is computed on bytes after the header, normalized. |
| `xtask add-keyword` (Task 12) becomes a friction point that contributors route around | Make the xtask fast (<5s) and accept all required input on the command line so CI can call it from a PR comment trigger. Document the bypass: there is no bypass; this is the point. |

---

## Phase 1 acceptance gate

Before declaring Phase 1 complete:

- [ ] All four `xtask gen-*` commands run cleanly with `--check` flag in CI.
- [ ] `vox-arch-check::generated-file-drift` rule lands as warning, escalates to error in the next minor.
- [ ] `vox-arch-check::decorator-without-attribute` rule lands as error.
- [ ] `vox-arch-check::closed-keyword-table` rule lands as error.
- [ ] `mens/config/system_prompt.txt` is now a generated artifact; its `.template.txt` source is hand-edited only.
- [ ] `docs/src/reference/builtins.generated.md`, `decorators.generated.md`, `grammar.generated.md` exist and render via `vox-doc-pipeline --check`.
- [ ] TypeScript codegen output carries the `@generated-hash` header.
- [ ] `contracts/reports/codegen-ledger.v1.json` is appended to on every generation.
- [ ] AGENTS.md §"Generated File Discipline" landed with backlinks.
- [ ] `where-things-live.md` updated.
- [ ] Retrospective note appended to this file.

---

## Retrospective

_To be appended within 5 working days of phase completion. Capture: actual vs estimated effort per task, scope changes, what surprised the team, what the next phase should reuse._
