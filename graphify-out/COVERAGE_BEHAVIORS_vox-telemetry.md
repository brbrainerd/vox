# Semantic Behavior Map — `vox-telemetry`

Deterministically synthesized from 98 distinct proven-behavior claims (of 98 extracted) across 35 symbols. 4 symbols have an explicit error-path proof; **27 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `validate_research_metric_row()`  (error, happy; EXTRACTED)
- [happy] accepts well-formed rows with valid session_id, metric_type, and metadata  (crates/vox-telemetry/tests/metric_validation_integration.rs)
- [happy] accepts benchmark metric types with standard session prefixes  (crates/vox-telemetry/tests/recorder_integration.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_ROUTE_RUN_STARTED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_ROUTE_CLUSTER_DECIDED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_ROUTE_RUN_COMPLETED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_ROUTE_RUN_FAILED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_EFFORT_RUN_STARTED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_EFFORT_COMMIT_JUDGED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_EFFORT_RUN_COMPLETED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_EFFORT_RUN_FAILED with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_AUDIT_RUN with session 'audit:vox'  (crates/vox-telemetry/src/types.rs)
- [happy] validate_research_metric_row accepts METRIC_TYPE_LINT_FINDING with session 'lint:vox'  (crates/vox-telemetry/src/types.rs)
- … +13 more claims

### `parse_user_config()`  (edge, happy; EXTRACTED)
- [happy] Parses boolean value 'true' for enabled flag  (crates/vox-telemetry/src/config.rs)
- [happy] Parses boolean value 'false' for remote_upload flag  (crates/vox-telemetry/src/config.rs)
- [happy] Parses string boolean values like 'false' for model_calls  (crates/vox-telemetry/src/config.rs)
- [happy] Parses numeric 1 as true for agent_orchestration  (crates/vox-telemetry/src/config.rs)
- [happy] Parses numeric 0 as false for build  (crates/vox-telemetry/src/config.rs)
- [happy] Parses string value 'on' as true for errors  (crates/vox-telemetry/src/config.rs)
- [happy] Parses bareword 'off' as false for debug_to_stderr  (crates/vox-telemetry/src/config.rs)
- [edge] Ignores keys under non-telemetry sections like [other]  (crates/vox-telemetry/src/config.rs)
- [happy] Parses telemetry keys when they appear in [telemetry] section  (crates/vox-telemetry/src/config.rs)
- [happy] parse_user_config() correctly parses enabled field from TOML [telemetry] section as Some(true)  (crates/vox-telemetry/src/config.rs)
- [happy] parse_user_config() correctly parses remote_upload field from TOML [telemetry] section as Some(false)  (crates/vox-telemetry/src/config.rs)
- [happy] parse_user_config() correctly parses research_metrics field from TOML [telemetry] section as Some(true)  (crates/vox-telemetry/src/config.rs)
- … +9 more claims

### `parse_policy()`  (edge, happy; EXTRACTED)
- [happy] Returns true when policy content contains 'enabled = false' in any format  (crates/vox-telemetry/src/config.rs)
- [happy] Detects 'enabled = 0' as equivalent to disabled  (crates/vox-telemetry/src/config.rs)
- [happy] Detects 'enabled = off' as equivalent to disabled  (crates/vox-telemetry/src/config.rs)
- [happy] Handles quoted strings in enabled field values  (crates/vox-telemetry/src/config.rs)
- [edge] Returns false when policy line is commented out with '#'  (crates/vox-telemetry/src/config.rs)
- [happy] Returns false when enabled is set to true  (crates/vox-telemetry/src/config.rs)
- [edge] Handles empty input by returning false  (crates/vox-telemetry/src/config.rs)
- [happy] Detects 'enabled = false' within a [telemetry] TOML section  (crates/vox-telemetry/src/config.rs)
- [happy] parse_policy() returns true when given TOML with [telemetry] section containing enabled = false  (crates/vox-telemetry/src/config.rs)

### `TelemetryConfig.enabled`  (error, happy; EXTRACTED)
- [happy] Defaults to true when VOX_TELEMETRY env var is unset  (crates/vox-telemetry/src/config.rs)
- [happy] Remains true when VOX_TELEMETRY is set to 'debug'  (crates/vox-telemetry/src/config.rs)
- [error] Is set to false when VOX_TELEMETRY is set to 'off'  (crates/vox-telemetry/src/config.rs)

### `TelemetryConfig::all_off()`  (happy; EXTRACTED)
- [happy] disables enabled, remote_upload, research_metrics, model_calls, agent_orchestration, build, and errors flags  (crates/vox-telemetry/tests/trace_context_smoke.rs)
- [happy] TelemetryConfig::all_off() sets enabled to false  (crates/vox-telemetry/tests/trace_context_smoke.rs)
- [happy] TelemetryConfig::all_off() sets remote_upload, research_metrics, model_calls, agent_orchestration, build, and errors all to false  (crates/vox-telemetry/tests/trace_context_smoke.rs)

### `TelemetryConfig::default_on()`  (happy; EXTRACTED)
- [happy] enables primary telemetry (enabled, research_metrics, model_calls, errors) while keeping remote_upload disabled  (crates/vox-telemetry/tests/trace_context_smoke.rs)
- [happy] TelemetryConfig::default_on() sets enabled to true but keeps remote_upload disabled  (crates/vox-telemetry/tests/trace_context_smoke.rs)
- [happy] TelemetryConfig::default_on() enables research_metrics, model_calls, and errors categories  (crates/vox-telemetry/tests/trace_context_smoke.rs)

### `is_master_enabled()`  (error, happy; EXTRACTED)
- [error] Returns false when VOX_TELEMETRY is set to 'off'  (crates/vox-telemetry/src/config.rs)
- [happy] Returns true when VOX_TELEMETRY is set to 'on'  (crates/vox-telemetry/src/config.rs)
- [happy] Returns true by default when VOX_TELEMETRY env var is unset  (crates/vox-telemetry/src/config.rs)

### `AuditEffortCommitJudgedEvent`  (happy; EXTRACTED)
- [happy] AuditEffortCommitJudgedEvent with Some Option fields serializes completely and round-trips successfully  (crates/vox-telemetry/src/types.rs)
- [happy] AuditEffortCommitJudgedEvent with None Option fields skips serializing optional keys and round-trips successfully  (crates/vox-telemetry/src/types.rs)

### `NoOpRecorder`  (happy; EXTRACTED, INFERRED)
- [happy] correctly participates in composite recorder without side effects  (crates/vox-telemetry/tests/recorder_integration.rs)
- [happy] NoOpRecorder can be composed into CompositeRecorder without blocking event propagation to other recorders  (crates/vox-telemetry/tests/recorder_integration.rs)

### `TelemetryConfig.research_metrics`  (error, happy; EXTRACTED)
- [happy] Defaults to true when VOX_TELEMETRY env var is unset  (crates/vox-telemetry/src/config.rs)
- [error] Is set to false when master enabled is false  (crates/vox-telemetry/src/config.rs)

### `Aggregator`  (happy; EXTRACTED)
- [happy] Accumulates total input tokens, output tokens, cost, and call count from multiple ModelCall events with the same task_id  (crates/vox-telemetry/src/aggregator.rs)

### `Aggregator.child_call_count`  (happy; EXTRACTED)
- [happy] Counts total number of ModelCall events observed  (crates/vox-telemetry/src/aggregator.rs)

### `Aggregator.observe()`  (edge; EXTRACTED)
- [edge] Ignores ModelCall events that have None task_id, leaving aggregation empty  (crates/vox-telemetry/src/aggregator.rs)

### `Aggregator.total_cost_usd`  (happy; EXTRACTED)
- [happy] Sums cost in USD across multiple observed ModelCall events with floating-point precision  (crates/vox-telemetry/src/aggregator.rs)

### `Aggregator.total_input_tokens`  (happy; EXTRACTED)
- [happy] Sums input tokens across multiple observed ModelCall events  (crates/vox-telemetry/src/aggregator.rs)

### `Aggregator.total_output_tokens`  (happy; EXTRACTED)
- [happy] Sums output tokens across multiple observed ModelCall events  (crates/vox-telemetry/src/aggregator.rs)

### `AuditEffortRunCompletedEvent`  (happy; EXTRACTED)
- [happy] AuditEffortRunCompletedEvent with f64 hybrid_coverage_percent field can be serialized and deserialized successfully  (crates/vox-telemetry/src/types.rs)

### `AuditEffortRunFailedEvent`  (happy; EXTRACTED)
- [happy] AuditEffortRunFailedEvent can be serialized and deserialized successfully with error_kind and message fields  (crates/vox-telemetry/src/types.rs)

### `AuditEffortRunStartedEvent`  (happy; EXTRACTED)
- [happy] AuditEffortRunStartedEvent can be serialized to JSON and deserialized back to an equal struct, preserving field values  (crates/vox-telemetry/src/types.rs)

### `AuditRouteClusterDecidedEvent`  (happy; EXTRACTED)
- [happy] AuditRouteClusterDecidedEvent can be serialized and deserialized with boolean verified field preserved  (crates/vox-telemetry/src/types.rs)

### `AuditRouteRunCompletedEvent`  (happy; EXTRACTED)
- [happy] AuditRouteRunCompletedEvent can be serialized and deserialized successfully preserving all integer field values  (crates/vox-telemetry/src/types.rs)

### `AuditRouteRunFailedEvent`  (happy; EXTRACTED)
- [happy] AuditRouteRunFailedEvent can be serialized and deserialized with error_kind and message fields  (crates/vox-telemetry/src/types.rs)

### `AuditRouteRunStartedEvent`  (happy; EXTRACTED)
- [happy] AuditRouteRunStartedEvent serializes to JSON containing run_id value and deserializes back to equal struct  (crates/vox-telemetry/src/types.rs)

### `CompositeRecorder`  (happy; EXTRACTED)
- [happy] distributes events to all composed recorders via fanout  (crates/vox-telemetry/tests/recorder_integration.rs)

### `CompositeRecorder::record()`  (happy; EXTRACTED)
- [happy] CompositeRecorder fans out events to all recorders in its composition including CaptureRecorder  (crates/vox-telemetry/tests/recorder_integration.rs)

### `TaskRootSummaryEvent.wall_time_ms`  (happy; EXTRACTED)
- [happy] Is populated with elapsed milliseconds since task started when filled with zero initial value  (crates/vox-telemetry/src/aggregator.rs)

### `TelemetryConfig`  (invariant; EXTRACTED)
- [invariant] TelemetryConfig all_off preset disables all telemetry categories  (crates/vox-telemetry/tests/trace_context_smoke.rs)

### `TelemetryConfig.debug_to_stderr`  (happy; EXTRACTED)
- [happy] Is set to true when VOX_TELEMETRY is set to 'debug'  (crates/vox-telemetry/src/config.rs)

### `TelemetryEvent::ModelCall`  (happy; EXTRACTED)
- [happy] TelemetryEvent::ModelCall variant can be serialized to JSON and deserialized back, preserving model, cache_read_input_tokens, and trace_id fields  (crates/vox-telemetry/src/types.rs)

### `TelemetryWriteOptions::new()`  (happy; EXTRACTED)
- [happy] TelemetryWriteOptions::new() constructs instance that session_bench() method returns "bench:" prefixed run_id  (crates/vox-telemetry/src/types.rs)

### `TelemetryWriteOptions::session_audit()`  (happy; EXTRACTED)
- [happy] TelemetryWriteOptions::session_audit() composes session prefix 'audit:' with repository name to produce 'audit:my-repo'  (crates/vox-telemetry/src/types.rs)

### `TelemetryWriteOptions::session_mcp()`  (happy; EXTRACTED)
- [happy] TelemetryWriteOptions::session_mcp() returns "mcp:" prefixed run_id string  (crates/vox-telemetry/src/types.rs)

### `TelemetryWriteOptions::session_route()`  (happy; EXTRACTED)
- [happy] TelemetryWriteOptions::session_route() returns "route:" prefixed run_id string  (crates/vox-telemetry/src/types.rs)

### `org_policy_disabled()`  (happy; EXTRACTED)
- [happy] org_policy_disabled() returns false when the org-policy file does not exist  (crates/vox-telemetry/src/config.rs)

### `record_task_started()`  (happy; EXTRACTED)
- [happy] Records task start time that is later used to populate wall_time_ms when filling task summary  (crates/vox-telemetry/src/aggregator.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Aggregator`** — only: _Accumulates total input tokens, output tokens, cost, and call count from multiple ModelCall events with the same task_id_
- **`Aggregator.child_call_count`** — only: _Counts total number of ModelCall events observed_
- **`Aggregator.total_cost_usd`** — only: _Sums cost in USD across multiple observed ModelCall events with floating-point precision_
- **`Aggregator.total_input_tokens`** — only: _Sums input tokens across multiple observed ModelCall events_
- **`Aggregator.total_output_tokens`** — only: _Sums output tokens across multiple observed ModelCall events_
- **`AuditEffortCommitJudgedEvent`** — only: _AuditEffortCommitJudgedEvent with Some Option fields serializes completely and round-trips successfully_
- **`AuditEffortRunCompletedEvent`** — only: _AuditEffortRunCompletedEvent with f64 hybrid_coverage_percent field can be serialized and deserialized successfully_
- **`AuditEffortRunFailedEvent`** — only: _AuditEffortRunFailedEvent can be serialized and deserialized successfully with error_kind and message fields_
- **`AuditEffortRunStartedEvent`** — only: _AuditEffortRunStartedEvent can be serialized to JSON and deserialized back to an equal struct, preserving field values_
- **`AuditRouteClusterDecidedEvent`** — only: _AuditRouteClusterDecidedEvent can be serialized and deserialized with boolean verified field preserved_
- **`AuditRouteRunCompletedEvent`** — only: _AuditRouteRunCompletedEvent can be serialized and deserialized successfully preserving all integer field values_
- **`AuditRouteRunFailedEvent`** — only: _AuditRouteRunFailedEvent can be serialized and deserialized with error_kind and message fields_
- **`AuditRouteRunStartedEvent`** — only: _AuditRouteRunStartedEvent serializes to JSON containing run_id value and deserializes back to equal struct_
- **`CompositeRecorder`** — only: _distributes events to all composed recorders via fanout_
- **`CompositeRecorder::record()`** — only: _CompositeRecorder fans out events to all recorders in its composition including CaptureRecorder_
- **`NoOpRecorder`** — only: _correctly participates in composite recorder without side effects_
- **`TaskRootSummaryEvent.wall_time_ms`** — only: _Is populated with elapsed milliseconds since task started when filled with zero initial value_
- **`TelemetryConfig.debug_to_stderr`** — only: _Is set to true when VOX_TELEMETRY is set to 'debug'_
- **`TelemetryConfig::all_off()`** — only: _disables enabled, remote_upload, research_metrics, model_calls, agent_orchestration, build, and errors flags_
- **`TelemetryConfig::default_on()`** — only: _enables primary telemetry (enabled, research_metrics, model_calls, errors) while keeping remote_upload disabled_
- **`TelemetryEvent::ModelCall`** — only: _TelemetryEvent::ModelCall variant can be serialized to JSON and deserialized back, preserving model, cache_read_input_tokens, and trace_id fields_
- **`TelemetryWriteOptions::new()`** — only: _TelemetryWriteOptions::new() constructs instance that session_bench() method returns "bench:" prefixed run_id_
- **`TelemetryWriteOptions::session_audit()`** — only: _TelemetryWriteOptions::session_audit() composes session prefix 'audit:' with repository name to produce 'audit:my-repo'_
- **`TelemetryWriteOptions::session_mcp()`** — only: _TelemetryWriteOptions::session_mcp() returns "mcp:" prefixed run_id string_
- **`TelemetryWriteOptions::session_route()`** — only: _TelemetryWriteOptions::session_route() returns "route:" prefixed run_id string_
- **`org_policy_disabled()`** — only: _org_policy_disabled() returns false when the org-policy file does not exist_
- **`record_task_started()`** — only: _Records task start time that is later used to populate wall_time_ms when filling task summary_
