---
title: "Vox Agentic Loop Overhaul + MENS Syntax-Intelligence Blueprint"
description: "254-task implementation blueprint covering OOPAV agent loop, MENS grammar-aware training, and GRPO reinforcement learning."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-05"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox Agentic Loop Overhaul + MENS Syntax-Intelligence Blueprint

> **Research completed:** 2026-04-05
> **Two interlocked workstreams:**  
> 1. **Agentic Loop** — Observe → Orient → Plan → Act → Verify (OOPAV)  
> 2. **MENS Syntax Intelligence** — Grammar-aware training, constrained inference, MCP pre-emit validation

---

## Part 0 — Gap & Limitation Audit (20 Gaps)

| # | Gap | Evidence location |
|---|-----|-------------------|
| G-01 | No Observer role — nothing watches the environment between steps | `orchestrator/agent_lifecycle.rs`, `planning/mod.rs` |
| G-02 | Completeness declared too early — `cargo check` only, no `cargo test` or Vox parse-rate gate | `validation.rs:161-183` |
| G-03 | Testing decision hard-wired — `heavy_without_test_hint` is a soft penalty, never blocks | `plan_adequacy.rs:321` |
| G-04 | Plan complexity is word-count heuristic — caps at 9, under-detects complex refactors | `plan_adequacy.rs:48-58` |
| G-05 | Socrates gate is post-hoc — scoring happens after LLM commits, not before | `socrates.rs` |
| G-06 | `HarnessGate.independent_verification` always `false` | `harness.rs:244-250` |
| G-07 | `QARouter::answer()` discards the answer — `_answer: &str` unused | `qa.rs:55` |
| G-08 | No autonomic replan trigger — only user-driven via `vox_replan` | `planning/replan.rs` |
| G-09 | Scaling ignores observer load / evidence quality | `orchestrator/scaling.rs` |
| G-10 | Scientia is a publication layer, not a live observation source | `vox-scientia-core/src/lib.rs` |
| G-11 | MENS corpus only 340 pairs, 39 negatives | `mens/data/metadata.json` |
| G-12 | `vox_grammar_prompt()` is a 27-line hand-written stub | `compiler/src/llm_prompt.rs` |
| G-13 | `golden_validated.jsonl` is 60 bytes (empty) | `mens/data/golden_validated.jsonl` |
| G-14 | No grammar-constrained decoding at inference | `inference_and_serving.md` |
| G-15 | `vox-eval` uses regex, not the real parser | `vox_eval_crate.md` |
| G-16 | No GRPO/RLVR training loop — SFT only | `training_orchestration.md` |
| G-17 | MCP code emit has no pre-validation before file write | `vox-mcp/` |
| G-18 | `vox_schola_submit` failures not converted to negative examples | MCP tool `vox_schola_submit` |
| G-19 | `plan_has_verification_hint` ignores file manifests | `plan_adequacy.rs:259-271` |
| G-20 | `fatigue_active` penalty never propagated to planner thresholds | `socrates.rs:271-276` |

archived_date: 2026-04-18
---

## Part 1 — OOPAV Loop Architecture

```
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

### Testing Decision Policy

```
Required    -> security/auth/schema keywords in description
Required    -> .vox file in manifest
Required    -> complexity >= 7 AND file_count > 2
Required    -> orient.risk_band == Red
Recommended -> new fn/type, >20 LOC estimate
Skip        -> docs-only or config-only manifest
Deferred    -> evidence_gap > 0.4
Optional    -> everything else
```

### 9-Tier Victory Conditions

| Tier | Check | When |
|------|-------|------|
| 1 | TOESTUB — zero stubs | Always |
| 2 | LSP zero errors on `.vox` write files | Always |
| 3 | `cargo check --workspace` | Always |
| 4 | `cargo test --doc --workspace` | `WithDocTests` or `Full` |
| 5 | `cargo test <filter>` | `TestDecision::Required` |
| 6 | `vox corpus eval` parse_rate >= 99.5% | Any `.vox` in manifest |
| 7 | Harness contract satisfaction | Always |
| 8 | Socrates confidence >= `answer_threshold` | Always |
| 9 | Plan adequacy retrospective >= 0.75 | `Full` |

---

## Part 2 — MENS Syntax Intelligence

### Grammar Export Pipeline

```
vox-compiler/src/parser/
    |  VoxGrammarExporter
    |-> EBNF text       -> docs/grammar/vox.ebnf
    |-> GBNF file       -> llama.cpp --grammar-file
    |-> JSON Schema     -> vox populi serve (constrained JSON mode)
```

### Corpus Verification Pipeline

```
synthetic.jsonl (3.2 MB, unverified)
    |  vox corpus validate-batch
    |-> synthetic_valid.jsonl   -> split=training
    |-> synthetic_invalid.jsonl -> split=negative + correction signal

golden_extracted.jsonl (16 KB)
    |  vox corpus validate-batch
    |-> golden_validated.jsonl  <- currently 60 bytes / EMPTY -> must reach >=500 pairs
```

### GRPO/RLVR Training Loop

```
for each prompt in training_set:
  candidates = generate_k(prompt, k=8, temperature=0.8)
  for each candidate:
    r_syntax   = vox_parser(candidate)         -> 0/1
    r_test     = run @test blocks              -> pass_rate
    r_coverage = ast_eval(candidate).score
    reward     = 0.6*r_syntax + 0.3*r_test + 0.1*r_coverage
  advantage_i = reward_i - mean(rewards)       # GRPO group mean baseline
  grpo_update(policy, advantages)
```

### MCP Pre-Emit Validation

```
vox_generate_code   -> mcp_pre_emit_validate("vox")
vox_speech_to_code  -> mcp_pre_emit_validate("vox")
PlanBridge step     -> mcp_pre_emit_validate("vox")
                             |
             parse OK?  -> write file
             parse ERR? -> VoxValidationError -> LLM retries
                        -> invalid snippet -> auto_ingest_negative(corpus)
```

archived_date: 2026-04-18
---

## Part 3 — Implementation Waves (254 Tasks)

---

### Wave 0 — Foundations & Schema (Days 1-3)

1. Define `ObservationReport` struct in `vox-orchestrator/src/observer.rs`
2. Define `ObserverAction` enum: `Continue`, `RequestMoreEvidence`, `TriggerReplan`, `EscalateToHuman`, `EmitNegativeExample`
3. Add `observer_enabled`, `observer_poll_interval_ms` to `OrchestratorConfig`
4. Define `TestDecision` enum: `Required`, `Recommended`, `Optional`, `Deferred`, `Skip`
5. Define `TestDecisionPolicy` struct with threshold, keyword, and extension fields
6. Add `test_decision_policy: TestDecisionPolicy` to `OrchestratorConfig`
7. Define `VictoryCondition` enum: `CompilationOnly`, `WithDocTests`, `WithUnitTests`, `WithCorpusValidation`, `Full`
8. Add `victory_condition: VictoryCondition` to `AgentTask`
9. Create `crates/vox-grammar-export/` with `Cargo.toml` and `src/lib.rs`
10. Define `GrammarFormat`, `GrammarExportConfig`, `GrammarExportResult`
11. Add Arca migration V38: `observer_events` table
12. Add Arca migration V38: `test_decisions` table
13. Add Arca migration V38: `victory_verdicts` table
14. Add Arca migration V38: `mens_corpus_quality` table
15. Add Arca migration V38: `grpo_training_run` table
16. Write Arca CRUD: `insert_observer_event`, `list_observer_events_for_task`, `insert_test_decision`, `insert_victory_verdict`, `upsert_corpus_quality`, `insert_grpo_step`
17. Add all five tables to `Codex` facade
18. Write unit tests for all CRUD methods (min 2 tests each)
19. Run `vox ci clavis-parity` and `vox stub-check --path crates/vox-grammar-export`
20. Confirm zero stubs in Wave 0 deliverables

archived_date: 2026-04-18
---

### Wave 1 — Grammar Export from Compiler (Days 4-7)

21. Audit `crates/vox-compiler/src/parser/` — catalog all production rules; write `docs/src/architecture/vox-grammar-production-rules.md`
22. Create `vox-grammar-export/src/ebnf.rs` — EBNF emitter
23. Implement `EbnfEmitter::emit_rule(name, alternates, terminals)`
24. Implement `EbnfEmitter::emit_all()` — covers all top-level Vox rules
25. Create `vox-grammar-export/src/gbnf.rs` — GBNF emitter for `llama.cpp`
26. Implement `GbnfEmitter::from_ebnf(ebnf) -> GbnfDocument`
27. Handle all Vox keywords in GBNF output
28. Implement `GbnfEmitter::emit_string() -> String`
29. Create `vox-grammar-export/src/json_schema.rs` — AST JSON Schema emitter
30. Define `VoxAstNode` JSON schema recursively
31. Expose `vox grammar export --format ebnf|gbnf|json-schema --output <file>` CLI
32. Expose `vox_grammar_export(format)` MCP tool
33. Write `vox-grammar-export/src/versioning.rs` — semver embedding + drift check
34. Replace `vox_grammar_prompt()` stub with derived cheatsheet from real grammar
35. Write tests: emitted EBNF structural validity
36. Write tests: 10 known-valid programs accepted by the GBNF
37. Write tests: 5 known-invalid programs rejected by the GBNF
38. Add `vox ci grammar-export-check` CI step
39. Add `grammar_export_path` to `MensTrainingConfig`
40. Run `vox stub-check --path crates/vox-grammar-export`; full test suite

---

### Wave 2 — Observer Sub-Agent (Days 8-12)

41. Create `vox-orchestrator/src/observer.rs` — `Observer` struct
42. Implement `Observer::observe_file(path) -> ObservationReport`
43. Implement `Observer::observe_rust_file(path) -> ObservationReport`
44. Implement `Observer::start_watching(file_paths) -> JoinHandle`
45. Implement `Observer::drain_reports() -> Vec<ObservationReport>`
46. Add `observer: Option<Arc<Observer>>` to `Orchestrator`
47. Wire Observer startup into `Orchestrator::spawn_agent`
48. Wire Observer shutdown into `Orchestrator::retire_agent`
49. Emit `VisualizerEventKind::ObservationRecorded` from `viz_sink`
50. Implement `Observer::compute_action(report, policy) -> ObserverAction`
51. Add `observation_history: VecDeque<ObservationReport>` (cap 20) -> `AgentTask`
52. Feed `ObservationReport` into Arca `observer_events`
53. Implement `Observer::summarize(task_id) -> ObservationSummary`
54. Add `observation_summary: Option<ObservationSummary>` to `CompletionAttestation`
55. Write unit tests: compute_action correctness
56. Write integration test: Observer on known-bad `.vox` → errors within 2 polls
57. Write integration test: Observer on `.rs` with `todo!()` → `EmitNegativeExample`
58. Write tests: `summarize` computes parse_rate trend from 3 sequential reports
59. Expose `vox_observer_status(task_id)` MCP tool
60. Run `vox stub-check`, `cargo test -p vox-orchestrator`

archived_date: 2026-04-18
---

### Wave 3 — Orient Phase & Enhanced Socrates (Days 13-17)

61. Define `OrientReport { evidence_gap, missing_namespaces, recommended_retrieval, risk_band, planning_complexity_multiplier }`
62. Implement `orient_phase(ctx, policy) -> OrientReport`
63. Add `evidence_gap_threshold` to `ConfidencePolicy`
64. Implement `OrientPhase::request_missing_evidence(gap) -> Vec<SearchResult>`
65. Add `orient_report: Option<OrientReport>` to `SocratesTaskContext`
66. Integrate `orient_phase()` into `runtime.rs` before each LLM inference request
67. Wire `risk_band`: `Red` -> block act; `Black` -> halt + escalate
68. Wire `planning_complexity_multiplier` into `PlannerConfig`
69. Implement `OrientPhase::propagate_fatigue(fatigue_active, config)`
70. Implement `OrientPhase::auto_dispatch_socratic_question(gap) -> CorrelationId`
71. Fix `QARouter::answer()` — store answer; add `get_answer(corr_id) -> Option<String>`
72. Wire answered questions back into `SocratesTaskContext`
73. Implement `OrientPhase::classify_task_category(description) -> TaskCategory`
74. Write tests: `orient_phase` with zero evidence -> `RequestMoreEvidence`
75. Write tests: `propagate_fatigue(true)` raises thresholds by >= 2
76. Write tests: `classify_task_category` returns `Security` for auth keywords
77. Write tests: `auto_dispatch_socratic_question` creates QARouter entry
78. Write tests: `get_answer()` returns stored string
79. Emit `VisualizerEventKind::OrientCompleted { risk_band, evidence_gap }`
80. Run `vox stub-check`, `cargo test -p vox-orchestrator`

---

### Wave 4 — Testing Decision Engine (Days 18-22)

81. Implement `TestDecisionPolicy::evaluate(task, orient) -> TestDecision`
82. Rule: security keywords -> `Required`
83. Rule: `.vox` in manifest -> `Required`
84. Rule: complexity >= threshold -> `Required`
85. Rule: file_count > threshold -> `Recommended`
86. Rule: risk_band Red -> `Required`
87. Rule: docs/config only -> `Skip`
88. Rule: evidence_gap > 0.4 -> `Deferred`
89. Rule: default -> `Optional`
90. Persist `TestDecision` to `test_decisions` table after every call
91. Fix `plan_has_verification_hint` to check file manifests
92. Promote `heavy_without_test_hint` to hard blocker `test_required_missing`
93. Add `test_required_count`, `test_present_count` to `PlanAdequacySummary`
94. Score = 0.0 when `test_required_count > test_present_count` for coding goals
95. Add `TestDecision` to `TaskDescriptor`
96. `PlanBridge`: block dispatch if `Required` and no test file in manifest
97. Add `test_decision_policy` to `OrchestratorConfig` with sane defaults
98. Write tests: auth migration -> `Required`
99. Write tests: markdown-only manifest -> `Skip`
100. Write tests: complexity-8 `.vox` with no test step -> `is_too_thin=true`, `test_required_missing`
101. Write tests: test file in manifest -> `plan_has_verification_hint=true`
102. Write tests: `PlanBridge` blocks `Required` task with no test file
103. Expose `vox_test_decision(task_id)` MCP tool
104. Update `vox plan new` CLI to render test decisions per step
105. Run `vox stub-check`, full test suite

archived_date: 2026-04-18
---

### Wave 5 — Multi-Tier Victory Conditions (Days 23-28)

106. Create `vox-orchestrator/src/victory.rs` — `VictoryEvaluator`
107. Implement `tier1_toestub(task) -> TierResult`
108. Implement `tier2_lsp(task) -> TierResult`
109. Implement `tier3_cargo_check(task) -> TierResult`
110. Implement `tier4_cargo_doc_test(task) -> TierResult` (120s timeout)
111. Implement `tier5_cargo_unit_test(task, filter) -> TierResult`
112. Implement `tier6_vox_corpus_eval(task) -> TierResult` (parse_rate >= 99.5%)
113. Implement `tier7_harness_contracts(task, harness) -> TierResult`
114. Implement `tier8_socrates_confidence(task, ctx, policy) -> TierResult`
115. Implement `tier9_plan_adequacy_retrospective(task) -> TierResult`
116. Implement `VictoryEvaluator::evaluate(task, condition) -> VictoryVerdict`
117. Define `VictoryVerdict { passed, tiers_run, first_failure, report }`
118. Replace `post_task_validate` with `VictoryEvaluator::evaluate`
119. Persist every `VictoryVerdict` to Arca `victory_verdicts`
120. Wire `passed=false` -> `TriggerReplan` via Observer
121. Add `max_victory_attempts: u32` to `AgentTask` (default 3)
122. Emit `VisualizerEventKind::VictoryEvaluated`
123. Update `AgentHarnessSpec::minimal_contract_first` — `independent_verification: true` for code tasks
124. Write tests: `tier3` fails on bad Rust
125. Write tests: `tier6` fails on invalid Vox
126. Write tests: `Full` passes for clean files + high confidence
127. Write tests: stub code -> `first_failure = TierResult::Toestub`
128. Write tests: `max_victory_attempts` guard
129. Expose `vox_victory_status(task_id)` MCP tool
130. Run `vox stub-check`, full test suite

---

### Wave 6 — Dynamic Replan Trigger (Days 29-33)

131. Add `replan_trigger: Option<ReplanTrigger>` to `AgentTask`
132. Define `ReplanTrigger { reason, failed_tier, observer_action, evidence_gaps }`
133. Implement `runtime.rs::handle_replan_trigger(task, trigger)`
134. Wire replan result back into orchestrator via `PlanBridge`
135. Add `replan_count: u32` to `AgentTask`; fail permanently after max
136. Implement `ReplanScheduler` — max 1 replan per 30s per session
137. Implement `ReplanScheduler::should_replan(task) -> bool`
138. Add `replan_history: Vec<ReplanRecord>` to `PlanSession`
139. Define `ReplanRecord { version, trigger_reason, previous_score, new_score, created_at }`
140. Emit `VisualizerEventKind::ReplanTriggered`
141. Implement `ReplanPolicy` in `planning/policy.rs`
142. Add `replan_policy: ReplanPolicy` to `OrchestratorConfig`
143. Expose `vox_replan_status(session_id)` MCP tool
144. Write tests: failed tier3 -> ReplanTrigger created -> replan called
145. Write tests: ReplanScheduler returns false within cooldown
146. Write tests: permanent failure after max replans
147. Write tests: replan_history persisted and retrievable
148. Write tests: MCP returns correct count and reason
149. Update `vox plan replan` CLI
150. Run full test suite, `vox stub-check`

archived_date: 2026-04-18
---

### Wave 7 — Scientia as Live Observer Feed (Days 34-38)

151. Audit `vox-scientia-*` crates; write `docs/src/architecture/scientia-surface-audit.md`
152. Define `ScientiaObservation { session_id, source_path, worthiness_score, construct_coverage, citation_count, recommended_for_corpus, reason }`
153. Implement `ScientiaObserver::observe_session(session_id) -> ScientiaObservation`
154. Implement `ScientiaObserver::recommend_corpus_ingestion(obs) -> bool`
155. Wire into `Observer::observe_file` for `.vox` files
156. Set `EmitNegativeExample` when `worthiness_score < 0.3`
157. Implement `ScientiaObserver::auto_ingest_to_mens(obs, codex)` -> `split=training` row
158. Implement `ScientiaObserver::auto_ingest_negative(path, error, codex)` -> `split=negative` row
159. Wire into `handle_replan_trigger` — replans >= max/2 emit negatives
160. Add `scientia_observation: Option<ScientiaObservation>` to `ObservationReport`
161. Expose `vox_scientia_observe(session_id)` MCP tool
162. Add `vox scientia observe --session <id>` CLI subcommand
163. Write tests: `recommend_corpus_ingestion` true for valid snippet with 3 constructs
164. Write tests: `auto_ingest_to_mens` inserts training row
165. Write tests: `auto_ingest_negative` inserts negative row
166. Write tests: full pipeline — Observer -> Scientia -> corpus row
167. Emit `VisualizerEventKind::ScientiaObserved`
168. Expose in VS Code extension telemetry push
169. Update `governance.md`
170. Run full test suite, `vox stub-check`

---

### Wave 8 — MENS Corpus Surgery & AST-Eval Upgrade (Days 39-46)

171. Write `vox-corpus/src/validate_batch.rs` — batch parse validation
172. Run validate-batch on `synthetic.jsonl` -> `synthetic_valid.jsonl` + `synthetic_invalid.jsonl`
173. Run validate-batch on `golden_extracted.jsonl` -> populate `golden_validated.jsonl`
174. Update `mens/data/metadata.json` with `parse_rate`, `last_validated_at`, `validator_version`
175. Implement `vox-eval/src/ast_eval.rs` — `ast_eval(code) -> AstEvalReport` using real parser
176. Define `AstEvalReport { parse_success, node_count, max_depth, construct_histogram, type_annotation_rate, has_tests, error_span }`
177. Implement `AstEvalReport::coverage_score()` — weighted composite
178. Update `vox-eval/src/lib.rs` — re-export `ast_eval`; `#[deprecated]` on `detect_constructs`
179. Update `construct_coverage_score(code)` to delegate to AST eval
180. Update `vox eval --mode ast` CI integration
181. Upgrade `vox corpus eval` to AST engine
182. Define `RewardSignal { parse_score, test_score, coverage_score, composite }` in `vox-tensor/src/data.rs`
183. Implement `reward_signal_for_pair(pair) -> RewardSignal`
184. Add `reward_signal: Option<RewardSignal>` to `TrainingPair`
185. Update `JsonlDataLoader` to compute `RewardSignal` during loading
186. Add `avg_reward_signal` per split to `metadata.json`
187. Add `vox corpus quality-report` CLI command
188. Add `mens/schemas/corpus_quality_record.schema.json`
189. **MILESTONE GATE: `golden_validated.jsonl` >= 500 pairs required before Wave 9**
190. Write tests: `ast_eval` on valid Vox function -> `parse_success=true`
191. Write tests: `ast_eval` on invalid snippet -> `parse_success=false`, non-None `error_span`
192. Write tests: `reward_signal_for_pair` -> `composite >= 0.8` for well-formed pair with tests
193. Write tests: `validate_batch` correctly separates mixed JSONL
194. Run `vox stub-check --path crates/vox-eval`, `cargo test -p vox-eval`

archived_date: 2026-04-18
---

### Wave 9 — Constrained Inference + GRPO Loop + MCP Pre-Emit (Days 47-60)

195. Create `crates/vox-constrained-gen/` — grammar-constrained token sampling
196. Implement `ConstrainedSampler::from_gbnf(gbnf_text) -> ConstrainedSampler` (FSA from Wave 1 GBNF)
197. Implement `ConstrainedSampler::mask_logits(logits, state) -> FsaState`
198. Integrate into `vox populi serve` via `?grammar=vox` or `X-Vox-Grammar: true`
199. Add `constrained_generation: bool` to `MensServeConfig`
200. Implement fallback: grammar deadlock -> `VoxValidationError`, request retry
201. Create `vox-constrained-gen/src/llguidance_bridge.rs` (optional feature-gated)
202. Define `VoxValidationError { code, span, message, suggested_correction }` in `vox-compiler/src/error.rs`
203. Implement `mcp_pre_emit_validate(code, format) -> Result<(), VoxValidationError>` in `vox-mcp/src/code_validator.rs`
204. Wire into `vox_generate_code` MCP tool
205. Wire into `vox_speech_to_code` MCP tool
206. Wire into `PlanBridge::plan_to_descriptors` for `.vox` steps
207. Implement Rust pre-emit: `rustc --parse-only` subprocess on temp file
208. Add `vox_validate_code(code, language) -> { valid, errors }` standalone MCP tool
209. Implement `MensGrpoTrainer::train_grpo(config, data) -> GrpoTrainingResult` in `vox-tensor/src/grpo.rs`
210. Define `GrpoConfig { k_samples, temperature, reward_weights, policy_lr, clip_epsilon, max_steps }`
211. Define `RewardWeights { parse_weight, test_weight, coverage_weight }` defaults `(0.6, 0.3, 0.1)`
212. Implement `generate_k_candidates(prompt, model, k) -> Vec<String>`
213. Implement `score_candidate(candidate) -> RewardSignal`
214. Implement `compute_advantages(rewards) -> Vec<f32>` (group mean baseline)
215. Implement `policy_gradient_update(model, candidates, advantages)` (PPO-clip style)
216. Expose `vox mens train --mode grpo` CLI flag
217. Expose `--k 8 --reward parse:0.6,test:0.3,coverage:0.1` arguments
218. Add GRPO telemetry: `group_rewards`, `mean_reward`, `policy_loss`, `clip_fraction` per step
219. Persist to Arca `grpo_training_run` table
220. Define `GrpoTrainingResult { steps_completed, final_mean_reward, parse_rate, checkpoint_path }`
221. Fix G-18: `vox_schola_submit` failures -> `auto_ingest_negative`
222. Add `vox mens eval --mode grpo-reward` (dry-run)
223. Add `mens/config/grpo_default.toml` (k=8, temp=0.8, max_steps=500)
224. Write tests: `compute_advantages` correctness
225. Write tests: constrained sampler produces only grammar-accepted tokens
226. Write tests: `mcp_pre_emit_validate` -> error for missing closing `}`
227. Write tests: `mcp_pre_emit_validate` -> `Ok(())` for valid function
228. Write tests: `vox_validate_code` -> errors for invalid Rust
229. Write tests: GRPO loop completes 10 steps without panic on RTX 4080 SUPER
230. Write tests: `train --mode grpo` -> checkpoint with `final_mean_reward > 0.5`
231. Integration test: constrained generation -> 100% parse rate on 50 generations
232. Integration test: invalid snippet via MCP -> `VoxValidationError`, no file written
233. Integration test: GRPO model vs SFT baseline -> >= 5pp parse rate improvement
234. Run `vox stub-check --path crates/vox-constrained-gen crates/vox-mcp`, `cargo test --workspace`
235. Update `docs/src/architecture/mens-training-ssot.md`
236. Update `examples/STYLE.md`
237. Add `vox ci grammar-constrained-gen-smoke-test`
238. Add `vox ci mens-corpus-health`
239. Add `vox ci grpo-reward-baseline`
240. Persist all CI results to Arca for trend analysis

---

## Part 4 — Observability & Telemetry (241-245)

241. Add `ObservationReport` to VS Code extension push-telemetry stream
242. Color-code agent viz nodes by `OrientReport.risk_band`
243. Add `VictoryVerdict` tier summary panel to workflow visualizer
244. Add `TestDecision` badge to each task card
245. Add `RewardSignal.composite` sparkline to MENS training progress panel

archived_date: 2026-04-18
---

## Part 5 — Documentation (246-254)

246. Write `docs/src/architecture/oopav-loop.md`
247. Write `docs/src/architecture/observer-design.md`
248. Write `docs/src/architecture/victory-conditions.md`
249. Write `docs/src/architecture/test-decision-policy.md`
250. Write `docs/src/architecture/mens-grammar-intelligence.md`
251. Update `docs/src/architecture/mens-training-ssot.md`
252. Update `docs/src/contributors/contributor-hub.md`
253. Update `AGENTS.md`
254. Update `docs/agents/governance.md`

---

## Milestone Gates

| After Wave | Gate |
|------------|------|
| 0 | All V38 Arca migrations applied; `vox stub-check` clean across all new crates |
| 1 | `vox grammar export --format gbnf` accepted by `llama.cpp --grammar-file` |
| 2 | Observer: live LSP error detection on modified `.vox` file integration test passes |
| 3 | Orient phase blocks `Red` band task from acting without evidence hydration |
| 4 | Complexity-8 `.vox` task with no test step rejected by `PlanBridge` |
| 5 | Full `VictoryCondition::Full` pass on a clean newly-generated Vox crate |
| 6 | Autonomic replan triggered and completed on a simulated tier-3 failure |
| 7 | `mens_corpus_quality` has >= 500 `split=training` rows from Scientia auto-ingestion |
| 8 | `golden_validated.jsonl` >= 500 pairs; AST eval parse_rate >= 99.5% |
| 9 | 100 consecutive constrained-inference generations parse_rate = 100%; GRPO dry-run `mean_reward > 0.4` |

archived_date: 2026-04-18
---

## Key Design Rationale

**GBNF over Outlines/llguidance first:** GBNF integrates natively with `llama.cpp` (already powering the local Populi server). `llguidance` added as optional bridge for dynamic grammars. Minimizes new dependencies.

**AST eval over regex:** Parse rate is binary. `AstEvalReport` provides a gradient signal — construct density, type annotation rate, test presence — enabling richer GRPO reward shaping.

**GRPO over PPO:** Eliminates the value network (critic), reducing memory ~40%. Critical under the 16 GB VRAM constraint on RTX 4080 SUPER. Group-relative baselines suit code generation's high candidate variance.

**Observer separate from Verifier:** Verifier is synchronous and post-hoc. Observer is asynchronous and continuous — allows Act to proceed without blocking while still delivering mid-flight course-corrections via `TriggerReplan`.

**MCP pre-emit failures as negative examples:** Each failure is high-signal teaching data. Invalid LLM-generated code becomes a structured negative pair (error = correction signal), closing the training loop organically without human annotation.


