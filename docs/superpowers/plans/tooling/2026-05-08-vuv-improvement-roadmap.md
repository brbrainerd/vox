# VUV Improvement Roadmap (VUV-9 → VUV-15) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the VUV authoring-layer improvements derived from the [Gradio & Streamlit research (2026)](../../../src/architecture/gradio-streamlit-research-2026.md) — naming-stability tooling, accessibility, layout vocabulary, runtime testability, a stdlib chat primitive, per-session state, and cache discipline — across seven shippable phases.

**Architecture:** Each phase is **independently shippable**, lands behind a flag where useful, and ends in a green test suite — same discipline as VUV-1–8. Phase VUV-9 ships the substrate (deprecation cycle + codemod) the others depend on. Detailed TDD steps in this document cover **VUV-9 only**; phases VUV-10 through VUV-15 each get a one-paragraph scope summary and a **follow-up plan trigger** (write a fresh plan when the prior phase lands and the next is unblocked). Open questions from the research doc are resolved with concrete design decisions in [§Open-question resolutions](#open-question-resolutions) so future phases have a fixed target.

**Tech Stack:** Rust (workspace crates under `crates/`), Vox (`.vox` source under `examples/golden/`, `crates/vox-dashboard/`), JSON contracts under `contracts/`, mdBook for docs.

---

## Status of prior VUV work (2026-05-08)

Per [gui-authoring-syntax-2026.md §Implementation status](../../../src/architecture/gui-authoring-syntax-2026.md#implementation-status-2026-05-08):

| Phase | Status |
|---|---|
| VUV-1 Token vocabulary | ✅ Done |
| VUV-2 Trailing-block parser | ✅ Done |
| VUV-3 Lowering trailing-block → DomNode | ✅ Done |
| VUV-4 Typed style kwargs | ✅ Done |
| VUV-5 Typed event handler kwargs | ✅ Done |
| VUV-6 Dashboard cutover (JSX retired) | ✅ Done |
| VUV-7 Golden corpus + MENS | 🟡 Partial — MENS retrain pending operator |
| VUV-8 Doc sweep | ✅ Done |

This plan picks up at **VUV-9** and runs to **VUV-15**.

---

## Open-question resolutions

Resolutions for the seven questions in [gradio-streamlit-research-2026.md §7](../../../src/architecture/gradio-streamlit-research-2026.md#7-open-questions-and-follow-ups). Each resolution becomes a fixed target for downstream phases.

### Q1 — Streamlit-magic prototype mode (auto-render bare expressions)

**Resolution: REJECT.** No `vox demo` mode where a bare expression renders to a default primitive. The Streamlit failure mode (top-level expressions are non-locally typed; semantics depend on whether the file is `app.py` or a helper module) is exactly the kind of context-dependent behavior LLMs struggle to maintain. VUV's `view: …` named binding stays explicit; bare expressions in component body remain ordinary Vox values.

**Why:** the cost is paying for explicit `view:` everywhere. The benefit is that an LLM reading any `.vox` file can tell, from local context, whether a value renders or not.

**Closes:** [gui-authoring-syntax-2026.md Open Question 1](../../../src/architecture/gui-authoring-syntax-2026.md#open-questions) ("bare string in child position — desugar?") was already resolved as "require explicit `text(…)`"; this is the same principle one level up.

### Q2 — Stdlib `ChatInterface`-equivalent

**Resolution: SHIP as VUV-13.** Lives in a new crate `crates/vox-ui-stdlib/` (created in VUV-13). API:

```vox
// vox:skip
chat_panel(
    messages: list[Message],
    on_send: fn(str) -> Action,
    streaming: bool = false,
    placeholder: str = "Type a message…",
    submit_label: str = "Send",
) {
    // optional children become an additional-inputs accordion (à la gr.ChatInterface)
}
```

Lowers to a composition of existing VUV primitives (`column`, `scroll`, `row`, `text_input`, `button`, `chat_message`) — no new render machinery. Streaming token support uses Vox's existing reactive state; no new runtime concept.

**Why:** the chat shape is universal in 2023–2026 LLM apps and `gr.ChatInterface` is the canonical "right abstraction at the right time" win from the research doc. Shipping it as a stdlib composition (not a primitive) means it's transparently overridable and stays consistent with Vox's "primitives compose; libraries don't introduce new render rules" discipline.

**Lock-in cost:** low — it's a Vox library component. If users want a custom shape they write `column { … }` directly with the underlying primitives.

### Q3 — Vox equivalent of Gradio's `share=True`

**Resolution: OUT OF VUV SCOPE.** Deploy-side concern, not authoring-layer. Tracked in the deploy roadmap (not this plan). The design space we want to keep open: `vox deploy --share` produces a public URL via a tunneling proxy (FRP-style) without standing up infrastructure. For now, `vox deploy` to Coolify is the supported path.

**Why:** mixing deploy concerns into VUV would couple authoring evolution to ops infrastructure. Keep the layers separate.

### Q4 — Per-session state primitive (`gr.State` analogue)

**Resolution: SHIP as VUV-14, design fixed below.**

Vox today has **component-local reactive state** (lowered to React hooks via `codegen_ts/reactive.rs`). It has no first-class "state that survives across components but not across sessions" — the Gradio `gr.State` shape and the Streamlit `st.session_state` shape.

**Decided shape:**

```vox
// vox:skip
@session
let cart: Cart = Cart.empty()

@session(scope: tab)        // tab-scoped (default; survives reload, dies on tab close)
let draft: Draft = Draft.empty()

@session(scope: window)     // window-scoped (survives navigation within tab)
let theme: Theme = Theme.dark()
```

- **Storage:** server-side in-memory map keyed by `session_id` (a cookie set on first request). The session-id cookie is HttpOnly + SameSite=Lax + Secure-when-HTTPS. Default eviction: 30 minutes idle.
- **Wire:** session values hydrate over the existing WebSocket on connect; writes propagate via the existing reactive channel. No new protocol; the wire-format SSOT ([Phase 2 of the interop plan](../../../src/architecture/external-frontend-interop-plan-2026.md)) handles serialization.
- **Type safety:** `@session let foo: T = expr` — `foo` is typed `T`, not `Any`. There is no string-keyed `session_state["foo"]` form (this is the Streamlit failure mode we are explicitly rejecting).
- **No global mutable state by convention:** for cross-session sharing, users go through `@table` (Vox's existing DB primitive). Same convention as Gradio.
- **Pluggable backend:** the in-memory store is the default; users can swap to Redis via `Vox.toml [session] backend = "redis"` without touching `.vox` source.

**Why this and not `st.session_state`-style:** typed binding from the source-of-declaration (the `@session let` line) to the use site eliminates the typo class of bug that dominates Streamlit maintenance pain. Same lesson as the rejection of positional input/output binding in Gradio.

**Why not just-use-reactive-state:** reactive state is per-component-instance. A user navigating between two pages of the same app needs cross-component state that survives unmount. That's a category gap.

### Q5 — Cache discipline (Streamlit-style intent split)

**Resolution: PARTIAL ADOPT as VUV-15.**

Streamlit's lesson is "do not unify cross-intent caches in one decorator" (the `@st.cache` → `@st.cache_data` / `@st.cache_resource` split). Vox's situation is structurally different: there is no whole-script rerun, so most of Streamlit's caching pressure does not apply. Per-component memoization is already handled by Vox's reactive `derived(deps)` story.

**What we ship in VUV-15:**

```vox
// vox:skip
@memo
fn expensive_pure(input: Input) -> Output { … }   // pure computation, returns a fresh value each call site

@resource
fn open_db() -> Db { … }                          // singleton-per-process; returns by reference
```

- `@memo` — keyed by argument hash + source-content hash; deep-immutable return; safe to call from any context. Equivalent of `@st.cache_data`.
- `@resource` — process-singleton; returns by reference; lifetime tied to process. Equivalent of `@st.cache_resource`.
- Closure capture of outer state is a **type error** at definition time on `@memo` (closures over outer state aren't part of the cache key — this is the Streamlit footgun we are explicitly rejecting).

**What we do NOT ship:** a generic `@cache` that tries to be both. The whole point of the split is the intent declaration.

**Why it is small:** Vox doesn't have Streamlit's rerun pressure. `@memo` is cheap to add; `@resource` is largely a singleton macro. The full design is one phase.

### Q6 — Migration corpus discipline (rename policy)

**Resolution: SUBSUMED BY VUV-9.** This phase ships the policy + tooling; every later phase uses it. Concrete shape in [§Phase VUV-9](#vuv-9--stable-naming-policy--codemod-tooling).

### Q7 (additional) — Where does the chat primitive live?

**Resolution:** new crate `crates/vox-ui-stdlib/` with its own `Cargo.toml`, lowered as a normal Vox library. The crate `vox-primitives/` is reserved for non-UI primitives (id, backoff). Don't conflate.

---

## Roadmap (phasing table)

Same shape as the [VUV-1-8 phasing table](../../../src/architecture/gui-authoring-syntax-2026.md#implementation-phasing). Each phase: **independently shippable, behind a flag where useful, ends in a green test suite.**

| Phase | Work | Surfaces touched | Approx. size | Gate |
|---|---|---|---|---|
| **VUV-9** Stable-naming policy + codemod tooling | Naming policy doc; rename registry contract; alias resolution + deprecation warnings in parser; `vox migrate` codemod. **No flag** — additive throughout. | `docs/src/architecture/`, `contracts/naming/`, `crates/vox-compiler/src/parser/`, `crates/vox-cli/src/commands/`, tests | medium | Old name parses with warning; `vox migrate` rewrites `.vox` corpus byte-equivalent |
| **VUV-10** First-class accessibility kwargs | Typed `aria_label`, `role`, `tab_index`, `aria_describedby`, `aria_live` as universal kwargs. Validator: clickable primitives (`button`, `link`) require label or `aria_label`. Default-on labels for icon-only buttons emit a warning. | `crates/vox-compiler/src/lowering_shared/`, validators, dashboard fixtures | medium | Dashboard a11y warnings = 0; lighthouse-axe golden test green |
| **VUV-11** Layout vocabulary expansion | Responsive variants on existing kwargs (`pad_md: 8`, `gap_lg: 4`, `cols_md: 2`). New `grid()` primitive with typed `cols`, `rows`, `gap`. New `stack()` and `cluster()` primitives (Every-Layout-style). Breakpoint registry in `tokens.v1.json`. | tokens contract, `lowering_shared/primitive_tags.rs`, dashboard fixtures, tests | large | Dashboard rebuilt on new vocabulary; visual diff = 0; `cluster`/`stack` golden tests green |
| **VUV-12** First-class view testability | New `vox test view <name> --props k=v` subcommand renders a view to a deterministic Web-IR snapshot. Property-based tests for layout invariants (no overflow, all primitives validated). | `crates/vox-cli/src/commands/test.rs`, new `crates/vox-test-harness/view_test.rs` module, tests | medium | Sample dashboard view renders deterministic snapshot; flake rate = 0 across 100 runs |
| **VUV-13** Stdlib `chat_panel` + companions | New crate `crates/vox-ui-stdlib/` with `chat_panel`, `chat_message`, `streaming_token` components composed from existing primitives. Auto-imported in `Vox.toml` projects with `target = "fullstack"`. | new crate, `Vox.toml` schema, `lowering_shared/`, integration tests | medium | Reference chat app is < 30 lines of `.vox` source; streaming demo renders tokens incrementally |
| **VUV-14** `@session` decorator + per-session store | Parser entry for `@session let …`; HIR carries session declarations; codegen emits a server-side store hookup + client-side hydration; in-memory backend default; Redis backend behind feature flag. | parser, AST, HIR, `codegen_rust/`, `codegen_ts/`, tests | large | Cart-survives-refresh integration test green; Redis backend smoke test green |
| **VUV-15** `@memo` and `@resource` decorators | Two new decorators on `fn`; `@memo` keys by argument-hash + source-content-hash, deep-immutable return; `@resource` is process-singleton; closure-capture is a type error on `@memo`. | parser, AST, HIR, `codegen_rust/`, tests | medium | Memoized function fixture: second call cache-hit; closure-capture test fails with helpful error |

**Atomicity:** VUV-9 ships first and unlocks everyone else; VUV-10 / VUV-11 / VUV-13 are pairwise independent and can land in any order; VUV-12 depends on VUV-11 (tests need the expanded vocabulary); VUV-14 / VUV-15 are independent, can ship after VUV-13.

**Each later phase will get its own dedicated plan when its predecessor lands.** This document holds the meta-roadmap and the detailed plan for **VUV-9 only**.

---

## Files to be created / modified across all phases

For VUV-9 (this plan's TDD-detailed phase). Other phases list paths in their scope summaries.

**Create (VUV-9):**
- `docs/src/architecture/vuv-naming-policy-2026.md` — policy doc; the ground rules.
- `contracts/naming/renames.v1.json` — rename registry. Single source of truth.
- `crates/vox-compiler/src/parser/renames.rs` — alias loader + resolution.
- `crates/vox-cli/src/commands/migrate.rs` — `vox migrate` subcommand.
- `crates/vox-compiler/tests/rename_alias_test.rs` — alias resolution + warning emission.
- `crates/vox-cli/tests/migrate_codemod_test.rs` — `vox migrate` codemod over a fixture.

**Modify (VUV-9):**
- `crates/vox-compiler/src/parser/mod.rs` — wire `renames::resolve` into the identifier-resolution path.
- `crates/vox-cli/src/commands/mod.rs` — register `migrate` subcommand.
- `crates/vox-cli/src/main.rs` (or wherever the clap derive lives) — add `Migrate` arm.
- `docs/src/SUMMARY.md` — regenerated by `vox-doc-pipeline`, not hand-edited.

---

## VUV-9 — Stable-naming policy + codemod tooling

**Goal:** Every primitive name, kwarg name, and decorator name in Vox follows a deprecation cycle: announce → alias for one major version → remove. A `vox migrate` subcommand rewrites a `.vox` corpus from old to new names automatically. The aliased name continues to parse and emits a one-line deprecation warning. This is the substrate every later VUV phase depends on.

**The lesson from the research doc:** Gradio's 3→4 break, Gradio's 4→5 component renames, and Streamlit's `cache` → `cache_data`/`cache_resource` rename all demonstrated that uncontrolled churn poisons the LLM training corpus and breaks user code. Vox is a corpus-aware language; we cannot afford the same.

### Task 1: Write the naming policy doc

**Files:**
- Create: `docs/src/architecture/vuv-naming-policy-2026.md`

- [ ] **Step 1: Write the policy doc**

```markdown
---
title: "VUV Naming Policy (2026)"
description: "Deprecation cycle for primitive names, kwarg names, and decorator names. Every rename is announced, aliased for one major version, then removed. A rename registry tracks every alias; the `vox migrate` codemod rewrites old names to new ones."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Canonical reference for how Vox renames evolve. Cited from VUV phase plans."
---

# VUV Naming Policy (2026)

**The rule:** Every public Vox identifier — primitive name, kwarg name, decorator
name, type name, decorator-argument enum value — follows a three-step lifecycle:

1. **Announce** in a release note: "X has been renamed to Y."
2. **Alias** X to Y in the rename registry. Both names parse. Using X emits a
   one-line deprecation warning at compile time.
3. **Remove** X in the next major version. The registry retains the entry with
   `removed_in: "1.X"` for tooling and historical reference.

**The codemod:** `vox migrate` reads the rename registry and rewrites every
occurrence of an old name in a `.vox` corpus to its new name. The codemod is
**byte-equivalent**: re-running on a migrated corpus produces no diff.

**The registry:** `contracts/naming/renames.v1.json`. Single source of truth.
Every rename has `from`, `to`, `kind` (one of `primitive`, `kwarg`, `decorator`,
`enum_value`, `type`), `since` (version where the alias was introduced), and
optional `removed_in` (version where the alias becomes a hard error).

**No silent renames.** A change to a public name without a registry entry is
a CI failure. Enforcement: see `crates/vox-arch-check`.

**Why:** the dominant LLM-author failure mode in Gradio and Streamlit is that
old training corpora contain dead names. A model trained on Gradio 4.x cheerfully
emits `concurrency_count`, `Interface.load()`, `.style()` — all dead. Vox is
itself a corpus-aware language: MENS retrains on `examples/golden/` and the
dashboard. We cannot afford uncontrolled churn.

**See also:** [Gradio & Streamlit Research (2026)](gradio-streamlit-research-2026.md)
for the historical evidence; [GUI Authoring Syntax (2026)](gui-authoring-syntax-2026.md)
for the current VUV phase plan.
```

- [ ] **Step 2: Verify the doc renders cleanly through the doc pipeline**

Run: `cargo run -p vox-doc-pipeline 2>&1 | tail -5`
Expected: `Successfully generated SUMMARY.md with all pages.`

- [ ] **Step 3: Commit**

```bash
git add docs/src/architecture/vuv-naming-policy-2026.md docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml
git commit -m "docs(vuv): add naming policy doc (VUV-9 task 1)"
```

### Task 2: Define the rename registry contract

**Files:**
- Create: `contracts/naming/renames.v1.json`
- Test: `crates/vox-compiler/tests/rename_alias_test.rs` (just the schema-load test in this task)

- [ ] **Step 1: Write the failing test for registry load**

```rust
// crates/vox-compiler/tests/rename_alias_test.rs
use vox_compiler::parser::renames::RenameRegistry;

#[test]
fn registry_loads_from_canonical_path() {
    let registry = RenameRegistry::load_canonical()
        .expect("should load contracts/naming/renames.v1.json");
    assert!(registry.entries().len() >= 0); // empty registry is valid
}

#[test]
fn registry_rejects_duplicate_from_keys() {
    let json = r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" },
          { "from": "Box", "to": "container", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    let result = RenameRegistry::from_str(json);
    assert!(result.is_err(), "duplicate `from` keys must be rejected");
}

#[test]
fn registry_rejects_alias_chain() {
    // `from` cannot itself be a `to` of another entry — no chains, only direct mappings.
    let json = r#"{
        "version": 1,
        "entries": [
          { "from": "Box",       "to": "container", "kind": "primitive", "since": "0.5.0" },
          { "from": "container", "to": "panel",     "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    let result = RenameRegistry::from_str(json);
    assert!(result.is_err(), "alias chains must be rejected");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-compiler --test rename_alias_test`
Expected: FAIL — `RenameRegistry` not found in `parser::renames`.

- [ ] **Step 3: Write the empty registry contract**

```json
{
  "$schema": "./renames.v1.schema.json",
  "version": 1,
  "comment": "Rename registry for Vox public identifiers. See docs/src/architecture/vuv-naming-policy-2026.md.",
  "entries": []
}
```

Save as `contracts/naming/renames.v1.json`. The empty entries array is intentional — VUV-9 ships the substrate; entries are added by future phases.

- [ ] **Step 4: Commit the contract**

```bash
git add contracts/naming/renames.v1.json
git commit -m "feat(contracts): add empty rename registry (VUV-9 task 2)"
```

### Task 3: Implement RenameRegistry

**Files:**
- Create: `crates/vox-compiler/src/parser/renames.rs`
- Modify: `crates/vox-compiler/src/parser/mod.rs` — export the new module

- [ ] **Step 1: Write the registry struct + loader**

```rust
// crates/vox-compiler/src/parser/renames.rs
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenameKind {
    Primitive,
    Kwarg,
    Decorator,
    EnumValue,
    Type,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameEntry {
    pub from: String,
    pub to: String,
    pub kind: RenameKind,
    pub since: String,
    #[serde(default)]
    pub removed_in: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenameRegistryFile {
    version: u32,
    #[serde(default)]
    entries: Vec<RenameEntry>,
}

#[derive(Debug, Clone)]
pub struct RenameRegistry {
    by_from: HashMap<String, RenameEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("registry version {0} is not supported (expected 1)")]
    UnsupportedVersion(u32),
    #[error("duplicate `from` key: {0}")]
    DuplicateFrom(String),
    #[error("alias chain: `from` {0} is also a `to` in another entry")]
    AliasChain(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] serde_json::Error),
}

impl RenameRegistry {
    pub fn from_str(json: &str) -> Result<Self, RegistryError> {
        let file: RenameRegistryFile = serde_json::from_str(json)?;
        if file.version != 1 {
            return Err(RegistryError::UnsupportedVersion(file.version));
        }
        let mut by_from: HashMap<String, RenameEntry> = HashMap::new();
        let to_set: std::collections::HashSet<&String> =
            file.entries.iter().map(|e| &e.to).collect();
        for entry in &file.entries {
            if by_from.contains_key(&entry.from) {
                return Err(RegistryError::DuplicateFrom(entry.from.clone()));
            }
            if to_set.contains(&entry.from) {
                return Err(RegistryError::AliasChain(entry.from.clone()));
            }
            by_from.insert(entry.from.clone(), entry.clone());
        }
        Ok(Self { by_from })
    }

    pub fn load_canonical() -> Result<Self, RegistryError> {
        let path = canonical_path();
        let bytes = std::fs::read_to_string(&path)?;
        Self::from_str(&bytes)
    }

    pub fn entries(&self) -> impl Iterator<Item = &RenameEntry> {
        self.by_from.values()
    }

    /// Resolve an old name to its canonical replacement. Returns `None` if the name
    /// is canonical (no rename applies).
    pub fn resolve(&self, name: &str) -> Option<&RenameEntry> {
        self.by_from.get(name)
    }
}

fn canonical_path() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR to find the workspace root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest
        .ancestors()
        .find(|p| p.join("contracts/naming/renames.v1.json").exists())
        .expect("workspace root with contracts/naming/renames.v1.json must be findable");
    workspace_root.join("contracts/naming/renames.v1.json")
}
```

- [ ] **Step 2: Wire the module into parser/mod.rs**

```rust
// crates/vox-compiler/src/parser/mod.rs — add at top with the other pub mod lines
pub mod renames;
```

- [ ] **Step 3: Run the registry tests**

Run: `cargo test -p vox-compiler --test rename_alias_test`
Expected: all three tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-compiler/src/parser/renames.rs crates/vox-compiler/src/parser/mod.rs crates/vox-compiler/tests/rename_alias_test.rs
git commit -m "feat(parser): RenameRegistry loader + validation (VUV-9 task 3)"
```

### Task 4: Wire alias resolution into the parser; emit deprecation warnings

This task makes old names parse to new names with a warning. The exact integration point depends on the existing parser's identifier-resolution path. The general shape:

**Files:**
- Modify: `crates/vox-compiler/src/parser/descent/expr/pratt_match.rs` (or wherever primitive-call parsing currently lives — see [VUV-2 reference](../../../src/architecture/gui-authoring-syntax-2026.md#implementation-status-2026-05-08))
- Modify: parser-error/diagnostic surface (likely `crates/vox-compiler/src/parser/error.rs`)
- Test: extend `crates/vox-compiler/tests/rename_alias_test.rs`

- [ ] **Step 1: Write the failing test for alias resolution**

Add to `crates/vox-compiler/tests/rename_alias_test.rs`:

```rust
use vox_compiler::parser;

/// When the registry maps "Box" -> "panel" and source uses "Box", the parser
/// resolves to "panel" and emits a deprecation warning citing the registry entry.
#[test]
fn deprecated_primitive_resolves_with_warning() {
    let registry_json = r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    let registry = parser::renames::RenameRegistry::from_str(registry_json).unwrap();

    let source = "component App() { view: Box() { } }";
    let result = parser::parse_with_registry(source, &registry)
        .expect("source should parse");

    // The resolved primitive name should be "panel"
    assert!(result.uses_primitive("panel"));
    assert!(!result.uses_primitive("Box"));

    // Exactly one warning, citing the rename
    let warnings = result.warnings();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("Box"));
    assert!(warnings[0].message.contains("panel"));
    assert!(warnings[0].message.contains("0.5.0"));
}
```

The exact APIs (`parse_with_registry`, `uses_primitive`, `warnings`) may need to be added to the parser surface. If the parser's existing entry point already returns warnings, extend that; if not, the simplest approach is a thin wrapper module.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-compiler --test rename_alias_test deprecated_primitive_resolves_with_warning`
Expected: FAIL — `parse_with_registry` not defined.

- [ ] **Step 3: Implement alias resolution in the primitive-call path**

In `pratt_match.rs` (or wherever the identifier check for primitives happens), wrap the existing primitive lookup:

```rust
// pseudocode — adapt to existing parser shape
fn resolve_primitive_name(name: &str, registry: &RenameRegistry, span: Span, diagnostics: &mut Diagnostics) -> &str {
    if let Some(entry) = registry.resolve(name) {
        if matches!(entry.kind, RenameKind::Primitive) {
            diagnostics.warn(span, format!(
                "primitive `{}` was renamed to `{}` in {}; use the new name (run `vox migrate` to update)",
                entry.from, entry.to, entry.since
            ));
            return &entry.to;
        }
    }
    name
}
```

The warning text format is fixed by this task and used by every later phase.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-compiler --test rename_alias_test deprecated_primitive_resolves_with_warning`
Expected: PASS.

- [ ] **Step 5: Verify nothing else regressed**

Run: `cargo test -p vox-compiler`
Expected: all tests pass. If a snapshot test changes because warnings now show up, regenerate snapshots **only after** verifying the warning is intended (insert a `.snapshot()` review step manually).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-compiler/src/parser/ crates/vox-compiler/tests/rename_alias_test.rs
git commit -m "feat(parser): resolve renamed primitives with deprecation warning (VUV-9 task 4)"
```

### Task 5: Add `vox migrate` subcommand skeleton

**Files:**
- Create: `crates/vox-cli/src/commands/migrate.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs` (or wherever subcommands are registered)
- Modify: clap derive entrypoint to add the `Migrate` variant
- Test: `crates/vox-cli/tests/migrate_codemod_test.rs`

- [ ] **Step 1: Write the failing CLI smoke test**

```rust
// crates/vox-cli/tests/migrate_codemod_test.rs
use std::process::Command;

#[test]
fn migrate_help_lists_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["migrate", "--help"])
        .output()
        .expect("vox binary should be runnable");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("migrate"));
    assert!(stdout.contains("rewrite a .vox corpus"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --test migrate_codemod_test migrate_help_lists_subcommand`
Expected: FAIL — `migrate` is not a known subcommand.

- [ ] **Step 3: Add the subcommand**

```rust
// crates/vox-cli/src/commands/migrate.rs
use clap::Args;
use std::path::PathBuf;

/// Rewrite a .vox corpus to the canonical names from contracts/naming/renames.v1.json.
#[derive(Args, Debug)]
pub struct MigrateArgs {
    /// Root directory of .vox sources to rewrite. Defaults to the current working directory.
    #[arg(default_value = ".")]
    pub root: PathBuf,

    /// Print what would change without writing.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: MigrateArgs) -> anyhow::Result<()> {
    let registry = vox_compiler::parser::renames::RenameRegistry::load_canonical()?;
    let files = collect_vox_files(&args.root)?;
    let mut total = 0usize;
    for path in &files {
        let before = std::fs::read_to_string(path)?;
        let after = rewrite(&before, &registry);
        if before != after {
            total += 1;
            if !args.dry_run {
                std::fs::write(path, after)?;
            }
            println!("{}: {}", if args.dry_run { "would update" } else { "updated" }, path.display());
        }
    }
    println!("{} file(s) {}", total, if args.dry_run { "would be updated" } else { "updated" });
    Ok(())
}

fn collect_vox_files(root: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(root, &mut out)?;
    Ok(out)
}

fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() { return Ok(()); }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip target/, node_modules/, .git/
            let name = path.file_name().unwrap_or_default();
            if name == "target" || name == "node_modules" || name == ".git" {
                continue;
            }
            walk(&path, out)?;
        } else if path.extension().map_or(false, |e| e == "vox") {
            out.push(path);
        }
    }
    Ok(())
}

fn rewrite(source: &str, registry: &vox_compiler::parser::renames::RenameRegistry) -> String {
    // Implemented in Task 6
    let _ = (source, registry);
    source.to_string()
}
```

- [ ] **Step 4: Register the subcommand**

In `crates/vox-cli/src/commands/mod.rs` (or main.rs — wherever clap subcommands are wired), add:

```rust
pub mod migrate;
// In the Command enum / dispatch match, add:
//   Command::Migrate(args) => commands::migrate::run(args),
```

- [ ] **Step 5: Run the smoke test to verify it passes**

Run: `cargo test -p vox-cli --test migrate_codemod_test migrate_help_lists_subcommand`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/migrate.rs crates/vox-cli/src/commands/mod.rs crates/vox-cli/src/main.rs crates/vox-cli/tests/migrate_codemod_test.rs
git commit -m "feat(cli): add vox migrate subcommand skeleton (VUV-9 task 5)"
```

### Task 6: Implement the codemod rewrite

**Files:**
- Modify: `crates/vox-cli/src/commands/migrate.rs` (the `rewrite` function)
- Test: extend `crates/vox-cli/tests/migrate_codemod_test.rs`

- [ ] **Step 1: Write the failing rewrite test**

Add to `crates/vox-cli/tests/migrate_codemod_test.rs`:

```rust
use vox_compiler::parser::renames::RenameRegistry;

#[test]
fn rewrite_renames_primitive_call_sites() {
    let registry = RenameRegistry::from_str(r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#).unwrap();

    let before = "component App() { view: Box() { Box() { text(\"hi\") } } }";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);

    assert_eq!(after, "component App() { view: panel() { panel() { text(\"hi\") } } }");
}

#[test]
fn rewrite_does_not_touch_string_literals() {
    let registry = RenameRegistry::from_str(r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#).unwrap();

    let before = "component App() { view: text(\"Box of crayons\") }";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);

    assert_eq!(after, before, "string literal contents must be preserved");
}

#[test]
fn rewrite_does_not_touch_unrelated_identifiers() {
    let registry = RenameRegistry::from_str(r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#).unwrap();

    let before = "let MyBox = 1; let Boxes = 2;";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);

    assert_eq!(after, before, "substring matches must not be rewritten");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli --test migrate_codemod_test rewrite_`
Expected: FAIL — `rewrite_for_test` not defined or returns the input unchanged.

- [ ] **Step 3: Implement token-based rewrite**

The codemod must use the existing Vox lexer to identify identifier tokens — not regex — so that string contents and substrings are not touched.

```rust
// in crates/vox-cli/src/commands/migrate.rs

pub fn rewrite_for_test(
    source: &str,
    registry: &vox_compiler::parser::renames::RenameRegistry,
) -> String {
    rewrite(source, registry)
}

fn rewrite(
    source: &str,
    registry: &vox_compiler::parser::renames::RenameRegistry,
) -> String {
    use vox_compiler::lexer::{tokenize, TokenKind};

    let tokens = match tokenize(source) {
        Ok(t) => t,
        Err(_) => {
            // If the source doesn't lex, leave it alone — the caller will see the
            // compile error on the next build. Don't half-rewrite broken code.
            return source.to_string();
        }
    };

    let mut out = String::with_capacity(source.len());
    let mut cursor = 0;
    for token in &tokens {
        // Copy any whitespace/comments between the previous emit and this token's start.
        out.push_str(&source[cursor..token.span.start]);
        match token.kind {
            TokenKind::Ident => {
                let name = &source[token.span.start..token.span.end];
                if let Some(entry) = registry.resolve(name) {
                    out.push_str(&entry.to);
                } else {
                    out.push_str(name);
                }
            }
            _ => {
                // Verbatim copy the token (string literals, numbers, punctuation).
                out.push_str(&source[token.span.start..token.span.end]);
            }
        }
        cursor = token.span.end;
    }
    // Trailing content after the last token.
    out.push_str(&source[cursor..]);
    out
}
```

The exact lexer API (`tokenize`, `TokenKind::Ident`, `Span` with `start`/`end`) may differ — adjust to whatever the existing `crates/vox-compiler/src/lexer/` exposes. The principle is: **operate on tokens, not text patterns.**

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli --test migrate_codemod_test rewrite_`
Expected: PASS for all three.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/migrate.rs crates/vox-cli/tests/migrate_codemod_test.rs
git commit -m "feat(cli): implement token-based codemod rewrite (VUV-9 task 6)"
```

### Task 7: End-to-end smoke test on a fixture corpus

**Files:**
- Create: `crates/vox-cli/tests/fixtures/migrate/before/sample.vox`
- Create: `crates/vox-cli/tests/fixtures/migrate/after/sample.vox`
- Test: extend `crates/vox-cli/tests/migrate_codemod_test.rs`

- [ ] **Step 1: Write fixture files**

`crates/vox-cli/tests/fixtures/migrate/before/sample.vox`:

```vox
// vox:skip
component Greeting(name: str) {
    view: Box() {
        text("Hello, " + name)
    }
}
```

`crates/vox-cli/tests/fixtures/migrate/after/sample.vox`:

```vox
// vox:skip
component Greeting(name: str) {
    view: panel() {
        text("Hello, " + name)
    }
}
```

(`Box` → `panel` is the rename used in test fixtures; in production the registry stays empty until a real rename is added.)

- [ ] **Step 2: Write the failing end-to-end test**

```rust
#[test]
fn migrate_dry_run_reports_diff_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("sample.vox");
    std::fs::write(&src, std::fs::read("crates/vox-cli/tests/fixtures/migrate/before/sample.vox").unwrap()).unwrap();

    // Use a custom registry path — see helper below.
    let output = run_migrate(&["--dry-run", temp.path().to_str().unwrap()]);

    let after_dry_run = std::fs::read_to_string(&src).unwrap();
    let original = std::fs::read_to_string("crates/vox-cli/tests/fixtures/migrate/before/sample.vox").unwrap();
    assert_eq!(after_dry_run, original, "dry run must not write");
    assert!(output.stdout_contains("would update"));
}

#[test]
fn migrate_writes_canonical_output() {
    // ... same setup, without --dry-run, and assert the file now equals after/sample.vox byte-for-byte.
}

// Helper: spawn the binary with VOX_RENAMES_PATH pointing at a test registry.
struct CliOutput { stdout: String, stderr: String, status: std::process::ExitStatus }
impl CliOutput {
    fn stdout_contains(&self, s: &str) -> bool { self.stdout.contains(s) }
}

fn run_migrate(args: &[&str]) -> CliOutput {
    // Write a temp registry with the Box->panel rename used by the fixture.
    let registry_dir = tempfile::tempdir().unwrap();
    let registry_path = registry_dir.path().join("renames.v1.json");
    std::fs::write(&registry_path, r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_vox"))
        .arg("migrate")
        .args(args)
        .env("VOX_RENAMES_PATH", &registry_path)
        .output()
        .expect("vox binary should be runnable");

    CliOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        status: output.status,
    }
}
```

The test needs the migrate command to honor a `VOX_RENAMES_PATH` env var (override of the canonical path) so test-only renames don't pollute the production registry. Add this in the same task.

- [ ] **Step 3: Add env-var override to RenameRegistry**

In `crates/vox-compiler/src/parser/renames.rs`, change `canonical_path()`:

```rust
fn canonical_path() -> PathBuf {
    if let Ok(custom) = std::env::var("VOX_RENAMES_PATH") {
        return PathBuf::from(custom);
    }
    // ... existing workspace-walk logic
}
```

- [ ] **Step 4: Run end-to-end tests to verify they pass**

Run: `cargo test -p vox-cli --test migrate_codemod_test migrate_`
Expected: PASS for both `migrate_dry_run_reports_diff_without_writing` and `migrate_writes_canonical_output`.

- [ ] **Step 5: Verify the production corpus is untouched**

Run: `cargo run -p vox-cli -- migrate --dry-run examples/golden`
Expected: `0 file(s) would be updated` (registry is empty in production).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/tests/fixtures/ crates/vox-cli/tests/migrate_codemod_test.rs crates/vox-compiler/src/parser/renames.rs
git commit -m "test(cli): end-to-end migrate fixture + VOX_RENAMES_PATH override (VUV-9 task 7)"
```

### Task 8: Wire `vox-arch-check` to fail CI on unregistered renames

This is the enforcement mechanism. Without it, the policy is voluntary.

**Files:**
- Modify: `crates/vox-arch-check/src/lib.rs` (or wherever the rules live)
- Test: extend the arch-check test suite

- [ ] **Step 1: Sketch the rule**

The arch-check rule, in plain English: "every `pub` identifier exposed from a primitive-defining or kwarg-defining surface must either (a) be unchanged from the previous published version, or (b) appear in `contracts/naming/renames.v1.json` as a `to` value with a matching `since`."

Implementing this fully requires comparing against a previous version — heavyweight. **Ship a lighter check now:** verify that every `from` in the registry is no longer a defined primitive name. (If `Box` is in the registry, the lexer/parser must not still recognize `Box` as a current canonical primitive name.)

- [ ] **Step 2: Write the failing arch-check test**

```rust
// crates/vox-arch-check/tests/rename_consistency_test.rs
#[test]
fn registry_from_names_are_not_canonical() {
    let registry = vox_compiler::parser::renames::RenameRegistry::load_canonical()
        .expect("load registry");
    let canonical_primitives: std::collections::HashSet<String> =
        vox_compiler::lowering_shared::primitive_tags::all_primitives()
            .iter()
            .map(|s| s.to_string())
            .collect();
    for entry in registry.entries() {
        if matches!(entry.kind, vox_compiler::parser::renames::RenameKind::Primitive) {
            assert!(!canonical_primitives.contains(&entry.from),
                "{} is in the rename registry but still a canonical primitive",
                entry.from);
        }
    }
}
```

The exact symbol to inspect (`primitive_tags::all_primitives()`) may need to be added if it doesn't exist — most primitive-defining modules already have a `pub const PRIMITIVES: &[&str]` or similar. If not, add a small accessor in `lowering_shared/primitive_tags.rs`.

- [ ] **Step 3: Run the test (passes trivially with empty registry)**

Run: `cargo test -p vox-arch-check rename_consistency_test`
Expected: PASS (registry is empty, so the for-loop body never runs).

- [ ] **Step 4: Verify the test catches a violation**

Temporarily add a fake entry to `contracts/naming/renames.v1.json`:

```json
{
  "version": 1,
  "entries": [
    { "from": "panel", "to": "FAKE", "kind": "primitive", "since": "0.5.0" }
  ]
}
```

(`panel` is a real primitive.) Run the arch-check test:

Expected: FAIL — `panel is in the rename registry but still a canonical primitive`.

Then revert the registry to empty.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-arch-check/tests/rename_consistency_test.rs crates/vox-compiler/src/lowering_shared/primitive_tags.rs
git commit -m "feat(arch-check): enforce rename-registry consistency (VUV-9 task 8)"
```

### Task 9: Documentation pass + roadmap update

**Files:**
- Modify: `docs/src/architecture/gui-authoring-syntax-2026.md` — add VUV-9 row to the implementation-status table
- Modify: `docs/src/architecture/gradio-streamlit-research-2026.md` — strike Q6 from open questions (subsumed by VUV-9)

- [ ] **Step 1: Update the VUV phasing table**

Add a row to the implementation-status table in [gui-authoring-syntax-2026.md](../../../src/architecture/gui-authoring-syntax-2026.md):

```markdown
| **VUV-9** Naming policy + codemod | ✅ Done | Policy at vuv-naming-policy-2026.md; registry at contracts/naming/renames.v1.json (empty until first rename); `vox migrate` codemod + arch-check enforcement. |
```

- [ ] **Step 2: Update research doc open questions**

In [gradio-streamlit-research-2026.md §7](../../../src/architecture/gradio-streamlit-research-2026.md#7-open-questions-and-follow-ups), mark Q6 as resolved with a link to this plan.

- [ ] **Step 3: Regenerate the doc indices**

Run: `cargo run -p vox-doc-pipeline 2>&1 | tail -3`
Expected: `Successfully generated SUMMARY.md with all pages.`

- [ ] **Step 4: Commit**

```bash
git add docs/src/architecture/gui-authoring-syntax-2026.md docs/src/architecture/gradio-streamlit-research-2026.md docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml
git commit -m "docs(vuv): mark VUV-9 done; resolve research-doc Q6"
```

### VUV-9 acceptance gate

All of the following must be green before VUV-9 is considered shipped:

- [ ] `cargo test -p vox-compiler --test rename_alias_test` — passes
- [ ] `cargo test -p vox-cli --test migrate_codemod_test` — passes
- [ ] `cargo test -p vox-arch-check rename_consistency_test` — passes
- [ ] `cargo run -p vox-cli -- migrate --dry-run examples/golden` — reports 0 changes (empty registry)
- [ ] `cargo run -p vox-doc-pipeline` — clean
- [ ] Existing dashboard build still succeeds: `cargo build -p vox-dashboard` (or whatever the canonical dashboard build target is)

---

## VUV-10 — First-class accessibility kwargs

**Scope summary.** Add typed `aria_label`, `aria_describedby`, `role`, `tab_index`, `aria_live`, `aria_hidden` as universal kwargs on UI primitives. The lowering emits proper ARIA attributes (no string-typed escape). The validator enforces:

- Clickable primitives (`button`, `link`, `icon_button`) require either visible label text or `aria_label`. Otherwise, compile-time error with location pointing at the primitive call.
- Icon-only primitives (`icon_button`, `icon`) require `aria_label`.
- `aria_hidden: true` on focusable primitives is a compile-time error.

**Files involved:**
- `crates/vox-compiler/src/lowering_shared/primitive_tags.rs` — extend `UNIVERSAL_STYLE_KWARGS` (or its a11y sibling) with the new kwargs.
- `crates/vox-compiler/src/lowering_shared/primitive_tags.rs` and the validator at `crates/vox-compiler/src/typeck/` — enforce required-label rules.
- `crates/vox-compiler/src/codegen_ts/jsx.rs` — emit `aria-*` attributes from the kwargs.
- Dashboard `.vox` files — add labels where the validator now flags them.
- `crates/vox-compiler/tests/` — golden tests for emit; failure-path tests for unlabeled buttons.

**Why a separate plan:** ~15 individual TDD steps; needs a fixture-by-fixture migration of dashboard components; accessibility audit pass (axe-core in CI).

**Trigger to write the plan:** VUV-9 lands and the registry is in production use. Use `superpowers:writing-plans` with the spec being the bullet list above.

---

## VUV-11 — Layout vocabulary expansion (responsive variants, grid, stack, cluster)

**Scope summary.** Three threads of work:

1. **Responsive variants on existing kwargs.** `pad_md: 8` means "padding `8` at the `md` breakpoint and up." Same suffix grammar as the typed-kwargs in VUV-4 — `_sm`, `_md`, `_lg`, `_xl` — drawing breakpoint values from a new section of `tokens.v1.json`.
2. **New `grid()` primitive.** Typed `cols: int | list[int]`, `rows: int | list[int]`, `gap: token`, `cols_md: int`, etc. Lowers to CSS Grid via Tailwind classes.
3. **New `stack()` and `cluster()` primitives.** Per "Every Layout" — `stack()` is vertical-with-gap, `cluster()` is horizontal-wrap-with-gap. They are already implementable via `column()` + `row()` but the named primitives hint to LLMs at the right semantic.

**Files involved:**
- `contracts/tokens/tokens.v1.json` — breakpoint section.
- `crates/vox-compiler/src/lowering_shared/primitive_tags.rs` — `grid`, `stack`, `cluster` registration; responsive kwarg suffix parsing.
- `crates/vox-compiler/src/codegen_ts/css_props.rs` and `jsx.rs` — emit responsive Tailwind classes (`md:p-8`, `lg:grid-cols-3`).
- Dashboard fixtures — at least one tab rebuilt on `grid()` to validate the surface.
- Golden tests.

**Why a separate plan:** the responsive suffix grammar interacts with VUV-4's existing kwarg parsing; the `grid` primitive needs its own design pass for child semantics (rows-as-children vs cells-as-children); breakpoint-token registry needs schema work.

**Trigger to write the plan:** VUV-9 lands. VUV-10 can land in parallel; VUV-11 doesn't depend on it.

---

## VUV-12 — First-class view testability

**Scope summary.** Two threads:

1. **Snapshot harness.** `vox test view <ComponentName> --props key=value,key2=value2` renders a component to a deterministic Web-IR or DOM snapshot. Snapshots saved next to the source under `__snapshots__/` (mirroring Jest convention). Re-running with `--update-snapshot` rewrites them. CI mode runs without `--update-snapshot` and fails on diff.
2. **Property-based invariants.** Property tests in `crates/vox-test-harness/` that load a `.vox` view, vary props through reasonable ranges, and assert layout invariants (no overflow, all primitives validated, no unbound identifiers in emitted TSX, no `aria-*` violations).

**Files involved:**
- New: `crates/vox-cli/src/commands/test_view.rs` (or extend existing `test.rs`).
- New: `crates/vox-test-harness/src/view_test.rs` (or new submodule).
- Modify: `crates/vox-compiler/src/lib.rs` — expose a `render_to_snapshot(component, props)` helper from the existing emit path.
- Test fixtures: at least one component with snapshot + property test.

**Why a separate plan:** snapshot file format needs to be stable across compiler versions (don't snapshot raw TSX — too volatile; snapshot Web-IR or a normalized form). Property-based test harness needs a strategy generator for typed props.

**Trigger to write the plan:** VUV-11 lands (need the expanded vocabulary so snapshot suite isn't trivially small).

---

## VUV-13 — Stdlib `chat_panel` and companions

**Scope summary.** New crate `crates/vox-ui-stdlib/` with three composed components:

1. **`chat_panel(messages, on_send, streaming, placeholder, submit_label)`** — a column with scrollable message list + bottom input row. Children are rendered as an additional-inputs accordion (à la `gr.ChatInterface`).
2. **`chat_message(role, content, timestamp?)`** — a single bubble. Already prototyped in [chat_message.vox](../../../../crates/vox-dashboard/app/src/lib/) (or wherever the dashboard's chat lives); promote to stdlib with hardened API.
3. **`streaming_token(stream)`** — a primitive that consumes a token stream and renders incrementally. Lowers to a reactive consumer hooked into the existing Vox reactive system.

**Files involved:**
- New crate `crates/vox-ui-stdlib/` with `Cargo.toml` and `.vox` source files (the components are written in Vox, not Rust).
- `Vox.toml` schema — auto-import the stdlib for `target = "fullstack"` projects.
- `crates/vox-compiler/src/lowering_shared/` — register the new components as known imports.
- Reference example: `examples/golden/chat_app.vox`.
- Tests: snapshot tests on the reference example; integration test for streaming.

**Why a separate plan:** stdlib auto-import is a new concept that needs design (when to auto-import; whether users can opt out; how versioning works). Streaming primitive needs reactive-consumer story; tied to Vox's existing reactive model.

**Trigger to write the plan:** VUV-9 lands. Independent of VUV-10/11/12 — can land at any time after VUV-9.

---

## VUV-14 — `@session` decorator + per-session store

**Scope summary.** Per [Q4 resolution](#q4--per-session-state-primitive-grstate-analogue), implement:

```vox
// vox:skip
@session
let cart: Cart = Cart.empty()

@session(scope: tab)        // default
let draft: Draft = Draft.empty()

@session(scope: window)
let theme: Theme = Theme.dark()
```

**Files involved:**
- Parser: recognize `@session` decorator on top-level `let` declarations.
- AST: new `Decl::SessionLet(SessionLetDecl { name, ty, init, scope })`.
- HIR: lower to `HirSessionState { name, ty, init_expr, scope }` per module.
- Codegen (Rust): emit a session-keyed store hookup in the Axum app — in-memory `HashMap<SessionId, HashMap<Name, Value>>` by default; Redis backend behind feature flag.
- Codegen (TS): emit a React context that hydrates from server on mount.
- Runtime: session-id cookie middleware in the generated Axum app (HttpOnly + SameSite=Lax + Secure-when-HTTPS; 30-min idle eviction default).
- `Vox.toml` schema: `[session] backend = "memory" | "redis"`, `[session] idle_timeout_seconds = 1800`.
- Wire format: session-state hydration over the existing WebSocket; serializes through wire-format SSOT.
- Tests: cart-survives-refresh integration test; Redis backend smoke test.

**Why a separate plan:** new decorator + new HIR node + cross-cutting codegen change (Rust + TS) + runtime middleware + Vox.toml schema. Easily 25+ TDD steps.

**Trigger to write the plan:** VUV-13 lands (so chat-panel reference example can use `@session let history`).

---

## VUV-15 — `@memo` and `@resource` decorators

**Scope summary.** Per [Q5 resolution](#q5--cache-discipline-streamlit-style-intent-split), two decorators on `fn`:

```vox
// vox:skip
@memo
fn expensive_pure(input: Input) -> Output { … }

@resource
fn open_db() -> Db { … }
```

- **`@memo`** — cache key is `(argument_hash, source_content_hash)`. Deep-immutable return (cloned-on-read). Closure capture is a type error at definition time (closures over outer state aren't part of the cache key — Streamlit footgun rejection).
- **`@resource`** — process-singleton; returns by reference; lifetime tied to process. Constructor runs on first call; subsequent calls return the cached reference.

**Files involved:**
- Parser, AST, HIR: new decorators.
- Typeck: closure-capture detection on `@memo` (helpful error: "captured `foo` is not part of the cache key — pass it as an argument").
- Codegen (Rust): emit a `OnceCell` for `@resource`, a `moka` (or equivalent) cache for `@memo`.
- Tests: second-call cache hit; closure-capture rejected with helpful message.

**Why a separate plan:** typeck rule for closure capture is non-trivial; cache backend choice needs a decision (in-process only in v1; future plan considers cross-process via the same backend abstraction as VUV-14).

**Trigger to write the plan:** VUV-14 lands (so the session-store and cache stories can share a backend abstraction).

---

## Cross-cutting concerns

### Migration corpus discipline (every phase)

Every phase that renames a primitive, kwarg, or decorator **must** add an entry to `contracts/naming/renames.v1.json` in the same commit. The arch-check rule from VUV-9 task 8 enforces this. The codemod's job is to make the rename free for users — they run `vox migrate` once and their corpus is up to date.

### MENS retraining (every phase that changes the surface)

Every phase that changes a public name or adds a primitive **must** flag a MENS retraining task in the operator handoff. This is the unfinished VUV-7 thread; this plan does not attempt to ship MENS automation, but every later phase reminds the operator.

### Doc updates (every phase)

Every phase ends with a `cargo run -p vox-doc-pipeline` step. SUMMARY.md, architecture-index.md, and feed.xml are auto-regenerated; never hand-edited.

### Backwards compatibility

VUV-9 ships the deprecation cycle. Every later phase honors it: if it renames `X` to `Y`, both names parse for one major version, then `X` is removed. No `// removed in 0.5.0` comments in source — the registry is the source of truth.

---

## Risks and non-goals

**Risks:**

- **Codemod imperfection.** A token-based codemod can't handle every edge case (e.g., a primitive name used as a string in a `raw_class()` call). Mitigation: `vox migrate` reports any case where the source fails to lex after the rewrite, and exits non-zero.
- **Registry sprawl.** If every phase adds 3 renames, the registry grows fast. Mitigation: each entry has a `removed_in` field; the registry is pruned when removed entries hit their target version.
- **MENS staleness.** Renames in the registry produce deprecation warnings, but if the training corpus isn't retrained, MENS will keep emitting old names. Mitigation: every phase plan flags MENS retraining; operators are responsible for actually doing it.
- **Cross-phase dependencies.** VUV-12 depends on VUV-11; VUV-14 references VUV-13's chat panel; VUV-15 may share infrastructure with VUV-14. Mitigation: each follow-up plan is written **after** its predecessors land, so the dependency state is concrete, not assumed.

**Non-goals:**

- Cloning Gradio or Streamlit. We are not building a `gr.Interface`-equivalent or an `st.write`-equivalent. The research doc explicitly rejected magic and polymorphic catch-alls.
- Native rendering (egui/iced/dioxus). [frontend-convergence-findings-2026.md](../../../src/architecture/frontend-convergence-findings-2026.md) is clear: "GUI native" means typed primitives, not a non-React renderer.
- Deploy-side concerns. `share=True` analogue, hosted demo platform, etc. — out of scope; tracked separately.

---

## Self-review checklist

(Done by the plan author before handoff.)

- [x] Spec coverage — every keep/adapt row in [research-doc §6](../../../src/architecture/gradio-streamlit-research-2026.md#6-lessons-for-vuv--keep-adapt-reject) maps to a phase in the roadmap.
- [x] Open questions — Q1–Q7 each have an explicit resolution.
- [x] Placeholder scan — no "TBD"; every "follow-up plan" entry has a concrete trigger condition.
- [x] Type consistency — `RenameRegistry`, `RenameEntry`, `RenameKind` named consistently across tasks 2–8.
- [x] No silent renames in the plan itself — `rewrite_for_test`, `parse_with_registry`, etc. are introduced and used consistently.

---

## Execution handoff

Plan complete and saved at `docs/superpowers/plans/tooling/2026-05-08-vuv-improvement-roadmap.md`. The detailed VUV-9 work is ready to execute.

**Two execution options:**

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration. Use `superpowers:subagent-driven-development`.
2. **Inline Execution** — execute tasks in this session using `superpowers:executing-plans`, batch with checkpoints.

**Which approach?**
