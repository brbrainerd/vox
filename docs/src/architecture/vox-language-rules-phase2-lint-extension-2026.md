---
title: "Vox Language Rules — Phase 2: Lint Extension with Stable Diagnostic IDs (2026-05-09)"
description: "Step-by-step plan to extend vox-code-audit with 14+ new detectors covering direct-LLM-call rejection, env.get-secret-shape rejection, ?-operator opportunity, ADR-citation discipline, decorator-position lint, duplicate-prefix names, long-range coupling, and more. Every detector ships with a stable diagnostic ID, a serializable LintFix descriptor, an --explain page, confidence + alternatives, and a negative-example corpus entry. Adds vox check --for-llm JSON mode optimized for LLM agents proposing fixes."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 2 child plan. Each detector descriptor is a complete spec for the implementing PR; the LLM-target additions (--for-llm, confidence, alternatives) are the largest single delta between Vox and a typical compiler."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-code-audit: 14 new detectors, --for-llm JSON mode, --explain subcommand, --rationale-required mode"
  - "examples/golden/anti/: every detector adds a negative example"
  - "docs/src/reference/diagnostics/: per-ID explain pages"
  - "AGENTS.md: each new advisory rule gets an enforced-by backlink"
---

# Phase 2 — `vox-code-audit` Lint Extension

> **Parent plan:** [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md)
> **Depends on:** Phase 1 Tasks 1–2 (diagnostic-catalog scaffolding + `#[vox_diagnostic]` macro).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans.

**Goal:** Add 14 new detectors to `vox-code-audit`, each with a stable ID, a serializable autofix descriptor, an `--explain` page, and a negative-example corpus entry. Add `vox check --for-llm` JSON output mode optimized for LLM agents quoting reproducers and applying fixes. Establish the diagnostic-shape contract that Phases 3–5 reuse.

**Architecture:** Detectors live in `crates/vox-code-audit/src/detectors/<category>/<id>.rs`. Each detector implements a `Detector` trait that returns `Vec<Diagnostic>` from a `&CompiledModule` input. Diagnostics carry the full LLM-friendly shape from the parent plan §"LLM-friendly diagnostic shape." Autofix descriptors are serializable `LintFix` enums (no closures), so a CI log entry can be replayed by an LLM agent or a `vox check --apply-fix <id>` invocation.

The detector trait:

```rust
pub trait Detector {
    fn id(&self) -> DiagnosticId;
    fn check(&self, module: &CompiledModule, ctx: &DetectorCtx) -> Vec<Diagnostic>;
}

pub struct Diagnostic {
    pub id: DiagnosticId,
    pub severity: Severity,
    pub span: Span,
    pub excerpt: Excerpt,           // 3 lines context + offending line + 3 lines after
    pub message: String,            // human-readable
    pub rationale: &'static str,    // constant per ID; pulled from the catalog
    pub suggested_fix: Option<LintFix>,
    pub confidence: Confidence,     // Certain / Likely / Speculative
    pub alternatives: Vec<LintFix>, // additional plausible fixes
    pub adr: Option<&'static str>,
}

pub enum LintFix {
    Replace { range: Span, new_text: String },
    InsertBefore { offset: usize, text: String },
    InsertAfter { offset: usize, text: String },
    RemoveDecorator { name: String, span: Span },
    RenameSymbol { old: String, new: String, sites: Vec<Span> },
    AddImport { path: String, after_line: usize },
    Composite(Vec<LintFix>),
}

pub enum Confidence { Certain, Likely, Speculative }
```

**Out of scope for Phase 2:**
- Type-system-level rules (Phase 3 owns `Id[T]`, named errors, syntax_version).
- Runtime traps (Phase 4).
- Effect-system enforcement (Phase 5).
- Renaming or removing existing `vox-code-audit` detectors. Existing `skeleton/*` detectors stay as-is.

---

## Verification setup

- `cargo test -p vox-code-audit --lib detectors::` — per-detector unit tests.
- `cargo test -p vox-code-audit --test golden` — input-`.vox`/expected-diagnostic-JSON snapshot tests.
- `cargo run -p vox-cli -- check examples/golden/ --json` — should emit zero diagnostics from new detectors against the curated golden corpus (after burn-down PRs).
- `cargo run -p vox-cli -- check --for-llm examples/golden/anti/ --json` — should emit exactly the diagnostics declared in each anti-example's `expected_diagnostic:` frontmatter.
- `cargo run -p vox-cli -- check --explain vox/llm/direct-provider-call` — should print the rationale and a worked-example fix.

---

## Task 1: Detector trait + catalog populated with all 14 IDs

**Files:**
- Modify: `crates/vox-code-audit/src/diagnostics/catalog.rs` — add 14 variants, each with `#[vox_diagnostic(...)]`.
- Modify: `crates/vox-code-audit/src/detectors/mod.rs` — declare 14 detector submodules (skeletons returning empty `Vec`).
- Create: `docs/src/reference/diagnostics/index.md` (hand-written index)
- Create (one each): `docs/src/reference/diagnostics/<id>.md` (skeleton with frontmatter + headings)

**Diagnostic IDs and metadata** (the full catalog landed in this task; implementations land in Tasks 2–14):

| ID | Severity at land | Severity at escalation | Phase | ADR | Audit item |
|---|---|---|---|---|---|
| `vox/llm/direct-provider-call` | error | (already error) | 2 | TBD-llm-call-discipline | A.5 |
| `vox/secret/env-get-shape` | error | (already error) | 2 | TBD-secret-discipline | A.6 |
| `vox/crypto/banned-crate-import` | error | (already error) | 2 | cryptography-ssot-2026 | A.30 |
| `vox/style/duplicate-prefix-name` | warning | warning (perma) | 2 | — | A.16 |
| `vox/style/long-range-coupling` | note | warning (next minor) | 2 | — | A.17 |
| `vox/control-flow/question-mark-opportunity` | note | warning (next minor) | 2 | — | A.19 |
| `vox/control-flow/option-combinator-opportunity` | note | warning (next minor) | 2 | — | A.20 |
| `vox/doc/missing-adr-citation` | note | warning in `vox-actor-runtime`/`vox-orchestrator`/`vox-compiler` | 2 | — | A.22 |
| `vox/decorator/position-mismatch` | warning | error (next minor) | 2 | — | A.27 |
| `vox/require/justification-prose-required` | note | warning (next minor) | 2 | — | A.28 |
| `vox/handler/panicking-builtin` | warning | error (after 2 minors) | 2 | ADR-019 | A.29 (descoped from A.4) |
| `vox/state-machine/unreachable-state` | warning | warning (perma — proof-tier deferred) | 2 | ADR-030 | A.54 (descoped) |
| `vox/secret/leaked-to-span` | error | (already error) | 2 | telemetry-trust-ssot | A.11 (descoped from full taint) |
| `vox/auth/endpoint-missing-decorator` | warning | error after Phase 5 effect system lands | 2 | TBD-endpoint-auth | A.58 (descoped) |

**Why these and only these:** Each is enforceable with the *current* compiler surface (no Phase 3/5 dependency) and addresses a known LLM-confusion or security smell. The detectors deferred to other phases are listed in §"Out of scope" of the parent plan.

**Verify:** `cargo build -p vox-code-audit` passes; `cargo run -p vox-cli -- check --explain <any-id-above>` returns the rationale from the catalog (skeleton text initially; populated by Tasks 2–14).

---

## Task 2: `vox/llm/direct-provider-call` detector

**Files:**
- Modify: `crates/vox-code-audit/src/detectors/llm/direct_provider_call.rs`
- Create: `examples/golden/anti/llm-direct-provider-call.vox`
- Modify: `docs/src/reference/diagnostics/llm-direct-provider-call.md` — fill in rationale prose

**Rule:** Reject any callsite where `std.http.post_json`, `std.http.get`, or `std.http.request` targets a hostname matching `*openrouter*|*anthropic*|*openai*|*cohere*|*mistral*|*together*|*replicate*|*huggingface*|*fireworks*|*deepinfra*|*perplexity*|*google.*generativelanguage*|*googleapis.*aiplatform*` (regex; configurable via `Vox.toml` `[lint.llm-providers]`).

**Suggested fix (`Confidence::Likely`):**
```
Replace { range: <call site>, new_text: "populi.complete(...)" }
```
with `alternatives` listing other `populi.*` shapes (`populi.stream`, `populi.embed`).

**Negative-example file:**
```vox
// vox:skip examples/golden/anti/llm-direct-provider-call.vox
// frontmatter:
// ---
// error_kind: lint
// expected_diagnostic: "vox/llm/direct-provider-call"
// mens_role: anti-example
// ---

fn ask_claude(prompt: str) -> Result[str, NetError] {
    std.http.post_json(
        url: "https://api.anthropic.com/v1/messages",
        body: { model: "claude-opus-4-7", input: prompt },
    )
}
```

**Why this is `error` immediately:** Direct provider calls bypass `vox-populi`'s telemetry, retry, cost accounting, and capability ledger. This is a security and observability surface, not a style preference.

**Suppression:** `// toestub-ignore(vox/llm/direct-provider-call) — <reason>` on the line above the call. Reviewer audit required.

**Verify:** Golden test in `crates/vox-code-audit/tests/golden/llm-direct-provider-call/`; `--explain` returns the rationale.

---

## Task 3: `vox/secret/env-get-shape` detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/secret/env_get_shape.rs`
- Create: `examples/golden/anti/secret-env-get-shape.vox`
- Modify: `docs/src/reference/diagnostics/secret-env-get-shape.md`

**Rule:** Pattern-match on `env.get(<literal>)` callsites. If the literal matches the case-insensitive regex `(KEY|SECRET|TOKEN|PASSWORD|CREDENTIAL|APIKEY|API_KEY|PRIVATE)`, fire as error.

**Suggested fix (`Confidence::Certain`):**
```
Replace {
  range: <env.get call>,
  new_text: "vox_secrets.resolve(\"<derived-secret-id>\")"
}
```
where `<derived-secret-id>` is computed by lowercasing + snake-casing the env var name. The fix is `Likely` (not `Certain`) when a matching `SecretId` does not yet exist in `crates/vox-secrets/src/spec.rs` — the diagnostic carries an `alternatives` entry suggesting addition of a `SecretSpec`.

**Composite fix shape** when no `SecretId` exists:
```
Composite([
    InsertBefore { offset: <spec.rs first var line>, text: "    SecretSpec::new(SecretId::<NewId>, ...),\n" },
    Replace { range: <env.get>, new_text: "vox_secrets.resolve(SecretId::<NewId>)" }
])
```

**LLM-target note:** This is exactly the kind of multi-file fix that benefits from the `Composite` `LintFix` shape. An LLM agent can see "I need to update spec.rs *and* the call site" without inferring it from prose.

**Verify:** Golden tests for: (a) literal matching pattern, (b) literal not matching, (c) non-literal arg (no fire), (d) suppression with reason works.

---

## Task 4: `vox/crypto/banned-crate-import` detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/crypto/banned_crate_import.rs`
- Modify: `deny.toml` — add `[[bans]]` entries for `aegis`, `ring`, transitive cmake/nasm-pulling crates (audit item [A.30])
- Create: `examples/golden/anti/crypto-banned-crate.vox`

**Rule:** This is dual-enforced: `deny.toml` for Rust crate dependencies (`cargo-deny`), and the `vox-code-audit` detector for `import` statements in `.vox` files referencing banned cryptography surfaces. The Vox-side detector matters because future `.vox` plugins may pull in Rust crates indirectly via `@host_crate(...)` style imports.

**Why dual enforcement:** [AGENTS.md:95–98] is currently prose; `deny.toml` enforces the Rust path; this detector enforces the Vox path. Symmetric coverage.

**Verify:** Synthetic `.vox` file importing a banned name fires; deleting the import passes; `cargo deny check` integrates into the same CI job.

---

## Task 5: `vox check --for-llm` JSON mode

**Files:**
- Modify: `crates/vox-cli/src/check.rs` (or appropriate command file) — add `--for-llm` flag
- Modify: `crates/vox-code-audit/src/report.rs` — add `LlmReport` struct alongside the existing `JsonReport`
- Create: `crates/vox-code-audit/tests/llm_mode_smoke.rs`

**Why this is the largest single LLM-target delta:** A typical compiler emits a diagnostic and assumes the human reader has the file open. An LLM agent invoked from a CI log or a tool-call response often *does not* have the file. The `--for-llm` mode includes everything the agent needs to propose a fix without further context fetches:

```json
{
  "schema": "vox.lint.llm-report.v1",
  "vox_version": "0.6.0",
  "diagnostics": [
    {
      "id": "vox/llm/direct-provider-call",
      "severity": "error",
      "since": "0.6.0",
      "adr": "TBD-llm-call-discipline",
      "file": "src/agents/researcher.vox",
      "line": 42,
      "column": 8,
      "message": "Direct provider call to api.anthropic.com bypasses vox-populi",
      "rationale": "Direct HTTP calls to LLM provider hostnames bypass the populi telemetry, cost accounting, capability ledger, and retry policy. All inference traffic must route through populi.*.",
      "minimal_repro": "fn f() {\n    std.http.post_json(url: \"https://api.anthropic.com/v1/messages\", body: {})\n}",
      "excerpt": {
        "lines": [40, 41, 42, 43, 44],
        "text": "fn ask_claude(prompt: str) -> Result[str, NetError] {\n    std.http.post_json(\n        url: \"https://api.anthropic.com/v1/messages\",\n        body: { model: \"claude-opus-4-7\", input: prompt },\n    )\n}"
      },
      "suggested_fix": {
        "kind": "replace",
        "range": { "start": [41, 4], "end": [44, 5] },
        "new_text": "populi.complete(model: \"claude-opus-4-7\", prompt: prompt)"
      },
      "confidence": "likely",
      "alternatives": [
        { "kind": "replace", "new_text": "populi.stream(...)" },
        { "kind": "replace", "new_text": "populi.embed(...)" }
      ],
      "explain_url": "https://voxlang.org/diag/vox/llm/direct-provider-call"
    }
  ]
}
```

**Compared with existing `--json` mode:** `--for-llm` adds `minimal_repro`, `excerpt.text`, `rationale`, `confidence`, `alternatives`, and `explain_url`. The existing `--json` shape is preserved as-is for tools that already consume it.

**Verify:** Snapshot test of the JSON output for one diagnostic per ID in the catalog (after Tasks 2–14 land their detectors).

---

## Task 6: `vox check --explain <id>` subcommand

**Files:**
- Create: `crates/vox-cli/src/explain.rs`
- Modify: `crates/vox-cli/src/main.rs` — wire the subcommand
- Modify: `docs/src/reference/diagnostics/<id>.md` files — content authored in Tasks 2–14

**Output shape:**

```
$ vox check --explain vox/llm/direct-provider-call

vox/llm/direct-provider-call (error, since 0.6.0)
ADR: TBD-llm-call-discipline
URL: https://voxlang.org/diag/vox/llm/direct-provider-call

Why this rule exists
--------------------
Direct HTTP calls to LLM provider hostnames bypass the populi telemetry,
cost accounting, capability ledger, and retry policy. All inference traffic
must route through populi.*.

Bad
---
fn ask_claude(prompt: str) -> Result[str, NetError] {
    std.http.post_json(
        url: "https://api.anthropic.com/v1/messages",
        body: { model: "claude-opus-4-7", input: prompt },
    )
}

Good
----
fn ask_claude(prompt: str) -> Result[str, NetError] {
    populi.complete(model: "claude-opus-4-7", prompt: prompt)
}

Suppress (audit-required)
-------------------------
// toestub-ignore(vox/llm/direct-provider-call) — <reason>
```

**Why bad/good is structured prominently:** LLMs asked "fix this Vox diagnostic" do dramatically better with a worked example pair than with prose alone. This format is the same input shape an LLM would naturally produce for in-context learning.

**Verify:** `vox check --explain` succeeds for every ID in the catalog; CI fails if any catalog ID lacks an `--explain` page.

---

## Task 7: `vox/doc/missing-adr-citation` detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/doc/missing_adr_citation.rs`

**Rule:** For every public `fn`, `actor`, `workflow`, `activity`, `@table`, `@endpoint` in scope of `crates/vox-actor-runtime/**`, `crates/vox-orchestrator/**`, `crates/vox-compiler/**`: the `///` doc must contain at least one match of `(ADR-\d+|TASK-\d+\.\d+|Phase \d+)`. Outside those scopes: same rule fires as `note`, escalates to `warning` per minor.

**Anti-T-number rule:** Active rejection of `T\d+` references in `///` doc as a corpus-drift signal. Honors anti-recommendation [A.69].

**Suggested fix:** Cannot autofix (the human must pick the citation). Diagnostic emits a list of recently-modified ADRs and TASKs touched in the same PR as candidates (computed from `git log` of the file).

**Why scope matters:** `vox-actor-runtime`/`vox-orchestrator`/`vox-compiler` are the highest-stakes correctness surfaces; doc drift there is more dangerous than in CLIs or examples.

**Verify:** Golden test with: (a) properly-cited fn, (b) uncited fn fires, (c) T-number cited fires the anti-rule.

---

## Task 8: `vox/decorator/position-mismatch` detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/decorator/position_mismatch.rs`

**Rule:** Detects:
- A bare keyword used in a position where a decorator is expected (`durable fn` instead of `@durable fn`).
- A decorator used in a position the grammar would parse as a bare-keyword declaration.
- A decorator that is *redundant* given the bare keyword (e.g., `@actor actor MyActor`).

**Suggested fix (`Confidence::Certain` for simple cases):**
- For `durable fn` → `Replace { range: <durable>, new_text: "@durable" }` plus space normalization.
- For redundant `@actor actor X` → `RemoveDecorator { name: "@actor", span: <decorator span> }`.

**LLM-target note:** Decorator-position errors are the most common Vox parse error LLMs produce. Surfacing them with a `Certain`-confidence autofix means an agent can fix the file in one tool call.

**Verify:** Golden tests covering all four mismatch shapes.

---

## Task 9: `vox/style/duplicate-prefix-name` detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/style/duplicate_prefix_name.rs`

**Rule:** Identifier matches `(\w+)_\1(?:_|$)` — e.g., `user_user_id`, `tasks_tasks`, `task_task_status`. Fires `warning` with `Confidence::Likely` autofix that strips the duplication (`user_user_id` → `user_id`). The fix is `Likely` not `Certain` because the duplication may be intentional (e.g., a join table column `user_user_id` referring to the FK from `users.users`); `alternatives` includes "no fix — keep as-is."

**Why it matters for LLM target:** This is a high-signal LLM-confusion smell. Models that hallucinate column names often produce duplicated prefixes; flagging early kills the smell at the lint layer.

**Verify:** Golden tests for matching/non-matching identifiers; rename fix updates all callsites within the module via `RenameSymbol`.

---

## Task 10: `vox/style/long-range-coupling` detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/style/long_range_coupling.rs`

**Rule:** Warn when:
- A let-binding's `definition span end` and its `last use span start` are > 80 lines apart.
- A name shadows another in a nested scope at any nesting level > 2.

**Suggested fix:** None (refactoring is too contextual). Diagnostic includes `excerpt` of both the definition and the farthest use.

**Why it matters:** AI generators frequently produce long single functions where local state is defined far from its use, confusing later edits. P3 (LANGUAGE_DESIGN_PRIORITIES.md) made checkable.

**Verify:** Golden test with synthetic 100-line function that uses a binding from line 3 at line 95.

---

## Task 11: `vox/control-flow/question-mark-opportunity` + `option-combinator-opportunity`

**Files:**
- Create: `crates/vox-code-audit/src/detectors/control_flow/question_mark_opportunity.rs`
- Create: `crates/vox-code-audit/src/detectors/control_flow/option_combinator_opportunity.rs`

**Rules:**
- Question-mark: `match foo() { Ok(x) => x, Err(e) => return Err(e) }` → `foo()?`. Mirror of `clippy::question_mark`. Autofixable with `Confidence::Certain`.
- Option combinator: `match opt { Some(x) => f(x), None => default }` → `opt.map(f).unwrap_or(default)`. Autofix `Confidence::Likely` (because side-effects in `f` may matter).

**Why both at once:** They share an AST-walking infrastructure; landing them in one PR is cheaper than two.

**Verify:** Golden tests; round-trip fix application produces clean re-lint.

---

## Task 12: `vox/require/justification-prose-required` + `vox/handler/panicking-builtin`

**Files:**
- Create: `crates/vox-code-audit/src/detectors/require/justification_prose.rs`
- Create: `crates/vox-code-audit/src/detectors/handler/panicking_builtin.rs`

**Rules:**

`vox/require/justification-prose-required`: A `@require(<expr>)` with > 1 operator in `<expr>` and no trailing prose comment ≥ 40 chars fires `note`. The fix appends a placeholder `// because: …` comment that the human fills in.

`vox/handler/panicking-builtin`: In any actor message handler body or workflow activity body, calls to known-panicking builtins (`std.unwrap`, `std.expect`, `std.panic`, `std.unreachable`, `std.todo`) fire `warning`. Suggested fix: replace with the `Result`-returning variant when one exists; otherwise wrap in `match`. Confidence varies.

**Why these together:** Both involve walking handler/activity bodies; shared infra.

**Verify:** Golden tests per rule. Anti-examples added to `examples/golden/anti/`.

---

## Task 13: `@example` decorator + doctest/corpus unification

**Files:**
- Modify: `crates/vox-compiler/src/lower/decorators/example.rs` (new decorator)
- Modify: `crates/vox-doc-pipeline/src/lib.rs` — `@example`-tagged blocks become doctests
- Modify: `crates/vox-corpus/src/build.rs` (or equivalent) — `@example`-tagged blocks become Mens corpus entries with `mens_role: exemplar`
- Create: `examples/golden/decorators/example.vox`

**Why:** Today a "good example" lives in three places: a docstring, a separate `examples/` file, and a Mens corpus entry. They drift. `@example` makes the source of truth singular: declare it once, the doctest runner verifies it compiles, the corpus pipeline includes it as training data.

**Shape:**

```vox
// vox:skip — @example decorator and named-arg syntax are planned features
@example("Resolve a secret and use it in an API call")
fn use_openai_key() -> Result[str, NetError] {
    let key = vox_secrets.resolve(SecretId::OpenAI)?;
    populi.complete(model: "gpt-4", api_key: key, prompt: "hi")
}
```

**Verify:** A `vox-doc-pipeline --check` run on a file with an `@example` decorator emits a doctest entry; a `vox-corpus build --include-examples` run includes the same fn in the corpus output. The decorator must error if applied to non-public fns (examples must be linkable from docs).

---

## Task 14: `vox check --rationale-required` + suppression hygiene

**Files:**
- Modify: `crates/vox-cli/src/check.rs` — add `--rationale-required` flag
- Modify: `crates/vox-code-audit/src/suppression.rs` — parse `// toestub-ignore(<id>) — <reason>` and `// vox:skip — <reason>` (note the em-dash), fail the check if `--rationale-required` is set and any suppression lacks a `reason` of ≥ 20 chars
- Modify: `contracts/toestub/suppressions.v1.json` schema — require `owner` (existing) and `reason` (existing) and add `expires_at:` (new, optional)

**Why:** Audit [A.28] applied to suppressions themselves. Without this, a suppression comment is itself a way to silently bypass a rule. With `--rationale-required` in CI, every suppression carries an auditable reason. The `expires_at` field lets transitional suppressions auto-expire.

**Verify:** Synthetic file with `// vox:skip` (no reason) → CI fails when `--rationale-required` is set; passes otherwise. Existing suppressions without reasons get a one-week grace period via a temporary allowlist.

---

## Task 15: Negative-example corpus auto-built from compiler test fixtures

**Files:**
- Create: `crates/xtask/src/bin/gen_anti_corpus.rs`
- Modify: parser/typecheck test fixtures to wear a frontmatter declaration when their failure is a corpus-worthy anti-example
- Create: `examples/golden/anti/from-compiler-tests/` (output dir)

**Why:** Audit [A.24]. Today the compiler test fixtures and the Mens anti-corpus are separate. This task makes every parser test that asserts a diagnostic become a corpus anti-example automatically, with `error_kind:` and `expected_diagnostic:` derived from the test assertion.

**Verify:** Generator produces N anti-examples where N = count of compiler tests with the corpus frontmatter. CI fails if a compiler test asserts a diagnostic that doesn't exist in the catalog.

---

## Task 16: AGENTS.md backlinks + where-things-live update

**Files:**
- Modify: `AGENTS.md`:
  - §73–93 (Secret Management) — add "Enforced by `vox/secret/env-get-shape`."
  - §95–98 (Cryptography Policy) — "Enforced by `vox/crypto/banned-crate-import` and `deny.toml`."
  - §131–164 (Grammar Unification) — "Decorator-position discipline enforced by `vox/decorator/position-mismatch`."
- Modify: `docs/src/architecture/where-things-live.md` — add row for `crates/vox-code-audit-macros` (if Phase 1 didn't), and one row per new detector module.

**Verify:** `cargo run -p vox-doc-pipeline -- --check` passes. `where-things-live.md` change is in same PR as detector adds (per Phase 1 Task 14 pattern).

---

## Burn-down PRs (corpus hygiene)

After each detector lands as `note` or `warning`, a burn-down PR fixes existing violations *before* the next severity escalation. These PRs are not numbered tasks but are gates for severity escalation. One per detector that escalates.

Burn-down sequence (do in this order to minimize churn):
1. `vox/llm/direct-provider-call` — 0 expected violations (provider calls already route through populi); confirm and lock.
2. `vox/secret/env-get-shape` — audit existing `env.get` callsites; migrate any to `vox_secrets.resolve`.
3. `vox/style/duplicate-prefix-name` — across `examples/golden/`; autofixable.
4. `vox/control-flow/question-mark-opportunity` — across all `.vox` files; autofixable.
5. `vox/doc/missing-adr-citation` — for `vox-actor-runtime`/`vox-orchestrator`/`vox-compiler` `pub fn` only.

---

## Risks specific to this phase

| Risk | Mitigation |
|---|---|
| New detectors fire on existing corpus, blocking CI | Land each as `note` first; require burn-down PR before escalating to `warning` or `error`. |
| `Confidence::Certain` autofixes are wrong, an LLM applies them, code breaks | `Certain` is reserved for fixes that pass round-trip lint AND a parse-only verification. Add `cargo test -p vox-code-audit -- round_trip_certain_fixes` that applies every `Certain` fix in the test corpus and re-runs the detector. |
| `--for-llm` JSON shape becomes a public API and we want to change it | Schema field `"schema": "vox.lint.llm-report.v1"` allows a v2 alongside; v1 is kept until v3 lands. |
| Suppression hygiene (Task 14) breaks existing CI | Grace period: 30-day allowlist of pre-existing reasonless suppressions; CI warning until expiry, then error. |
| `@example` decorator (Task 13) overlaps with existing `examples/golden/` files | Migration: convert representative `examples/golden/` entries to `@example`-decorated fns living in test files; keep the rest as standalone golden examples (different role). |

---

## Phase 2 acceptance gate

- [ ] Catalog populated with 14+ IDs, all carry `since`/`severity`/`adr`/`explain` metadata.
- [ ] All 14 detectors implemented with golden tests passing.
- [ ] Per-ID `--explain` page exists; `vox check --explain <any-id>` succeeds for every ID.
- [ ] `vox check --for-llm` JSON output matches the documented schema; snapshot test passes.
- [ ] `vox check --rationale-required` mode works; existing suppressions either carry reasons or are on the 30-day allowlist.
- [ ] `@example` decorator lands; `vox-doc-pipeline` and `vox-corpus` consume it.
- [ ] Burn-down PRs land for `vox/style/duplicate-prefix-name` and `vox/control-flow/question-mark-opportunity` (corpus-wide).
- [ ] AGENTS.md backlinks added.
- [ ] `where-things-live.md` updated.
- [ ] Retrospective appended.

---

## Retrospective

_Appended within 5 working days of phase completion._
