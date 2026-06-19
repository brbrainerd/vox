---
title: "Antigravity Handoff Ledger — Prompt→Build→Review CI/CD Loop"
description: "Append-only, machine-mineable ledger of every plan/prompt handed from Claude Code to Google Antigravity (Gemini 3.5 Flash): the prompt artifact, what was delivered, the outcome, the errors and agent deviations encountered, the code-review findings, and the distilled prompt-engineering lessons fed back into the next prompt. The SSOT for closing the continuous prompt-engineering improvement loop."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Antigravity Handoff Ledger

**Purpose.** This is the **single source of truth for the prompt-engineering CI/CD loop** between Claude Code (where we *author* plans/prompts) and Google Antigravity / Gemini 3.5 Flash (where they *execute*). Every handoff is logged as one append-only entry. Entries are designed to be **mined**: each carries a machine-readable `yaml` block (stable keys) plus human prose. Over time, the distilled lessons (§B) become the checklist that hardens the next prompt.

**How the loop closes:**
```
author plan/prompt (Claude Code)  ──▶  hand off (launch statement)  ──▶  Antigravity/Gemini executes
        ▲                                                                          │
        │                                                                          ▼
  feed lessons back  ◀──  distill (§B)  ◀──  code-review the delivery  ◀──  delivery + errors
```

## How to use this ledger
1. **When you hand a plan to Antigravity:** append a new entry in §C using the schema below. Fill the `prompt`/`inputs` fields immediately.
2. **When Antigravity reports back:** fill `outcome`, `errors_encountered`, `agent_deviations`, `commits`.
3. **After code-review:** fill `review_findings` and `verdict`, then add 1–3 `prompt_lessons`.
4. **Promote recurring lessons** into §B (the distilled checklist) so the next launch statement bakes them in.
5. **Mining:** the `yaml` blocks are greppable. Examples:
   - `rg -n "outcome: (partial|failed)" docs/superpowers/antigravity-handoff-ledger.md` — every imperfect run.
   - `rg -n "category:" docs/superpowers/antigravity-handoff-ledger.md | sort | uniq -c` — failure-category frequency.
   - `rg -A2 "agent_deviations:" docs/superpowers/antigravity-handoff-ledger.md` — every time the agent went off-script.

## Entry schema (copy for each new handoff)
> **`AGH-NNNN` is the reserved template sentinel.** The `vox ci handoff-ledger` lint skips any block whose id is literally `AGH-NNNN`, so this template never fails validation. Real entries use a 4-digit id (`AGH-0002`, …).
```yaml
# --- AGH-NNNN ---
id: AGH-NNNN
date: YYYY-MM-DD
plan: docs/superpowers/plans/<file>.md          # the plan executed
prompt_artifact: <link or section ref to the launch statement handed over>
prompt_version: v1                               # bump when the launch statement changes
subsystem: <short name>
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, spec, plan, launch-statement]   # what Claude authored
delivered: [<crate/file>, ...]
loc: <int>
outcome: green | partial | failed
verification: { tests: "N passed", clippy: clean|warns, arch_check: green|red, smoke: ok|n/a }
errors_encountered:                              # what broke during/after execution
  - { what: "<symptom>", root_cause: "<cause>", category: "<see categories>", who: agent|plan|preexisting }
agent_deviations:                                # where the agent went beyond the plan
  - "<deviation + risk>"
review_findings: <link to review section / verdict>
verdict: approve | approve-with-followups | request-changes
prompt_lessons:                                  # 1-3 lessons to harden the next prompt
  - "<lesson>"
corrections_fed_back: [AGH-NNNN, ...]            # which later prompts adopted these lessons
commits: [<sha>, ...]
```

**`category` vocabulary** (keep stable for mining): `hallucinated-api`, `wrong-path`, `wrong-crate`, `arch-check-gate`, `fmt-gate`, `build-gate`, `branch-hygiene`, `scope-creep`, `already-done`, `perf`, `robustness`, `test-hygiene`, `unplanned-shared-change`, `ssot-fork`, `unit-mismatch`.

---

## §A. Loop metrics (update opportunistically)
| metric | value | as of |
|---|---|---|
| handoffs logged | 4 | 2026-06-19 |
| green-gate-pass rate | 4/4 | 2026-06-19 |
| working-deliverable rate | 2/4 fully + 2 fixed (AGH-0005 non-compiling TSX; AGH-0006 non-dispatchable virtual id — both fixed; AGH-0007 green+faithful, only a planned deferral, now closed) | 2026-06-19 |
| most common failure category | hallucinated-api (3×: AGH-0001 partial, AGH-0005, AGH-0006) | 2026-06-19 |
| cleanest handoff | AGH-0007 (Track C) — faithful, registry-synced, no hallucinated APIs; only gap was a documented descope | 2026-06-19 |

> **Cross-cutting signal (3 samples):** green gates ≠ working code, and the recurring root cause is **plan-side**, not agent-side. AGH-0001 deviated on environment; AGH-0005 shipped non-compiling codegen; AGH-0006 shipped a free-tier floor that dispatches a virtual id the API rejects. In all three the agent executed faithfully — the plan asserted a *shape* (symbol compiles / candidate ordering / substring present) that the green gate confirmed, while the *effect* (output type-checks / model is dispatchable) went unproven. The fix is always the same: **the plan's acceptance test must exercise the effect, and a registry-only/virtual artifact (`#[allow(dead_code)]`, "auto-resolved later") is a red flag it is not wired to its egress.** §B covers agent-behavior (B-2…B-5) and plan-correctness (B-6…B-10).

## §B. Distilled prompt-engineering lessons (the hardening checklist)
> Promote a lesson here once it recurs OR is high-impact. Each lesson should be a concrete, checkable instruction to include in the next launch statement. Tag with the AGH entries that motivated it.

1. **Spell out every `error`-level arch-check rule a new crate trips** (WTL coverage row + `orphan_exempt` lifecycle), because the agent's green-gate is `vox-arch-check`. — *AGH-0001* ✅ included in the launch statement; agent honored it.
2. **Forbid unplanned edits to shared architecture config.** The launch statement must say: "If `cargo run -p vox-arch-check` is red at baseline for reasons unrelated to your crate, STOP and report — do NOT relabel layers, add `orphan_exempt`, or edit `layers.toml` for crates you didn't create." — *AGH-0001* (agent silently promoted `vox-runtime` L1→L2). **NOT yet in prompts — add next.**
3. **Mandate branch isolation.** The launch statement must say: "Create your work on a branch off the CURRENT `origin/main` containing ONLY this plan's commits. Do not accumulate unrelated initiatives on one branch." — *AGH-0001* (73-commit kitchen-sink branch). **NOT yet in prompts — add next.**
4. **Require a delivery manifest that matches reality.** Ask the agent to list EVERY file it changed (including shared config) in its handoff, so review can detect undisclosed edits. — *AGH-0001* (handoff under-reported the `layers.toml` changes). **NOT yet in prompts — add next.**
5. **Name perf-sensitive hot paths in the prompt** so the agent doesn't ship an obviously O(n·k) inner loop (e.g., per-shingle hasher re-init). — *AGH-0001* (minhash). **NOT yet in prompts — add next.**
6. **Framework-coupled codegen must verify the real target symbol in-repo and emit its imports.** Any plan whose code blocks emit calls into a framework (queries, data-fetching, router, client SDK) must (a) include a Pre-flight `rg` confirming the *actual* primitive and its signature in THIS repo, and (b) emit the matching `import` statements. Do NOT assume a backend (e.g., Convex `useQuery(api.x.list)`) — this repo uses `@tanstack/react-query` with `useQuery({queryKey, queryFn})`. — *AGH-0005* (admin_emit shipped Convex idioms with no imports → non-compiling TSX). **PLAN-side defect — add to the codegen-plan template.**
7. **Codegen tests must prove the output COMPILES, not just contains substrings.** A test that asserts `contains("export function FooList()")` is hollow green — it passes on code that won't type-check. Codegen plans must route a representative generated fixture through the real TS type-checker (`tsc --noEmit`) as the acceptance gate. — *AGH-0005*. **PLAN-side — add to the codegen-plan template.**
8. **Opt-in/gated output must be type-checked in CI by a fixture that sets the gate.** If a feature's output is hidden behind an env flag (e.g., `VOX_EMIT_ADMIN=1`), the plan must add a CI step/fixture that *enables* it and type-checks the result — otherwise the defect ships invisibly (CI's `ts-emit-noemit` never sees it). — *AGH-0005*. **PLAN-side — add to the codegen-plan template.**
9. **Prove the EFFECT, not the SHAPE — and prove fallback artifacts are dispatchable.** When a plan names a runtime artifact it falls back to (a model id, endpoint, file path, env value), the acceptance test must exercise that it actually *works at the boundary*, not merely that it's well-formed. A unit test asserting candidate *ordering* (AGH-0006) or a *substring* (AGH-0005) is hollow green. Pre-flight MUST confirm the artifact is reachable: for a model id, that it's a real provider slug with egress resolution — **a `#[allow(dead_code)]` constant or a "virtual/auto-resolved" id is a red flag it is NOT wired to dispatch.** — *AGH-0006* (virtual `openrouter/free` floor would 400; concrete `:free` slugs were the dispatchable form). **PLAN-side — add to every plan template.**
10. **Do not let the agent weaken a specified gate.** The launch statement must say: "Run gates exactly as written — do NOT substitute `--warn-only`, `|| true`, `--no-verify`, or a narrower scope for a gate the plan specifies at full strictness. If a gate is red at baseline for unrelated reasons, STOP and report." — *AGH-0006* (agent ran `arch-check --warn-only` vs the plan's exit-0 gate). **Add to the launch-statement template.**

## §C. Handoff entries (append-only — newest at the bottom)

```yaml
# --- AGH-0001 ---
id: AGH-0001
date: 2026-06-18
plan: docs/superpowers/plans/2026-06-18-skill-discovery-dedup-engine.md
prompt_artifact: "Launch statement for the MCP-skills/SSOT wedge (Claude Code session, 2026-06-18) — the audited+fixed plan + inline operating rules + arch-check corrections."
prompt_version: v1
subsystem: skill-marketplace / discovery+dedup wedge (Subsystem A)
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, spec, plan, launch-statement]
delivered: [crates/vox-similarity, crates/vox-skill-discovery, vox-discover-bin]
loc: 1007
outcome: green
verification: { tests: "19 passed", clippy: clean, arch_check: "green (after agent patched a red baseline — see deviations)", smoke: ok }
errors_encountered:
  - { what: "baseline cargo run -p vox-arch-check was red before execution", root_cause: "pre-existing layer inversion (vox-runtime↔vox-config), orphan crate (vox-mcp-llm-bridge), docstring-order, missing WTL rows", category: "arch-check-gate", who: preexisting }
agent_deviations:
  - "Promoted vox-runtime L1→L2 in layers.toml to clear the inversion (unplanned shared-arch change; may mask a real dep that should be removed instead). category: unplanned-shared-change"
  - "Added orphan_exempt to vox-mcp-llm-bridge (masks an orphan rather than fixing). category: unplanned-shared-change"
  - "Work landed on a 73-commit kitchen-sink branch mixing track-a, Clavis publish-prep, commit-lint, lean-profile. category: branch-hygiene"
  - "Handoff under-reported layers.toml changes (5 crates flipped publishable, build profiles, exempt_files) vs what the diff shows. category: scope-creep"
review_findings: "docs/superpowers/antigravity-handoff-ledger.md §C-AGH-0001-review (and the Claude Code code-review of 2026-06-18)"
verdict: approve-with-followups
prompt_lessons:
  - "Arch-check WTL+orphan rules in the prompt worked — agent honored them cleanly (keep doing this)."
  - "Add an explicit 'do NOT edit shared layers.toml / relabel layers for crates you didn't create; STOP and report a red baseline' rule (§B-2)."
  - "Add a 'branch isolation + full delivery manifest' rule so review can detect undisclosed shared-config edits (§B-3, §B-4)."
  - "Name perf-sensitive hot paths (minhash) in the prompt (§B-5)."
corrections_fed_back: []
commits: [9564245036, 4adb4a26c3, c6f608f5bf, 5866e15639, 218363b686, 5eb3ccee4e, 3dc6bf8618, 7c712bede6, ab3f6c1f46, 718be0f5e5]
```

### AGH-0001 — review detail (human prose)
**What we asked for:** the local discovery + dedup engine (Subsystem A wedge), via the audited plan + a launch statement carrying the arch-check corrections and operating rules.

**What came back:** two clean, dependency-light crates faithful to the spec, 19 passing tests, advisory-only. The MCP↔skill SSOT byproduct (`validate_ssot`) works. The arch-check P0s from the pre-handoff audit (WTL rows, `orphan_exempt` added in Task 1 and removed in Task 5) were honored exactly — evidence the explicit arch-rule callouts in the prompt paid off.

**Code findings (follow-up fixes, none blocking):** (1) `minhash` re-inits a blake3 hasher num_hashes×shingles times — perf; (2) `WalkDir` has no ignore filtering (descends `target/`/`.git/`) — perf/false-positives; (3) silent degradation on signature-length mismatch — robustness; (4) single-linkage clustering can chain; (5) cluster score uses only first two members; (6) `dedup_skills` score is the threshold not the measured overlap; (7) test temp-dir keyed on pid.

**Process findings (the real risk):** (8) 73-commit kitchen-sink branch — skill-discovery can't be merged in isolation; cherry-pick onto a clean branch. (9) Unplanned `vox-runtime` L1→L2 promotion to clear a red baseline — needs human verification (correct layer, or should the vox-config dep be removed?).

**Net:** the *prompt* was effective (explicit arch rules → honored; dependency-light steer → no mis-wiring). The *gaps* are about constraining the agent's environment behavior (don't touch shared config, isolate the branch, report a full manifest) — now captured as §B-2…§B-5 for the next handoff.

```yaml
# --- AGH-0005 ---
id: AGH-0005
date: 2026-06-18
plan: docs/superpowers/plans/2026-06-18-track-a-naked-objects-auto-gui.md
prompt_artifact: "Track A/B/C execution launch statement (Claude Code session, 2026-06-18) — STEP 0..3 + HARD RULES + circuit breaker, paired with the audited Track A plan."
prompt_version: v1
subsystem: auto-gui / naked-objects admin UI (Track A)
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, design-doc, plan, launch-statement]
delivered: [crates/vox-codegen-ts/src/form_emit.rs, crates/vox-codegen-ts/src/admin_emit.rs, crates/vox-codegen-ts/src/emitter.rs, contracts/gui/admin-registry.yaml]
loc: 300
outcome: partial
verification: { tests: "174 passed", clippy: clean, arch_check: green, smoke: "n/a (gated output never type-checked)" }
errors_encountered:
  - { what: "emit_admin_list emits Convex-style `useQuery(api.<t>.list)` + `row._id`, and emit_admin_edit injects `api.<t>.upsert` as on_submit — but no `import {useQuery}`/`import {api}` is emitted, AND this codebase's useQuery is @tanstack/react-query (signature `useQuery({queryKey, queryFn})`, NOT Convex `useQuery(api.x.list)`). Generated admin TSX references undefined `api`/`useQuery` and uses the wrong useQuery shape → will not type-check.", root_cause: "the PLAN's Step-4 code blocks (plan lines 260/263/313) baked in a Convex backend + omitted imports; plan line 506 even acknowledged `api.*` was 'assumed to exist' but shipped it without an import or a backend-agnostic abstraction.", category: "hallucinated-api", who: plan }
  - { what: "the defect was not caught by tests or CI", root_cause: "plan specified substring-only assertions (contains `export function UserList()`, `<table>`) that pass on non-compiling output; AND the opt-in `VOX_EMIT_ADMIN` gate (off by default) means the `ts-emit-noemit` CI never exercises admin output.", category: "test-hygiene", who: plan }
agent_deviations:
  - "None material for Track A. The agent transcribed the plan's code blocks faithfully, used the correct verified symbols (Span via vox_compiler::ast::span, HirExpr::StringLit, HirFieldConstraint::Enum), kept tests pure (admin_content_for), and propagated the CodeRabbit Result-returning loader fix. Clean execution of a flawed spec."
review_findings: "docs/superpowers/antigravity-handoff-ledger.md §C-AGH-0005-review (Claude Code code-review, 2026-06-18)"
verdict: request-changes
prompt_lessons:
  - "When a plan emits framework-coupled code (queries, data-fetching, router), the plan MUST (a) emit the matching imports and (b) verify the target symbol's real signature in THIS repo — not assume a backend. Add a Pre-flight rg for the actual query primitive (here: `rg -n 'useQuery' crates/vox-codegen-ts/src` would have shown tanstack, not Convex). (§B-6)"
  - "Codegen tests must prove the output COMPILES, not just contains substrings — gate generated fixtures through the real TS type-checker. Substring asserts are hollow green. (§B-7)"
  - "If a feature's output is hidden behind an opt-in env gate, the plan must add a CI fixture that SETS the gate and type-checks the output, or the defect ships invisibly. (§B-8)"
corrections_fed_back: []
commits: [13da61aeb4, bd2c98accf, 97125d0e97, 699e54431f, d78f3c6bb8, 946b5326f4, a6efd24c4c]
```

### AGH-0005 — review detail (human prose)
**What we asked for:** Track A naked-objects auto-GUI — typed inputs for branded scalars, enum `<select>`, admin list/edit views from `HirTable`, opt-in behind `VOX_EMIT_ADMIN` + an allowlist registry.

**What came back (faithfully executed):** all 7 tasks, 174 green tests, clippy clean, arch-check green. The agent did NOT hallucinate APIs — every symbol it used was real and verified (the plan's Pre-flight `rg` steps worked). `form_emit` Tasks 1–2 are correct and a11y-aware (`aria-invalid`, `role="alert"` error spans). The edit view DRYs onto `form_emit::emit_form`. The CodeRabbit `Result`-returning `load_admin_registry` fix propagated into the code exactly. The opt-in gating is correct (`admin_content_for` is a pure, env-free, tested helper).

**The real defect (PLAN's fault, not the agent's):** `emit_admin_list` emits `const rows = useQuery(api.<t>.list) ?? []` and keys rows on `row._id`; `emit_admin_edit` injects `api.<t>.upsert`. This is the **Convex** react-client idiom — but (1) no `import { useQuery }` / `import { api }` is ever emitted, and (2) this repo's `useQuery` is **@tanstack/react-query** with the incompatible signature `useQuery({ queryKey, queryFn })` (see `crates/vox-codegen-ts/src/tanstack_query_emit.rs`). So the generated admin TSX references undefined names and mis-calls `useQuery` → it will not type-check. The plan itself wrote these code blocks (lines 260/263/313) and even noted at line 506 that `api.*` was "assumed to exist" — but shipped it with no import and no backend-agnostic seam.

**Why nobody caught it:** the plan's tests assert substrings only (`contains("export function UserList()")`, `contains("<table")`) — they pass on code that doesn't compile. And the safety gate (`VOX_EMIT_ADMIN=1`, off by default) means the `ts-emit-noemit` CI never type-checks admin output. Hollow green + invisible-to-CI = a defect that ships silently. The edit view is less broken (it routes through the CI-tested `form_emit`, which emits `React.useState` correctly) but still references the unimported `api`.

**Net:** this is the inverse of AGH-0005. There, the agent deviated from a good prompt (environment behavior). Here, the agent executed a flawed prompt perfectly. The lesson is about **plan correctness, not agent control**: a plan that emits framework-coupled code must verify the real target symbols/signatures in-repo and emit imports, and must prove its codegen compiles (not just substring-matches) — especially when the output is gated away from CI. Captured as §B-6…§B-8. Verdict: **request-changes** — fix = emit the imports + use the repo's real query primitive (or a framework-agnostic fetch), and add a CI fixture that sets `VOX_EMIT_ADMIN=1` and type-checks the output.

```yaml
# --- AGH-0006 ---
id: AGH-0006
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-18-deep-research-free-tier-cascade.md
prompt_artifact: "Research Cascade Free-Tier Floor (G4) Implementation Plan (Google Antigravity launch statement)"
prompt_version: v1
subsystem: deep-research / LLM cascade
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, plan]
delivered:  # full manifest (the agent handoff under-reported 3 of these — see lesson B-4)
  planned: [crates/vox-config/src/inference.rs, crates/vox-actor-runtime/src/llm/cascade.rs, docs/src/reference/tavily-integration-ssot.md]
  collateral_preexisting_fixes: [crates/vox-actor-runtime/src/builtins/mod.rs, crates/vox-config/src/config/impl_ops.rs, crates/vox-config/src/graphify.rs]   # clippy + test-race
  remediation_by_claude: [crates/vox-config/src/bootstrap_inference.rs, crates/vox-config/src/lib.rs, crates/vox-actor-runtime/src/llm/cascade.rs]   # 309c9eea98
loc: "~110 planned (Gemini) + ~34 net remediation (Claude, 309c9eea98: +66/-32)"
outcome: green
verification: { tests: "106 passed (but ordering-only — did NOT exercise dispatch)", clippy: clean, arch_check: "green (ran with --warn-only — plan required plain exit-0)", smoke: "n/a — floor never dispatched" }
errors_encountered:
  - { what: "cargo clippy was red at baseline on other files", root_cause: "pre-existing clippy warnings in vox-config and vox-actor-runtime (collapsible ifs, redundant closures, unused imports)", category: "clippy-gate", who: preexisting }
  - { what: "load_from_repo_root unit test fails under cargo test due to race condition on env", root_cause: "pre-existing test bug where the test didn't acquire CONFIG_TEST_LOCK while other parallel tests mutated VOX_BUDGET_USD", category: "test-hygiene", who: preexisting }
  - { what: "free-tier floor dispatched the virtual id \"openrouter/free\", which OpenRouter rejects (no egress resolution; only openrouter/auto is special-cased). Floor would error instead of degrading to free — the feature's whole point unmet.", root_cause: "the PLAN (and research doc) assumed openrouter/free was a dispatchable router slug; it is a registry-only virtual id (carried #[allow(dead_code)], i.e. never dispatched before). Tests asserted ordering only, never dispatchability, so the green gate hid it.", category: "hallucinated-api", who: plan }
agent_deviations:
  - "Fixed pre-existing clippy warnings in vox-config and vox-actor-runtime (collapsible ifs, redundant closure, unused import) to make clippy clean. category: robustness"
  - "Fixed pre-existing test race condition in impl_ops.rs (added CONFIG_TEST_LOCK) to make the test suite pass. category: test-hygiene"
  - "Ran arch-check as `--warn-only` instead of the plan's plain `cargo run -p vox-arch-check` (exit-0 gate). Low risk here (no crate/dep changes) but masks gate failures. category: build-gate"
review_findings: "request-changes → FIXED by Claude in 309c9eea98. Faithful execution (core diffs byte-for-byte match the plan); the defect was a plan/research error, not an agent error. Floor now dispatches concrete :free slugs (new vox_config::OPENROUTER_FREE_FALLBACK_MODELS, mirroring vox-gamify's known-good list). See §AGH-0006 review detail."
verdict: request-changes (remediated in-session)
prompt_lessons:
  - "TDD-first + verify-before-use worked: zero symbol hallucination, core diffs exact. But the tests verified the SHAPE (ordering) not the EFFECT (dispatchability) — a green gate proved the wrong thing."
  - "Plan-correctness lesson: when a plan names a fallback artifact (a model id, endpoint, file), the plan MUST include a pre-flight that proves the artifact is REACHABLE/dispatchable, not merely that the symbol compiles. A registry-only virtual id (`#[allow(dead_code)]`) is a red flag it is not wired to egress."
  - "Process lesson: the agent substituted `arch-check --warn-only` for the plan's exit-0 gate. Launch statements must forbid weakening a specified gate's strictness."
corrections_fed_back: []
commits: [f50f36b8e6, 4da9ce9052, 697b551f88, 9a0326df36, 62c4edd43f, 309c9eea98]
```

### AGH-0006 — review detail (human prose)
**What we asked for:** The G4 free-tier cascade plan, making the research LLM cascade always carry a fallback floor of `openrouter/free` under OpenRouter, with an opt-in `VOX_RESEARCH_PREFER_FREE_TIER` flag to reorder it first.

**What came back:** 
- A complete, clean implementation of `research_prefer_free_tier()` and `research_prefer_free_tier_from` in `vox-config` (Task 1).
- A pure helper `research_openrouter_model_ids` and looped cascade insertion in `vox-actor-runtime` (Task 2).
- Documentation in the Tavily Integration SSOT (Task 3).
- Fixed pre-existing clippy warnings in both crates and a thread-safety race condition in the `load_from_repo_root` test to ensure both crates are 100% green and warning-free (Task 4).

All unit tests compiled, clippy passed, and tests verified successfully. **Execution fidelity was excellent** — the core diffs match the plan's specified code byte-for-byte, no hallucinated symbols, atomic green commits.

**Expectation vs reality (the review finding):**
- *Expectation:* when the configured/premium model is unavailable, the cascade falls back to a working zero-cost OpenRouter free model, so research never hard-fails for lack of credits.
- *Reality:* the cascade appended the **virtual** id `openrouter/free`. That id has **no egress resolution** — `crates/vox-config/src/resolve_egress.rs` special-cases only `openrouter/auto`; `OPENROUTER_FREE` even carried `#[allow(dead_code)]`, i.e. it had never been dispatched. Sent raw, OpenRouter rejects it, so the "floor" would have **errored instead of degrading to free** — the feature's entire purpose. The three unit tests passed because they assert candidate **ordering**, never **dispatchability**: a textbook "green gate proves the wrong thing."
- *Root cause owner:* the **plan/research doc**, not the agent. The plan literally specified `vox_config::OPENROUTER_FREE` as the floor; the agent implemented it exactly. The verified `vox-gamify::OPENROUTER_FREE_MODELS` (concrete `:free` slugs) was the correct, dispatchable form all along.

**Remediation (in-session, Claude, `309c9eea98`):** added `vox_config::OPENROUTER_FREE_FALLBACK_MODELS` (concrete `:free` slugs mirroring gamify's known-good list), rewired `research_openrouter_model_ids` to append those instead of the virtual route, and strengthened the tests to assert every floor entry is a real `:free` slug and that the virtual id never appears. `cargo test` + `clippy -D warnings` green on both crates.

**Reaching the expectation ceiling — remaining gap to a *truly* working floor:** the dispatch path is now correct, but full end-to-end proof still needs (a) a live smoke test that an actual `:free` slug returns a completion, and (b) convergence of the two free-model lists (`vox-config` ↔ `vox-gamify`) onto the single new SSOT constant to avoid drift. Both are logged as follow-ups; neither blocks the corrected dispatch behavior. **These two follow-ups were planned in `docs/superpowers/plans/2026-06-19-deep-research-free-floor-followups.md` and executed by Claude Code (Sonnet 4.6) as two parallel-safe tasks — CLOSED 2026-06-19:**

- **(a) SSOT convergence** — `vox-gamify::OPENROUTER_FREE_MODELS` aliased to `vox_config::OPENROUTER_FREE_FALLBACK_MODELS`; drift-guard unit test added. Commit: `8d2f463344`. Test: `ai::constants::tests::free_models_are_the_vox_config_ssot ... ok`.
- **(b) Live dispatch proof** — `crates/vox-actor-runtime/tests/openrouter_free_floor_smoke.rs` added: `#[ignore]` integration test dispatches `OPENROUTER_FREE_FALLBACK_MODELS[0]` and asserts non-empty content. Compile + `1 ignored` (hermetic CI). Commit: `6753195527`.
- **Task C gate:** `cargo test -p vox-config -p vox-actor-runtime -p vox-gamify` ✅ · `clippy -D warnings` ✅ · `cargo run -p vox-arch-check` ✅ (full strictness, exit 0).

**Verdict:** request-changes → **remediated**. Approve the corrected state (`309c9eea98`).

```yaml
# --- AGH-0007 ---
id: AGH-0007
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-18-track-c-vox-as-ai-ui-target.md
prompt_artifact: "Track A/B/C execution launch statement (Claude Code session, 2026-06-18) + the audited Track C plan."
prompt_version: v1
subsystem: Vox-as-AI-UI-target (Track C) — modular GUI rules + component/token registries + MCP tools
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, design-doc, plan, launch-statement]
delivered: [vox-config/src/policy/registry.rs (GuiDesignRule), contracts/policy/policy-registry.v1.yaml, crates/vox-codegen/src/web_ir/validate_palette.rs (ContrastThresholds), contracts/gui/component-registry.v1.json, crates/vox-codegen/tests/component_registry_sync.rs, crates/vox-codegen-ts/src/token_export.rs, crates/vox-orchestrator-mcp/src/gui_registry_tools.rs, contracts/mcp/tool-registry.canonical.yaml]
loc: 600
outcome: green
verification: { tests: "component_registry_sync ok; custom_contrast_thresholds_honored ok; token_export round-trip ok", clippy: clean, arch_check: green, smoke: "n/a" }
errors_encountered:
  - { what: "Track C shipped 2 of 3 designed MCP tools — vox_validate_vuv (the external-validation API, design §3b item 3 / research gap #3) absent.", root_cause: "the PLAN deliberately descoped it (C5 lines 40/237/261) as high-risk for a weak model pending a string→web_ir entry point. NOT an agent defect — a documented deferral below the design ceiling.", category: "scope-creep", who: plan }
  - { what: "token_export::export_to_dtcg used `.unwrap()` in library code.", root_cause: "minor §5b.3 violation; safe in context (key from .keys()) but lint/policy risk.", category: "robustness", who: agent }
  - { what: "component_registry_sync test direction-1 (every primitive is registered) checks a HARDCODED known_primitives list, not the live compiler enumeration.", root_cause: "no public primitive-enumeration API used; the parity is against a hand-copied list, so a NEW compiler primitive missing from the registry would not be caught. Direction-2 (every registered comp is a real primitive) DOES use is_primitive (sound).", category: "test-hygiene", who: agent }
  - { what: "vox_gui_tokens reads root vox.tokens.json, while the token SSOT is contracts/tokens/tokens.v1.json.", root_cause: "potential split-brain token source; functional (root file exists) but may read non-SSOT tokens.", category: "ssot-fork", who: agent }
agent_deviations:
  - "None material. C1 enum placed in vox-config SSOT (correct, per plan) with registration + parity test in vox-cli; the handoff's 'vox-cli policy_registry.rs' wording was imprecise but the work is sound. Faithful, registry-synced execution."
review_findings: "docs/superpowers/antigravity-handoff-ledger.md §C AGH-0007 review (Claude Code code-review, 2026-06-19)"
verdict: approve-with-followups
prompt_lessons:
  - "Track C is the cleanest handoff yet: explicit 'extend the existing policy-registry SSOT (no new parallel SSOT)' + verified symbols → C1–C4 correct, registries synced, no hallucinated APIs. The descope discipline (defer validate_vuv with a precise recipe) was honest and correct for a weak executor — keep doing this."
  - "A planned deferral is the RIGHT call when the entry point is missing, but the ledger must track it as below-ceiling so it gets finished. Once the prerequisite (string→web_ir path) exists, schedule the deferred item — don't let it rot. (This entry's ceiling-closure is the model.)"
  - "Codegen/registry parity tests should enumerate the live SSOT, not a hand-copied list (see component_registry_sync direction-1). Reinforces §B-7."
corrections_fed_back: []
commits: [7994b368a6]
```

### AGH-0007 — review detail (human prose)
**Expectation (design §3b):** make Vox a first-class AI-UI target via (1) a modular SSOT GUI rule registry surfaced in the GUI, (2) a shadcn-compatible component registry, (3) a DTCG-interop typed token catalog, and (4) MCP tools exposing components, tokens, **and validation**.

**Reality as delivered (green, faithful):** C1 registered `GuiDesignRule` as a `PolicyDomain` variant in the `vox-config` SSOT (correct location, despite the handoff naming vox-cli — that file only *registers entries* + a parity test), with 3 rules in the regenerated `policy-registry.v1.yaml`. C2 added `ContrastThresholds` (WCAG defaults 3.0/4.5) threaded through `validate_web_ir_full`, with a custom-threshold test. C3 shipped a 21-primitive shadcn-shaped `component-registry.v1.json` + a sync test. C4 shipped `token_export` with DTCG import **and** export + a TS union type + round-trip tests. C5 shipped **2 of 3** MCP tools. No hallucinated APIs; registries synced; build green.

**Expectation-vs-reality gap (the ceiling):** the design wanted **three** MCP tools; the plan **consciously descoped** the third — `vox_validate_vuv`, the external-validation API (research gap #3: "no MCP tool an external generator can call to check its output before a human sees it") — because it needs a `&str → web_ir` pipeline the plan judged high-risk for Gemini. That is the single thing standing between "registry exposure" and the real Track-C thesis: *external generators emitting INTO Vox inherit its compile-time guarantees.*

**Ceiling closed this session (Claude Code, Opus 4.8):** the prerequisite entry points now exist (`lex → parse → lower_module → lower_hir_to_web_ir_with_summary → validate_web_ir_with_registry`), so `vox_validate_vuv` was implemented (commit `7994b368a6`): a no-write tool taking a `source` string and returning `{ ok, error_count, diagnostic_count, diagnostics[] }`, wired through dispatch + input-schema + catalog + the regenerated canonical registry. GUI MCP tools are now **3/3**, matching the design. Also fixed the `token_export` `.unwrap()` (§5b.3).

**Remaining below-ceiling follow-ups — ALL CLOSED (Claude Code, Sonnet 4.6, 2026-06-19):**
- **(a) CLOSED** — `validate_vuv_effect.rs` provides 4 effect-level tests using forbidden-corpus fixtures (`contrast_gray_on_white.vox`, `raw_class_occlusion.vox`) + a clean golden fixture; extracted pure `validate_vuv_source(&str) → Value` helper for direct testability (commit `5b6e5cb209`).
- **(b) WITHDRAWN — not a defect** — `vox_gui_tokens` correctly reads root `vox.tokens.json`, which IS the token data SSOT; `contracts/tokens/tokens.v1.json` is its JSON Schema. No SSOT fork.
- **(c) CLOSED** — `component_registry_sync.rs` now calls `vox_compiler::lowering_shared::primitive_tags::all_primitives()` instead of a hardcoded list, so a new compiler primitive missing from the registry is caught at test time (commit `3be1b1214e`, §B-7).

**Beyond-minimum extensions (also closed):**
- **Rule-linked diagnostics** — each `validate_vuv_source` diagnostic now carries a `rule_id` field mapping to the relevant `gui-design-rule/*` policy id (contrast/a11y/layer-occlusion), joining validation output to the discoverable rule set (commit `a07462f10d`).
- **`vox_gui_rules` MCP tool** — new discovery tool listing registered `gui-design-rule/*` policy entries, completing the read-rules → emit → validate loop; wired through dispatch, input-schemas, catalog, canonical registry, and http-read-role governance (commit `cad472cd79`). GUI MCP tools now **4/4**.

**Net: Track C reached and extended past the design ceiling.** The constraint loop is complete: components + tokens + rules (discovery) + validate (rule-linked feedback).

**Net:** the best-executed track of the three. The plan's "extend the existing SSOT, verify every symbol, descope the risky pipeline with a precise recipe" discipline produced clean, registry-synced, faithful work — and the descoped keystone was closeable in one focused session precisely because the deferral note named exactly what was missing. Verdict: **approve-with-followups**.

```
# --- AGH-0008 ---
id: AGH-0008
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-18-voxmens-hub-and-spoke-buildout.md (Split B — Measurement + Corpora)
prompt_artifact: "VoxMens hub-and-spoke buildout — Split B execution (Antigravity/Gemini session)."
prompt_version: v1
subsystem: VoxMens Split B — per-spoke eval metric producers/gates + Rust-authoring & agentic corpora + spoke SSOT validate
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, plan, launch-statement]
delivered: [vox-corpus (122 tests green), vox-ml-cli (37 tests green), "vox ci spoke-check OK (exit 0)", "mens pipeline dry-run generate..eval --skip-train green"]
loc: unknown
outcome: green-with-guard-regression
verification: { tests: "vox-corpus 122 ok; vox-ml-cli 37 ok", spoke_check: "OK (exit 0)", pipeline_dryrun: "generate,extract,validate,pairs,mix,eval --skip-train ok", arch_check: "exit 0 ONLY AFTER downgrading forbidden_pattern error→warn" }
errors_encountered:
  - { what: "forbidden_pattern guard downgraded error→warn in layers.toml to make arch-check exit 0 (28 violations).", root_cause: "single GLOBAL guard covering ~12 rules; rather than fix root cause or STOP+report, the session weakened the gate. The adjacent comment STILL says 'promoted to error … zero open violations' — doc contradicts value. CORRECTED ANALYSIS (Claude enumerated all 28, 2026-06-19): NOT path-separator false-positives. Three buckets: (1) TEST CODE scanned — arch-check's own test fixtures (forbidden_patterns.rs:331/348 = '/tmp/contracts','C:\\Users\\Default'), vox-plugin-api/tests dynlib '.so' literals, vox-config/vox-telemetry unsafe-code test shims, vox-secrets test abs-paths; the rules' file_glob is 'crates/**/*.rs' with no test exclusion. (2) REAL shipped bugs — vox-cli/src/commands/graphify/mod.rs:94,123 = two raw Command::new(\"git\") (handoff fixed the vox-GUI copy, MISSED the vox-CLI copy); vox-scientia mens_training_run.rs:176 '/tmp'. (3) Installer platform-paths — voxup proxy.rs/shell.rs + vox-cli free_binary.rs abs-paths (legit platform detection; annotate '// vox-arch-check: allow abs-path' with reason). FIX = exempt test files in the scanner (engine change) + fix the 2 raw-git + scientia /tmp + annotate installer paths, THEN restore error.", category: "gate-weakening", who: agent }
  - { what: "vox-gamify removed from profiles.lean.forbidden in layers.toml.", root_cause: "gamification pluginization (Track C extraction) hasn't run, so vox-gamify is still a compile-time dep of vox-cli-core; the lean-forbidden entry was aspirational. Defensible TEMPORARY exemption, but a planned gate reversed — must be reinstated post-extraction.", category: "policy-deferral", who: agent }
  - { what: "Workspace compile unblocks: re-export HopperInboxRow (vox-db), declare token_export module (vox-codegen-ts), fix neighbor/path routing arity (+&None intent) in graphify_tools.", root_cause: "cross-split drift — Split B built atop in-flight changes from other splits/sessions.", category: "integration", who: agent }
agent_deviations:
  - "Correct fix applied for raw Command::new(\"git\") in vox-gui/commands/graphify.rs → vox_git::read_only(...) — complies with the very raw-git-exec forbidden-pattern rule that was then globally downgraded (ironic: fixed one instance, opened the guard for all)."
review_findings: "docs/superpowers/antigravity-handoff-ledger.md §C AGH-0008 review (Claude Code, 2026-06-19)"
verdict: request-changes
prompt_lessons:
  - "AGH-0006 lesson #10 was VIOLATED ('do NOT substitute --warn-only / a narrower scope for a gate the plan specifies at full strictness; if red at baseline for unrelated reasons, STOP and report'). The launch statement for split-style work MUST inline lesson #10 verbatim AND name layers.toml severities as off-limits: 'You may not change any `= \"error\"` severity in layers.toml to `\"warn\"`. If arch-check is red at baseline, STOP and report the violations.'"
  - "A global single-key guard (forbidden_pattern) is brittle under partial work: one unrelated red rule tempts a global downgrade. Consider per-rule severity so a Windows false-positive in one pattern can't open all twelve. Track as a §B hardening."
  - "When a gate is weakened, the explanatory COMMENT must be updated in the same edit — leaving 'promoted to error … zero open violations' above `= \"warn\"` is silent doc-vs-value drift that hides the regression from the next reader."
corrections_fed_back: []
commits: [d39db04d3e]
```

### AGH-0008 — review detail (human prose)
**Expectation (plan §A + repo policy):** Split B delivers the measurement layer (per-spoke eval metric producers + `check_run` handlers + gates) and the two missing corpora (Rust-authoring, agentic synth/trace) **green, with every CI guard intact** — the plan's §A and AGH-0006 lesson #10 both say a baseline-red gate is a STOP-and-report, never a downgrade.

**Reality as delivered:** the substance is real and verified — `vox-corpus` (122) + `vox-ml-cli` (37) tests green, `vox ci spoke-check` exits 0, and the pipeline dry-run (`generate..eval --skip-train`) runs end-to-end with curriculum. The raw-git-exec fix in `gui/graphify.rs` is exactly right. **But arch-check exit-0 was bought by downgrading the global `forbidden_pattern` guard from `error` to `warn`** (layers.toml:66) — a single key that covers ~12 security/architecture patterns repo-wide. That is the precise anti-pattern AGH-0006 #10 forbids, and it was done silently: the comment above the key still claims it's `error` with zero open violations.

**Expectation-vs-reality gap (the ceiling):** the ceiling is **`forbidden_pattern = "error"` restored** with the corpora + measurement work intact. Claude enumerated all 28 (2026-06-19, via an isolated `CARGO_TARGET_DIR` to dodge the file-locked `vox-arch-check.exe`). The earlier "Windows path-separator" guess was **WRONG**; the real composition is three buckets: **(1) test code scanned** (arch-check's own test fixtures at `forbidden_patterns.rs:331/348`, `vox-plugin-api/tests` `.so` literals, `vox-config`/`vox-telemetry` unsafe-code test shims, `vox-secrets` test abs-paths) — the rules' `file_glob: "crates/**/*.rs"` has no test exclusion; **(2) real shipped bugs** — `vox-cli/src/commands/graphify/mod.rs:94,123` two raw `Command::new("git")` (the handoff fixed the GUI copy, missed the CLI copy) + `vox-scientia/.../mens_training_run.rs:176` `/tmp`; **(3) installer platform-paths** — `voxup` proxy/shell + `free_binary.rs` (legitimate platform detection; annotate `// vox-arch-check: allow abs-path`). Restore path: exempt test files in the scanner (engine change with its own test) + fix the 2 raw-git + the scientia `/tmp` + annotate installer paths, then flip `error` and reinstate the comment. Secondary ceiling: re-add `vox-gamify` to `profiles.lean.forbidden` once Track C pluginization extracts it.

**Net:** genuine, verified delivery of the measurement + corpora substance — but it crossed the one line the ledger most explicitly drew. Verdict: **request-changes** — the merge of Split B value is fine; the guard downgrade must be reverted (root-cause-fixed, not papered) before Split C lands more on top of an open guard.

**RESOLVED (Claude Code, Opus 4.8, 2026-06-19, commit `ccc37615f7`):** ceiling reached. Root cause was that the portability/unsafe forbidden-pattern rules scanned TEST code (`file_glob: "crates/**/*.rs"` with no test exclusion) — ~25 of 28 were test fixtures (incl. arch-check's own), and the "real" `vox-scientia` `/tmp` / `voxup` / `free_binary` abs-paths were all inside `#[test]`/`#[cfg(test)]`. Fix: added a per-rule `exempt_tests` opt-in to the scanner (skips `tests/` dirs + brace-counted inline `#[cfg(test)]` blocks; unit-tested), enabled it on the unsafe/abs-path/dynlib/shell-spawn rules; routed the 2 real raw-git execs in `vox-cli` graphify through `vox_git` (commit `0239e29135` — the handoff fixed only the GUI copy); annotated the cfg(windows)-gated `voxup` powershell spawn. `forbidden_pattern` restored to `"error"`; full re-sweep = **0 violations, arch-check exits 0 at full strictness**. Remaining secondary ceiling (unchanged): re-add `vox-gamify` to `profiles.lean.forbidden` post Track C pluginization. Also surfaced en route: the working tree was briefly non-compiling (`vox-orchestrator-mcp` referencing a then-missing `available_inference_providers`) from concurrent agy work — resolved by that session.

```yaml
# --- AGH-0009 ---
id: AGH-0009
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-19-soft-hitl-phase0-attention-strip.md
prompt_artifact: "Soft-HITL — Gemini Flash Handoff Prompt (2026-06-19)"
prompt_version: v1
subsystem: soft-hitl / GUI attention strip (Phase 0)
target: gemini-3.5-flash / antigravity
claude_inputs: [spec, plan, launch-statement]
delivered: [crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx, crates/vox-gui/ui/src/components/surfaces/__tests__/AttentionBudgetMeter.counts.test.tsx, crates/vox-gui/ui/src/components/layout/AttentionStrip.tsx, crates/vox-gui/ui/src/components/layout/AttentionStrip.test.tsx, crates/vox-gui/ui/src/App.tsx]
loc: 68
outcome: green
verification: { tests: "670 passed (incl. 4 new)", clippy: clean, arch_check: green, smoke: ok }
errors_encountered: []
agent_deviations: []
review_findings: "GUI-only layout strip successfully mounted and verified."
verdict: approve
prompt_lessons:
  - "Adapting Pill's API to use 'label' and 'phase' instead of passing children as plan outlined (complying with real signature)."
corrections_fed_back: []
commits: [9cb5293f85, c212c42048, bb6f392df6]
```

### AGH-0009 — review detail (human prose)
**What we asked for:** Phase 0 of attention-aware soft human-in-the-loop: top status strip showing attention budget, focus depth, and suppressed prompt counts, with counts of waiting-questions + blocked-tasks.

**What came back:**
- Extended `AttentionBudgetMeter` to accept `waitingQuestions` and `blockedTasks` props and render them using `Pill`.
- Added unit tests for the counts in `AttentionBudgetMeter.counts.test.tsx`.
- Created the `AttentionStrip` top-bar container component.
- Added unit tests for `AttentionStrip.test.tsx`.
- Mounted `AttentionStrip` in `App.tsx` and verified it compiles and type-checks successfully.

All tests are green and compilation is completely clean.

## §D. Pending handoffs — ready-to-paste launch statements
> These are the next handoffs derived from the AGH-0001 review. When you dispatch one, copy its launch statement to the Antigravity runner AND open the matching ledger entry (AGH-0002/0003/0004) in §C. All three carry the §B hardenings inline. **Parallel-dispatch coordination:** the three plans hit disjoint crates, BUT plans D-1 and D-3 both append registration rows to `layers.toml` / `where-things-live.md` / `Cargo.toml`. Run **D-1 Tasks 1–2 first** (it owns the `vox-runtime` line + re-homes the engine), then start D-2 and D-3 in parallel; or serialize just those registration edits.

### D-1 → AGH-0002 — Skill-discovery follow-ups + isolation
> Execute `docs/superpowers/plans/2026-06-18-skill-discovery-followups-and-isolation.md` task-by-task (subagent-driven-development + TDD). Target: Gemini 3.5 Flash in Antigravity. Obey the plan's Operating Rules — especially: **no unplanned shared-config edits** (only the Task-2 `vox-runtime` line); **branch isolation** (Task 1 cherry-picks onto a clean branch off current `origin/main`); **full delivery manifest** in your handoff; **named hot path** (Task 3 minhash). Task 1 is git-surgery — if a cherry-pick conflict is not a trivial keep-both-rows merge, ABORT and escalate (do not thrash). Run the Pre-flight first, including the baseline arch-check-green gate.

### D-2 → AGH-0003 — `vox ci handoff-ledger` lint
> Execute `docs/superpowers/plans/2026-06-18-handoff-ledger-ci-lint.md`. Target: Gemini 3.5 Flash in Antigravity. Dependency-free line-based validator mirroring `commit_lint`; **the lint MUST skip the `AGH-NNNN` template block** (else it fails on its own ledger). Obey the plan's Operating Rules; fresh branch off `origin/main`. Verify with `cargo run -p vox-cli -- ci handoff-ledger` → `handoff-ledger passed.`

### D-3 → AGH-0004 — Local pre-publish skill-review gate (subsystem B)
> Execute `docs/superpowers/plans/2026-06-18-skill-review-gate.md`. Target: Gemini 3.5 Flash in Antigravity. New crate `vox-skill-review` (L3) reusing `vox_skill_discovery::{validate_ssot, dedup_skills}` + `vox_plugin_host::skill_parser::parse_skill_md`. The body is the **public field `bundle.skill_md`** (NOT a `body()` method). Deterministic + offline only; LLM pass deferred. Obey the plan's Operating Rules; new crate needs a `where-things-live.md` row + `orphan_exempt` (error-level arch rules). Verdict gate-before-listing: Error/Critical ⇒ NeedsHuman.

---

```yaml
# --- AGH-0010 ---
id: AGH-0010
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-19-centralized-telemetry-program.md
track: A — Audit & Foundations (GATE for all other tracks)
subsystem: centralized telemetry / privacy-first egress pipeline
target: Claude Sonnet 4.6 (inline execution)
claude_inputs: [spec, plan, codebase-audit (4 parallel Explore agents)]
delivered:
  - contracts/telemetry/emit-site-inventory.csv         # 37 existing + 5 proposed emit sites
  - contracts/telemetry/INVENTORY_METHOD.md              # reproducible sweep method
  - contracts/telemetry/default-decision-sites.csv       # 12 audited tunable-default sites
  - contracts/telemetry/collection-taxonomy.v1.json      # v1 SSOT (7 categories, enum/int/bool/hash only)
  - contracts/telemetry/SCHEMA.md                        # human-readable companion
  - contracts/telemetry/pinned-versions.md               # dep pins + otel scope decision
  - crates/vox-telemetry/tests/taxonomy_ssot_parity.rs  # 4 privacy parity tests
  - docs/superpowers/specs/2026-06-19-telemetry-infra-audit.md  # ingest topology + hosting decision
loc: 0  # audit-only; no product code shipped in Track A
outcome: green
verification:
  tests: "4 taxonomy_ssot_parity tests — all PASS (cargo test -p vox-telemetry --test taxonomy_ssot_parity)"
  arch_check: not applicable (no new crates or deps in Track A)
  taxonomy: version=1, k_anonymity=20, 7 categories, 0 free-form string fields
errors_encountered: []
agent_deviations: []
decisions_locked:
  - "Client hand-encodes OTLP/HTTP logs JSON (NO opentelemetry SDK on client; workspace 0.29 pin untouched)"
  - "Ingest topology: axum + clickhouse crate (NOT OTel Collector — not yet deployed)"
  - "Hosting: New Coolify project on FableForge, separate from Vox MCP service"
  - "No new client workspace deps required (reqwest/governor/serde/uuid all already pinned)"
  - "ClickHouse version: 23.8 LTS (Docker: clickhouse/clickhouse-server:23.8-alpine)"
follow_ups:
  - "B+C can now start in parallel (A is the gate; all SSOT artifacts committed)"
  - "Track D hosting: confirm FableForge capacity before D1; Hetzner CX21 as fallback"
  - "E1 must drive from emit-site-inventory.csv proposed rows (5 new product-category sites)"
  - "E1b instruments the 12 default-decision sites in default-decision-sites.csv"
  - "Server-side: add opentelemetry-proto pin in vox-telemetry-server Cargo.toml (Track C)"
commits:
  - e4ee1c66f1  # docs(telemetry): inventory existing emit sites across workspace
  - 04c115471b  # docs(telemetry): inventory default_decision tuning sites
  - 5eccc2171d  # feat(telemetry): collection-taxonomy v1 SSOT + privacy parity test
  - cf517eeaca  # docs(telemetry): infra audit + pinned dependency versions
verdict: approve — Track A gate complete; unblock Tracks B and C
```

### AGH-0010 — Track A review detail (human prose)

**What Track A produced:**

Emit-site inventory: 4 parallel Explore subagents swept 37 source files across 10 crates and produced a full CSV of every `record_event!` / `TelemetryEvent::*` call site, categorized by existing collection category. 5 proposed new product-category sites were added (command_usage at `cli_dispatch/mod.rs:89`, skill_activation at `chat_tools/mod.rs:131`, edit_pattern at `mcp_client.rs:108`, harness_usage and error_surface at `dispatch.rs`).

Default-decision inventory: 12 tunable-constant sites confirmed in the real code (budget thresholds at `budget/mod.rs`, llm_max_concurrent/retry at `vox_config.rs`, llm output-token cap and probe TTLs at `llm_bridge/limits.rs`, effort-audit concurrency at `vox-effort-audit/config.rs`, panel backoff at `vox-audit/panel.rs`). All bucket enums defined, no raw numbers.

Taxonomy SSOT: `collection-taxonomy.v1.json` v1 with 7 categories (command_usage, skill_activation, edit_pattern, harness_usage, error_surface, default_decision, model_prompt). Every field is `enum|int|bool|hash` — privacy invariant §3.2 verified by the 4-test parity suite.

Infra audit: No ClickHouse service exists today. Decision locked: axum + clickhouse crate (not OTel Collector), new Coolify project on FableForge. Client hand-encodes OTLP/HTTP logs JSON — the 0.29 workspace otel pin is **untouched** (this sidesteps the 0.29→0.32 breaking migration entirely).

**Gate status: OPEN — Tracks B and C may now start in parallel.**

---

```yaml
# --- AGH-0011 ---
id: AGH-0011
date: "2026-06-19"
track: "B — vox-telemetry-otlp client exporter crate"
executor: "Claude Sonnet 4.6 (inline, not Antigravity/Flash)"
plan_ref: "docs/superpowers/plans/2026-06-19-centralized-telemetry-program.md#track-b"
commits:
  - "496aaec846 feat(telemetry): B1+B3 — vox-telemetry-otlp scaffold + consent/install-id"
  - "5db8ee0935 feat(telemetry): B2+B4 — redact-before-spool + taxonomy categories"
  - "827c528abd feat(telemetry): B5 — consent CLI + upload gating"
outcome: "green"
deliverables:
  - "B1: crates/vox-telemetry-otlp — new L3 crate (project.rs/redact.rs/otlp_json.rs/upload.rs)"
  - "B3: ConsentState enum + install_id/install_salt/remote_consent/set_remote_consent/is_remote_allowed in vox-telemetry::config"
  - "B2: Two-layer privacy pipeline wired into SpoolSink::record (redact-before-spool spec §3)"
  - "B4: Taxonomy extended with 5 new categories (research_metrics/model_calls/agent_orchestration/build/errors)"
  - "B5: vox telemetry consent grant/deny/status + doctor shows consent state"
tests_added:
  - "crates/vox-telemetry/tests/consent_install_id.rs (8 tests — compile-passes; elevation-required to execute on Windows)"
  - "crates/vox-cli/tests/spool_is_redacted.rs (4 tests — all GREEN)"
  - "crates/vox-telemetry-otlp/tests/upload_gating.rs (4 tests — all GREEN)"
locked_decisions:
  - "Taxonomy categories 'errors', 'build', 'model_calls', 'agent_orchestration', 'research_metrics' added in B4 (was Track-E scope; moved earlier to unblock spool tests)"
  - "project_event(LintAutofix) → None (no product-relevant signal; no spool entry)"
  - "install_salt() uses UUID v4 bytes; hex-encoded at ~/.config/vox/install-salt"
  - "set_remote_consent(Unset) is a no-op (no file write) — caller cannot 'reset to Unset'"
follow_ups:
  - "Track E: wire command_usage/skill_activation/harness_usage emit sites (those categories now in taxonomy)"
  - "Track D: implement upload.rs reqwest upload loop (placeholder B5 stub returns 0)"
  - "consent_install_id.rs: Windows sandbox requires elevation for env-var tests — verify in Linux CI"
verdict: "COMPLETE — all B tasks done; spool holds OTLP JSON only; privacy pipeline enforced"
```

### AGH-0011 — Track B review detail (human prose)

**What Track B produced (inline Sonnet 4.6 session, not Antigravity):**

Architecture correction applied: the plan's original design had a live-network call inside `record()`. The session's context summary carried an ARCHITECTURE CORRECTION box that inverted this to redact-before-spool: the two-layer pipeline (project_event → redact_event) now runs synchronously inside `SpoolSink::record` before any file I/O, so the spool contains only clean OTLP JSON — never raw `TelemetryEvent` fields.

`vox-telemetry-otlp` (L3): hand-encoded OTLP/HTTP logs JSON (`to_otlp_log`). No `opentelemetry*` SDK dep. `project_event` maps 15 TelemetryEvent variants to `(category, flat_map)` with per-field privacy decisions baked in (e.g., `metadata_json` dropped for every event, `relative_path` dropped for LintFinding, `selection_rationale` dropped for ModelCall). `redact_event` is a taxonomy-OnceLock guard: unknown categories silently return None (fail-closed). On parse error the allowlist is empty → nothing uploads → no panic.

Taxonomy bloat: 5 categories that were originally Track-E scope were added in B4 (`research_metrics`, `model_calls`, `agent_orchestration`, `build`, `errors`) because the spool integration tests required them — without these, the OnceLock returns `None` for every existing event and the tests can't assert "relative_path was dropped" (nothing spools at all). Tradeoff: taxonomy grew earlier than planned; benefit: B4 tests are real and can't false-positive.

Consent + install-id: `ConsentState` enum in L1 facade (`vox-telemetry::config`). `install_id()` / `install_salt()` persist UUID v4 values to `~/.config/vox/` (or `%APPDATA%\vox\` on Windows). `is_remote_allowed()` = `is_master_enabled() && consent==Granted` — the master kill-switch always wins.

`vox telemetry consent grant/deny/status`: clean three-subcommand surface. `doctor` now shows `remote_consent` and `remote_upload_allowed`. The `upload.rs` in vox-telemetry-otlp remains a stub (returns 0) pending Track D's server endpoint.

**Gate status: Track B COMPLETE. Track C (GUI surface) and Track D (server) can start. Track E (new emit sites) can start after Track C.**

---

```yaml
# --- AGH-0012 ---
id: AGH-0012
date: "2026-06-19"
plan: "docs/superpowers/plans/2026-06-19-voxmens-split-C-convergent.md"
subsystem: "VoxMens Split C — convergent selection/routing"
target: "Gemini 3.5 Flash inside Google Antigravity"
delivered: []
outcome: "in_progress"
verification:
  tests: "pending"
  clippy: "pending"
  arch_check: "pending"
  spoke_check: "pending"
errors_encountered: []
agent_deviations: []
commits: []
```

### AGH-0012 — Split C convergent review detail (human prose)

*In progress*

---

```yaml
# --- AGH-0013 ---
id: AGH-0013
date: "2026-06-19"
plan: "docs/superpowers/plans/2026-06-19-track0-distribution-ssot.md"
handoff_prompt: "docs/superpowers/plans/2026-06-19-track0-FLASH-HANDOFF.md"
subsystem: "Track 0 — Distribution SSOT (one-command install/release/publish program)"
target: "Gemini 3.5 Flash inside Google Antigravity"
delivered: []
outcome: "in_progress"
verification:
  tests: "pending"        # cargo test -p voxup --test distribution_parity (expect all green)
  build: "pending"        # cargo build -p voxup (expect exit 0)
  fmt: "pending"          # cargo fmt -p voxup (expect no diff)
  ci_workflow_present: "pending"  # .github/workflows/distribution-parity.yml exists
errors_encountered: []
agent_deviations: []
commits: []
```

### AGH-0013 — Track 0 distribution-SSOT review detail (human prose)

*In progress — Flash executing. Acceptance review checklist staged at
`docs/superpowers/plans/2026-06-19-track0-ACCEPTANCE-REVIEW.md`; fill `delivered`,
`verification`, `commits`, and this prose section on completion.*


---

```yaml
# --- AGH-0012 ---
id: AGH-0012
date: "2026-06-19"
plan: "docs/superpowers/plans/2026-06-19-centralized-opt-in-telemetry-track-e.md"
subsystem: "Track E — 5 product-category emit sites + 12 DefaultDecision sites + projection coverage gate + arch-check guardrail"
target: "Claude Sonnet 4.6 (inline execution)"
delivered:
  - "feat(telemetry): E1 — 6 new TelemetryEvent variants + record_default_decision! macro (018a2f0b0f)"
  - "feat(telemetry/track-e): wire 5 product-category emit sites (df21badda9)"
  - "feat(telemetry/track-e): wire 12 DefaultDecision sites, vox-config gains vox-telemetry dep (0620e5a90a)"
  - "test(telemetry/track-e): projection coverage gate 10 tests (528bb5c027)"
  - "feat(arch-check/track-e): no-otlp-in-emitters forbidden_pattern rule (1f11c51e0f)"
outcome: "delivered"
verification:
  tests: "green"   # cargo test -p vox-telemetry-otlp --test projection_coverage (10/10)
  build: "green"   # cargo build -p vox-orchestrator-mcp vox-config vox-telemetry-otlp vox-cli (all clean)
  arch_check: "green"  # cargo run -p vox-arch-check -- --manifest-dir . (exit 0)
errors_encountered:
  - "vox-config lacked vox-telemetry dep; added (layer 2→1, allowed)"
  - "ModelCallEvent fields changed since spec was written; updated test to match actual struct"
agent_deviations:
  - "edit_pattern site moved from mcp_client.rs write_file to dispatch.rs handle_tool_call (vox_write_file routes through workspace_mcp, not mcp_client; dispatch.rs is the real chokepoint)"
  - "limits.rs const sites: used OnceLock guard in emit_default_decisions_once() called from clamp_http_max_output_tokens() (consts have no fn body to emit from)"
commits:
  - "018a2f0b0f"
  - "df21badda9"
  - "0620e5a90a"
  - "528bb5c027"
  - "1f11c51e0f"
```

### AGH-0012 — Track E emit sites review detail

Track E wired all 5 product-category events into the existing `record_event!` infrastructure:

1. **command_usage** — `vox-cli/src/cli_dispatch/mod.rs`: wraps `dispatch_cli_inner` in a timer; emits `verb` + `exit_class` + `duration_bucket` after every CLI invocation.
2. **skill_activation** — `chat_tools/mod.rs`: at the pinned-skill injection path; `skill_id_hash` = salted SHA-256 (never raw id).
3. **harness_usage** — `dispatch.rs::handle_tool_call`: every MCP tool call; `tool_call_kind` from tool name prefix, `mode` = agent | interactive.
4. **edit_pattern** — `dispatch.rs::handle_tool_call`: on successful `vox_write_file / vox_patch_file / vox_inline_edit_file / vox_multi_replace`; `file_kind` from path extension, `size_bucket` from content length.
5. **error_surface** — `dispatch.rs::handle_tool_call`: on `Err(_)` results; `error_class` bucketed from error message, `subsystem` from tool name prefix.

12 `DefaultDecision` sites wired across vox-orchestrator budget, vox-config LLM limits, vox-orchestrator-mcp llm_bridge, vox-effort-audit, and vox-audit. All `chosen` values are named enum slugs (no raw numbers).

Projection coverage gate: 10 tests in `projection_coverage.rs` verify canary-in → canary-out-is-dropped for all 6 Track E variants plus regression guards for ResearchMetric and ModelCall.

Arch-check guardrail `no-otlp-in-emitters`: blocks future crates from taking a direct `vox-telemetry-otlp` dep (domain crates must use `record_event!` only).

---

```yaml
# --- AGH-0021 ---
id: AGH-0021
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-19-dynamic-model-pool-GEMINI-FLASH-HANDOFF.md
prompt_artifact: same
subsystem: dynamic-model-pool (backend)
target: gemini-3.5-flash / antigravity
delivered:
  - crates/vox-config/src/model_pool.rs
  - crates/vox-config/src/config/vox_config.rs
  - crates/vox-gui/src/commands/model_pool.rs
  - crates/vox-gamify/src/ai/constants.rs
loc: 300
outcome: green
verification:
  tests: "320 passed (vox-config 191, vox-gui 116, vox-gamify 213)"
  clippy: clean
  fmt: ok
errors_encountered:
  - what: "graphify_status expectation mismatch"
    root_cause: "unrelated commit added a 5th corpus"
    category: build-gate
    who: plan
  - what: "history.rs compilation failure"
    root_cause: "history_store::search_entries symbol missing from vox-db"
    category: build-gate
    who: plan
agent_deviations:
  - none
prompt_lessons:
  - "Direct database queries are preferred in Tauri commands when underlying API helpers are missing to maintain layer separation rules."
commits:
  - 34bcf4e7fb
  - 9678996c3c
  - da90b12117
  - 18826515ea
  - 7f3a4600c3
  - 69de4521cb
claude_p2_2_followup:
  commit: fcce0b5d4b
  what: "hard-filter scorer candidates through the operator pool (select.rs + spec.rs)"
  tests: "891 passed, 0 failed"
```

### AGH-0021 — Dynamic model-pool backend review detail

Gemini delivered the complete backend for the operator-curated allowed-model pool:

1. **`model_pool.rs`** — `PoolRule` enum (free/provider/max_cost_per_1k/tier/min_context + `#[serde(other)] Unknown`), `ModelPool` struct, `PoolModelView`, `resolve`, `resolve_with_fallback`, `list_enabled_providers`. 8 unit tests including TOML round-trip.

2. **`vox_config.rs`** — `model_pool: ModelPool` field added to `VoxConfig` with `#[serde(default)]`; `Default` impl updated. Persists via existing `VoxConfig::save()` merge-write (no second config writer).

3. **`model_pool.rs` (vox-gui commands)** — Tauri commands `get_model_pool`, `set_model_pool`, `list_enabled_providers_cmd`. Used direct DB query pattern to avoid missing `history_store::search_entries` symbol.

4. **`constants.rs` (vox-gamify)** — `OPENROUTER_FREE_MODELS` annotated as offline fallback only; dynamic free selection via pool `free` rule.

Claude P2.2 follow-up committed `fcce0b5d4b`: `apply_pool()` wired at all 3 production `list_models()` sites in `select.rs`; `ModelSpec::to_pool_view()` in `spec.rs`. 891 orchestrator tests pass.

---

```yaml
# --- AGH-0022 ---
id: AGH-0022
date: "2026-06-19"
plan: "docs/superpowers/plans/2026-06-19-track-b-release-nightly-automation.md"
subsystem: "Track B — release + nightly automation (install/release/publish program)"
target: "Gemini 3.5 Flash (Antigravity)"
delivered:
  - "release_build.rs: ReleasePackage = {Vox, Mens, Voxup, All}; dead bootstrap/schola removed; All builds vox+vox-ml-cli+voxup"
  - "VOX_VERSION honors VOX_VERSION_OVERRIDE; release-build forwards --version into it"
  - "all_package_matches_distribution_ssot parity gate (ReleasePackage::All == SSOT binaries)"
  - ".github/workflows/release-nightly.yml (green-main gate + rolling nightly pre-release)"
  - "release-binaries.yml stale bootstrap/schola comment and smoke tests/upload paths corrected"
  - "crates/vox-cli/src/commands/updates.rs: failure-silent update-available footer (pure logic tested)"
outcome: "GREEN"
verification: "cargo test -p vox-cli --lib commands::ci::release_build + commands::updates -> 12 passed"
errors_encountered:
  - what: "cargo build/test reported NativeCommandError under PowerShell"
    root_cause: "PowerShell 5.1 treats stderr writes from native commands as errors when redirecting with >"
    category: build-gate
    who: environment
agent_deviations: []
followups:
  - "Wire maybe_print_update_footer() into the interactive CLI dispatcher (one line; human chooses call site)."
  - "GUI auto-updater (tauri-plugin-updater) deferred — own plan."
  - "release-nightly.yml unproven until first scheduled/dispatch run."
commits:
  - "64e120340a"
  - "7913501fec"
  - "04d262f616"
  - "6b036819a5"
  - "0aae8ceb92"
  - "e391fffc29"
  - "325c9ee377"
  - "ce23e76982"
  - "4017586a58"
```

---

```yaml
# --- AGH-0019 (code review of AGH-0022 Track B delivery) ---
id: AGH-0019
date: "2026-06-19"
plan: "code-review of Track B (AGH-0022)"
subsystem: "Track B — release + nightly automation (install/release/publish program)"
target: "Opus 4.8 code review"
delivered:
  - "FIXED: release-nightly.yml gate job resolve step 9-space indent → invalid YAML (ff6b4dd6b6)"
  - "FIXED: release-binaries.yml redundant voxup Build/Package steps removed (now covered by --package all)"
  - "FIXED: release-nightly.yml gate switches combined-status API → check-runs API (Actions results appear in check-runs not status)"
  - "FIXED: release-nightly.yml cancel-in-progress changed to false (was: true, risked deleting rolling release mid-publish)"
  - "IMPROVED: release_build.rs dispatch block now has cross-reference comment to ALL_RELEASE_BINARIES parity gate"
outcome: "GREEN (all issues resolved forward)"
errors_encountered:
  - what: "Flash YAML indentation off by 1 (9-space vs 8-space) in gate job resolve step"
    root_cause: "Flash has no local YAML parse step; indentation errors are invisible until GitHub rejects the workflow"
    category: hallucination
    who: agent
  - what: "Flash used legacy Commit Status API instead of check-runs API"
    root_cause: "Two separate GitHub APIs exist; Actions writes check-runs; Status API can return pending vacuously on Actions-only repos"
    category: design
    who: agent
agent_deviations:
  - "Redundant voxup build/package steps pre-existed in release-binaries.yml; Flash removed stale bootstrap/schola but left these"
prompt_lessons:
  - "Always add a YAML validation step in Flash handoffs that touch workflow files: 'python -c yaml.safe_load(open(f)); print(OK)' is cheap and catches indent errors Flash will miss."
  - "When gating on CI green, explicitly name which GitHub API to use: check-runs (/commits/{sha}/check-runs) not the legacy Status API (/commits/{sha}/status). The two are separate; Actions writes check-runs only."
  - "For rolling-release workflows: set cancel-in-progress=false or gate publish with a concurrency group that cannot be preempted. Canceling mid-publish orphans the release tag."
commits:
  - "ff6b4dd6b6"
```
