---
title: "Vox Feature Discoverability Audit (2026)"
description: "Audit of CLI/GUI single-source-of-truth gaps, shell completion coverage, LSP capability parity, and discoverability improvements across the Vox toolchain."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-13"
training_eligible: true
training_rationale: "Captures systemic gaps in user-facing discoverability that affect both CLI and GUI surfaces."
sort_order: 50
---

# Vox Feature Discoverability Audit (2026)

**Scope:** CLI ↔ GUI single-source-of-truth integrity, shell auto-completion, LSP capability parity, and discoverability gaps across all user-facing surfaces.

**Audited surfaces:** `crates/vox-cli`, `crates/vox-gui`, `crates/vox-lsp`, `contracts/cli/command-registry.yaml`, `contracts/operations/catalog.v1.yaml`, `crates/vox-compiler/src/language_surface.rs`.

---

## Executive Summary

The architecture has **strong SSoT scaffolding** already in place — `command_catalog.rs` derives the command tree from Clap at runtime, the GUI calls it directly via `get_command_catalog`, and the `command-registry.yaml` is machine-projected from `contracts/operations/catalog.v1.yaml`. The shell completion plumbing (`vox completions <shell>`) exists and is wired. However, **several high-impact discoverability gaps remain**:

1. Shell completions are **generated but not installed** — no install automation, no `vox doctor` probe, no documentation.
2. The LSP `completions.rs` is **context-unaware** — it returns a flat union of all keywords/decorators regardless of cursor position or trigger character.
3. The GUI command catalog has **no fuzzy search**, no argument detail panel, and no feature-gate explanations in the Tauri frontend.
4. `vox commands` is classified `CatalogTier::Advanced` — it is not surfaced in the recommended first-run experience despite being the primary discoverability entry point.
5. **Feature-gated commands** (`mens`, `dei`, `populi`, `oratio`, `ludus`) are listed in the CLI but absent in GUI with no explanation of the missing feature gate.
6. **LSP hover** covers only 5 builtins (`Speech`, `transcribe`, `HTTP`, `OpenClaw`, `print`, `len`, `ret`); all `std.*` stdlib, `Id[T]`, `Result[T, E]`, and all effect decorators are undocumented.
7. `vox doctor` checks are not reflected in the GUI — a command that requires a feature gate does not show the doctor check path.
8. **Tree-sitter grammar** (`tree-sitter-vox/`) exists but the LSP uses a hand-written `grammar.rs` semantic token emitter instead — changes to the Vox grammar must be applied in four separate places.

---

## Gap 1 — Shell Auto-Completion: Exists but Unactivated

### Current state

`Cli::Completions { shell }` is defined in `crates/vox-cli/src/lib.rs:118-123` and dispatched in `crates/vox-cli/src/cli_dispatch/mod.rs:43`. It calls `clap_complete` to generate shell scripts for bash, zsh, fish, PowerShell, and elvish.

The CI surface has `vox ci completion-audit`, `vox ci completion-gates`, and `vox ci completion-ingest` registered in `contracts/cli/command-registry.yaml` — intent to gate on completions is present.

### Gaps

| Gap | Detail |
|-----|--------|
| **No install automation** | `vox completions powershell` writes to stdout; no `vox completions --install` flag auto-appends to `$PROFILE` or `/etc/bash_completion.d/`. |
| **No `vox doctor` check** | `vox doctor` does not verify whether completions are active in the current shell session. |
| **No docs** | `docs/src/reference/cli.md` has no "Enable shell completions" section. `README.md` has no mention. |
| **GUI has no completion surface** | The Tauri GUI exposes `get_command_catalog` and `execute_command` but has no keyboard-driven fuzzy palette that replicates completion behavior. |

### Recommended fixes

1. **Add `--install` flag** to `vox completions`:
   ```rust
   // crates/vox-cli/src/lib.rs
   Completions {
       shell: Shell,
       /// Auto-install completions into shell profile.
       #[arg(long)]
       install: bool,
   }
   ```
   When `--install` is set, detect the shell profile path (`$PROFILE` on PowerShell, `~/.bashrc` for bash, `~/.zshrc` for zsh) and append a `source` or `. <(vox completions <shell>)` directive. Emit a warning if the path is already present.

2. **Add a `vox doctor` completion probe** in `crates/vox-cli/src/commands/diagnostics/`. Check for presence of `vox` completions activation in `$FPATH` (zsh), `/etc/bash_completion.d/`, or the PowerShell `$PROFILE` file.

3. **Document** in `docs/src/contributors/getting-started.md` with per-shell examples.

---

## Gap 2 — LSP Completions: Flat, Context-Unaware

### Current state

`crates/vox-lsp/src/completions.rs` always returns the full union of keywords, decorators, types, and builtins regardless of context. The `_params: CompletionParams` argument is **entirely ignored** — trigger character, cursor position, and document text are all discarded.

### Consequences

- `@table` appears as a completion inside a `fn` body (semantically invalid).
- Keywords like `actor`, `workflow`, `activity` appear inside type expressions.
- Decorator snippets appear without `@` context — the trigger character `@` is registered in `capabilities.rs:43` but never inspected in `completions.rs`.
- No user-defined symbol completions (cross-file identifiers, locally declared functions).

### Recommended fixes (prioritized)

| Priority | Fix |
|----------|-----|
| **High** | Inspect `params.context.trigger_character`: if `@`, return only `LSP_DECORATOR_SNIPPETS`. If `.`, return method completions. Otherwise, top-level declarations only. |
| **High** | Parse the current line to detect syntactic context: inside a `type {}` block → field completions; inside `fn` body → builtins + local vars; at top level → bare-keyword declarations. |
| **Medium** | Cache the last-valid AST's declared names and include them in completions for cross-reference within the file. |
| **Medium** | Add `detail` and `documentation` fields to all keyword `CompletionItem`s (currently only decorators carry docs). |
| **Low** | Return `is_incomplete: true` for partial symbol matches to enable incremental server-side refinement. |

---

## Gap 3 — LSP Hover: Sparse Builtin Coverage

### Current state

`builtin_hover_markdown` in `crates/vox-lsp/src/lib.rs:401-429` covers only:
`Speech`, `transcribe`, `HTTP`, `OpenClaw`, `print`, `len`, `ret` — 7 identifiers total.

### Missing hover coverage

| Category | Missing symbols |
|----------|-----------------|
| Core types | `Result[T, E]`, `Option[T]`, `Id[T]`, `str`, `int`, `float`, `bool`, `List[T]`, `Map[K, V]` |
| Effect decorators | `@uses(net)`, `@uses(fs)`, `@pure`, `@durable`, `@scheduled`, `@auth`, `@public`, `@endpoint` |
| Stdlib namespaces | `std.fs.*`, `std.process.*`, `std.http.*`, `std.io.*`, `std.csv.*`, `std.toml.*`, `std.yaml.*` |
| Non-deterministic builtins | `uuid()`, `time.now()`, `random.*()`, `crypto.random_bytes()` (with workflow-body warning) |
| MENS decorators | `@inference`, `@training_step`, `@distributed_train` |
| Actor/workflow keywords | `workflow`, `activity`, `actor` (with durability semantics note) |

### Recommended fix

Add `LSP_BUILTIN_HOVER_DOCS: &[(&str, &str)]` to `crates/vox-compiler/src/language_surface.rs` (the existing SSOT file). The LSP `builtin_hover_markdown` function should look up this table instead of using a `match` expression. This ensures CLI `vox check` suggestion text, LSP hover, and the GUI command palette can all source from the same definitions.

---

## Gap 4 — GUI Command Catalog: No Argument Detail or Fuzzy Search

### Current state

- `crates/vox-gui/src/commands/catalog.rs` calls `vox_cli::command_catalog::build_catalog()` and serializes it.
- `CommandCatalogEntry` **already** includes `arguments: Vec<CommandCatalogArgument>` with `help`, `required`, `takes_value`, `short`, `long`.
- `crates/vox-gui/src/commands/dynamic_mapping.rs` exposes only `CommandMetadata` (product_lane, feature_gate, catalog_group, status) — `arguments` and `about` text are not forwarded as a Tauri command for UI consumption.
- `vox_cli::command_catalog::search_entries` / `search_entries_scored` exist and are fuzzy-capable (when `fuzzy-search` feature is active) but are **not exposed as a Tauri command**.

### Gaps

| Gap | Detail |
|-----|--------|
| **No fuzzy search** | CLI `vox commands --search <pattern>` works; GUI has no equivalent palette. |
| **No argument panel** | No UI for showing argument detail when a user selects a command in the GUI. |
| **Feature-gated commands invisible** | Commands behind `dei`, `mens`, etc. silently absent with no explanation. |
| **`vox gui` self-description** | The `gui` command is `#[cfg(feature = "gui")]` — builds without the feature show nothing. |

### Recommended fixes

1. **Add `search_catalog` Tauri command** in `crates/vox-gui/src/commands/catalog.rs`:
   ```rust
   #[tauri::command]
   pub fn search_catalog(pattern: String) -> Result<serde_json::Value, String> {
       let catalog = vox_cli::command_catalog::build_catalog();
       let results = vox_cli::command_catalog::search_entries(catalog.entries, &pattern);
       serde_json::to_value(&results).map_err(|e| e.to_string())
   }
   ```
   Register in `main.rs` `invoke_handler`.

2. **Expose `arguments` in catalog API**: `CommandCatalogEntry` already carries them with `#[serde(default)]`; they will serialize. Ensure the Tauri frontend consumes them for an argument detail panel.

3. **Add feature-gate explanation panel**: When `feature_gate` is non-null, show: *"This command requires the `<gate>` Cargo feature. Rebuild with `--features <gate>` or install the appropriate plugin bundle."*

---

## Gap 5 — `vox commands` Not Prominent

### Current state

`vox commands` (supporting `--recommended`, `--format json`, `--include-nested`, `--search`) is the primary discoverability entry point, but:

- It is classified `CatalogTier::Advanced` by `tier_for_path` in `crates/vox-cli/src/command_catalog.rs:333-347` because only 8 specific names are hard-coded as `Recommended`.
- `vox commands --recommended` does not include `commands` itself.
- There is no mention of `vox commands` in the `vox doctor` output or the first-run onboarding flow.

### Recommended fixes

1. Promote `commands` to `CatalogTier::Recommended` in `tier_for_path`:
   ```rust
   "build" | "check" | "run" | "test" | "bundle" | "dev" | "doctor" | "completions" | "commands"
   ```

2. Add a `vox doctor` hint: *"Run `vox commands --recommended` to see the getting-started command set."*

---

## Gap 6 — Tree-Sitter Grammar vs LSP Semantic Tokens: Duplication

### Current state

- `tree-sitter-vox/` contains a full `grammar.js`.
- `crates/vox-lsp/src/grammar.rs` implements a **separate** hand-written semantic token emitter that walks the compiler AST.
- The two are not synchronized: any new keyword or decorator requires updates in **four** places:
  1. `crates/vox-compiler/src/language_surface.rs`
  2. `crates/vox-lsp/src/grammar.rs`
  3. `tree-sitter-vox/grammar.js`
  4. `contracts/cli/command-registry.yaml` (if it becomes a CLI surface)

### Recommended fix

Generate `crates/vox-lsp/src/grammar.rs`'s keyword/decorator token patterns from `language_surface.rs` constants. Add a failing assertion in `vox ci grammar-drift` (already registered in `command-registry.yaml`) that compares the tree-sitter grammar's keyword list against `language_surface::BARE_KEYWORDS` and `language_surface::DECORATORS`.

---

## CLI ↔ GUI ↔ LSP Parity Matrix

| Feature | CLI | GUI | LSP | Gap |
|---------|-----|-----|-----|-----|
| Command catalog | ✅ `vox commands` | ✅ `get_command_catalog` | ❌ n/a | None (CLI→GUI path is sound) |
| Fuzzy command search | ✅ `--search` | ❌ no Tauri cmd | ❌ n/a | GUI needs `search_catalog` Tauri cmd |
| Shell completions | ✅ generated | ❌ no palette | ⚠️ LSP completions only | GUI needs command palette; CLI needs `--install` |
| LSP completions | n/a | n/a | ⚠️ flat/unfiltered | Needs context-aware completions |
| `vox check` diagnostics | ✅ full HIR | ❌ not surfaced | ✅ `validate_document_with_hir` | GUI should surface LSP diagnostics in editor |
| `vox doctor` health | ✅ full checks | ❌ not surfaced | ❌ not surfaced | Both surfaces need a health panel |
| Feature-gate awareness | ✅ `feature_gate` field | ⚠️ metadata only | ❌ none | GUI and LSP need user explanations |
| Argument detail | ✅ catalog JSON | ⚠️ data present, UI absent | ❌ n/a | GUI needs argument panel |
| LSP hover docs | n/a | n/a | ⚠️ 5 builtins only | LSP needs `language_surface.rs` SSOT expansion |
| `vox completions --install` | ❌ not implemented | ❌ n/a | n/a | Add `--install` flag + doctor probe |

---

## Recommended Implementation Order

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| **P0** | `vox completions --install` + `vox doctor` completion probe | Low | High |
| **P0** | Document completions in `docs/src/contributors/getting-started.md` | Low | High |
| **P1** | LSP context-aware completions (trigger char + body vs top-level context) | Medium | High |
| **P1** | `search_catalog` Tauri command + GUI command palette search | Medium | High |
| **P2** | Expand LSP hover to full stdlib + decorators via `language_surface.rs` | Medium | Medium |
| **P2** | GUI argument detail panel (data already in catalog JSON) | Low | Medium |
| **P2** | Promote `vox commands` to `CatalogTier::Recommended` | Trivial | Medium |
| **P3** | GUI feature-gate explanation panel | Low | Medium |
| **P3** | `vox doctor` integration in GUI health panel | Medium | Medium |
| **P3** | Align tree-sitter grammar with `language_surface.rs` via `vox ci grammar-drift` | Medium | Low (CI hygiene) |

---

## Where to Make Changes

| Change | File |
|--------|------|
| `--install` flag for completions | [`crates/vox-cli/src/lib.rs`](../../../crates/vox-cli/src/lib.rs), `crates/vox-cli/src/cli_dispatch/mod.rs` |
| Doctor completion probe | `crates/vox-cli/src/commands/diagnostics/` (new `completion_probe.rs`) |
| Context-aware LSP completions | [`crates/vox-lsp/src/completions.rs`](../../../crates/vox-lsp/src/completions.rs) |
| LSP hover SSOT | [`crates/vox-compiler/src/language_surface.rs`](../../../crates/vox-compiler/src/language_surface.rs) (add `LSP_BUILTIN_HOVER_DOCS`) |
| `search_catalog` Tauri command | [`crates/vox-gui/src/commands/catalog.rs`](../../../crates/vox-gui/src/commands/catalog.rs), [`crates/vox-gui/src/main.rs`](../../../crates/vox-gui/src/main.rs) |
| Promote `commands` tier | [`crates/vox-cli/src/command_catalog.rs`](../../../crates/vox-cli/src/command_catalog.rs) `tier_for_path` |
| Grammar drift CI | `crates/vox-cli/src/commands/ci/grammar_drift.rs` (already registered in `command-registry.yaml`) |

---

*See also:* [vox-lsp-capabilities-ssot-2026.md](./vox-lsp-capabilities-ssot-2026.md), [vox-compiler-architecture-research-2026.md](./vox-compiler-architecture-research-2026.md), [cli-command-surface.generated.md](../reference/cli-command-surface.generated.md).
