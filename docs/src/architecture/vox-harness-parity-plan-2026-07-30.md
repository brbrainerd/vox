---
title: "Vox Harness Parity Plan 2026-07-30"
description: "Sequenced remediation plan turning the eleven-document Claude-Code-parity research set into concrete Vox changes: fix the inert model scorer and the loop-less GUI chat first, then wire the model catalog to local inference, then the skill promotion/registry/provenance gate, then panel and noise-control UX, then the multi-sample eval gate — each phase scoped to what's already built versus what's net-new."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Vox Harness Parity Plan (2026-07-30)

> **Inputs.** This plan is the synthesis of eleven documents produced 2026-07-30: the
> graph-backed [audit](vox-harness-graph-audit-2026-07-30.md) (23 ranked findings, F1–F23),
> eight `deep-research` reports (Claude Code [mechanics](claude-code-harness-mechanics-2026-07-30.md),
> [routing](multi-provider-local-cloud-routing-research-2026-07-30.md),
> [skill induction](skill-discovery-and-induction-research-2026-07-30.md),
> [UX/noise](agent-chat-ux-and-noise-research-2026-07-30.md),
> [registry trust](skill-registry-trust-and-curation-research-2026-07-30.md),
> [testing](agent-harness-testing-and-regression-gating-research-2026-07-30.md),
> [tool comparison](coding-agent-local-model-ux-comparison-2026-07-30.md),
> [marketplace security](skill-marketplace-security-and-provenance-research-2026-07-30.md)),
> the [adversarial cross-check](harness-research-adversarial-crosscheck-2026-07-30.md), and the
> [gap-fill](harness-research-gap-fill-2026-07-30.md) (Aider/Zed/Cursor local-model mechanisms,
> hand-recovered from a run whose synthesis stage failed). Windsurf's local-model story remains
> unresearched after three attempts and is the one item in this whole set still fully open.
>
> **Framing, stated explicitly because it governs every phase below**: the goal is not to build
> what Claude Code has. It is to match its "it just works" *quality* — every failure mode
> handled by a specific, boring, verifiable mechanism (mechanics doc §10) — while **keeping and
> extending Vox's multi-provider, local-model-native capability**, which the tool-comparison
> research shows **no surveyed coding agent, including Claude Code, currently has** (Claude Code
> has zero local-model support; none of the three well-documented tools do hardware
> capability-gating). Vox is not behind the field on this axis. It is ahead, once the wiring is
> fixed.

---

## 0. The one-sentence diagnosis

**Vox already built most of what a Claude-Code-grade harness needs — a good multi-axis model
selector, a complete three-tier skill-disclosure system, a working MCP chat lane, an ARIA-aware
notification taxonomy — and then didn't connect the GUI chat window to any of it.** Nine of
thirteen routing-recipe elements are already built (routing doc §6). The skill system is fully
built and unreachable from the GUI (audit §2.1–2.2). The scorer that decides which model
answers is empirically inert regardless of input (audit §4A.2, reconfirmed four times in the
cross-check). **This is a wiring and correctness problem, not an architecture problem.** The
plan below is sequenced accordingly: fix what's broken before building what's missing.

---

## Phase 0 — Stop the bleeding (do not build on top of a broken selector)

**Goal:** the two defects that would turn every later phase into "wiring a broken thing more
places." Both are cheap to fix and expensive to leave broken.

### 0.1 Fix the inert model scorer — CRITICAL, blocks everything downstream

- **Finding:** F3a. `vox model explain` returns byte-identical top-5 rankings and selection for
  `"hi"` and a hard concurrency-design task, confirmed across **four independent probes**
  including explicit `--category codegen --complexity 9` (cross-check §2.1).
- **Root cause, to be isolated first** (audit §4A.2 leaves this open): is it
  `best_for_with_filter`, the unpopulated `Tier`/`pricing_source` metadata (every one of 364
  models reports `Tier: Unknown`), or the `explain.rs` display layer? **Do not guess — add a
  unit test that asserts the ranking changes between a trivial and a complex task, watch it
  fail, and bisect from there.**
- **Acceptance test, per the testing research (testing doc §7, item 4):** do not accept a single
  before/after run. The fix must be verified against **at least 5 distinct task descriptions
  spanning trivial→hard**, with the selection differing appropriately, before this item is
  closed.
- Wiring `models::decide` into GUI chat (Phase 1) **must not ship before this is fixed** — per
  the cross-check §1.1, shipping the wire-up first would make a broken selector look like a fix.

### 0.2 Defang the secretary — CRITICAL, the "doesn't chat properly" root cause

- **Finding:** F2. `secretary::classify` (secretary.rs:21) substring-matches 15 verbs
  ("add", "fix", "build"…) against any message ≥10 words, with a position-heuristic
  "confidence" (85/60) that is not a probability. "I already fixed it" or "why did you remove
  that?" silently becomes an orchestrator task.
- **Fix, informed by HAX G10** ("scope services when in doubt," UX doc §7): change the
  secretary from **auto-dispatch** to **propose-only** by default. The existing
  `SecretaryProposed` toast machinery already exists (`emit_secretary_proposed`) — repoint it so
  the task is created in a *pending* state requiring one click to confirm, not already fired.
  This directly matches the misalignment-taxonomy finding (UX doc §7A.1) that 91.49% of
  agent-side misalignment still requires explicit user correction — don't add a *second* source
  of unreviewed auto-action on the user-input side.
- Word-boundary match, not substring, as a minimum bar even for the propose-only version.
- **This is standalone and should ship independently of everything else** — it requires no other
  phase to be useful, and it is the single change most likely to make chat "feel" fixed.

---

## Phase 1 — Wire the chat window to the harness that already works

**Goal:** close the seven-node, depth-1, no-model-call reachability gap (audit §0) by routing
the GUI through the MCP chat lane that already does everything right, rather than rebuilding it.

### 1.1 Route GUI chat through `build_system_prompt_with_skill` + `models::decide`

- **Finding:** F1, F6. `build_system_prompt` (which assembles VOX.md, MEMORY.md, the Tier-1
  skill catalog, and pinned-skill injection) has **zero references** in `crates/vox-gui`. The
  MCP `chat_message` tool does this correctly today.
- **This is not "build a system prompt."** It is "call the function that already builds one."
  The three-tier skill disclosure (audit §2.1) is complete, cache-stable, and degrades
  gracefully to prompt-only models — better than needing to invent anything.
- Concretely: `chat_append_message` in `crates/vox-gui/src/commands/chat.rs` should route the
  actual model call through the same path `chat_tools/chat/message.rs::chat_message` uses,
  instead of only writing to the DB and firing the secretary.

### 1.2 Fix `"active_skill": null` and the nudge-only injection timing

- **Findings:** F5, F4. The chat submit payload hardcodes `active_skill: null`
  (`chat.rs:195`); separately, skill instructions are only injected on **idle-continuation
  nudges** (`monitor.rs:103`), never on initial dispatch.
- Fix both as one change: thread the GUI's pinned-skill selector (Loquela already has one —
  `Loquela.tsx:444`) through to the same `build_system_prompt_with_skill(pinned_skill: …)` call
  from 1.1, and move the `<active_skill>` injection to fire on **every** dispatch, not just
  nudges.

### 1.3 Add a task classifier before routing (closes the "no classifier anywhere" gap)

- **Finding:** F21. `TaskCategory` is only ever a hardcoded call-site constant
  (`models.rs:199` unconditionally passes `CodeGen`).
- Per the routing research's substitution recommendation (routing doc §2.4): Vox doesn't need
  OpenRouter's proprietary 30-type classifier. A cheap first pass — keyword/heuristic
  classification into Vox's existing `TaskCategory` enum, upgradable later to an LLM-classified
  pass — is sufficient to stop every chat message being scored as `CodeGen` complexity 5.

### 1.4 Ship `vox chat` as a CLI verb

- **Finding:** F22. There is no `vox chat`. The only two chat entry points are the GUI (broken
  until 1.1) and the MCP server (works, but requires an MCP client). A CLI verb is the cheapest
  way to get a human-reachable test surface for 1.1's acceptance testing, and it directly serves
  the testing doc's recommendation (§7, item 1) to eval against Vox's own real workload rather
  than a public benchmark.

---

## Phase 2 — Model catalog: add local inference, fix routing correctness

**Goal:** make good on "the same capacity that Ollama has, the ability to natively add any
model" — and, per the tool-comparison research, **exceed** every surveyed coding agent on this
axis rather than merely catch up.

### 2.1 Make local models actually get selected (not "get into the catalog" — corrected 2026-07-31)

> **⚠ Correction.** This section originally targeted "zero of 364 catalog entries are local"
> (F9a). Re-verified directly during Phase 2 planning: **that finding was wrong.**
> `vox model discover --force` genuinely reports `Ollama: 3 models`, and all 3
> (`qwen3:8b`, `qwen3-vl:8b`, `vox-mens-v1:latest`) are present in
> `~/.vox/cache/model-catalog.v1.json` with `provider_type: "ollama"`. The original test
> (`vox model list | grep -i ollama` → 0 hits) was an artifact of `vox model list`'s default
> `--limit 100` combined with an alphabetical sort over 381 total models — local model ids sort
> past position 100. **Discovery/registration/`ProviderType::Ollama` all already work.**

- **The real, empirically-confirmed gap:** `vox model explain "hi" --category codegen --complexity 1`
  — the single case where a $0 local model should most plausibly win — still ranks **zero**
  Ollama models in its top-5. Nothing in the (now correctly input-sensitive, post-Task-0.1)
  scorer favors a local candidate even at the lowest complexity. This is the actual target: not
  populating the catalog, but making local models *reachable by selection* at all.
- Fix `vox model list`'s default UX so this class of bug can't recur invisibly: either raise the
  default `--limit`, change the default sort to something that doesn't bury free/local models
  (e.g. cost-ascending), or add a `--local-only`/`--free-only` flag — cheap, and directly
  prevents "silently invisible via pagination" from masking a real selection gap again.
- Add `vox model add <local-endpoint>` for LM Studio / vLLM / any OpenAI-compatible local server
  not auto-discovered (Ollama auto-discovery already works; this covers what it doesn't).
- **Also accept `ollama_chat/<model>`-style prefixed identifiers** anywhere a model id is
  expected. This is not a replacement for the array schema above — it's a low-cost addition.
  Aider and LiteLLM independently converged on this exact prefix convention
  (gap-fill doc §1.1, §4), which is meaningful corroboration that it's becoming a de facto
  standard worth Vox recognizing even though the array/`provider` field remains the primary
  schema.
- **Cursor is not useful prior art here** — its "Ollama support" was verified to be nothing
  more than the generic OpenAI-compatible-base-URL override every one of these tools has by
  default (gap-fill doc §0), not a dedicated integration. Continue.dev remains the strongest
  model to copy.

### 2.2 Add a privacy axis as a hard filter, and stop the `local_only` name collision

- **Findings:** F7, routing doc §2.3/§7 item 1. `VOX_MESH_EXEC_POLICY`'s `local_only` governs
  mesh task placement, not inference — a privacy-motivated user setting it believes they've
  disabled cloud inference and hasn't.
- Model OpenRouter's `zdr` semantics exactly: a privacy filter that is a **one-way ratchet**
  (can only tighten, never loosen via a per-request override) and is enforced as a **hard
  filter** on the candidate set, not a ranking hint (routing doc §2.2's soft/hard distinction).
- Enforce the actual boundary in `vox-llm-egress`, not in the router — routing chooses among
  *permitted* candidates; it must never be the thing that makes a candidate impermissible
  (routing doc §3.4).
- Rename or clearly re-scope `VOX_MESH_EXEC_POLICY` so its name stops implying inference
  privacy.

### 2.3 Add context-window filtering and a normalized overflow cascade

- Routing doc §1.5, §7 item 2. Copy LiteLLM's error-string normalization (each provider reports
  overflow differently; normalize into one condition) and add a `context_window_fallbacks`
  cascade across Vox's five provider lanes.

### 2.4 ⚠ CORRECTED (2026-07-31): already built and live — no task needed

> Direct code verification during Phase 2 execution found this largely already implemented,
> contradicting the routing research's assumption (which was about the field generally, not
> Vox's actual codebase). `ModelRegistry::inject_scoreboard_latency` (`models/registry.rs:136`)
> writes measured p50 latency from `model_scoreboard` into the exact field
> `scoring::latency_score` reads. `vox model eval` writes `quality_score = pass_rate` back to
> `model_scoreboard` (`vox-cli/src/commands/model/eval.rs:7,180,277`). Both are wired into
> `auto_score_model` via `scoreboard_feedback_boost`. `refresh_model_scoreboard` — which calls
> both injectors — runs at orchestrator init (`core/init.rs:98`) **and** periodically in the live
> observer loop (`observer_loop.rs:104`), not just in tests. **Latency and eval-derived quality
> both already feed live model selection.** No remaining task; original text below preserved for
> the record.

- ~~Routing doc §7 items 3–4. Record TTFT per deployment in a rolling window
  (feeds `responsiveness`); feed `vox-eval` results into the `intelligence` axis as Vox's
  substitute for OpenRouter's crowd-spend signal — a better signal because it's measured on
  Vox's actual workload, not aggregate community usage.~~

### 2.5 One preset dial, three axes behind "advanced"

- Routing doc §7 item 6, echoing OpenRouter's single `cost_quality_tradeoff` scalar (routing doc
  §2.5) as the right *default* UX even though Vox's 3-axis engine underneath is more expressive.
  Presets: `private` (hard local-only filter) · `frugal` · `balanced` · `best`.
- **Never expose a bare "prefer local" boolean** (routing doc §7 item 7) — pair every locality
  preference with a capability floor, or reproduce LiteLLM's documented zero-cost-wins-always
  trap exactly (routing doc §1.4).

### 2.6 Hardware/capability gating — Vox's chance to lead, not follow

- Routing doc §8 item 1; tool-comparison doc §5. **No surveyed tool does this.** Wire
  `vox-plugin-nvml-probe` (already in-tree, currently unwired) to gate which local models are
  *offered*, not just which are *installed* — e.g. don't recommend a model whose weights don't
  fit in available VRAM. This is additive scope, sequence it after 2.1–2.5 land, but it is the
  single most differentiating item in this entire plan relative to the surveyed field.
- **Design the check as advisory, not a hard block**, matching the routing research's soft/hard
  distinction (routing doc §2.2): deprioritize an oversized model, don't refuse to show it,
  unless it provably cannot load. Zed's own approach — doing no hardware checking itself and
  deferring to the local tool's UI (LM Studio) for that signal — is the low bar Vox should
  clear by *doing* the check natively, while keeping its consequence non-blocking
  (gap-fill doc §4).

---

## Phase 3 — Skill system: fix the cap mechanism, wire the miner, add a real gate

**Goal:** organic skill growth with the testing/induction research's promotion discipline, not
an ungated firehose.

### 3.1 Fix the 64-skill cap mechanism, not the number

- **Finding:** F6a. `render_skill_catalog(&entries, 64)` silently truncates alphabetically. The
  induction research (§3) confirms **64 is roughly the right order of magnitude** — Anthropic's
  own data puts degradation at 30–50 tools — so **raising the cap would make selection worse,
  not better.**
- Fix: (a) log/surface when truncation happens — silent truncation is the actual defect; (b)
  rank by `skill_reliability` + recency instead of alphabet, since that table already exists and
  is already populated (F14); (c) if the library later outgrows a few hundred entries, add
  "Active Tool Request" (induction doc §1.2) — the model asking "I need a skill that does X" —
  before building full semantic retrieval, since it's free and MCP-Zero's ablation shows the
  full architecture's win is not cleanly attributable to retrieval alone.
- Also raise the 256-char description truncation toward the documented 1,024-char standard
  (F6b) — it currently cuts the "when to use" half of every description.

### 3.2 Wire the skill miner and give it somewhere to land

- **Findings:** F8, F13. `code_miner`/`op_miner` have one manual CLI caller; there is no
  `skill_candidates` table, so mined output cannot persist, rank, or get promoted.
- Add the table. Wire the miner to run on a schedule (or post-task, gated) rather than only
  manually.

### 3.3 Implement the promotion gate — this is the "adequate testing for what gets pushed in" answer

Per the induction doc §4, synthesized from five independent research systems' gates
(Voyager/CRAFT/AWM/SkillWeaver/Memp) plus SkillOps's contract model (testing doc §5):

```
CANDIDATE (mined from repeated trajectories)
  1. EXECUTION GATE     — must reproduce the outcome of its source trajectories
  2. ABSTRACTION        — generalize constants; description meets the 1024-char/
                           third-person/"use when" standard (mechanics doc §4.4)
  3. DEDUPE              — reject near-duplicates via vox-similarity (currently 65%
                           isolated — this is its wiring point)
  4. INDEPENDENT VERIFY  — verified by a DIFFERENT model than the one that mined it
                           (cheap given Vox's multi-provider catalog once Phase 2 lands)
  5. GENERALITY GATE     — require ≥N distinct source trajectories, not one
  6. V = ∅ CHECK         — reject any candidate with no attached validator
                           (SkillOps, testing doc §5.1) — cheapest, do this first in
                           practice even though it's listed last here
  7. SHADOW PERIOD       — promote as `provisional`; track skill_reliability;
                           promote to `confirmed` on sustained success, RETIRE on
                           sustained failure
  8. PROVENANCE BINDING & RE-VERIFY ON CHANGE — (security doc §5) sign the promoted
                           skill body (Sigstore-style: source→build→artifact); ANY
                           post-promotion change to the body re-enters the gate from
                           step 1, dropping confirmed status until re-verified
```

- **The Voyager ablation number (induction doc §2.2) is the reason this isn't optional**:
  removing self-verification alone cost 73% of discovered skills' value. An ungated miner isn't
  a cheaper version of this — it's a materially worse system.
- **Step 8 is not optional either, and the evidence for that is the strongest single number in
  the entire research set:** OpenAI's GPT Store runs builder verification, combined
  human+automated review, and brand-impersonation classifiers — a materially more thorough
  process than anything else surveyed — and independent academic testing still found **95%+ of
  14,904 listed GPTs exploitable**, with roleplay jailbreaks succeeding 96.51% of the time
  (security doc §2.2). **Review-time checking, however thorough, does not stop what a skill does
  after promotion.** This is precisely the "MCP rug pull" pattern documented by both Invariant
  Labs and CyberArk (security doc §1) — a tool that passes review once and changes behavior
  after. Step 8 is Vox's direct structural defense: any body change forfeits trust until
  re-verified, and where feasible the skill runs sandboxed (`vox-skill-runtime`'s existing
  WASM/container tiers) so even a compromised-but-still-confirmed skill has contained blast
  radius.
- Reuse Vox's existing `ModelConfidence`/`is_routing_eligible` state-machine pattern
  (`select.rs:118-161`) for the provisional→confirmed→deprecated skill lifecycle — it's already
  proven in-tree for models; this is the same shape applied to skills.

### 3.4 Namespace and identity, borrowed from package-registry prior art

- Registry doc §4, item 1. Adopt reverse-DNS or `io.github.<user>/<skill>` naming tied to
  GitHub OAuth/OIDC or DNS verification for any skill published outside a single workspace —
  cheap, decentralized, working prior art to fork rather than design from scratch.
- **Adopt crates.io's explicit anti-squatting rule near-verbatim** (security doc §3.1):
  first-come-first-served naming; a name reserved without genuine working content is deletable
  on sight, without notice; namespace cannot be bought, sold, or traded separately from the
  skill itself.
- **Unlike the MCP Registry, gate on reliability/vulnerability signals** (registry doc §4,
  item 3) — Vox controls the full stack end-to-end in a way the multi-vendor MCP ecosystem
  doesn't, so it can and should be stricter than the prior art it's borrowing the namespace
  scheme from. The MCP Registry's explicit policy of *never* delisting for a disclosed
  vulnerability (registry doc §1.4) is the negative example, not the model.

---

## Phase 4 — Chat UX: accessibility fixes, noise reduction, panel redesign

**Goal:** the "it just works" feeling is partly about not drowning the user, and this is the
cheapest phase — mostly small, well-scoped fixes with binding standards backing them.

### 4.1 Accessibility — one binding, three concrete

- **F10 (binding — WCAG 2.2 SC 4.1.3, Level AA):** remove `aria-hidden="true"` from the 28
  `SurfaceMiniRender`-based dashboard widgets; give them real `role="status"`/`role="log"`
  semantics per the ARIA mapping in UX doc §1.
- **F23:** move the three `role="alert"` regions carrying buttons
  (`VersionMismatchBanner.tsx:14`, `Console.tsx:108`, `ErrorBoundary.tsx:41`) to an alert
  dialog or persistent banner — `alert` must not carry interactive controls (MDN, UX doc §1.2).
- Audit `SecretaryToast`'s 5-second auto-dismiss-with-action-button against the same
  constraint, and reconsider it moot once 0.2 makes the secretary propose-only.

### 4.2 Toast triage — the taxonomy is fine, the input is the problem

- **F12.** 70% of all toasts are `backend-error`. The `cause`-required taxonomy
  (`ToastCause`) is already well-designed — credit it, don't rebuild it. Fix:
  (a) replace `MAX_TOASTS`'s oldest-drop truncation with coalescing (`×N`, preserving the
  first/root error, per WCAG SC 2.2.4's frequency concern — UX doc §3); (b) add a user control
  to defer/mute, the literal remedy SC 2.2.4 names; (c) dedupe identical errors from poll loops
  (the 2-second `APPROVALS_POLL_MS` loop is the worst offender).

### 4.3 Progress indication — never a bare spinner for an agent turn

- UX doc §4 (NN/g's 10-second rule, with its own step-count fallback for nondeterministic
  work). Vox already has `PhaseChip`/`ChatExecutionRail` — make them the default view for any
  turn expected to run >10s, not optional chrome.

### 4.4 Panel redesign around the four documented oversight forms

- UX doc §7A: **a priori control, co-planning, real-time monitoring, post hoc review**
  (Chen et al., 17-developer interview study). Vox's surfaces map to three of four already
  (permission modes → a priori; `PlanPanel` → co-planning; `ChatExecutionRail`/`PhaseChip` →
  real-time monitoring); post hoc review is the only one currently well-served by diffs.
  **Deliberately audit panel coverage against all four**, not just add more diff-review
  surface area — the CHI-2024 finding that 22.4% of session time is verification (UX doc §6)
  argues for prioritizing *cheap-to-scan* diffs and plan state specifically within the
  post-hoc-review quadrant, not for expanding that quadrant at the expense of the other three.
- Replace 60%-scale `aria-hidden` thumbnails with real summary tiles where a widget doesn't
  merit a purpose-built component (F10/F11) — no direct evidence favors tiles over thumbnails
  on data density grounds (UX doc §6.1, an honestly-flagged open question), but the
  accessibility requirement in 4.1 forces the change regardless.

---

## Phase 5 — Observability and the regression gate

**Goal:** make Phase 0–4's fixes verifiable over time, not just once, and give the skill/model
system the "what gets pushed out" half the user asked for.

### 5.1 Instrument with OTel GenAI-shaped spans

- Testing doc §4. `invoke_agent` → `chat` → `execute_tool` span hierarchy. Vox's
  `vox-telemetry`/`vox-telemetry-otlp` and `PromptDispatchTelemetryEvent` already emit stage
  labels — the gap is adopting the OTel GenAI *span shape* specifically so traces are
  comparable across a regression, not just per-event.

### 5.2 Multi-sample eval gate, not single pass/fail

- Testing doc §2.2, §7 items 4/7. Report pass^k (all-k-succeed), not pass@1, for any
  harness-reliability claim — τ-bench's own numbers show even strong models collapse under
  repetition. No specific sample-size number survived this research's own verification (testing
  doc §6, item 1) — start with a provisional n=5, instrument it, and calibrate empirically
  against Vox's own production trajectory data rather than importing an unverified citation.
- Use Vox's own workspace as the eval corpus (testing doc §7 item 1) — closes the
  SWE-Bench-style contamination vector (testing doc §1) by construction, since there's no public
  corpus for a model to have memorized.
- Prefer deterministic outcome-checking (compiles, type-checks, tests pass) over LLM-judge
  scoring wherever a checkable end-state exists (testing doc §7 item 2) — sidesteps the
  demonstrated 98%-swing artifact-fragility of LLM judges (testing doc §3) entirely for those
  cases.

---

## Sequencing summary

```
Phase 0  ── Fix scorer + defang secretary ─────────────── standalone, ship first, ship fast
Phase 1  ── Wire GUI chat to the working MCP lane ───────── depends on 0.1
Phase 2  ── Local models + routing correctness ──────────── depends on 0.1; unblocks the
                                                              user's core ask
Phase 3  ── Skill promotion gate + registry ─────────────── independent of 1-2; can run parallel
Phase 4  ── UX/accessibility/noise ───────────────────────── independent; can run parallel;
                                                              4.1 is a conformance fix, prioritize
Phase 5  ── Observability + eval gate ────────────────────── wraps all of the above; needed to
                                                              verify 0.1's fix meets its own bar
```

Phases 3 and 4 do not block Phase 1 or 2 and should run in parallel once Phase 0 lands. Phase 5
is not "last" in priority — the multi-sample verification standard should be applied to Phase
0.1's fix specifically, which means a minimal version of 5.2 is needed early, not deferred to
the end.

---

## Appendix — findings not yet actioned above

For completeness, findings referenced elsewhere but not given a dedicated plan item:

- **F15/F16** (Graphify has no query verb; `vox graph coverage` returns a uniform,
  unreliable verdict) — out of scope for this harness plan; tracked separately as a Graphify
  tooling fix.
- **F17** (`permission_mode: None` hardcoded at 4 CLI dispatch sites) — should be fixed
  alongside Phase 1.4's `vox chat` verb, same code path.
- **F18** (Gemini provider identity inferred by URL substring) — low severity, fold into
  Phase 2.1's catalog work opportunistically.
- **F19** (panel-resurrection guard documented as a footgun) — fold into Phase 4 if touching
  `DockWorkspaceShell` for other reasons; not worth a standalone pass.
- **F20** (`skill-runtime/detect.rs` name collision with the absent skill-detection feature) —
  now resolved in framing since audit §2.1 established real skill detection lives in
  `skill_catalog.rs`; a rename of `detect.rs` (to something like `sandbox_probe.rs`) is a
  trivial cleanup, not a functional fix.
