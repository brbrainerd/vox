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
| handoffs logged | 3 | 2026-06-19 |
| green-gate-pass rate | 3/3 | 2026-06-19 |
| working-deliverable rate | 1/3 (AGH-0005 emits non-compiling TSX; AGH-0006 green-gated but the free floor dispatched a non-dispatchable virtual id — both fixed) | 2026-06-19 |
| most common failure category | hallucinated-api (3×: AGH-0001 partial, AGH-0005, AGH-0006) | 2026-06-19 |

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
delivered: [crates/vox-config/src/inference.rs, crates/vox-actor-runtime/src/llm/cascade.rs, docs/src/reference/tavily-integration-ssot.md]
loc: 110
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

**Reaching the expectation ceiling — remaining gap to a *truly* working floor:** the dispatch path is now correct, but full end-to-end proof still needs (a) a live smoke test that an actual `:free` slug returns a completion, and (b) convergence of the two free-model lists (`vox-config` ↔ `vox-gamify`) onto the single new SSOT constant to avoid drift. Both are logged as follow-ups; neither blocks the corrected dispatch behavior.

**Verdict:** request-changes → **remediated**. Approve the corrected state (`309c9eea98`).

## §D. Pending handoffs — ready-to-paste launch statements
> These are the next handoffs derived from the AGH-0001 review. When you dispatch one, copy its launch statement to the Antigravity runner AND open the matching ledger entry (AGH-0002/0003/0004) in §C. All three carry the §B hardenings inline. **Parallel-dispatch coordination:** the three plans hit disjoint crates, BUT plans D-1 and D-3 both append registration rows to `layers.toml` / `where-things-live.md` / `Cargo.toml`. Run **D-1 Tasks 1–2 first** (it owns the `vox-runtime` line + re-homes the engine), then start D-2 and D-3 in parallel; or serialize just those registration edits.

### D-1 → AGH-0002 — Skill-discovery follow-ups + isolation
> Execute `docs/superpowers/plans/2026-06-18-skill-discovery-followups-and-isolation.md` task-by-task (subagent-driven-development + TDD). Target: Gemini 3.5 Flash in Antigravity. Obey the plan's Operating Rules — especially: **no unplanned shared-config edits** (only the Task-2 `vox-runtime` line); **branch isolation** (Task 1 cherry-picks onto a clean branch off current `origin/main`); **full delivery manifest** in your handoff; **named hot path** (Task 3 minhash). Task 1 is git-surgery — if a cherry-pick conflict is not a trivial keep-both-rows merge, ABORT and escalate (do not thrash). Run the Pre-flight first, including the baseline arch-check-green gate.

### D-2 → AGH-0003 — `vox ci handoff-ledger` lint
> Execute `docs/superpowers/plans/2026-06-18-handoff-ledger-ci-lint.md`. Target: Gemini 3.5 Flash in Antigravity. Dependency-free line-based validator mirroring `commit_lint`; **the lint MUST skip the `AGH-NNNN` template block** (else it fails on its own ledger). Obey the plan's Operating Rules; fresh branch off `origin/main`. Verify with `cargo run -p vox-cli -- ci handoff-ledger` → `handoff-ledger passed.`

### D-3 → AGH-0004 — Local pre-publish skill-review gate (subsystem B)
> Execute `docs/superpowers/plans/2026-06-18-skill-review-gate.md`. Target: Gemini 3.5 Flash in Antigravity. New crate `vox-skill-review` (L3) reusing `vox_skill_discovery::{validate_ssot, dedup_skills}` + `vox_plugin_host::skill_parser::parse_skill_md`. The body is the **public field `bundle.skill_md`** (NOT a `body()` method). Deterministic + offline only; LLM pass deferred. Obey the plan's Operating Rules; new crate needs a `where-things-live.md` row + `orphan_exempt` (error-level arch rules). Verdict gate-before-listing: Error/Critical ⇒ NeedsHuman.
