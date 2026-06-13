# Semantic Behavior Map — `vox-research-shim`

Deterministically synthesized from 57 distinct proven-behavior claims (of 57 extracted) across 35 symbols. 3 symbols have an explicit error-path proof; **20 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `parse_agent_frontmatter()`  (edge, error, happy; EXTRACTED)
- [happy] parses YAML frontmatter delimiters and returns Frontmatter with model and scope fields populated from valid input  (crates/vox-research-shim/src/agent_frontmatter.rs)
- [happy] parses YAML model field correctly and leaves scope field as None when not specified  (crates/vox-research-shim/src/agent_frontmatter.rs)
- [edge] tolerates whitespace between YAML key and colon separator during parsing  (crates/vox-research-shim/src/agent_frontmatter.rs)
- [error] returns None when frontmatter opening delimiter lacks closing delimiter  (crates/vox-research-shim/src/agent_frontmatter.rs)

### `persist_research_event_metrics()`  (edge, happy; EXTRACTED)
- [happy] persist_research_event_metrics persists a whitelisted TelemetryObservation event to the database and returns retrievable row  (crates/vox-research-shim/src/research/research_event_metrics_bridge.rs)
- [edge] persist_research_event_metrics silently skips (does not persist) TelemetryObservation events with metric_type not on the whitelist  (crates/vox-research-shim/src/research/research_event_metrics_bridge.rs)
- [happy] Persists whitelisted TelemetryObservation events to database metrics table  (crates/vox-research-shim/src/research/research_event_metrics_bridge.rs)
- [edge] Skips persisting events with metric_types not in whitelist, leaving no database records  (crates/vox-research-shim/src/research/research_event_metrics_bridge.rs)

### `parse_claims_response()`  (edge, happy; EXTRACTED)
- [happy] parse_claims_response parses JSON codeblocks (```json...```) and extracts claim objects with correct text field values  (crates/vox-research-shim/src/research/claims.rs)
- [happy] Parses JSON codeblocks and extracts claims with unique, non-zero claim_ids  (crates/vox-research-shim/src/research/claims.rs)
- [edge] Filters out claims with whitespace-only text, returning empty collection  (crates/vox-research-shim/src/research/claims.rs)

### `parse_planner_response()`  (edge, happy; EXTRACTED)
- [edge] parse_planner_response deduplicates repeated subqueries and limits total subqueries to the specified max count  (crates/vox-research-shim/src/research/planner.rs)
- [happy] parse_planner_response parses JSON codeblocks (```json...```) and extracts subquery text in order  (crates/vox-research-shim/src/research/planner.rs)
- [happy] Deduplicates subqueries and limits them to max_subqueries parameter  (crates/vox-research-shim/src/research/planner.rs)

### `task_and_flags_to_profile()`  (happy; EXTRACTED)
- [happy] task_and_flags_to_profile(TaskCategory::CodeGen, true, false, false) returns RoutingProfile::Vision when vision flag is true  (crates/vox-research-shim/src/selection/tests.rs)
- [happy] task_and_flags_to_profile(TaskCategory::CodeGen, false, true, false) returns RoutingProfile::Research when web_search flag is true  (crates/vox-research-shim/src/selection/tests.rs)
- [happy] task_and_flags_to_profile(TaskCategory::CodeGen, false, false, false) returns RoutingProfile::General when all flags are false  (crates/vox-research-shim/src/selection/tests.rs)

### `Claim`  (happy; EXTRACTED)
- [happy] Claim struct supports construction with text field that can be retrieved unchanged  (crates/vox-research-shim/tests/scientia_phase_0a_claims_stub.rs)
- [happy] Has text and is_numeric fields that are populated from parsed response  (crates/vox-research-shim/src/research/claims.rs)

### `ModelScorer::score_with_mode()`  (happy; EXTRACTED)
- [happy] score_with_mode() in Efficient mode scores free models higher than paid models  (crates/vox-research-shim/src/selection/tests.rs)
- [happy] score_with_mode() in Precision mode scores Pro tier models higher than Free tier models  (crates/vox-research-shim/src/selection/tests.rs)

### `ResearchEvent::kind()`  (happy; EXTRACTED)
- [happy] ResearchEvent::TelemetryObservation variant has a kind() method that returns ResearchEventKind::TelemetryObservation  (crates/vox-research-shim/src/research/research_event_metrics_bridge.rs)
- [happy] Returns ResearchEventKind::TelemetryObservation for TelemetryObservation variant  (crates/vox-research-shim/src/research/research_event_metrics_bridge.rs)

### `ResearchPlan`  (happy; EXTRACTED)
- [happy] parse_planner_response preserves the input ResearchQuery scope field in the returned plan  (crates/vox-research-shim/src/research/planner.rs)
- [happy] parse_planner_response preserves the input ResearchQuery max_sources field in the plan's max_sources_per_subquery  (crates/vox-research-shim/src/research/planner.rs)

### `ResearchQuery`  (happy; EXTRACTED)
- [happy] ResearchQuery can be constructed with named fields and the query field is readable and matches the assigned string value  (crates/vox-research-shim/tests/scientia_phase_0a_types_round_trip.rs)
- [happy] ResearchQuery can be constructed with named fields and the max_sources field is readable and matches the assigned integer value  (crates/vox-research-shim/tests/scientia_phase_0a_types_round_trip.rs)

### `parse_claims_response`  (edge, happy; EXTRACTED)
- [happy] parse_claims_response correctly preserves the is_numeric boolean field from JSON input  (crates/vox-research-shim/src/research/claims.rs)
- [edge] parse_claims_response filters out claims with blank text (whitespace-only strings) and returns empty vector  (crates/vox-research-shim/src/research/claims.rs)

### `parse_json_response()`  (happy; EXTRACTED)
- [happy] parse_json_response extracts and deserializes JSON from markdown codeblocks (```json...```) in plain text  (crates/vox-research-shim/src/research/json_parse.rs)
- [happy] Extracts JSON objects from markdown codeblocks in text  (crates/vox-research-shim/src/research/json_parse.rs)

### `parse_verifier_response()`  (edge, happy; EXTRACTED)
- [happy] Maps supporting and contradicting indices from JSON to evidence_spans array with correct SpanType variants  (crates/vox-research-shim/src/research/verifier.rs)
- [edge] Sets verdict to Unverified and clears supporting_count when confidence falls below threshold  (crates/vox-research-shim/src/research/verifier.rs)

### `run_research()`  (happy, invariant; EXTRACTED)
- [invariant] returns research metadata where subquery_count is at least 1, source_count matches the sources vector length, and citation count is bounded by source count  (crates/vox-research-shim/tests/scientia_phase_0a_pipeline_smoke.rs)
- [happy] persists research session to database with session_id > 0, correct query_text, and status marked as 'completed'  (crates/vox-research-shim/tests/scientia_phase_0a_pipeline_smoke.rs)

### `task_strengths()`  (happy; EXTRACTED)
- [happy] task_strengths() returns a non-empty collection for TaskCategory::CodeGen  (crates/vox-research-shim/src/selection/tests.rs)
- [happy] task_strengths() returns a non-empty collection for TaskCategory::Review  (crates/vox-research-shim/src/selection/tests.rs)

### `Claim::claim_id`  (happy; EXTRACTED)
- [happy] Claim objects generated by parse_claims_response have non-zero unique claim_id values for distinct claims  (crates/vox-research-shim/src/research/claims.rs)

### `ProviderRegistry::default()::primary_name()`  (happy; EXTRACTED)
- [happy] ProviderRegistry::default() returns a registry whose primary_name() method returns the string 'stub'  (crates/vox-research-shim/tests/scientia_phase_0a_provider_stub.rs)

### `ResearchScope`  (happy; EXTRACTED)
- [happy] Defaults to Both when not specified in planner response  (crates/vox-research-shim/src/research/planner.rs)

### `ResearchStage::ORDERED`  (invariant; EXTRACTED)
- [invariant] Is an ordered array of 8 stages starting with Queued and ending with Completed  (crates/vox-research-shim/src/research/types.rs)

### `ResearchStage::as_str()`  (invariant; EXTRACTED)
- [invariant] Produces values that round-trip through JSON serde serialization for all variants  (crates/vox-research-shim/src/research/types.rs)

### `RetrievalDiagnostics`  (happy; EXTRACTED)
- [happy] RetrievalDiagnostics serializes to serde_json::Value as an object  (crates/vox-research-shim/tests/scientia_phase_0a_types_round_trip.rs)

### `RetrievalDiagnostics::fusion_weights`  (happy; EXTRACTED)
- [happy] RetrievalDiagnostics fusion_weights tuple field serializes as a 3-element JSON array  (crates/vox-research-shim/tests/scientia_phase_0a_types_round_trip.rs)

### `RoutingTier::DeepResearch`  (invariant; EXTRACTED)
- [invariant] RoutingTier::DeepResearch Debug format produces the exact string 'DeepResearch'  (crates/vox-research-shim/tests/scientia_phase_0a_types_round_trip.rs)

### `RoutingTier::Direct`  (invariant; EXTRACTED)
- [invariant] RoutingTier::Direct Debug format produces the exact string 'Direct'  (crates/vox-research-shim/tests/scientia_phase_0a_types_round_trip.rs)

### `RoutingTier::Light`  (invariant; EXTRACTED)
- [invariant] RoutingTier::Light Debug format produces the exact string 'Light'  (crates/vox-research-shim/tests/scientia_phase_0a_types_round_trip.rs)

### `Verdict`  (happy; EXTRACTED)
- [happy] Can be Supported and tracks supporting_count and contradicting_count from parsed evidence  (crates/vox-research-shim/src/research/verifier.rs)

### `decompose_query_with_config()`  (error; EXTRACTED)
- [error] falls back to single subquery equaling the original query when no LLM model is available for decomposition  (crates/vox-research-shim/tests/scientia_phase_0a_planner_stub.rs)

### `extract_claims_with_model()`  (error; EXTRACTED)
- [error] returns empty vector when no LLM model is available for claim extraction  (crates/vox-research-shim/tests/scientia_phase_0a_claims_stub.rs)

### `load_rolling_search_policy_feedback()`  (happy; EXTRACTED)
- [happy] Derives citation_precision and source_hit_rate from database metric records with floating-point precision  (crates/vox-research-shim/src/research/search_policy_feedback.rs)

### `persist_research_event_metrics`  (happy; EXTRACTED)
- [happy] persist_research_event_metrics correctly preserves the numeric value field from ResearchEvent in database row  (crates/vox-research-shim/src/research/research_event_metrics_bridge.rs)

### `primary_strength()`  (happy; EXTRACTED)
- [happy] primary_strength(TaskCategory::CodeGen) returns the string 'codegen'  (crates/vox-research-shim/src/selection/tests.rs)

### `score_with_config()`  (happy; EXTRACTED)
- [happy] routes to RoutingTier::Direct when all inputs (claims, citations, retrieval hits, answer) are empty or zero  (crates/vox-research-shim/tests/scientia_phase_0a_gate_stub.rs)

### `slug_from_query()`  (happy; EXTRACTED)
- [happy] converts queries to URL-safe slugs by lowercasing, removing punctuation, substituting spaces with hyphens, defaulting empty strings to 'untitled', and capping output at 80 characters  (crates/vox-research-shim/tests/scientia_phase_0a_persistence.rs)

### `verify_claims_with_config`  (edge; EXTRACTED)
- [edge] verify_claims_with_config returns an empty vector when called with an empty evidence slice, regardless of input claims  (crates/vox-research-shim/tests/scientia_phase_0a_verifier_stub.rs)

### `write_research_doc()`  (happy; EXTRACTED)
- [happy] creates a markdown research document at the expected nested docs/src/research/ path when given slug and content  (crates/vox-research-shim/tests/scientia_phase_0a_persistence.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Claim`** — only: _Claim struct supports construction with text field that can be retrieved unchanged_
- **`Claim::claim_id`** — only: _Claim objects generated by parse_claims_response have non-zero unique claim_id values for distinct claims_
- **`ModelScorer::score_with_mode()`** — only: _score_with_mode() in Efficient mode scores free models higher than paid models_
- **`ProviderRegistry::default()::primary_name()`** — only: _ProviderRegistry::default() returns a registry whose primary_name() method returns the string 'stub'_
- **`ResearchEvent::kind()`** — only: _ResearchEvent::TelemetryObservation variant has a kind() method that returns ResearchEventKind::TelemetryObservation_
- **`ResearchPlan`** — only: _parse_planner_response preserves the input ResearchQuery scope field in the returned plan_
- **`ResearchQuery`** — only: _ResearchQuery can be constructed with named fields and the query field is readable and matches the assigned string value_
- **`ResearchScope`** — only: _Defaults to Both when not specified in planner response_
- **`RetrievalDiagnostics`** — only: _RetrievalDiagnostics serializes to serde_json::Value as an object_
- **`RetrievalDiagnostics::fusion_weights`** — only: _RetrievalDiagnostics fusion_weights tuple field serializes as a 3-element JSON array_
- **`Verdict`** — only: _Can be Supported and tracks supporting_count and contradicting_count from parsed evidence_
- **`load_rolling_search_policy_feedback()`** — only: _Derives citation_precision and source_hit_rate from database metric records with floating-point precision_
- **`parse_json_response()`** — only: _parse_json_response extracts and deserializes JSON from markdown codeblocks (```json...```) in plain text_
- **`persist_research_event_metrics`** — only: _persist_research_event_metrics correctly preserves the numeric value field from ResearchEvent in database row_
- **`primary_strength()`** — only: _primary_strength(TaskCategory::CodeGen) returns the string 'codegen'_
- **`score_with_config()`** — only: _routes to RoutingTier::Direct when all inputs (claims, citations, retrieval hits, answer) are empty or zero_
- **`slug_from_query()`** — only: _converts queries to URL-safe slugs by lowercasing, removing punctuation, substituting spaces with hyphens, defaulting empty strings to 'untitled', and capping output at 80 characters_
- **`task_and_flags_to_profile()`** — only: _task_and_flags_to_profile(TaskCategory::CodeGen, true, false, false) returns RoutingProfile::Vision when vision flag is true_
- **`task_strengths()`** — only: _task_strengths() returns a non-empty collection for TaskCategory::CodeGen_
- **`write_research_doc()`** — only: _creates a markdown research document at the expected nested docs/src/research/ path when given slug and content_
