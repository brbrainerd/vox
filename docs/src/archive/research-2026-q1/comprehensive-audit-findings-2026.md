---
title: "Comprehensive Vox audit and improvement plan (April 2026, v2)"
description: "Full-spectrum audit + ~90 prioritized improvements. Targets the 'premier LLM destination for web app code' thesis."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-18"
training_eligible: false
training_rationale: "Strategic audit context for engineering prioritization."
schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Comprehensive Vox audit and improvement plan (April 2026, v2)

Bert — full audit triangulating `crates/`, the 280 docs in `docs/src/architecture/`, ADRs 001–023, the CHANGELOG, parser/build artifacts, and the 43 golden examples. Goal: name the real gaps blocking *Vox is the premier programming language for LLMs to write web apps* and give a sequenced improvement list you can actually execute.

This v2 supersedes the prior file; it corrects two count errors (crates=63, ADRs=23) and notes the `vox-doc-pipeline` build break is **fixed** as of today.

## Part I — Executive diagnosis

### What is real and good

- A real Rust compiler (lexer/parser/HIR/IR/typeck + dual Rust+TS codegen) in `crates/vox-compiler/`.
- A real ~17 kLOC orchestrator with A2A messaging, observability, daemon parity (ADR 022), trust telemetry.
- A real Populi mesh control plane with NVML probes and HTTP scope tenancy (ADR 008/017/018).
- A real native MENS training pipeline (Candle QLoRA, Qwen 3.5 4B on 16 GB) with deterministic corpus and OpenAI-compatible serve.
- A real RAG stack (Scientia/`vox-search`) with BM25 + FTS5 + vector + RRF + CRAG + Socrates gating, plus the brand-new symbol-proximity split-brain detector.
- Serious governance: 23 ADRs, `.voxignore` SSoT, Clavis secret resolution, TOESTUB detectors, completion-policy gates, `vox ci ssot-drift`.
- A working `@table` → SQL → typed client → typed server collapse on the golden corpus. This is the moat.
- 102 MCP tools + VS Code extension + OpenCode bridge + dashboard.

### The five biggest structural problems

1. **The parser still rejects top-level statements.** `parse_top_level_item` accepts only `fn`/`import`/`type`. `RunMode::Script` exists at the CLI but no implicit `main()` wrapping or script grammar exists. AGENTS.md mandates `vox run scripts/foo.vox` as the only project glue, yet `examples/actor.vox` and `examples/mcp_tool.vox` were archived as "non-parseable on current grammar" and `VOX_EXAMPLES_STRICT_PARSE` is opt-in. You are simultaneously banning Python/PowerShell glue and shipping a parser that can't run the replacement.
2. **The web-UI pillar embeds React + JSX directly inside `.vox` source.** Authors write `<div className=…>`, `use_state`, `onClick`. The "Path C / Vox-native reactivity" recommended in `web-architecture-analysis-2026.md` is documented but optional. This contaminates the MENS corpus with React idioms and undermines the K-complexity thesis.
3. **LLM-alignment scaffolding is 60–70 % unshipped.** GBNF export is ~30 lines (expressions only) with CVE-2026-2069 in downstream llama.cpp; XGrammar-2 not adopted. `correction_hint` is populated in 1 of 31 HIR validation sites. `@llm` decorator parses but emits no runtime dispatch. AST-token weighted loss is checkboxes-only. There is **no public HumanEval-Vox or SWE-bench-Vox** showing that Vox-generating models hallucinate less than TS/Python-generating models at matched compute.
4. **Doc/code divergence is becoming a governance crisis.** 280 architecture docs, 43 currently passing goldens, ~6 parallel SSoT generators (operations / MCP / CLI / capability / language-surface / completion-policy), retired surfaces (HTMX, Pico, TanStack virtual routes) still ghost-referenced in many pages, research → blueprint pipeline unclear. The doc tree is growing faster than the code tree is shipping.
5. **Scope is too wide for the evidence base.** 63 workspace crates, 280+ CLI subcommands, 23 ADRs, telemetry subsystem, cryptography SSoT, gamification, speech-to-code, vision lane, plugins marketplace, MENS, mesh, RAG, publication pipeline, workflow runtime, package manager. There is no plausible world in which all of these are simultaneously stable. `feature-growth-boundaries.md` and `vox-bell-curve-strategy.md` are trying to restore discipline; the horse left the barn months ago.

### Single-sentence verdict

*Vox has the best architectural governance of any v0.4 language I've seen, the only real compiler+training+orchestrator stack at this tier, and zero empirical evidence for its headline LLM-native claim — while being too sprawled to ship that evidence at current scope.*

## Part II — ~90 prioritized improvements

Priority key: **P0** = ship-stopping / credibility-risking · **P1** = materially advances the v1.0 thesis · **P2** = high-value within ~2 quarters · **P3** = hygiene.

### A. Parser & compiler correctness — the oxygen layer

1. **[P0]** Make `vox run --mode script` actually work end-to-end: either add a `parse_script` entrypoint that wraps top-level statements in an implicit `fn main()`, or extend `parse_top_level_item` to accept statement sequences. Today's CLI flag is a lie because no parser path exists. This is the single most embarrassing gap.
2. **[P0]** Parallel error collection in the parser. Replace first-error-bail with a `collect_all_errors` pattern; emit every syntactic fault per file. Prerequisite for usable LSP, LLM self-repair, and training-time error mining.
3. **[P0]** Delete the stale `check.log` and `parser_errors.txt` artifacts from repo root (the build blocker is fixed). Add them to `.gitignore`. Stale ground-truth-looking files in the root train both humans and models on bugs that no longer exist.
4. **[P0]** Add a `cargo check -p <every-crate> --all-features` matrix lane to CI. The current `--default-features` lane misses combinatorial breakage (yesterday's `lint_file` E0603 only surfaced on full-feature build).
5. **[P1]** Populate `correction_hint` in every HIR validation site. Currently 1/31. Each error must carry a minimal patch + expected shape + one concrete example. This is the channel through which agents self-repair.
6. **[P1]** Finish the bool-exhaustiveness check in `match_exhaust.rs`. Today match-on-`bool` with a missing arm passes; emitted training data teaches LLMs unsound patterns.
7. **[P1]** Fix postcondition injection on early returns (BUG-1 in the LLM-target plan). Contracts silently skip on multi-return functions.
8. **[P1]** Unify route emission behind a single `RouteIR`. Rust and TS emitters currently walk `Decl::Route` independently; LLMs learn two patterns instead of one.
9. **[P1]** Carve out and freeze a "Vox-1" surface subset matching the golden corpus; publish `contracts/language/vox-language-surface.v1.json`. Mark all syntax outside the freeze as experimental. You cannot have a "Stable" tier in the README without this artifact.
10. **[P2]** Wire `DeprecatedUsageDetector` into `vox check`'s default output (not just CI). `ret`, `@component fn`, classic React hooks should warn at edit time so MENS doesn't keep training on them.
11. **[P2]** Eliminate the `legacy_ast_nodes` untyped fallback for full-stack declarations (`@page`, `@layout`, `@action`, `@theme`). Either parse typed or reject.
12. **[P2]** CI gate that `vox fmt` is parse-print-reparse idempotent on the entire golden corpus. `vox fmt` is wired but no idempotence test exists; idempotence is a prerequisite for training normalization.
13. **[P2]** Make `VOX_EXAMPLES_STRICT_PARSE=1` the default in CI (currently opt-in). If we can't parse our own examples, we don't ship.
14. **[P3]** Delete `spec.rs.bak` (199 673 lines) at repo root — backup file that contaminates training corpora and choke-loads reviewers/grep.

### B. Web/UI pillar — the principal target

15. **[P0]** Ship a Vox-native reactivity primitive that does **not** require `import react.use_state`. The recommended Path C must become the *only* path. `component` bodies should declare `state` / `derived` / `effect`; the compiler owns the lowering to React hooks (or other backends later). This is the single biggest contradiction to the thesis.
16. **[P0]** Eliminate raw JSX from `.vox` source. Replace `<div className="…" onClick={…}>` with a Vox-native view language (`view: <div class="…" on:click={…}>`) that lowers to JSX at codegen time. Freeze the old form in goldens, warn on new use, remove in v1.
17. **[P0]** Treat the `@table` → typed client + server slice as v1's signature demo. It works end-to-end, it is roughly 10× less code than Next.js + Prisma + tRPC, and it is the only fully credible piece of the pitch today. Every tutorial, marketing page, and MENS training sample should orbit this slice.
18. **[P1]** Auto-type route loaders from `@query` signatures. A route declaring `with loader: list_posts` should require the loader's return type to unify with the component's prop surface at compile time. Currently the parser doesn't enforce.
19. **[P1]** Compile-time typed island props. `data-prop-*` attributes stringify at hydration today; emit a typed accessor (`readProp<T>(el, "name")`) and validate every island prop has a Vox→JSON codec.
20. **[P1]** Default `vox run` to SSR-orchestrated mode. The `VOX_ORCHESTRATE_VITE=1` requirement breaks the "single command runs a full-stack app" pitch every time.
21. **[P1]** Manifest-first routing everywhere. `tanstack-start-codegen-spec.md` is already marked historical; finish the cutover, retire the dead emitter branches, archive the cancelled backlog.
22. **[P2]** Adopt modern CSS primitives in `style:` blocks now (Container Queries, View Transitions, `:has()`, `@scope`, native nesting). Match `css-determinism-implementation-plan-2026.md` but ship.
23. **[P2]** Second codegen backend: compile-away signals (Svelte 5 / Solid style). Demonstrates "framework as compiler target, not dependency". Publish bundle-size comparisons; this is where you can credibly out-Next.js Next.js.
24. **[P2]** Restore a classless theme system. Pico is retired; there's no replacement. Ship `@theme` emitting a CSS-variables system + utility shorthands. Without it, LLM-generated apps are visually ugly-or-bespoke and adoption stalls.
25. **[P2]** Move the v0.dev TSX normalizer into the compiler (`vox-compiler/codegen_ts`) instead of `vox-cli/v0_tsx_normalize.rs`. Make `vox island generate` a compiler pass.
26. **[P2]** Ship an official `@vox/next` interop adapter that lets Vox `@island` components mount inside an existing Next.js app. Wedge into real production codebases that won't rip out Next.
27. **[P3]** GUI visual-intelligence loop (`vox-gui-vision-virtuous-cycle-implementation-plan-2026.md`): either start a 4-week spike or explicitly defer to v1.1.

### C. LLM-native language features — prove the thesis

28. **[P0]** Publish HumanEval-Vox and SWE-bench-Lite-Vox at matched compute. Without public, reproducible benchmarks showing Vox-generating models outperform TS/Python-generating models at the same parameter count, the LLM-native claim is marketing. **This is the single most important external-facing deliverable that does not yet exist.**
29. **[P0]** Complete grammar export — replace the ~30-line GBNF stub with full XGrammar-2 emitter, drop the CVE-2026-2069-vulnerable downstream path. Until grammar-constrained decoding is real, models hallucinate out of your coverage.
30. **[P1]** Wire `correction_hint` into a compiler→LLM repair loop. Add a `vox repair --on-error <model-endpoint>` command that streams compile error + hint to a model, applies the proposed patch, re-checks. Demo on the golden corpus.
31. **[P1]** Implement `@llm` runtime dispatch. Parsing already lands; codegen does nothing. `@llm fn classify(s: str) -> Category` should emit an Axum handler that calls a Clavis-resolved LLM, validates against the return schema, and falls back to an `@llm.fallback` function.
32. **[P1]** Vox-source-purity filter on the MENS corpus builder. Add `context_filter: "vox_pure"`; emit a contamination score per file; auto-exclude files whose React/JSX content exceeds threshold.
33. **[P1]** Publish the first public Vox adapter on Hugging Face: `qwen-3.5-4b-vox` QLoRA adapter with a reproducible recipe. Even a modest adapter that makes one public foundation model speak Vox fluently validates the whole MENS thesis.
34. **[P1]** Implement AST-token weighted loss (`align_tokens_to_ast`) per `ast-token-alignment-2026.md`; measure loss-curve delta on a held-out Vox eval set.
35. **[P2]** A Vox Rosetta benchmark suite. Expand `examples/golden/inventory_rosetta_*.vox` into a multi-language matched-task repo; publish "lines of code per task" and "LLM accuracy per task" charts. Evergreen marketing with hard numbers.
36. **[P2]** Effect tracking exposed in HIR JSON. Each function's effects (DB read, DB write, HTTP egress, FS access) inferable and visible. Lets agents plan safely; doubles as a security primitive.
37. **[P2]** Promote `vox check --format json` to parity with the human renderer; expose as an MCP tool any agent can call.
38. **[P2]** Ship `docs/agents/vox-language-surface.v1.json` enumerating every decorator, keyword, and builtin with three gold examples apiece. Ground truth for in-context learning across all frontier models.
39. **[P2]** Determinism audit lane (`vox ci determinism-audit`) on goldens. For fixed Vox input, emitted Rust/TS/SQL/manifest bytes must be byte-identical run to run. Essential for reproducible training.
40. **[P3]** Time-boxed graph-IR research spike (2 weeks). Measure whether serialized AST-as-JSON already wins the gain before investing in a graph IR per the frontier doc.

### D. Training pipeline (MENS) and corpus

41. **[P0]** Reach the 100 k `organic_vox.jsonl` threshold gating CPT in `vox-lang-training-ssot-2026.md`. Publish current count weekly on the dashboard. Below it, all CPT claims are aspirational.
42. **[P1]** Synthetic corpus generator from golden templates. Use existing foundation models + `vox check` to generate *and verify* millions of Vox examples from parameterized templates. Compiler-verified synthesis is one of Vox's unique training advantages.
43. **[P1]** Close MENS Gap A: full-graph Candle QLoRA. Proxy-graph backward pass blocks loss parity with Burn. Highest-leverage MENS technical item.
44. **[P1]** Close MENS Gap D: nested-LoRA serving in Candle. `merge-qlora` → `.safetensors` works but inference can't consume nested adapters; iterative fine-tune + eval is crippled without it.
45. **[P1]** Ship the GRPO reward-shaping upgrade per Cluster A1: replace additive reward with `R = r_syntax × (w1·r_test + w2·r_coverage)`; add negative sampling for parse failures; curriculum seeding.
46. **[P2]** Close MENS Gap B (MoE). Qwen3-Coder is MoE; you're training the wrong architecture long-term.
47. **[P2]** Close MENS Gaps C (RoPE+LoRA merge in Burn) and E (research-reasoner lane), or name explicit ETAs.
48. **[P2]** Reproducible training runbook. Single `vox populi train --config qlora-qwen35-vox.toml` any 16 GB-GPU researcher can reproduce, plus `docs/src/tutorials/train-your-own-vox-model.md`.
49. **[P2]** Continuous-learning safety benchmark. Implement the anti-collapse / structure-snowballing penalty from `research-synthesis-grand-strategy-seed-2026.md`; publish before/after loss curves.
50. **[P3]** Federated training inside Populi. Pooled Vox adapter run with license provenance per contribution. On-brand, not urgent.

### E. Agent orchestration & runtime

51. **[P1]** Formalize the OOPAV (Understand → Plan → Act → Verify) state machine in `runtime.rs`. Encode as a `TaskState` enum with transition guards observable from telemetry.
52. **[P1]** Trust-tier RBAC in dispatch. `NodeRecord` carries `trust_tier`; `Orchestrator::dispatch` ignores it. Wire a capability matrix (FS writes, secret reads, network egress) so low-trust nodes can't perform high-impact ops.
53. **[P1]** Workflow durability parity: ADR 021 is a design gate. Ship the determinism test that proves the interpreted and generated-Rust paths replay identically.
54. **[P2]** Minimum HTN pod manager: `goal → tasks → sub-agents` decomposition with observable state.
55. **[P2]** Populi lease-based remote execution (ADR 017): ship the minimal lease semantics so "run this task on that GPU node" actually executes remotely.
56. **[P2]** Tier the 102 MCP tools into `core` (≤20) / `dev` / `advanced` with a loading manifest. 102 tools blow most LLMs' context budgets gracefully — segment the surface.
57. **[P2]** Retire the `VOX_MCP_ORCHESTRATOR_RPC_READS/WRITES` env-flag lattice. Ship the "write pilots GA" milestone, then delete the flags.
58. **[P2]** Socrates gating on agent-emitted code (currently mostly RAG answers). Gate `@island`, `@server`, and workflow generations against tests + coverage before accepting.
59. **[P3]** Sub-agent circuit breakers per Cluster A2/A3 research.

### F. Developer experience (LSP, CLI, errors)

60. **[P1]** Connect `vox-lsp` to the live HIR validator. Today LSP is decoupled from the compiler; live diagnostics are stale. Surface `correction_hint` as LSP quickfixes.
61. **[P1]** Single canonical error renderer (`vox check --format human|json|sarif`). Today errors are formatted twice (miette + CLI-local).
62. **[P1]** Make `vox doctor` opinionated and project-specific. "Your project is missing a Clavis ID for FOO"; "your `.voxignore` is stale"; "MSRV is N minors behind." Today's doctor is a generic toolchain probe.
63. **[P2]** Cut the CLI surface to a tiered model. `vox --recommended` exists; enforce it. Move `ludus`, `opencode`, `ars`, `schola`, `oratio` behind a `vox ext` namespace.
64. **[P2]** SSE stream of structured diagnostics from `vox check`. Agents benefit far more from live streaming than from monolithic JSON.
65. **[P2]** VS Code snippet pack auto-generated from goldens. 90 % of new-user typing is a minor variation on a golden; ship the snippets deterministically.
66. **[P2]** A `vox play` zero-install REPL/playground that starts a "hello full-stack" in 60 seconds. Most underrated adoption lever.
67. **[P2]** `vox llm prompt <task>` — print the ideal system prompt + context files + MCP tool list for the current repo. Removes friction between Vox and frontier coding agents.
68. **[P3]** Error renderer surfaces a unified-diff fix-me when `correction_hint` is present.

### G. Documentation consolidation — stop the doc bloat

69. **[P0]** Two-week archive freeze: every `docs/src/architecture/*.md` without `status: current` moves to `docs/src/archive/research-2026-q1/`. 280 docs is unmanageable; re-surface only what's load-bearing today.
70. **[P0]** One auto-generated "what's shipped today" page. `vox ci capability-snapshot` emits `docs/src/reference/shipped-v0.4.md` weekly; the README tier table becomes a rendered fragment, not hand-edited.
71. **[P1]** Promote `contracts/documentation/canonical-map.v1.yaml` to actual root-of-truth. Operations, MCP, CLI, capability, language-surface, telemetry-trust, completion-policy SSoTs all become *views* of it. Six SSoT generators is too many.
72. **[P1]** Every roadmap doc must carry an owner and an ETA. No owner + no ETA = close or archive.
73. **[P1]** Global retire of HTMX / Pico / TanStack-virtual-routes ghosts across the doc tree. CI guard for banned terms.
74. **[P2]** Diátaxis audit by agent. `vox scientia` job classifies every page as tutorial / how-to / reference / explanation; flag misclassified. Infra exists.
75. **[P2]** A 500-word external explainer on the landing page. The README is excellent at 445 lines; there's no two-paragraph version for a skeptical engineer.
76. **[P3]** Quarterly ADR retrospective. Which ADRs shaped code? Which became dead letters? Retire the dead.

### H. SSOT convergence & CI

77. **[P1]** Finish Stream H (language-surface SSOT generator). Adding a decorator should be a single-file edit that cascades. Named blocker in `orphan-surface-inventory.md`.
78. **[P1]** Single `vox ci drift` job that fans out into the half-dozen guard commands. Stop adding point-solution guards weekly.
79. **[P1]** Default `VOX_BUILD_TIMINGS_BUDGET_FAIL=1` on main; block merges that blow past budgets.
80. **[P2]** Workspace-hack (`cargo-hakari`) verified in CI; lapses silently undo its value.
81. **[P2]** `vox ci completion-audit` blocks merges, not just reports. 768 task IDs is impressive; enforcement is what makes it credible.
82. **[P2]** Implement `vox ci dep-sprawl` per `dependency-sprawl-research-2026.md` with a real per-crate cap.
83. **[P3]** Single unified runner contract across GitHub and GitLab.

### I. Ecosystem, adoption, external credibility

84. **[P0]** Pick a marquee app and ship it in public Vox, on a real domain, with telemetry on. Public blog, small SaaS landing page, or an open-source Slack-clone, deployed and readable. **No external claim lands without a production reference.**
85. **[P1]** Language-benchmark game: Vox vs. Next.js vs. Phoenix LiveView. Pick five canonical full-stack tasks (auth, CRUD, background jobs, realtime, uploads); build all three; publish LOC, time-to-complete, bundle size, LLM one-shot correctness. Linkable for years.
86. **[P1]** A single pinned landing-page demo: "write a full app by talking to it." Oratio + MENS + scaffold producing a habit tracker with SQLite, email reset, charts. Ship the recording. *This is the only thing that makes "premier LLM destination" intuitively real to outsiders.*
87. **[P1]** Render `docs/agents/ai-ide-feature-matrix-2026.json` as a public HTML page on the site, updated weekly. Vox-on-Claude/Cursor/Gemini/Codex/Cody compatibility is a shopping page for adopters.
88. **[P2]** Submit `tree-sitter-vox` to GitHub's linguist registry. Without inclusion, every Vox repo on GitHub looks amateur (no syntax highlighting).
89. **[P2]** Public free MCP endpoint hosting Vox tooling (`vox_check`, `vox_lint`, `vox_explain`). Adoption goes up an order of magnitude when there's zero install.
90. **[P2]** Weekly `vox status` post: shipped / in-flight / numbers / research highlights. Consistency beats scope for building a following.
91. **[P3]** Onboarding-funnel telemetry: time-from-clone-to-working-app, target sub-10-minute, publish the number.
92. **[P3]** Document Foundation governance (Open Collective bylaws). Write before anyone asks.

### J. Scope discipline & organizational shape

93. **[P0]** Pick 10 crates that are v1.0-track and feature-gate or freeze the rest. Suggested core: `vox-compiler`, `vox-cli`, `vox-db`, `vox-clavis`, `vox-runtime`, `vox-orchestrator`, `vox-populi`, `vox-tensor` (MENS), `vox-search` (Scientia), `vox-toestub`. Publish the freeze.
94. **[P0]** Name a v1.0 release criterion publicly. You cannot have a "Stable" tier in the README *and* be at v0.4 with parser regressions. Pick three measurable criteria (e.g. golden corpus green for 60 days, HumanEval-Vox published, one marquee app in production). Let them drive everything else.
95. **[P1]** Research → blueprint → backlog pipeline gate: any `*-research-2026.md` that hasn't produced an `*-implementation-plan-*.md` in 90 days is auto-archived. Stops the doc tree growing without code shipping.
96. **[P1]** Per-quarter scope budget for new crates: no new top-level crate without retiring one. Crate ledger pinned in README.
97. **[P1]** Resolve `vox-dei`: it's a stub `lib.rs` carried in the workspace with an active CI guard forbidding imports. Either delete or resurrect with real code.
98. **[P2]** Finish migrating remaining root-level `scripts/*.py` and `scratch_update*.py` to `.vox` per AGENTS.md `vox-as-glue` mandate. Today the policy is honored in the breach.
99. **[P2]** Prune the 22 `scratch_*` / `tmp_*` files at repo root and the 8 `target*` sibling directories. They leak into MENS corpus and clutter contributor searches. Add to `.gitignore`.
100. **[P3]** Adopt `cargo-nextest` as canonical test runner; measure CI wall-time delta; document.

## Part III — Sequencing

If resources are scarce, the ordering that protects credibility fastest:

1. **Week 1 — bleeding stops.** 1, 2, 3, 4, 69, 70, 93, 94. Parser/build stops failing publicly; scope gets honest in print.
2. **Weeks 2–4 — flagship demo + benchmark.** 17, 28, 33, 84, 85, 86. You need a public bench, a public adapter, a public app. Without them, the pitch floats.
3. **Months 2–3 — pillar repair.** 15, 16, 18, 19, 30, 31, 41, 51, 60, 77. Vox-native reactivity lands; LLM repair loop real; trained adapter real; orchestrator formalizes.
4. **Months 4–6 — ecosystem.** 23, 26, 35, 87, 88, 89, 90. Second backend; Next adapter; GitHub linguist; weekly cadence; public MCP endpoint.
5. **Ongoing — hygiene.** Everything else, with SSoT convergence (77, 78) and doc archival (69) as the steady-state rituals.

## Part IV — Open questions

Four answers materially reshape the priority list. They were surfaced separately via `AskUserQuestion`. The questions were:

- **Audience optimization.** Foundation-model labs adopting Vox naturally vs. devs shipping apps vs. enterprises with their own LLMs vs. open-source contributors.
- **Web-framework fork.** React-embedded forever (low risk, low differentiation) vs. Vox-native reactivity (high risk, high payoff) vs. multi-backend (high cost) vs. hybrid (compromise).
- **Team scale.** Determines whether a 100-item list is a backlog or fantasy.
- **v1.0 gating criterion.** Determines what "done" actually looks like.

## Part V — Reshaped plan, given the answers

Bert answered on 2026-04-18:

| Question | Answer |
|---|---|
| Audience | **Solo devs shipping web apps** |
| Web stack | **React-embedded forever (status quo)** |
| Team | **Solo + AI agents** |
| v1.0 gate | **Real production users** (N external apps deployed) |

This is a *coherent* posture. The plan collapses significantly.

### What those four answers together mean

1. "Real production users" + "solo + AI agents" + "solo devs shipping web apps" means the top-line KPI is now **time-from-`vox new` to a public URL** for a solo dev. Everything that doesn't move that number is secondary.
2. Choosing "React-embedded forever" kills a dozen items that presuppose a Vox-native component model. No Vox-native reactivity DSL. No multi-backend codegen. Web-stack work is narrow: make @island + TanStack Start boring and reliable, and stop the raw JSX leakage into `.vox` source.
3. "Solo + AI agents" means the 63-crate → 10-crate compression (item 93) is no longer a suggestion, it's the governing constraint. Every non-core crate needs a dated freeze/retire decision.
4. "v1.0 = production users" means **the runtime, deploy, and support story** outrank everything else — including MENS and Scientia — because a user's app going down on Saturday is the only thing that matters at that point.

### Items promoted (now P0 or P1 regardless of original tag)

- **1, 2, 3, 4** — parser statement-position regression, `vox-doc-pipeline` stale artifact, examples v0.5.0 migration, golden corpus. Still first. Nothing matters without a parser.
- **28** — `vox new web` scaffold → `vox deploy` → public URL in one command. **Now the single most important feature in the repo.** Promote from P0 to "this is the product."
- **33** — TanStack Start integration hardening. Promoted to P0.
- **16** — reduce raw JSX leakage from `.vox` source into compiler-owned surface. Promoted to P1 (was P2). Even though we're React-embedded forever, LLM-authoring suffers when components are 50% JSX strings.
- **84, 85, 86** — one marquee app in production, public deploy template, written case study. Promoted to P0/P1. These *are* the v1.0 gate.
- **93, 94** — 10-crate freeze + public v1.0 criteria. Promoted to Week 1.
- **49, 50, 51** — runtime crash recovery, supervisor trees, graceful degradation. Promoted to P1 (from P2). "Production users" means a running app that doesn't die.
- **62, 63, 64** — observability basics: structured logs, error reporting, uptime ping. Promoted to P1. You cannot support users blind.
- **40** — `vox fmt` + linter stability. P1. Required for AI authoring to be clean.

### Items demoted or shelved

- **15** — Vox-native reactivity primitives. **Shelved.** User explicitly chose status quo.
- **23, 26, 35** — Solid / Svelte / Next adapters. **Shelved indefinitely** (multi-backend was the rejected option).
- **17 (Vox-native component DSL R&D)** — Shelved.
- **18, 19, 20** — GBNF/XGrammar-2, AST-token alignment, correction-hint expansion. **Demoted to P2.** Still valuable but not on the solo-dev-to-production critical path. These become compelling *after* someone is shipping an app.
- **30, 31** — HumanEval-Vox, SWE-bench-Vox. **Demoted to P2.** Labs aren't the audience.
- **60, 61** — MENS training pipeline hardening. **Demoted to P2.** MENS is a research pillar but no solo dev cares that their Vox code is training data on day one.
- **41** — Scientia/RAG hardening beyond current state. **Demoted to P2.**
- **77, 78** — SSoT generator consolidation. **Demoted to P2** (was P1). Still needed for contributor sanity, but 1 contributor ≈ you, so the pain is smaller than the audit assumed.
- **96, 97** — crate ledger, resurrecting `vox-dei`. P1 → P2; the freeze decision (93) supersedes the ledger.

### Items deleted from scope entirely

- Foundation governance (92), weekly status post (90), OSS onboarding funnel (91), Linguist submission (88), public MCP endpoint (89) — these were community-scale moves premised on either OSS contributors or labs as audience. With solo-dev-as-audience, they become post-v1.0.
- Everything downstream of a Foundation or funded-team shape.

### The reshaped week-by-week

1. **Week 1 — bleed stops, scope honest.** 1, 2, 3, 4, 93, 94, 69 (archive stale research). Publish a `crates/_frozen.md` list. Publish three-criterion v1.0 gate: (a) 3 external solo-dev apps in production for 30 days, (b) `vox new web` → public URL in under 10 minutes, (c) zero P0 open on `vox-compiler`/`vox-runtime`.
2. **Weeks 2–3 — the product *is* `vox new web`.** 28, 33, 40. One command from empty dir to a deployed TanStack Start app on a free tier (Fly.io / Vercel / Cloudflare). Golden path in docs. Record the video.
3. **Weeks 4–6 — the first real user.** 84, 85, 86. Find one solo dev externally (could be a friend, could be you in a second persona) and walk them through shipping a real app. File every paper-cut as an issue. Fix it or freeze it within the week.
4. **Months 2–3 — keep the app up.** 49, 50, 51, 62, 63, 64. Runtime recovery, observability, error reporting. Users on production = support channel.
5. **Months 4–6 — second and third users.** Repeat the loop. Count: production apps deployed, days-without-runtime-incident, time-from-clone-to-URL. Publish the number weekly to yourself (private dashboard, not public).
6. **After the third production user, then and only then** — reopen the P2 shelf: grammar-constrained decoding, HumanEval-Vox, MENS hardening, SSoT consolidation. The research pillars become v1.x work.

### The one-sentence product posture

**Vox v1.0 is the fastest way for a single human-plus-AI team to ship and operate a real web app, and we will not add any feature that does not measurably shorten that loop.**

Everything in the audit is still valid as a long-term backlog. But the active working set, given these four answers, is ~25 items, not 100.

— End of audit v2.


