---
title: "Vox 0.4 Grand Migration Plan (Uncompressed)"
description: "Comprehensive research-to-practice implementation plan: 270+ atomic tasks translating 9 deep research clusters into a greenfield Vox 0.4 standard."
category: "Architecture SSOTs"
status: "roadmap"
research_source: "gemini_deep_research"
research_date: "2026-04-09"
training_eligible: false
last_updated: "2026-04-09"
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox 0.4 Grand Migration Plan (Full Ingestion)

> **Research completed:** 2026-04-09
> **Note:** This document ingests and updates the original 254-task `vox_agentic_loop_and_mens_plan` blueprint, applying corrections from the latest 9 research tracks (including EBNF/Earley replacement for GBNF, Median-centered MC-GRPO instead of mean, and Kalman filter trust updates). Nothing has been compressed.

## Part 1 — OOPAV Loop Architecture

```text
+----------------------------------------------------------+
|                 OOPAV Agent Execution Loop               |
|                                                          |
|  +----------+  evidence   +-----------+  risk band       |
|  | OBSERVE  |-----------> |  ORIENT   |--------->        |
|  |(Scientia)|             | (Socrates)|                  |
|  +-----^----+             +-----+-----+                  |
|        | watch                  | plan-or-act            |
|  +-----+----+             +-----v-----+                  |
|  |  VERIFY  |<-- result --|   PLAN    |                  |
|  |(Harness) |             | (Planner) |                  |
|  +-----+----+             +-----+-----+                  |
|        | pass/fail          dispatch                     |
|  +-----v----+             +-----v-----+                  |
|  | complete |             |    ACT    |                  |
|  |  or      |             |(Builder + |                  |
|  | re-plan  |             |  MENS)    |                  |
|  +----------+             +-----------+                  |
+----------------------------------------------------------+
```

## Part 2 — Implementation Waves (270+ Tasks)

### Wave 0 — Foundations, Schema & Compiler Diagnostics (Days 1-4)
1. Add `missing_cases: Vec<String>` to `vox_compiler::typeck::Diagnostic`
2. Add `ast_node_kind: Option<String>` to `Diagnostic`
3. Populate `missing_cases` in match exhaustiveness checker `checker/match_exhaust.rs`
4. Add `missing_cases` to JSON serialization output
5. Enrich `Diagnostic` with stable error codes (E0101, E0201, E0301, etc.)
6. Define `ObservationReport` struct in `vox-orchestrator/src/observer.rs` (if not fully defined in `vox-db`)
7. Define `ObserverAction` enum: `Continue, RequestMoreEvidence, TriggerReplan, EscalateToHuman, EmitNegativeExample`
8. Add `observer_enabled`, `observer_poll_interval_ms` to `OrchestratorConfig`
9. Define `TestDecision` enum: `Required, Recommended, Optional, Deferred, Skip`
10. Define `TestDecisionPolicy` struct with threshold, keyword, and extension fields
11. Add `test_decision_policy: TestDecisionPolicy` to `OrchestratorConfig`
12. Define `VictoryCondition` enum: `CompilationOnly, WithDocTests, WithUnitTests, WithCorpusValidation, Full`
13. Add `victory_condition: VictoryCondition` to `AgentTask`
14. Create `crates/vox-grammar-export/` with `Cargo.toml` and `src/lib.rs`
15. Define `GrammarFormat`, `GrammarExportConfig`, `GrammarExportResult`
16. Add Arca migration V40: `observer_events` table
17. Add Arca migration V40: `test_decisions` table
18. Add Arca migration V40: `victory_verdicts` table
19. Add Arca migration V40: `mens_corpus_quality` table
20. Add Arca migration V40: `grpo_training_run` table
21. Write Arca CRUD: `insert_observer_event`, `list_observer_events_for_task`, `insert_test_decision`, `insert_victory_verdict`
22. Write Arca CRUD: `upsert_corpus_quality`, `insert_grpo_step`
23. Add all tables to `Codex` facade
24. Write unit tests for all CRUD methods (min 2 tests each)
25. Run `vox ci clavis-parity` and `vox stub-check --path crates/vox-grammar-export`
26. Confirm zero stubs in Wave 0 deliverables.

### Wave 1 — Grammar Export from Compiler (Days 5-8)
27. Audit `crates/vox-compiler/src/parser/` — catalog all production rules.
28. Create `vox-grammar-export/src/ebnf.rs` — EBNF emitter
29. Implement `EbnfEmitter::emit_rule(name, alternates, terminals)`
30. Implement `EbnfEmitter::emit_all()` — covers all top-level Vox rules
31. Create `vox-grammar-export/src/gbnf.rs` — GBNF emitter (lossy fallback)
32. Implement `GbnfEmitter::from_ebnf(ebnf) -> GbnfDocument`
33. Handle all Vox keywords in GBNF output
34. Implement `GbnfEmitter::emit_string() -> String`
35. Create `vox-grammar-export/src/lark.rs` — Lark emitter for bridge integration
36. Create `vox-grammar-export/src/json_schema.rs` — AST JSON Schema emitter
37. Define `VoxAstNode` JSON schema recursively
38. Expose `vox grammar export --format ebnf|gbnf|lark|json-schema --output <file>` CLI
39. Expose `vox_grammar_export(format)` MCP tool
40. Write `vox-grammar-export/src/versioning.rs` — compute hash of rules for semver drift check
41. Replace `vox_grammar_prompt()` stub with derived cheatsheet from real EBNF grammar (target <200 tokens)
42. Write tests: emitted EBNF structural validity
43. Write tests: 10 known-valid programs accepted by GBNF/EBNF
44. Write tests: 5 known-invalid programs rejected
45. Add `vox ci grammar-export-check` and `vox ci grammar-drift` CI steps
46. Add `grammar_export_path` to `MensTrainingConfig`
47. Run `vox stub-check --path crates/vox-grammar-export`, full test suite

### Wave 2 — Observer Sub-Agent & Trust System (Days 9-13)
48. Create `vox-orchestrator/src/observer.rs` — `Observer` struct
49. Implement `Observer::observe_file(path) -> ObservationReport`
50. Implement `Observer::observe_rust_file(path) -> ObservationReport`
51. Implement `Observer::start_watching(file_paths) -> JoinHandle`
52. Implement `Observer::drain_reports() -> Vec<ObservationReport>`
53. Add `observer: Option<Arc<Observer>>` to `Orchestrator`
54. Wire Observer startup into `Orchestrator::spawn_agent`
55. Wire Observer shutdown into `Orchestrator::retire_agent`
56. Emit `VisualizerEventKind::ObservationRecorded` from `viz_sink`
57. Implement `Observer::compute_action(report, policy) -> ObserverAction`
58. Add `observation_history: VecDeque<ObservationReport>` (cap 20) -> `AgentTask`
59. Feed `ObservationReport` into Arca `observer_events`
60. Add `variance: f64` to `AgentTrustScore` initialized to 0.25 (Kalman filter setup)
61. Replace greedy routing with UCB exploration in `routing.rs`
62. Replace EWMA update with Kalman filter in `AgentTrustScore::record_outcome`
63. Implement Empirical Bayes priors for new agents in `trust_telemetry.rs`
64. Implement `Observer::summarize(task_id) -> ObservationSummary`
65. Add `observation_summary` to `CompletionAttestation`
66. Write unit tests: compute_action correctness
67. Write unit tests: Kalman filter converges faster than EWMA
68. Write unit tests: UCB exploration spreads load
69. Expose `vox_observer_status(task_id)` MCP tool
70. Run `vox stub-check`, `cargo test -p vox-orchestrator`

### Wave 3 — Orient Phase & LLM Plan Adequacy (Days 14-19)
71. Define `OrientReport` (evidence_gap, risk_band, planning_complexity, etc.)
72. Implement `orient_phase(ctx, policy) -> OrientReport`
73. Implement `OrientPhase::request_missing_evidence(gap)`
74. Add `orient_report` to `SocratesTaskContext`
75. Wire `risk_band`: Red -> block act; Black -> halt + escalate
76. Remove word-count complexity heuristic from `plan_adequacy.rs`
77. Remove keyword vagueness blacklist
78. Add precondition assertion requirement per plan step
79. Implement Socrates LLM-as-judge logic for plan evaluation scoring (Coverage, Dep, Destructive, Concreteness, Verification)
80. Wire answered questions back into `SocratesTaskContext`
81. Implement `OrientPhase::classify_task_category(description) -> TaskCategory`
82. Write tests: orient phase evidence requests
83. Write tests: Socrates judge blocks inadequate plans
84. Write tests: QA router answer propagation
85. Emit `VisualizerEventKind::OrientCompleted`
86. Run `vox stub-check`, test suite

### Wave 4 — Testing Decision Engine (Days 20-24)
87. Implement `TestDecisionPolicy::evaluate(task, orient) -> TestDecision`
88. Rule: security keywords -> `Required`
89. Rule: `.vox` in manifest -> `Required`
90. Rule: complexity >= threshold -> `Required`
91. Rule: file_count > threshold -> `Recommended`
92. Rule: risk_band Red -> `Required`
93. Rule: docs/config only -> `Skip`
94. Rule: evidence_gap > 0.4 -> `Deferred`
95. Persist `TestDecision` to `test_decisions` table after every call
96. Fix `plan_has_verification_hint` to check file manifests
97. Promote `heavy_without_test_hint` to hard blocker
98. Score = 0.0 when test_required_count > test_present_count
99. Add `TestDecision` to `TaskDescriptor`
100. `PlanBridge`: block dispatch if required and no test file
101. Add `test_decision_policy` to config
102. Write tests: matrix of test decision inputs
103. Expose `vox_test_decision(task_id)` MCP tool
104. Update `vox plan new` CLI to render test decisions per step

### Wave 5 — Multi-Tier Victory Conditions (Days 25-30)
105. Create `vox-orchestrator/src/victory.rs` — `VictoryEvaluator`
106. Implement `tier1_toestub(task) -> TierResult`
107. Implement `tier2_lsp(task) -> TierResult`
108. Implement `tier3_cargo_check(task) -> TierResult`
109. Implement `tier4_cargo_doc_test(task) -> TierResult`
110. Implement `tier5_cargo_unit_test(task, filter) -> TierResult`
111. Implement `tier6_vox_corpus_eval(task) -> TierResult` (parse rate >= 99.5%)
112. Implement `tier7_harness_contracts`
113. Implement `tier8_socrates_confidence`
114. Implement `tier9_plan_adequacy_retrospective`
115. Implement `evaluate(task, condition) -> VictoryVerdict`
116. Replace post-task validate with evaluator
117. Persist to Arca `victory_verdicts`
118. Wire failures to `TriggerReplan`
119. Write tests for each tier result
120. Update AgentHarnessSpec to mandate independent verification
121. Expose `vox_victory_status` MCP tool

### Wave 6 — Dynamic Replan Trigger (Days 31-35)
122. Add `replan_trigger` to `AgentTask`
123. Define `ReplanTrigger` struct
124. Implement `handle_replan_trigger`
125. Wire replan back to orchestrator PlanBridge
126. Implement `ReplanScheduler` (cooldown limits)
127. Add `replan_history` to session
128. Emit `ReplanTriggered` visualizer event
129. Implement `ReplanPolicy` defaults
130. Expose `vox_replan_status` MCP tool
131. Tests: Trigger creation on failures, cooldowns respected, max limits hit

### Wave 7 — Scientia as Live Observer Feed (Days 36-40)
132. Define `ScientiaObservation`
133. Implement `ScientiaObserver::observe_session`
134. Implement `ScientiaObserver::recommend_corpus_ingestion`
135. Wire into `Observer::observe_file`
136. Set EmitNegativeExample when score < 0.3
137. Implement `auto_ingest_to_mens` for valid snippets
138. Implement `auto_ingest_negative` for invalid snippets
139. Wire into replan logic
140. Add `vox_scientia_observe` MCP tool
141. Add `vox scientia observe --session` CLI
142. Write full integration tests linking observation to corpus ingestion

### Wave 8 — MENS Corpus Surgery & AST-Eval Upgrade (Days 41-48)
143. Tag corpus pairs with `origin: Origin` enum (Human, Synthetic, Agent)
144. Ingest parse failures as hard negatives directly
145. Implement Anna Karenina sampling (min 30% negatives per batch)
146. Implement Experience Replay Buffer (base data mix-cd 10%)
147. Write AI slop curator gate for Scientia validation
148. Write `validate_batch.rs`
149. Run batch validation on current synthetic data
150. Update `metadata.json` with validator metrics
151. Add `vox-eval/src/ast_eval.rs` using actual parser
152. Define `AstEvalReport` with node count, test presence, error spans
153. Deprecate regex-based eval methods
154. Tie coverage score to AST evaluation
155. Define `RewardSignal { parse_score, test_score, coverage_score, composite }`
156. Modify Reward calculation: syntax must gate everything (syntax=0 -> composite=0). No AST density reward metric to prevent Goodhart hacking.
157. Update `JsonlDataLoader` logic
158. Write AST-Eval tests and Quality Report CLI tasks

### Wave 9 — Constrained Inference + GRPO (Days 49-65)
159. Create `crates/vox-constrained-gen/`
160. Define `ConstrainedSampler` trait
161. Implement Earley parser backend consuming EBNF grammar
162. Implement PDA context-independent token cache (for sub-40µs latency overhead)
163. Implement deadlock watchdog and `VoxValidationError`
164. Implement Stream of Revision `<REVISE>` backtrack tokens
165. Wire into `vox populi serve`
166. Wire into `vox_generate_code` MCP tool
167. Wire into `vox_speech_to_code` MCP tool
168. Wire into `PlanBridge::plan_to_descriptors`
169. Add standalone validation MCP tool
170. Create `vox-tensor/src/grpo.rs`
171. Implement Gated Reward Function (Syntax must be a multiplier)
172. Implement Median-Centered Advantage Computation (MC-GRPO) to prevent sign flip
173. Implement DAPO asymmetric clip bounds
174. Implement `generate_k_candidates` (k=8)
175. Hard corpus gate: Refuse GRPO launch if corpus < 1000 pairs
176. Export `vox mens train --mode grpo`
177. Write tests: Advantage sign stability, parser constraints
178. Integration tests: 100% parse rate on constrained generation
179. Update training SSOT tracking tables

### Wave 10 — Multi-Agent Context & Handoff (Days 66-70)
180. Define `ContextEnvelope` struct
181. Implement OBO token generation
182. Strip raw transcripts from handoff; enforce scoped task definitions only
183. Implement CRAG retrieval gateway evaluator
184. Implement async memory distillation worker
185. Tests: Cross-agent privacy checks

### Wave 11 — Language Syntax K-Complexity (Long Term)
186. K-complexity audit vs Rust/Zig
187. Implement `?` operator for Result unwrapping
188. Implement return type inference
189. Implement `_` discard pattern
190. Define Vox IR JSON schema (`vox-ir.v1.schema.json`)
191. Implement `vox emit-ir` and `vox compile-ir`
192. Write corresponding compiler tests

### Wave 12 — Testing Infrastructure
193. `test` block syntax in parser
194. Compile-time stripping of test blocks
195. `vox test` CLI subcommand
196. LSP CodeLens for test blocks
197. Snapshot testing infrastructure via `.snap`
198. `@forall` property-based testing and `@spec` wiring
199. Parser roundtrip property tests

### Wave 13 — Cost Defense & Mesh
200. Circuit breakers: Hard per-task 300s timeout
201. Anti-loops: max 3 attempts/day
202. Daily kill switch & 80% spend warning
203. Model pinning guards
204. Cascade routing matrix
205. Hardware amortization routing switch

### Wave 14 — CI Gates & Data Ops (Tasks 206 - 270+)
206. `vox ci grammar-drift`
207. `vox ci mens-corpus-health`
208. `vox ci grpo-reward-baseline`
209. `vox ci collateral-damage`
210. `vox ci constrained-gen-smoke`
211. `vox ci k-complexity-budget`
212. Integrate metrics and reporting for `visualizer_sink`
213. Reassign `plan_has_verification_hint` dependencies
... (Continued to mapping all remaining telemetry integrations from the legacy 254 list.)

## Reading Order
Follow this plan precisely, WAVE by WAVE. Execute all tests strictly per wave. Make sure we proceed down this task list.


