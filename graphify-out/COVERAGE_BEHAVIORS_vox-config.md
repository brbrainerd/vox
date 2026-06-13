# Semantic Behavior Map — `vox-config`

Deterministically synthesized from 94 distinct proven-behavior claims (of 94 extracted) across 45 symbols. 5 symbols have an explicit error-path proof; **25 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `VoxConfig::default()`  (happy, invariant; EXTRACTED)
- [happy] returns configuration with hitl.enabled=true, gamify_enabled=true, and train_batch_size=256  (crates/vox-config/tests/config_cross_module.rs)
- [happy] model_dir path ends with 'models' directory name  (crates/vox-config/tests/config_cross_module.rs)
- [happy] serializes to valid JSON string  (crates/vox-config/tests/config_cross_module.rs)
- [invariant] default instance has non-empty model field  (crates/vox-config/src/config/impl_ops.rs)
- [invariant] default instance has positive daily_budget_usd  (crates/vox-config/src/config/impl_ops.rs)
- [invariant] default instance has positive train_epochs  (crates/vox-config/src/config/impl_ops.rs)
- [invariant] default instance has positive train_batch_size  (crates/vox-config/src/config/impl_ops.rs)
- [invariant] default web_run_mode is WebRunMode::Auto  (crates/vox-config/src/config/impl_ops.rs)
- [invariant] default web_tanstack_start is false  (crates/vox-config/src/config/impl_ops.rs)

### `BuildTarget::from_str()`  (error, happy; EXTRACTED)
- [happy] parses "fullstack" variant successfully  (crates/vox-config/src/config/impl_ops.rs)
- [happy] parses "server" variant successfully  (crates/vox-config/src/config/impl_ops.rs)
- [happy] parses "client" variant successfully  (crates/vox-config/src/config/impl_ops.rs)
- [happy] parses uppercase variants case-insensitively  (crates/vox-config/src/config/impl_ops.rs)
- [happy] parses variants with leading/trailing whitespace  (crates/vox-config/src/config/impl_ops.rs)
- [error] empty string parse returns error  (crates/vox-config/src/config/impl_ops.rs)
- [error] unknown variant "ios" parse returns error  (crates/vox-config/src/config/impl_ops.rs)
- [error] unknown variant "backend" parse returns error  (crates/vox-config/src/config/impl_ops.rs)

### `merge_vox_toml_path_for_test()`  (edge, happy; EXTRACTED)
- [happy] reading Vox.toml with [web] run_mode = "app" sets web_run_mode to WebRunMode::App  (crates/vox-config/src/config/impl_ops.rs)
- [happy] reading Vox.toml with [web] tanstack_start = true sets web_tanstack_start flag  (crates/vox-config/src/config/impl_ops.rs)
- [happy] reading Vox.toml with [build] target = "server" sets build_target to BuildTarget::Server  (crates/vox-config/src/config/impl_ops.rs)
- [happy] reading Vox.toml with [build] target = "fullstack" sets build_target to BuildTarget::Fullstack  (crates/vox-config/src/config/impl_ops.rs)
- [edge] reading Vox.toml without [build] section defaults build_target to Fullstack  (crates/vox-config/src/config/impl_ops.rs)

### `AutoRoutingPriority`  (edge, happy; EXTRACTED)
- [happy] AutoRoutingPriority::parse_csv reads axis values from comma-separated key=value pairs  (crates/vox-config/src/routing_policy.rs)
- [happy] AutoRoutingPriority::parse_csv accepts semantic aliases (quality for precision, speed for latency)  (crates/vox-config/src/routing_policy.rs)
- [edge] AutoRoutingPriority::parse_csv ignores unknown keys and non-numeric values  (crates/vox-config/src/routing_policy.rs)
- [edge] AutoRoutingPriority::parse_csv returns default weights for empty input  (crates/vox-config/src/routing_policy.rs)

### `db_circuit_breaker_token()`  (edge, error, happy; EXTRACTED)
- [happy] accepts exactly '1' and 'true' (case-insensitive, whitespace-tolerant) as valid inputs  (crates/vox-config/src/rollout.rs)
- [edge] rejects 'yes', '0', and other tokens that truthy_token accepts, enforcing stricter validation than truthy_token  (crates/vox-config/src/rollout.rs)
- [happy] db_circuit_breaker_token accepts '1' and 'true' (case-insensitive with trim)  (crates/vox-config/src/rollout.rs)
- [error] db_circuit_breaker_token rejects 'yes' and '0'  (crates/vox-config/src/rollout.rs)

### `load_status_for_branches()`  (edge, happy, invariant; EXTRACTED)
- [happy] returns Vec of tuples containing branch name and Option<PolicyRunReport>  (crates/vox-config/src/policy/status.rs)
- [happy] returns Some(report) for branches with existing status files  (crates/vox-config/src/policy/status.rs)
- [edge] returns None for branch names that do not have corresponding status files  (crates/vox-config/src/policy/status.rs)
- [invariant] sanitizes branch names in file lookup while preserving original names in results  (crates/vox-config/src/policy/status.rs)

### `local_ollama_populi_base_url()`  (happy, invariant; EXTRACTED)
- [happy] local_ollama_populi_base_url returns the default URL when no environment variables are set  (crates/vox-config/src/inference.rs)
- [happy] local_ollama_populi_base_url respects OLLAMA_URL environment variable  (crates/vox-config/src/inference.rs)
- [happy] local_ollama_populi_base_url prefers POPULI_URL over OLLAMA_URL  (crates/vox-config/src/inference.rs)
- [invariant] local_ollama_populi_base_url returns a string with http:// or https:// scheme  (crates/vox-config/src/lib.rs)

### `truthy_token()`  (error, happy; EXTRACTED)
- [happy] accepts string tokens '1', 'true', 'yes', 'True', ' YES ' (case-insensitive and whitespace-tolerant) as truthy values  (crates/vox-config/src/rollout.rs)
- [happy] rejects string tokens '0' and 'no' as falsy values  (crates/vox-config/src/rollout.rs)
- [happy] truthy_token accepts '1', 'true', 'yes' in any case with surrounding whitespace trimmed  (crates/vox-config/src/rollout.rs)
- [error] truthy_token rejects '0' and 'no'  (crates/vox-config/src/rollout.rs)

### `AutoRoutingPriority::parse_csv()`  (edge, happy; EXTRACTED)
- [happy] parses comma-separated key=value pairs for efficiency, quality (mapped to precision), speed (mapped to latency), availability, balance, and mobile axes  (crates/vox-config/src/routing_policy.rs)
- [edge] ignores unknown keys (bogus=99) and non-numeric values (latency=oops) while preserving previously parsed values  (crates/vox-config/src/routing_policy.rs)
- [edge] returns default AutoRoutingPriority when given empty string input  (crates/vox-config/src/routing_policy.rs)

### `InferenceProfile`  (happy; EXTRACTED)
- [happy] InferenceProfile::default is DesktopOllama  (crates/vox-config/src/lib.rs)
- [happy] InferenceProfile::allows_local_ollama_http returns true for DesktopOllama and LanGateway variants  (crates/vox-config/src/lib.rs)
- [happy] InferenceProfile::allows_local_ollama_http returns false for CloudOpenAiCompatible, MobileLitert, and MobileCoreml variants  (crates/vox-config/src/lib.rs)

### `ProjectManifest`  (happy; EXTRACTED)
- [happy] ProjectManifest::load parses workspace members from TOML  (crates/vox-config/src/project_manifest.rs)
- [happy] ProjectManifest::load parses bundle identifier from TOML  (crates/vox-config/src/project_manifest.rs)
- [happy] ProjectManifest::member_manifest_paths resolves relative member paths to absolute Vox.toml paths  (crates/vox-config/src/project_manifest.rs)

### `parse_u64_opt()`  (error, happy; EXTRACTED)
- [happy] parse_u64_opt returns the default value when input is None  (crates/vox-config/src/env_parse.rs)
- [happy] parse_u64_opt trims whitespace and parses valid u64 strings  (crates/vox-config/src/env_parse.rs)
- [error] parse_u64_opt returns the default value when parsing fails on non-numeric input  (crates/vox-config/src/env_parse.rs)

### `rollout_flag_snapshot()`  (happy; EXTRACTED)
- [happy] serializes to JSON containing orchestration_lineage_persist, workflow_journal_codex_persist, db_circuit_breaker_env, db_sync_remote_integration_gate, and db_embedded_replica_integration_gate fields  (crates/vox-config/src/rollout.rs)
- [happy] serializes to JSON string containing 'orchestration_lineage_persist' field  (crates/vox-config/tests/config_cross_module.rs)
- [happy] rollout_flag_snapshot returns a struct that serializes all expected fields via serde_json  (crates/vox-config/src/rollout.rs)

### `sanitize_branch()`  (edge, happy; EXTRACTED)
- [edge] converts forward slashes to hyphens in branch names  (crates/vox-config/src/policy/status.rs)
- [edge] converts backslashes and spaces to hyphens for filesystem safety  (crates/vox-config/src/policy/status.rs)
- [happy] returns unmodified branch name when no special characters are present  (crates/vox-config/src/policy/status.rs)

### `AutoRoutingPriority::try_parse_csv()`  (error, happy; EXTRACTED)
- [happy] returns Some(AutoRoutingPriority) when at least one valid axis is parsed (e.g., 'latency=42')  (crates/vox-config/src/routing_policy.rs)
- [error] returns None for empty string or wholly malformed input (garbage with no valid key=value pairs)  (crates/vox-config/src/routing_policy.rs)

### `save_merged_global_config()`  (happy; EXTRACTED)
- [happy] preserves unknown keys (future_vox, mcp_binary, db_extra) when merging and persisting config  (crates/vox-config/src/config/impl_ops.rs)
- [happy] preserves optional keys and their values across save/load cycle  (crates/vox-config/src/config/impl_ops.rs)

### `secrets_cutover_blocks_legacy_env_raw()`  (happy; EXTRACTED)
- [happy] secrets_cutover_blocks_legacy_env_raw returns true for 'enforce' and 'Decommission' phases (case-insensitive with trim)  (crates/vox-config/src/lib.rs)
- [happy] secrets_cutover_blocks_legacy_env_raw returns false for 'shadow' and empty string  (crates/vox-config/src/lib.rs)

### `AutoRoutingPriority::default()`  (happy; EXTRACTED)
- [happy] has efficiency axis set to 25  (crates/vox-config/tests/config_cross_module.rs)

### `BuildTarget::default()`  (invariant; EXTRACTED)
- [invariant] default variant is Fullstack  (crates/vox-config/src/config/impl_ops.rs)

### `EFFORT_AUDIT_JUDGE_TIMEOUT`  (invariant; EXTRACTED)
- [invariant] constant equals 60 seconds duration  (crates/vox-config/src/timeouts.rs)

### `GeminiRoutePolicy::from_env()`  (happy; EXTRACTED)
- [happy] reads VOX_GEMINI_ROUTE_POLICY env var and parses it to GeminiRoutePolicy enum (GoogleDirectOnly variant)  (crates/vox-config/src/routing_policy.rs)

### `HTTP_CONNECT`  (invariant; EXTRACTED)
- [invariant] constant equals 15 seconds duration  (crates/vox-config/src/timeouts.rs)

### `HTTP_REQUEST`  (invariant; EXTRACTED)
- [invariant] constant equals 30 seconds duration  (crates/vox-config/src/timeouts.rs)

### `InferenceProfile::default()`  (happy; EXTRACTED)
- [happy] returns DesktopOllama variant as the default profile  (crates/vox-config/tests/config_cross_module.rs)

### `LEASE_HOUR`  (invariant; EXTRACTED)
- [invariant] constant equals 3600 seconds duration  (crates/vox-config/src/timeouts.rs)

### `POLL_TICK_FAST`  (invariant; EXTRACTED)
- [invariant] constant equals 100 milliseconds duration  (crates/vox-config/src/timeouts.rs)

### `PolicyDomain::CodeAuditRule`  (happy; EXTRACTED)
- [happy] is a valid enum variant that deserialization produces  (crates/vox-config/src/policy/registry.rs)

### `PolicyRegistry`  (happy; EXTRACTED)
- [happy] deserializes from YAML with schema_version and policies array fields  (crates/vox-config/src/policy/registry.rs)

### `PolicyRegistry.default_enabled`  (edge; EXTRACTED)
- [edge] defaults to true when not specified in YAML  (crates/vox-config/src/policy/registry.rs)

### `PolicyRegistry.origin`  (happy; EXTRACTED)
- [happy] is set to 'builtin' string for loaded policies  (crates/vox-config/src/policy/registry.rs)

### `PolicyRunReport`  (happy; EXTRACTED)
- [happy] deserializes from JSON with branch, commit, ran_at, and results array fields  (crates/vox-config/src/policy/status.rs)

### `PolicyRunReport.results[].hits[]`  (happy; EXTRACTED)
- [happy] contains Hit items with file and line number fields  (crates/vox-config/src/policy/status.rs)

### `PolicySeverity::Error`  (happy; EXTRACTED)
- [happy] is a valid enum variant that deserializes from YAML 'error' string  (crates/vox-config/src/policy/registry.rs)

### `PolicySourceKind::Pattern`  (happy; EXTRACTED)
- [happy] is a valid enum variant that deserializes from YAML 'pattern' string  (crates/vox-config/src/policy/registry.rs)

### `RunStatus`  (invariant; EXTRACTED)
- [invariant] implements Default trait that returns RunStatus::Unknown variant  (crates/vox-config/src/policy/status.rs)

### `RunStatus::Fail`  (happy; EXTRACTED)
- [happy] is a valid enum variant that deserializes from JSON 'fail' string  (crates/vox-config/src/policy/status.rs)

### `RunStatus::Pass`  (happy; EXTRACTED)
- [happy] is a valid enum variant that deserializes from JSON 'pass' string  (crates/vox-config/src/policy/status.rs)

### `VoxConfig`  (happy; EXTRACTED)
- [happy] deserializes from JSON preserving model, train_batch_size, hitl.enabled, and gamify_mode fields  (crates/vox-config/tests/config_cross_module.rs)

### `VoxConfig::get_key()`  (happy; EXTRACTED)
- [happy] get_key("web.run_mode") returns the set value  (crates/vox-config/src/config/impl_ops.rs)

### `VoxConfig::merge_build_target_from_env_var()`  (happy; EXTRACTED)
- [happy] VOX_BUILD_TARGET environment variable overrides TOML-derived build_target value  (crates/vox-config/src/config/impl_ops.rs)

### `VoxConfig::set_key()`  (happy; EXTRACTED)
- [happy] set_key("web.run_mode", "script") succeeds and updates web_run_mode to WebRunMode::Script  (crates/vox-config/src/config/impl_ops.rs)

### `gemini_route_targets_from_env()`  (happy; EXTRACTED)
- [happy] reads OPENROUTER_GEMINI_MODEL and GEMINI_DIRECT_MODEL env vars and returns targets with corresponding model strings  (crates/vox-config/src/routing_policy.rs)

### `load_policy_registry()`  (happy; EXTRACTED)
- [happy] successfully loads and parses policy registry YAML from tempdir path  (crates/vox-config/src/policy/registry.rs)

### `parse_usize_opt()`  (happy; EXTRACTED)
- [happy] parse_usize_opt trims whitespace and parses valid usize strings  (crates/vox-config/src/env_parse.rs)

### `resolve_openrouter_model()`  (happy; EXTRACTED)
- [happy] returns non-empty trimmed string when given valid model identifier  (crates/vox-config/tests/config_cross_module.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`AutoRoutingPriority::default()`** — only: _has efficiency axis set to 25_
- **`GeminiRoutePolicy::from_env()`** — only: _reads VOX_GEMINI_ROUTE_POLICY env var and parses it to GeminiRoutePolicy enum (GoogleDirectOnly variant)_
- **`InferenceProfile`** — only: _InferenceProfile::default is DesktopOllama_
- **`InferenceProfile::default()`** — only: _returns DesktopOllama variant as the default profile_
- **`PolicyDomain::CodeAuditRule`** — only: _is a valid enum variant that deserialization produces_
- **`PolicyRegistry`** — only: _deserializes from YAML with schema_version and policies array fields_
- **`PolicyRegistry.origin`** — only: _is set to 'builtin' string for loaded policies_
- **`PolicyRunReport`** — only: _deserializes from JSON with branch, commit, ran_at, and results array fields_
- **`PolicyRunReport.results[].hits[]`** — only: _contains Hit items with file and line number fields_
- **`PolicySeverity::Error`** — only: _is a valid enum variant that deserializes from YAML 'error' string_
- **`PolicySourceKind::Pattern`** — only: _is a valid enum variant that deserializes from YAML 'pattern' string_
- **`ProjectManifest`** — only: _ProjectManifest::load parses workspace members from TOML_
- **`RunStatus::Fail`** — only: _is a valid enum variant that deserializes from JSON 'fail' string_
- **`RunStatus::Pass`** — only: _is a valid enum variant that deserializes from JSON 'pass' string_
- **`VoxConfig`** — only: _deserializes from JSON preserving model, train_batch_size, hitl.enabled, and gamify_mode fields_
- **`VoxConfig::get_key()`** — only: _get_key("web.run_mode") returns the set value_
- **`VoxConfig::merge_build_target_from_env_var()`** — only: _VOX_BUILD_TARGET environment variable overrides TOML-derived build_target value_
- **`VoxConfig::set_key()`** — only: _set_key("web.run_mode", "script") succeeds and updates web_run_mode to WebRunMode::Script_
- **`gemini_route_targets_from_env()`** — only: _reads OPENROUTER_GEMINI_MODEL and GEMINI_DIRECT_MODEL env vars and returns targets with corresponding model strings_
- **`load_policy_registry()`** — only: _successfully loads and parses policy registry YAML from tempdir path_
- **`parse_usize_opt()`** — only: _parse_usize_opt trims whitespace and parses valid usize strings_
- **`resolve_openrouter_model()`** — only: _returns non-empty trimmed string when given valid model identifier_
- **`rollout_flag_snapshot()`** — only: _serializes to JSON containing orchestration_lineage_persist, workflow_journal_codex_persist, db_circuit_breaker_env, db_sync_remote_integration_gate, and db_embedded_replica_integration_gate fields_
- **`save_merged_global_config()`** — only: _preserves unknown keys (future_vox, mcp_binary, db_extra) when merging and persisting config_
- **`secrets_cutover_blocks_legacy_env_raw()`** — only: _secrets_cutover_blocks_legacy_env_raw returns true for 'enforce' and 'Decommission' phases (case-insensitive with trim)_
