# vox-db → vox-db-types move candidates

Audit produced for Phase 4 of the Vox-DB & Memory Management Audit PR
(plan: `docs/superpowers/plans/data-audit/2026-05-08-vox-db-and-memory-audit-pr.md`).

This is a working checklist for Phase 4.2-4.3. Each MOVE candidate gets
ticked off as it lands.

Method: walked every `pub use` line in `crates/vox-db/src/lib.rs`, found
the defining file with ripgrep, and inspected each type for connection,
actor handle, tokio, or `&VoxDb` references. When in doubt, marked KEEP
and noted why so a follow-up PR can revisit.

## Already in vox-db-types (no work needed)

These are already defined in `crates/vox-db-types/src/` and re-exported
from `vox-db` for back-compat. Listed for completeness so we don't
re-discover them in Phase 4.2-4.3.

- `EvalRunParams` — `crates/vox-db-types/src/eval_params.rs`
- `MemoryParams` (alias for `SaveMemoryParams<'a>`) — `crates/vox-db-types/src/lib.rs`
- `OratioEvalRunRecord`, `OratioEvalRunStartParams`, `OratioEvalSampleRecord` — `crates/vox-db-types/src/store_types/oratio.rs`
- `ObservationReport`, `ObserverAction`, `TestDecision`, `TestDecisionPolicy`, `VictoryCondition`, `VictoryVerdict`, `TierResult` — `crates/vox-db-types/src/store_types/mens.rs`
- `ExternalResearchPacket`, `ResearchIngestRequest`, `ResearchIngestResult`, `CapabilityMapRecord`, `ResearchEvalRunRecord`, `ResearchEvalSampleRecord` — `crates/vox-db-types/src/store_types/research.rs`
- All `*Params` request types (`SaveMemoryParams`, `SaveSnippetParams`, `LogExecutionParams`, `LogInteractionParams`, `ModelOutcome`, `ModelAttempt`, `PublishArtifactParams`, `PublicationManifestParams`, `PublicationMediaAssetParams`, `ExternalSubmissionJobUpsertParams`, `ExternalSubmissionAttemptParams`, `ExternalStatusSnapshotParams`, `PublicationExternalLinkUpsertParams`, `PublicationExternalRevisionUpsertParams`, `RegisterAgentParams`, `SkillExecutionParams`, `QuestionSessionCreateParams`, `QuestionEventParams`, `QuestionOptionParams`, `QuestionOptionOutcomeParams`, `QuestionStopEventParams`, `A2aClarificationMessageParams`, `UpsertAccountSecretCiphertextParams`, `ExternalReview*Params`) — `crates/vox-db-types/src/store_types/params.rs`
- All core entry rows (`ExecutionEntry`, `ScheduledEntry`, `ComponentEntry`, `MemoryEntry`, `EmbeddingEntry`, `LearnedPatternEntry`, `BehaviorEventEntry`, `CommandFrequencyEntry`, `TrainingPair`, `UserEntry`, `AgentDefEntry`, `SnippetEntry`, `PackageSearchResult`, `ArtifactEntry`, `SkillManifestEntry`, `KnowledgeNodeSummary`, `BuilderSessionEntry`, `SessionTurnEntry`, `TypedStreamEventEntry`, `ReviewEntry`, `CodexChangeLogEntry`, `NodeIdentityRow`, `ModelScoreboardRow`, `ModelPricingCatalogRow`) — `crates/vox-db-types/src/store_types/rows_core.rs`
- All extended rows (`SkillReliabilityReport`, `EndpointReliabilityEntry`, `TrustRollupEntry`, `CorpusRow`, `SkillExecutionRow`, `WorkflowExecutionRow`, `Question*Row`, `A2AMessageRow`, `AgentEventRow`, `BenchmarkEventRow`, `SessionRow`, `SessionEventRow`, `BuildRunRow`, `CrateSampleRow`, `CloudDispatchRow`, `ThroughputProfileRow`, `Publication*Row`, `ScholarlySubmissionRow`, `ExternalSubmissionJobRow`, `ExternalSubmissionAttemptRow`, `ExternalStatusSnapshotRow`, `PublicationExternalLinkRow`, `PublicationExternalRevisionRow`, `LocalTrainRow`, `WarningRow`, `Plan*Row`, `GamifyPolicySnapshotListRow`, `GamifyLudusKpiRollup`, `AccountSecretCiphertextRow`, `ExternalReview*Row`, `Visus*Row`) — `crates/vox-db-types/src/store_types/rows_extended.rs`

## Confirmed pure data — MOVE

Checked: only owned primitives / `String` / `Option` / `Vec` / sub-types
already in this list; no `turso::*`, no `tokio::*`, no `Arc<...>`, no
`&VoxDb`, no actor handles. All derive `serde::Serialize` (and most
`Deserialize`) and need only `serde` + maybe `chrono`/`thiserror` —
which `vox-db-types` already has.

- [x] `ExecOutcome` — `crates/vox-db/src/exec_time_telemetry.rs:23` — copy enum (`Success`/`Timeout`/`Error`); only an `as_str` method. No deps.
- [x] `ExecTimeRecord<'a>` — `crates/vox-db/src/exec_time_telemetry.rs:8` — borrowed-string struct; primitive + `Option` fields + `ExecOutcome`. No turso refs.
- [x] `ToolLatencyProfile` — `crates/vox-db/src/exec_time_telemetry.rs:41` — `String` + `f64`/`i64` fields; pure summary record.
- [x] `CircuitState` — `crates/vox-db/src/circuit_breaker.rs:41` — copy enum; pure marker. (`DbCircuitBreaker` itself stays.)
- [ ] `CircuitBreakerError` — `crates/vox-db/src/circuit_breaker.rs:52` — `thiserror` enum, single variant. **DEFERRED**: audit was wrong — `vox-db-types` does NOT depend on `thiserror`. Adding `thiserror` to vox-db-types is a sensitive new dep; deferred to a follow-up that plans the dep change explicitly.
- [ ] `WorkspaceTranscriptTurnRow` — `crates/vox-db/src/codex_chat.rs:16` — `String`/`Option<String>`/`u64` fields only. Sister of the `*Row` types already in `vox-db-types`.
- [ ] `CodexApiReadiness` — `crates/vox-db/src/codex_schema.rs:15` — `i64` + `String` + `Vec<String>` + `bool`. The two free functions (`evaluate_codex_api_readiness`, `missing_codex_reactivity_tables`) stay in vox-db; only the struct moves.
- [ ] `DbConfig` — `crates/vox-db/src/config.rs:3` — variants are pure `String`/`PathBuf`-shaped data. Some variants are feature-gated (`local`, `replication`); preserve those `cfg`s on the move. Heavy users: every `VoxDb::connect` caller; this is the highest-leverage move on the list.
- [ ] `DbConnectSurface` — `crates/vox-db/src/connect_policy.rs:17` — pure marker enum (`Mcp`, `Runtime`, `CliStrict`, etc.). `format_degraded_optional_connect` and the two `connect_canonical_*` helpers stay.
- [ ] `DataFlowMap` and `DataFlowEntry` — `crates/vox-db/src/data_flow.rs:12` (and following) — `Vec<DataFlowEntry>` / `Vec<String>` only. Move both as a pair. `build_data_flow` (constructor) operates on a `SchemaDigest` and stays in vox-db until SchemaDigest moves.
- [ ] `SchemaDiff` — `crates/vox-db/src/ddl/diff.rs:11` — tuples of `String`/`Vec<String>`. `diff_schemas`/`table_to_ddl`/`tables_to_ddl` consume a `SchemaDigest` and stay in vox-db.
- [ ] `EnrichedDbError` — `crates/vox-db/src/error_enrichment.rs:12` — `String` + `Vec<String>` + `Option<TableInfo>`. Couples to `TableInfo` (part of `SchemaDigest`); move only when SchemaDigest's pure-data tree is moved as a unit. (Soft MOVE — see Notes.)
- [ ] `Migration` — `crates/vox-db/src/migration.rs:15` — `i64` + `String` + `String` (`up_sql`). The struct itself is pure data; `Migration::apply`-like helpers live in `VoxDb::apply_migrations` (already in vox-db). `builtin_migrations()` stays. (Soft MOVE — see Notes.)
- [ ] `UnifiedLlmTurnRowIds` — `crates/vox-db/src/outcome_recorder.rs:13` — two `i64`/`Option<i64>` fields.
- [ ] `QuestioningKpiSnapshot` — `crates/vox-db/src/questioning_telemetry.rs:30` — `usize` + `f64` summary metrics.
- [ ] `QuestioningResearchArtifact<'a>` — `crates/vox-db/src/questioning_telemetry.rs:16` — borrowed-string params struct. Mirrors the `*Params` types already moved.
- [ ] `RetrievalDiagnostics` — `crates/vox-db/src/research.rs:487` — `usize`/`f64`/`Option<u64>`/`Vec<String>` only.
- [ ] `RetrievalEvidenceSource` — `crates/vox-db/src/retrieval.rs:12` — small enum.
- [ ] `RetrievalMode` — `crates/vox-db/src/retrieval.rs:42` — small enum (Vector/FullText/Hybrid).
- [ ] `RetrievalQuery` — `crates/vox-db/src/retrieval.rs:53` — `String` + `RetrievalMode` + counts.
- [ ] `RetrievalResult` — `crates/vox-db/src/retrieval.rs:347` — `String` + score fields.
- [ ] `SearchIntent` — `crates/vox-db/src/retrieval.rs:78` — small enum.
- [ ] `SearchCorpus` — `crates/vox-db/src/retrieval.rs:95` — small enum.
- [ ] `SearchBackend` — `crates/vox-db/src/retrieval.rs:107` — small enum.
- [ ] `SearchRefinementAction` — `crates/vox-db/src/retrieval.rs:124` — small enum.
- [ ] `SearchPlan` — `crates/vox-db/src/retrieval.rs:137` — `String`/`Option<String>` + above enums. Move with the rest of `retrieval` enums as a batch (`heuristic_search_plan` is the constructor and stays).
- [ ] `SearchDiagnostics` — `crates/vox-db/src/retrieval.rs:310` — `u32` + `Vec<String>` summary record.
- [ ] `SocratesSurfaceTelemetry` — `crates/vox-db/src/socrates_telemetry.rs:37` — `String`/`f64`/`Option<String>` + `RiskDecision`. (RiskDecision should move with it; it's a small enum in the same file.)
- [ ] `SocratesSurfaceAggregate` — `crates/vox-db/src/socrates_telemetry.rs:115` — counts + means; pure summary.
- [ ] `SyntaxKEventMeta` — `crates/vox-db/src/syntax_k_telemetry.rs:14` — `String`/`Option<serde_json::Value>` only.
- [ ] `TrustObservationWindowStats` — `crates/vox-db/src/trust_drift.rs:11` — four numeric fields.
- [ ] `TrustObservationDriftReport` — `crates/vox-db/src/trust_drift.rs:20` — composes the above + `Option<String>`/`f64`.
- [ ] `TrustPropagatedScore` — `crates/vox-db/src/trust_propagation.rs:12` — `String`/`f64` only. (`propagate_trust_rollups_domain_cliques` is a free fn over `&[TrustRollupEntry]`; could also move once its only dep — `TrustRollupEntry`, already in `vox-db-types` — stays satisfied. Defer the function.)
- [ ] `TrustRollupGroupSummary` — `crates/vox-db/src/trust_telemetry.rs:10` — counts + means.
- [ ] `TrustObservationInput<'a>` — `crates/vox-db/src/trust_telemetry.rs:27` — borrowed-string params struct (mirrors other `*Params` already in vox-db-types). Has a `with_defaults` constructor — pure constant init, no deps.
- [ ] `TrustObservationEntry` — `crates/vox-db/src/trust_telemetry.rs:47` — owned-string row entry (sister of types already in vox-db-types).
- [x] `WorkspaceJourneyStoreMode` — `crates/vox-db/src/workspace_journey_store.rs:20` — two-variant enum.
- [x] `BuildHealthSummary` — `crates/vox-db/src/store/ops_build.rs:19` — counts + `Vec<CrateSample>`. Move with `CrateSample`.
- [x] `CrateSample` — `crates/vox-db/src/store/ops_build.rs:36` — `String`/`Option<i64>`/`Option<String>`.
- [x] `RegressionRow` — `crates/vox-db/src/store/ops_build.rs:47` — pure data (`String`/`i64`/`f64`/`Option<String>`).
- [x] `BuildDependencyShape` — `crates/vox-db/src/store/ops_build.rs:10` — `i64` + `serde_json::Value`. (`vox-db-types` already pulls `serde_json` indirectly via existing types.)
- [ ] `CloudCostSummary` — `crates/vox-db/src/store/ops_mens_cloud.rs:283` — `u32`/`f64` only.
- [ ] `CorpusQualitySummary` — `crates/vox-db/src/store/ops_mens_intelligence.rs:9` — `u64`/`f64` only.
- [ ] `GrpoStepRow` — `crates/vox-db/src/store/ops_mens_intelligence.rs:24` — `String`/`u32`/`f32` only.

## Mixed / KEEP in vox-db

These cannot move without dragging `turso`, `tokio`, or actor plumbing
into `vox-db-types`. Reasons given so a future audit can re-check.

- `VoxDb` — `crates/vox-db/src/lib.rs:247` — owns `turso::Connection`, `Arc<DbCircuitBreaker>`, optional `VoxWriteHandle`. KEEP.
- `Codex` — `crates/vox-db/src/lib.rs:228` — `pub type Codex = VoxDb;`. KEEP.
- `ReadConsistency` — `crates/vox-db/src/lib.rs:232` — small marker enum, but tightly tied to `VoxDb` API surface; not separately re-exported via `pub use`. Defer (low value to move).
- `AutoMigrator<'a>` — `crates/vox-db/src/auto_migrate.rs:180` — owns `&'a Connection`. KEEP.
- `DbCircuitBreaker` — `crates/vox-db/src/circuit_breaker.rs:69` — holds `Arc<RwLock<...>>` (tokio), `AtomicU32`. KEEP. (Companion enums `CircuitState`/`CircuitBreakerError` are MOVE candidates.)
- `Collection` — `crates/vox-db/src/collection.rs:34` — owns `turso::Connection` + `Arc<DbCircuitBreaker>`. KEEP. (Companion `CollectionError` is also turso-coupled.)
- `DbWriteCmd` — `crates/vox-db/src/writer_actor.rs:6` — variants embed `oneshot::Sender<Result<_, StoreError>>` (tokio + StoreError). KEEP.
- `VoxWriteHandle` — `crates/vox-db/src/writer_actor.rs:77` — wraps `mpsc::Sender<DbWriteCmd>`. KEEP.
- `VoxDbPool` — `crates/vox-db/src/pool.rs:18` — `Arc<DbBackend>` + tokio `OnceCell` + `RwLock`. KEEP.
- `Migration` — `crates/vox-db/src/migration.rs:15` — listed under MOVE above with a soft tag; the struct itself is pure data, but `validate_migrations` and `builtin_migrations` form a tight unit with `VoxDb::apply_migrations`. If moving the struct splits that unit awkwardly, defer. Phase 4.2 should attempt and back out if it cascades.
- `StoreError` — `crates/vox-db/src/store/types/error.rs:7` — `#[from]` arms for `turso::Error` and `CircuitBreakerError`. KEEP. (Used as the canonical error in every `vox-db` operation; moving would force the turso dep into vox-db-types.)
- `SchemaDigest` (and its sub-tree: `TableInfo`, `CollectionInfo`, `FieldInfo`, `Relationship`, `RelationshipKind`, `IndexInfo`, `IndexKind`, `FunctionInfo`) — `crates/vox-db/src/schema_digest/digest_types.rs:7` — pure data structurally, **but** `generate_schema_digest` consumes `vox_ast::Module`. If we move only the struct tree to vox-db-types, the tree's main producer stays in vox-db (still fine). Defer to a follow-up batch — moving ~8 inter-dependent types at once is its own PR.
- `EnrichedDbError` — listed soft-MOVE; depends on `TableInfo` from `SchemaDigest`. Move only after the `SchemaDigest` tree moves.
- `InvocableSyncEngine<'a>` — `crates/vox-db/src/sync_invocables.rs:11` — `db: &'a VoxDb`. KEEP.
- `TimedExecution` — `crates/vox-db/src/exec_time_telemetry.rs:57` — holds `Option<crate::VoxDb>`. KEEP. (Sister type `ExecTimeRecord` is MOVE.)
- Free functions (`now_unix_ms`, `enrich_error`, `evaluate_codex_api_readiness`, `connect_canonical_*`, `resolve_canonical_config`, `user_global_sqlite_path`, `open_project_db*`, `digest_to_json`, `format_llm_context`, `generate_schema_digest`, `fuse_hybrid_results`, `heuristic_search_plan`, `retrieval_diagnostics`, `propagate_trust_rollups_domain_cliques`, `hallucination_risk_proxy`, `add_suppression`, `*_baseline`, `*_task_queue`, `set_file_cache_blocking`, `connect_workspace_journey_*`, `workspace_journey_*`, `builtin_migrations`, `validate_migrations`, `table_to_ddl`, `tables_to_ddl`, `diff_schemas`, `build_data_flow`) — all consume a `&VoxDb` or a `turso::Connection` or `SchemaDigest`. KEEP. Phase 4.2-4.3 is about moving **types**, not free functions.

## Notes / decisions made during the audit

- **Bias toward conservatism.** Per task instructions, when a struct is
  pure data but co-located with operational code that uses `&VoxDb`, the
  struct is still listed as MOVE — but the entry calls out that the
  free functions stay. Phase 4.2-4.3 should expect to leave a thin
  forwarding layer in vox-db.
- **`Migration` is a judgment call.** Pure data, but `validate_migrations`
  takes `&[Migration]` and `builtin_migrations()` returns `Vec<Migration>`.
  If both stay in vox-db, callers re-exporting `Migration` need to keep
  importing from vox-db-types. This is the same pattern already used for
  `EvalRunParams`, so it's fine — flagged here only so the reviewer
  isn't surprised.
- **`SchemaDigest` deserves its own follow-up PR.** Moving the struct
  alone is easy; moving the **tree** (TableInfo, FieldInfo,
  Relationship, IndexInfo, FunctionInfo, plus the nested enums) is ~8
  types and will touch every caller of `SchemaDigest::tables[].fields`.
  Better to land the trivial moves first (this checklist), then plan a
  separate batch for the digest tree. `EnrichedDbError` waits with it.
- **Retrieval enums are a coherent batch.** `RetrievalMode`,
  `RetrievalEvidenceSource`, `SearchIntent`, `SearchCorpus`,
  `SearchBackend`, `SearchRefinementAction`, plus the structs
  `RetrievalQuery`, `RetrievalResult`, `SearchPlan`, `SearchDiagnostics`,
  `RetrievalDiagnostics` — move them all in one PR or none. Splitting
  causes circular `pub use` re-exports across the crate boundary.
- **Trust telemetry is similarly a coherent batch.** Move
  `TrustObservationInput<'a>`, `TrustObservationEntry`,
  `TrustRollupGroupSummary`, `TrustObservationWindowStats`,
  `TrustObservationDriftReport`, `TrustPropagatedScore` together.
- **Soft "deferred" items** (entries flagged with parenthetical caveats
  above) are still tickable — Phase 4.2-4.3 may decide to defer them
  individually if the cost-benefit looks bad. Don't treat them as hard
  blockers on each other.
- **Total MOVE count: ~45 types.** Plan ceiling was ~30; the extra came
  from the retrieval and trust batches, which need to move as units.
  Phase 4.2-4.3 may split this into two follow-up PRs (Phase 4.2 = small
  independent types; Phase 4.3 = the retrieval + trust batches).

## Row↔Domain bridges (Phase 6) — deferred

Investigated 2026-05-09. No domain types found that map 1:1 to row types
without inventing new types. Bridges deferred to a follow-up PR when
domain types are established.

**Top row types by cross-crate reference count** (all tied at 4–6 files):
`MemoryEntry` (6), `AgentDefEntry` (5), `SkillExecutionRow` (5),
`PlanSessionRow` (5), `A2AMessageRow` (5), `SkillExecutionRow` (5).

**Domain types found:**
- `Session` — `crates/vox-orchestrator/src/session/state.rs` (L3)
- `A2AMessage` — `crates/vox-orchestrator/src/types/messages.rs` (L3)

**Why bridges were not implemented:**

1. **Layer constraint:** `vox-db-types` is L0; the only crates with
   matching domain types (`vox-orchestrator`) are L3. Adding a dep
   `vox-db-types → vox-orchestrator` would be a prohibited upward
   inversion. The reverse (putting bridges in `vox-orchestrator`) is
   layer-legal (L3 can depend on L0) but `vox-orchestrator` does not
   currently depend on `vox-db-types` — adding that dep purely for
   cosmetic bridges is not justified yet.

2. **Structural mismatch:** `Session` (in-memory) carries `turns:
   Vec<SessionTurn>`, `plugin_state`, `total_tokens`, and `turn_count`
   that have no column in `SessionRow`. Likewise `A2AMessage` uses
   strongly-typed `AgentId`/`MessageId` newtypes and `A2AMessageType`
   enum, while `A2AMessageRow` stores raw strings plus delivery-tracking
   columns (`claim_owner`, `delivery_attempts`, `last_claim_error`) that
   do not exist on the domain type. A lossless `From` conversion is not
   possible without inventing new types or losing data.

3. **No matching domain types for the other top rows:** `MemoryEntry`,
   `AgentDefEntry`, `SkillExecutionRow`, `PlanSessionRow` have no
   corresponding application-layer struct in any crate. The only
   consumers that reference them are: `vox-db` (constructing the rows),
   `vox-cli` (displaying `AgentDefEntry`), and `vox-orchestrator` (one
   call-site reading `MemoryEntry` for retrieval scoring). None of these
   destructure the row and reconstruct a domain object — they use the row
   directly.

**Recommended follow-up:** When a domain `AgentDefinition` or
`MemoryRecord` struct is introduced (e.g. in a future `vox-agent-types`
L0 crate), revisit this phase. At that point a bridge in the new L0
crate (depending on `vox-db-types`) would be clean and layer-compliant.
