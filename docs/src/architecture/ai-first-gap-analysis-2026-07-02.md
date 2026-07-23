---
title: "AI-First Gap Analysis: Language, CLI, and GUI (Journey-Spine)"
description: "Adversarially audited gap analysis scoring Vox against an AI-as-author target state across the intent → authorship → verification → operation → comprehension journey. Every claim verified against the codebase by independent refuters (2026-07-02); produces three implementation plans plus three follow-on spec candidates."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
authored: "2026-07-02"
---

# AI-First Gap Analysis: Language, CLI, and GUI

## Framing

**Primary thesis (ratified):** Vox is AI-first in the sense that **AI is the author** — the language, compiler output, CLI, and tooling should optimize for machine authorship: deterministic structured output, machine-checkable contracts, self-describing errors, and graph-queryable semantics.

**GUI purpose ranking (ratified):** if AI writes the code, the GUI exists — in priority order — for:

1. **B — Direction & intent.** The human expresses *what* they want; agents figure out how.
2. **A — Review & trust.** The human verifies and approves what agents wrote.
3. **C — Observation & operations.** Mission-control over agents working.
4. **D — Comprehension.** Understanding a codebase the human didn't write.

**Key observation:** today's GUI nav is organized roughly C > D > B > A. The ratified [gui-ia-blueprint.md](../../agents/gui-ia-blueprint.md) fixes taxonomy hygiene but does not re-center the nav on intent; this analysis supplies the ordering rationale and extends the blueprint.

## Audit provenance (read this before trusting older docs)

Every claim below was adversarially verified against the codebase on 2026-07-02 (10 independent agents: 5 refuters over 17 claims, 5 gap-hunters). Five widely-repeated claims from earlier docs are **false against current code** — do not re-import them:

| Stale claim (source) | Reality |
|---|---|
| "@ai structured_output parsed but never codegen'd" | It IS emitted — `crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:8-31,299-301` builds a `json_schema` response_format, with tests in `crates/vox-codegen/tests/ai_structured_output_emit.rs`. Real gap is narrower (G-A1 below). |
| "@subagent/@hole/@search/@prompt not in the lexer" | All four tokens exist: `crates/vox-compiler/src/lexer/token.rs:222-229`. Gaps are semantic, downstream (G-L2/G-L3). |
| "StreamCard Doubt/Overrule buttons are dead" (gui-honesty-triage.md:64-65) | Wired end-to-end since triage: `App.tsx:947-959` → `doubt_orchestrator_task`/`overrule_orchestrator_task` (`control_plane.rs:148,169`). Triage record is stale. |
| "Graphify not wired into vox-search / no MCP serve" | Both wired: `vox-search/src/bundle.rs:36-38` + `execution.rs:689-728` (GraphifyStructural corpus); MCP tools `vox_search_status/structural/neighbors` in `vox-orchestrator-mcp/src/graph_tools.rs` + `dispatch.rs:647-655`. |
| "Skill sandbox is structural-only, ungated" | `SandboxedSkillRunner::run` (`vox-skills/src/sandbox/runner.rs:106-203`) spawns real docker/podman with cap-drop/read-only/memory limits and gates the MCP `vox_skill_run` path (`skills_tools.rs:419-421`). Residual gaps in G-A6. |

Also adjusted: `needs-you` is wired to `vox_feedback_list`/`vox_resolve_feedback` (NOT `vox_resolve_approval` — that belongs to MissionControl/Approvals; blueprint line 34 conflates them); attention polling is **four** pollers at 2s/2s/5s/60s, not three at ~10s; the blueprint is ratified in its body (§0, 2026-06-26) but its YAML frontmatter still says PRE-RATIFICATION.

## The Journey Spine

**Intent → Authorship → Verification → Operation → Comprehension.** Every capability and GUI surface is scored by the stage it serves; surfaces serving no stage are cut candidates.

---

## Stage 1 — Intent (rank 1; weakest today)

**Ideal:** goals, constraints, budget, and acceptance criteria are expressed in structured, durable form; agents decompose them; intent objects persist and are traceable spec → plan → diff → merge.

**Exists:** Chat/Loquela composer (plain text); `vox plan` CLI (`commands/plan.rs` — but Create only prints JSON to stdout, Status prints "not fully wired"); `@form`; Matrix surface (single mutating command `nudge_routing_intention`, Matrix.tsx:95).

| ID | Gap | Severity |
|----|-----|----------|
| G-I1 | No durable intent object — intent lives in chat transcripts; nothing in vox-db links spec → plan → diff. | High |
| G-I2 | Composer is a bare text box: no goal/constraints/budget/acceptance fields. | High |
| G-I3 | `vox plan` output is surfaced nowhere in the GUI (verified: no vox-gui consumer of PlanNode; the PhaseChip "Approve Plan" button surfaces daemon task phases, not `vox plan`). | Medium |

---

## Stage 2 — Authorship (the AI-as-author core)

### 2a. Language semantics — the flagship constructs are shallower than they look

**Exists and real:** effects system (12 variants incl. per-tool `Mcp(String)`); phonetic operators; `Id[T]`; `Result` exhaustiveness; decorator tokens for all AI fixtures; fixtures catalog `contracts/agentos/ai-first-fixtures.v1.yaml`; isolation tiers (`vox run --isolation` wasm/container/bounded-fs).

| ID | Gap | Severity |
|----|-----|----------|
| G-L1 | `@ai` structured output is **name-only**: the emitted `json_schema` payload carries just `{"name": "<ReturnType>"}` — no schema body derived from the return type (`ai_fixture/llm.rs:25-31`). The model is never actually constrained to the declared shape. | **Critical** |
| G-L2 | `@ai` has **no prompt channel**: the prompt is hardcoded as "Implement the function: {name}" + Debug-printed params (`ai_fixture/llm.rs:33-46`); HirFn carries no doc-comment field; `@prompt` only selects a cascade stage. The function's semantic contract is inferred from its identifier alone. | **Critical** |
| G-L3 | No `llm`/`ai` effect: `EffectAnnotation` has no LLM variant, and the network call is injected at codegen *after* effect checking — an `@ai` function type-checks as effect-free. The flagship safety primitive can't see the flagship AI construct. | High |
| G-L4 | Budget annotations are parsed then **silently dropped**: `cost_ceiling_usd_per_call`, `@subagent(budget_usd)`, `ai_max_iterations` exist in HIR/AST but zero codegen reads them (grep confirms). Declared bounds don't hold; no diagnostic says so. | High |
| G-L5 | Model pins are unvalidated free-form strings: `@ai(model = "gpt-4o-mimi")` compiles clean and fails at runtime; a runtime ModelRegistryEntry registry exists but typeck never consults it. Violates the language's own typo-rejection principle. | Medium |
| G-L6 | Grammar-constrained decoding (`vox-constrained-gen`, real Earley/PDA logit masking) is **wired into no in-repo sampler**: both mens candle plugins explicitly exclude it, the CI smoke is a placeholder, and vox-populi serving uses vLLM `guided_json` instead. "Invalid Vox cannot be sampled" is aspiration, not deployment. | High |

### 2b. The agent's edit-build-test loop (toolchain)

**Exists:** `vox check --output-format json` emitting `VoxCompilerDiagnosticPayload`; global `--json` → `VOX_CLI_GLOBAL_JSON` (read by exactly two commands: check, catalog); diagnostics code registry (prefix reservations).

| ID | Gap | Severity |
|----|-----|----------|
| G-T1 | Build lane has no structured output: `vox build`/`run`/`test` define no JSON flags at all (cli_args.rs:42-163). | High |
| G-T2 | No per-test results anywhere: `vox test` shells `cargo test` with inherited stdio and collapses to exit-code prose (`test.rs:46-72`); watch mode discards each run's Result (`let _ = run_once(...)`). An agent cannot learn *which* test failed. ~192 report schemas exist in contracts/reports/ — none for a test run of generated code. | **Critical** |
| G-T3 | Test failures point at generated Rust (`target/generated/src/lib.rs:412`) with **no source-map back to `.vox`** — every failure iteration pays a reverse-engineering tax on code the agent never wrote. | High |
| G-T4 | Advertised test affordances are no-ops: `--coverage` dead-ends at raw .profraw (no llvm-cov step — while the repo's own CI has full llvm-cov+gates plumbing for itself); `--update-snapshots` sets INSTA_UPDATE for a generated crate whose dev-deps contain no insta/expect-test; `--forall-iterations` sets an env var nothing reads (emit hardcodes `ProptestConfig::with_cases` from the annotation). Worse than absent: agents reading --help will trust them. | High |
| G-T5 | No exit-code taxonomy: every failure exits 1 via anyhow (main.rs:57); `vox test` even discards cargo's compile-fail vs test-fail distinction. Agents must regex prose to branch "fix code" vs "tool broke". | Medium |
| G-T6 | Single-entry-file lane: `vox check`/`test`/`fmt` each require exactly one file (`#[arg(required = true)] file`); no project-wide "verify everything I touched" gate; no repo-wide `vox fmt --check` for .vox at all. | Medium |
| G-T7 | `vox doctor` has no `--diag <id>` filter despite emitting `[diag id=..]` tags (DoctorArgs verified flag-by-flag; repo-wide grep for `--diag` = zero). | Medium |
| G-T8 | Honesty detectors are **compiled out of the default binary**: stub-check is a non-default cargo feature; the entire lint_findings block of `vox check --for-llm` is `#[cfg(feature = "stub-check")]`. A default-build agent self-checks against nothing. | High |
| G-T9 | Diagnostics emit dead explain URLs on the wrong domain (`https://vox-lang.org/diag/{code}`; docs site is voxlang.org, no diag/ pages exist) and there is no `vox explain <code>`. Actively misleading to agents that follow it. | High |
| G-T10 | `vox repair` — the sanctioned self-heal loop — is hardwired to OpenRouter (bails without the key, raw reqwest to openrouter URL), bypassing `vox_actor_runtime::llm`; a dead parallel `autofix.rs` duplicates the violation. Recovery is unavailable offline/local-MENS. | Medium |
| G-T11 | `vox lsp` is a dead-end outside a dev checkout: the binary is never built/bundled/installed by any distribution path; the command bails "ensure vox-lsp is in your PATH". The working compiler-backed LSP is unreachable exactly where AI harnesses would consume it. | Medium |
| G-T12 | No deterministic replay/mock LLM provider: any `.vox` program containing `@ai` is untestable offline (no cassette, no VOX_LLM mock env; wiremock exists only as vox-llm-egress's internal dev-dep). An AI author cannot close its own verify loop on AI-bearing code. | High |

### 2c. Residuals on things that mostly work

- G-A6 (revised): skill sandbox is real and gates the MCP `vox_skill_run` path, **but** the CLI `vox skill run` ARS path bypasses it entirely (`ArsRuntime::execute_skill` is a stub echo), `ApprovalGuard`/`resolve_policy` run only in tests, and no test exercises actual container execution. | Medium
- G-A7 (unchanged): authorship→training flywheel open — no producer wires the miner to `vox_propose_skill` (SP-5); MENS corpus small. | Medium

---

## Stage 3 — Verification & review (rank 2; the gate exists but is blind, synchronous, and partly dangerous)

**Exists:** approvals HITL gate covering exactly the mutating tools; ACI mutation classifier; policy checks; doubt/overrule wired end-to-end (see audit table).

| ID | Gap | Severity |
|----|-----|----------|
| G-V1 | **The reviewer cannot see the change**: approval requests carry only `tool + summary` where summary is the args JSON truncated at 200 chars (`dispatch.rs:147-151`, `pending_approvals.rs:27-36`). A `vox_write_file` approval shows 200 chars of the content. `vox_snapshot_diff` exists as a tool; nothing in the Approvals surface uses it. The gate is blind rubber-stamping. | **Critical** |
| G-V2 | **Asynchronous review is impossible**: hard-coded `APPROVAL_TIMEOUT = 300s` auto-fails the parked call (`dispatch.rs:163-171`); the registry is in-memory, lost on restart. Review inboxes can only ever show items younger than 5 minutes — structurally incompatible with agents working while the human is away. | **Critical** |
| G-V3 | **"Modified" outcome is dangerous**: backend accepts it and executes the ORIGINAL args (`dispatch.rs:182-193` — "fall through and execute the tool", no channel for modified args); GUI never offers it; no comment/reason field exists anywhere in the resolve path, so rejected agents retry blind and corrections can't feed the flywheel (G-A7). | High |
| G-V4 | Risk class and cost are computed but never attached to approval **decisions**: ACI envelope is attached to results post-execution, not to pending approvals; no per-approval cost; no batch resolve (single `resolving` id). Reviewers can't triage; bursts make the human a serial bottleneck → blanket auto-approval pressure. | High |
| G-V5 | The GUI's only undo affordance is guaranteed to fail: `/rollback` invokes `vox_undo` with `{}` but `operation_id` is required (`oplog.rs:22-29`); success toast lies ("Reverted…"). No oplog list surface exists. Humans learn reversal is impossible → under-approval. | High |
| G-V6 | NeedsYou (feedback inbox, wired to `vox_feedback_list`/`vox_resolve_feedback`) is a nav orphan; Runs is worse than orphaned — its visible sub-tab **bounces** to Approvals (`resolveNavigation('runs')` → child 'approvals'). | High |
| G-V7 | Attention is fragmented across **four** pollers (approvals 2s in App + 2s in useAgentApprovals, feedback+hopper 5s, policy 60s) and four activity surfaces sharing one `activity_query`. No single "what needs me" number. | High |
| G-V8 | No provenance surface: agent-produced changes show no model, cost, reasoning trace, or link back to intent. (Depends on G-I1.) | Medium |

---

## Stage 4 — Operation (rank 3; strongest today, over-weighted)

**Exists:** Dashboard (live pool, stream, inline approvals, doubt/overrule), Flow, Mesh, Runs (real backend commands), telemetry, orchestrator policy modules.

| ID | Gap | Severity |
|----|-----|----------|
| G-O1 | Nav over-weights ops relative to intent/review; gamify sits under Agents. | Medium |
| G-O2 | Five parent-shell duplicate registry surfaces (agents/commands/compute/workspace/knowledge) mirror their default children. | Medium |

---

## Stage 5 — Comprehension (rank 4)

**Exists:** graphify wired into vox-search (GraphifyStructural corpus) AND served over MCP (`vox_search_structural`/`vox_search_neighbors`); VoxGraph surface; Memory search; Scientia.

| ID | Gap | Severity |
|----|-----|----------|
| G-C1 | The MCP-served agent discovery index (`resource://vox/llms.txt`) is hand-maintained, stale (last touch 2026-06-13), and split-brain with the site's auto-generated llms.txt — the one doc artifact exempt from the repo's own "generated, never hand-edited" regime, and it's the canonical first thing an agent reads. No generator, no drift gate. | High |
| G-C2 | `where-things-live.md` — the normative placement SSOT every authoring agent must consult — is machine-*validated* (arch-check hashes it) but not machine-*queryable*: no contracts/ projection, no `vox where <concept>`. The repo already proves the needed pattern (registry YAML + generated doc + drift gate) on the CLI surface. | Medium |
| G-C3 | Diagnostics registry (`contracts/diagnostics/registry.v1.yaml`) is prefix-reservations only; the actual code catalog lives solely in Rust source — invisible to non-compiling consumers. | Low |
| G-C4 | "Search" is a 1-child degenerate nav group; hollow Latin labels (scientia→Findings, oratio→Voice, mens→Training, populi→Nodes per blueprint). | Low |

---

## Section organization: the intent-first nav

Executes all ratified blueprint merges/cuts/renames, **plus** reorders top-level groups to match B > A > C > D:

| # | Group | Children | Journey stage |
|---|-------|----------|---------------|
| 1 | **Direct** | chat (matrix folded into chat rail) | Intent |
| 2 | **Review** | approvals, needs-you (promoted), runs (promoted), policies | Verification |
| 3 | **Agents** | dashboard, flow, tasks, mesh, sub-agents | Operation |
| 4 | **Knowledge** | memory, scientia→Findings, research, discovery (4→1 with presets), publications, vox-search | Comprehension |
| 5 | **Workspace** | console, repository, browser, harness | (supporting) |
| 6 | **Commands** | catalog, skills | (supporting) |
| 7 | **Compute** | models, mens→Training, populi→Nodes, oratio→Voice | (supporting) |
| 8 | **Settings** | settings, coverage, gamify (moved) | (supporting) |

Plus: remove the 5 parent-shell duplicates; one aggregated attention badge on **Review**; fix the blueprint's own frontmatter status to RATIFIED while touching it.

---

## Prioritized roadmap

| Order | Sub-project | Plan / spec | Gaps closed |
|-------|-------------|-------------|-------------|
| 1 | Language/toolchain AI-authorship | [ai-first-plan-1-language-toolchain-2026-07-02.md](ai-first-plan-1-language-toolchain-2026-07-02.md) | G-L1 (schema body), G-T1, G-T7 (+G-T9 if it fits) |
| 2 | GUI IA intent-first reorg | [ai-first-plan-2-gui-ia-reorg-2026-07-02.md](ai-first-plan-2-gui-ia-reorg-2026-07-02.md) | G-V6 (nav half), G-O1, G-O2, G-C4 |
| 3 | GUI intuitiveness | [ai-first-plan-3-gui-intuitiveness-2026-07-02.md](ai-first-plan-3-gui-intuitiveness-2026-07-02.md) | G-I2, G-V6 (inbox half), G-V7, honesty-triage burn-down |
| 4 | **Approval-gate integrity** (next spec — highest-value unplanned work) | needs own spec: diff payload on approvals, configurable/durable timeout, real Modified-args channel or removal, risk+cost columns, batch resolve, oplog-wired undo | G-V1–G-V5 |
| 5 | **Honest test loop** (next spec) | per-test structured results + .vox source-map + real coverage/snapshot/forall flags (or remove them) + exit-code taxonomy + stub-check in default build | G-T2–G-T6, G-T8 |
| 6 | **@ai semantic completion** (next spec) | schema-body emission beyond G-L1, prompt channel, llm effect, budget enforcement, model-pin validation, offline replay provider | G-L2–G-L6, G-T12 |

Deferred with pointers: G-I1/G-V8 (intent objects + provenance — one spec, after #4), G-A7 (SP-5 flywheel), G-T10/G-T11 (repair boundary + lsp distribution — small independent fixes), G-C1/G-C2 (llms.txt generator + `vox where` — small independent fixes).

Plans 1 and 2 are independent. Plan 3 shares files with Plan 2 (navigation.ts, Sidebar, registry YAML) — whichever lands second reconciles per the coordination notes embedded in Plan 3.

---

## Appendix: scorecard (post-audit)

| Area | Status | Evidence |
|------|--------|----------|
| Effects system | EXISTS | `vox-ast/src/decl/effect.rs` (but no llm effect — G-L3) |
| Structured diagnostics (check) | EXISTS | `vox check --output-format json`, VoxCompilerDiagnosticPayload |
| Diagnostics explain path | **BROKEN** | dead URLs, wrong domain, no `vox explain` (G-T9) |
| Rule pack (F1-scored) | EXISTS as crate / **OFF by default** | non-default cargo feature (G-T8) |
| Grammar-constrained decoding | LIBRARY ONLY | no in-repo sampler consumes it (G-L6) |
| Isolation tiers | EXISTS | `vox run --isolation` |
| Model-agnostic LLM boundary | EXISTS (with violations) | `vox repair` + autofix.rs bypass it (G-T10) |
| MCP tool exposure | EXISTS | 100+ tools; graphify served (`graph_tools.rs`) |
| `@ai` structured output | PARTIAL | name-only json_schema (G-L1) |
| `@ai` prompt/contract channel | MISSING | hardcoded name+Debug prompt (G-L2) |
| Budget/cost annotations | DECORATIVE | parsed, never enforced (G-L4) |
| Fixture decorators | TOKENS EXIST | semantic depth missing (G-L2) |
| CLI `--json` build lane | MISSING | G-T1 |
| Per-test structured results | MISSING | G-T2 |
| .vox source-mapped failures | MISSING | G-T3 |
| Coverage/snapshots/forall flags | NO-OPS | G-T4 |
| `vox doctor --diag` | MISSING | G-T7 |
| Offline LLM replay/mock | MISSING | G-T12 |
| Skill sandbox | EXISTS (MCP path) / BYPASSED (CLI ARS path) | G-A6 |
| Approval gate: visibility | **BLIND** | 200-char summary, no diff (G-V1) |
| Approval gate: async | **BROKEN** | 300s in-memory timeout (G-V2) |
| Approval gate: modify/comment | DANGEROUS/MISSING | executes original args (G-V3) |
| GUI undo | GUARANTEED-FAIL | vox_undo without operation_id (G-V5) |
| GUI intent capture | PARTIAL | plain-text composer (G-I2) |
| GUI review inbox | PARTIAL | orphans + 4 pollers (G-V6/G-V7) |
| GUI operations | EXISTS | Dashboard/Flow/Mesh/Runs |
| Graphify in search/MCP | EXISTS | corpus + MCP tools (audit-corrected) |
| Agent discovery index (llms.txt) | STALE/SPLIT-BRAIN | G-C1 |
| Placement SSOT queryability | MISSING | G-C2 |
| `vox lsp` distribution | MISSING | G-T11 |
