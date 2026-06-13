# vox-db Semantic Behavior Coverage Map

## Summary

This report synthesizes 293 extracted `Behavior` claims for the `vox-db` crate into a per-symbol semantic map, deduplicating near-identical claims (including the same symbol spelled with and without parens, e.g. `validate_identifier()` vs `validate_identifier`, and duplicate src/ vs tests/ assertions over the same call). About 120 distinct symbols remain. Proof of failure-mode behavior (error paths) and boundary behavior (edge/invariant) is concentrated in a small set of security- and integrity-critical surfaces: SQL identifier validation, `record_review_decision` validation, clavis ciphertext backup integrity, the circuit breaker, `UpsertIdentityMismatch` enforcement, `LegacySchemaChain` rejection, and CAS idempotency. The remaining majority of `VoxDb` persistence and telemetry methods are proven ONLY on the happy path — round-trips and "returns a positive id" assertions with no error or boundary coverage. Those happy-only symbols are the semantic holes that line coverage hides: the code runs during a test, but what it does on bad input, empty input, or conflict is unproven.

## Per-symbol behaviors

### Security / validation (well-covered failure modes)
- **validate_identifier (collection.rs & sql_util.rs)** — accepts normal identifiers; rejects empty, leading-digit, spaces, special chars, SQL keywords/injection, comments, backticks, null bytes, non-ASCII, >64 chars. (error)
- **collection_ddl / collection_index_ddl** — generate correct DDL; reject SQL injection in any name argument with `CollectionError::InvalidInput`. (error)
- **record_review_decision** — round-trips; rejects invalid decision value, non-array `model_fingerprints_json`, empty `bound_digest`/`publication_id`/`actor`. (error)
- **upsert_user_identity (store/ops_user_identity)** — round-trips; rejects empty `nanopub_key_ref` and does not persist. (error)

### Integrity / conflict (covered)
- **clavis backup export/import** — round-trips with integrity check; import rejects corrupted ciphertext (`checksum mismatch during restore`). (error)
- **upsert_scholarly_submission / upsert_external_submission_job** — reject identity/content mismatch with `StoreError::UpsertIdentityMismatch`. (error)
- **VoxDb::connect / connect_default / migrate** — reject legacy / above-baseline schema with `StoreError::LegacySchemaChain`. (error)
- **DbCircuitBreaker / ::call** — Open after threshold, success resets, disabled stays Closed; Open does not invoke closure and returns error. (error + edge)
- **CAS store/get** — hash round-trip identity, duplicate returns same hash, survives file reopen. (edge + invariant)
- **insert_finding_candidate** — duplicate `(producer_name, signal_fingerprint)` returns `AlreadySeen` with no duplicate row. (invariant)
- **latest_decision_for_claim** — supersede-by-timestamp, None for unknown, per-publication scope, tiebreak by row id. (edge)
- **try_remote_from_compat_env** — None under HARD_CUT=1 or CUTOVER_PHASE=enforce; Remote only in lenient mode. (error)
- **record_socrates_eval_summary** — `StoreError::Db` ('no socrates_surface') when window empty. (error)
- **is_activity_completed** — false for error status (not only happy ok-status). (error)

### Edge-only boundary coverage
- **lookup_hint** (None for unknown crate), **load_actor_state** / **get_agent_reliability** / **get_user_identity** / **get_nanopub_by_trusty_uri** / **get_workflow_execution** (None when absent), **store_trusted_evidence** (None on empty hits), **list_skill_executions_by_skill** (empty vec), **scientia_cost_by_phase** (empty DB), **count_publication_approvers_by_role** (zero, no error), **research_session_upsert** / **start_workflow_execution** (idempotent), schema-shape contract tests.

## Semantic gaps

The following symbols are proven ONLY on the happy path — no `error`, `edge`, or `invariant` claim. These are the "looks tested but failure modes unproven" cases. Highest concern are the ones whose contract clearly *has* a failure/empty/conflict mode that no test pins:

**Data-shape / pure functions with untested malformed input:** `json_to_sql_literal`, `build_data_flow`, `enrich_error`, `levenshtein`, `turso_cell_value_to_json`, `table_to_ddl`/`index_to_ddl`/`diff_schemas`/`diff_to_sql`, `type_to_sqlite_type` family, `current_quarter_window_ms`, `fuse_hybrid_results`, `RetrievalEvidenceSource::merge`, `heuristic_search_plan`.

**Persistence/telemetry methods (no not-found / empty / conflict path):** `record_exec_time`/`query_tool_latency`, `record_unified_llm_turn`, `list_model_arm_stats`, `ingest_mens_scorecard_summary_json`, `persist_questioning_research_artifact_dual_write`, `record_endpoint_infra_failure`, `record_socrates_surface_event`, `aggregate_socrates_surface_metrics`, `merge_scientia_live_socrates_into_metadata_json`, trust telemetry (`record_trust_observation`, `list_trust_rollups`, `summarize_trust_rollups`, `trust_observation_drift_two_window`, `propagate_trust_rollups_domain_cliques`), `create_session`/`close_session`, `log_interaction`/`submit_feedback`, `bind_name`, the full clavis ciphertext upsert/get/list/delete surface (only backup has an error test), skill manifest publish/get/list/unpublish, research-metric list family, endpoint reliability observation, external-review insert/upsert/state/deadletter flow, `acquire_distributed_lock`, heartbeats (`list_live_nodes`/`evict_dead_heartbeats`), planning (`load_plan_head`/`list_runnable_nodes`), retention count/delete, `append_publication_status_event`, `has_dual_publication_approval_for_digest`, `list_publication_ids_with_scholarly_submissions`, the entire questioning-telemetry method cluster, `ResearchMetricsSink::record`, `record_agent_event`, research-session create/update/get/list, `store_claim_verdict`/`store_training_pair`/`get_research_artifact`, `start_provider_run`/`rollup_model_scoreboard_with_scientia`, `record_pm_registry_mirror`/`get_package_versions`, `sync_schema_from_digest`, embedded-replica/`sync_for`, codex_chat usage+conversation round-trips.

**Migration / schema / config:** `validate_migrations` (accepts sorted-unique — no unsorted/duplicate rejection test), `apply_migrations` (no failure/rollback path), `MigrationAction::to_sql` (only AddColumn happy), `schema_baseline_digest_hex`, `SCHEMA_FRAGMENTS`, `schema::baseline_sql`/`orchestrator_schema_digest`, `migrate`, `data_dir`/`default_db_path`/`state_dir`.

**Serialization wrappers:** `ExecOutcome`, `DbAgentId`, `Suggestion`, `Codex`, `count_scientia_nanopubs_for_claim`, `list_developer_journey_steps`.

The most actionable gaps are `validate_migrations` (a validator with no rejection test), `apply_migrations` (no failure path), and the clavis ciphertext CRUD surface (integrity-critical, only the backup path has a negative test) — these are validation/integrity surfaces where a happy-only proof is most misleading.