# Semantic Behavior Map — `vox-orchestrator-mcp`

Deterministically synthesized from 232 distinct proven-behavior claims (of 232 extracted) across 85 symbols. 15 symbols have an explicit error-path proof; **55 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `attach_aci_envelope()`  (happy; EXTRACTED)
- [happy] attach_aci_envelope() adds aci envelope with tool name, mutation_kind read_only, and empty side_effects for vox_git_status  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() preserves JSON success field in roundtrip  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() sets mutation_kind to local_mutation for vox_write_file tool  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() includes workspace_state_change in side_effects for vox_write_file  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() defaults shell_backend to powershell for vox_run_shell  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() includes shell_exec and external_io in side_effects for vox_run_shell  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() sets shell_backend to nushell when args backend is nu  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() preserves execution_probe from meta in aci envelope  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() successfully attaches ACI metadata envelope to a tool result JSON, preserving the success field and adding aci.tool, aci.mutation_kind as 'read_only', empty side_effects array, and null shell_backend for read-only tools  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() for 'vox_write_file' sets mutation_kind to 'local_mutation' and includes 'workspace_state_change' in side_effects array  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() for 'vox_run_shell' defaults shell_backend to 'powershell' and adds 'shell_exec' and 'external_io' to side_effects  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- [happy] attach_aci_envelope() respects the 'backend' parameter in args and sets shell_backend to 'nushell' when args contains {"backend": "nu"}  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)
- … +1 more claims

### `enqueue_hints_from_submit_params()`  (happy; EXTRACTED)
- [happy] Returns None when no task_category or campaign signals are present in SubmitTaskParams  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] Maps task_category='testing' to execution_role=AgentExecutionRole::Verifier  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] Extracts campaign_id and benchmark_tier from description parsing and preserves complexity parameter  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] Prefers structured campaign_id and benchmark_tier parameters over parsed description tags  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] Returns EnqueueHints with campaign_id from structured field, overriding parsed description tags  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] Returns EnqueueHints with benchmark_tier from structured field, overriding parsed description tags  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] enqueue_hints_from_submit_params() returns None when task description contains no special signals or tags  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] enqueue_hints_from_submit_params() maps task_category 'testing' to AgentExecutionRole::Verifier  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] enqueue_hints_from_submit_params() extracts campaign_id from description tokens and returns in hints  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] enqueue_hints_from_submit_params() extracts benchmark_tier from description and returns ReconstructionBenchmarkTier::IssueRepair in hints  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] enqueue_hints_from_submit_params() preserves complexity field from params in returned hints  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)

### `resolve_mcp_chat_model_sync()`  (error, happy, invariant; EXTRACTED)
- [error] Sticky Ollama route is rejected when inference profile is cloud_openai_compatible (error contains profile hint)  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [error] enforce_free_tier_only constraint fails with error when cloud profile forbids Ollama and only Ollama is free  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Sticky model re-route preserves canonical MCP decision model id  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] VoxLocal provider is preferred for CodeGen tasks under desktop_ollama inference profile  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] VoxLocal provider is not preferred for Research (non-code) tasks under desktop_ollama profile  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [error] When inference profile is set to cloud_openai_compatible and sticky model is llama-local with only Ollama available, resolve_mcp_chat_model_sync() fails with an error mentioning the profile constraint.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [error] When enforce_free_tier_only=true and inference profile is cloud_openai_compatible with only Ollama free in registry, resolve_mcp_chat_model_sync() fails with an error.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] When resolve_mcp_chat_model_sync() is called with a sticky model ID matching a previously resolved canonical decision, it returns the same model ID.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] When inference profile is desktop_ollama and task category is CodeGen, resolve_mcp_chat_model_sync() prefers VoxLocal provider type.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] When inference profile is desktop_ollama and task category is Research, resolve_mcp_chat_model_sync() does not select VoxLocal provider.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)

### `run_retrieval_bundle()`  (happy; EXTRACTED)
- [happy] Returns evidence with used_bm25=true, used_lexical_fallback=false, and retrieval_tier='bm25'  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)
- [happy] run_retrieval_bundle() populates evidence.selected_mode field when invoked  (crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs)
- [happy] run_retrieval_bundle() preserves RetrievalTriggerMode::VerificationPass in evidence.trigger field  (crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs)
- [happy] run_retrieval_bundle() extracts search intent from query and populates evidence.search_intent field  (crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs)
- [happy] run_retrieval_bundle() selects 'hybrid' retrieval mode and populates evidence.selected_mode field  (crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs)
- [happy] run_retrieval_bundle() populates evidence.retrieval_tier field with non-empty value  (crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs)
- [happy] run_retrieval_bundle() prefers BM25 algorithm over lexical fallback when keyword matches exist  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)
- [happy] run_retrieval_bundle() sets evidence.used_bm25 to true when BM25 algorithm is used  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)
- [happy] run_retrieval_bundle() sets evidence.used_lexical_fallback to false when BM25 finds matches  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)
- [happy] run_retrieval_bundle() populates evidence.retrieval_tier with 'bm25' string when BM25 algorithm is used  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)

### `route_free_tier_latency()`  (error, happy; EXTRACTED)
- [happy] Latency-critical routing prefers Fast tier models over Pro tier models with larger token limits via +5.0 latency bonus  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/free_tier_adapter.rs)
- [happy] Route selection returns model.id matching the selected model  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/free_tier_adapter.rs)
- [happy] Route selection returns non-empty rationale string  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/free_tier_adapter.rs)
- [happy] When free_tier_fill_in_middle is true, FIM-capable provider (Mistral) is preferred over FIM-incapable provider (Cerebras)  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/free_tier_adapter.rs)
- [error] Non-vision model does not satisfy vision capability requirement (returns None)  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/free_tier_adapter.rs)
- [happy] Vision-capable model satisfies vision capability requirement (returns Some)  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/free_tier_adapter.rs)
- [error] Accept filter gate blocks Ollama provider when predicate returns false for Ollama type  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/free_tier_adapter.rs)

### `is_origin_allowed()`  (edge, error, happy; EXTRACTED)
- [happy] is_origin_allowed returns true for loopback origins (`http://localhost:3000` and `https://127.0.0.1:8080`)  (crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs)
- [error] is_origin_allowed returns false when Origin header is non-loopback (https://attacker.com) even if Host is spoofed  (crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs)
- [error] is_origin_allowed returns false for external origins (https://malicious.com)  (crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs)
- [edge] is_origin_allowed returns true for /v1/eval and /health paths when public_eval_enabled is true, regardless of origin  (crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs)
- [error] is_origin_allowed returns false for origins containing localhost or 127.0.0.1 as subdomain prefixes (localhost.evil.com, 127.0.0.1.evil.com)  (crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs)
- [edge] is_origin_allowed returns false for WebSocket upgrade requests without origin or host header, true when origin is present  (crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs)

### `mcp_provider_telemetry_labels()`  (invariant; EXTRACTED)
- [invariant] OpenRouter provider type telemetry labels match runtime ChatProviderRouteKind::OpenRouter telemetry labels  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Ollama provider type telemetry labels match runtime PopuliLocal route telemetry labels  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] GoogleDirect provider type telemetry labels match runtime ManualOpenAiCompatible Gemini route telemetry labels  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Groq provider type telemetry labels match runtime ManualOpenAiCompatible custom route telemetry labels  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Mistral provider type telemetry labels match HuggingFace router route telemetry labels  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Route telemetry output matches canonical route backend for resolved model  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)

### `parse_branch_exists()`  (edge, happy; EXTRACTED)
- [happy] Returns true when git output is nonempty after trimming whitespace  (crates/vox-orchestrator-mcp/src/vcs_tools/branch_tools.rs)
- [happy] Returns false for empty string input  (crates/vox-orchestrator-mcp/src/vcs_tools/branch_tools.rs)
- [happy] Returns false when input contains only whitespace characters  (crates/vox-orchestrator-mcp/src/vcs_tools/branch_tools.rs)
- [happy] Returns true when given non-empty output containing whitespace and newline  (crates/vox-orchestrator-mcp/src/vcs_tools/branch_tools.rs)
- [edge] Returns false when given empty string  (crates/vox-orchestrator-mcp/src/vcs_tools/branch_tools.rs)
- [edge] Returns false when given whitespace-only string  (crates/vox-orchestrator-mcp/src/vcs_tools/branch_tools.rs)

### `parse_campaign_from_description()`  (happy; EXTRACTED)
- [happy] Extracts campaign_id and ReconstructionBenchmarkTier from [campaign:X] and [tier:Y] bracketed tags  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] Accepts case-insensitive [Campaign:] and [TIER:] tag prefixes alongside lowercase variants  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] parse_campaign_from_description() extracts campaign ID from [campaign:alpha1] tag  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] parse_campaign_from_description() extracts benchmark tier ReconstructionBenchmarkTier::CrateRegen from [tier:crate_regen] tag  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] parse_campaign_from_description() accepts uppercase [Campaign:] prefix variant (case-insensitive)  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)
- [happy] parse_campaign_from_description() accepts uppercase [TIER:] prefix variant and maps to ReconstructionBenchmarkTier::RepoRegen  (crates/vox-orchestrator-mcp/src/task_tools/tests.rs)

### `route_policy_allows_model()`  (happy; EXTRACTED)
- [happy] route_policy_allows_model() returns true for OpenRouter cloud models when VOX_ROUTE_ALLOW_NET and VOX_ROUTE_ALLOW_PROVIDER_NETWORK environment variables are set to 1  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] route_policy_allows_model() returns false for local Ollama models when VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP is not set, even when VOX_ROUTE_ALLOW_NET and VOX_ROUTE_ALLOW_PROVIDER_NETWORK overrides are enabled  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] route_policy_allows_model() returns true for local Ollama models when VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP environment variable is explicitly set to 1  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] Restricted route profile allows cloud (OpenRouter) when VOX_ROUTE_ALLOW_NET and VOX_ROUTE_ALLOW_PROVIDER_NETWORK are set  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] Restricted route profile blocks local HTTP models until VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP is explicitly set  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] Restricted route profile allows local HTTP models when VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP is truthy  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)

### `PlanTask`  (edge, happy; EXTRACTED)
- [happy] Plan JSON schema supports summary and tasks fields with nested task id description files estimated_complexity and depends_on  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [edge] Plan JSON schema accepts empty tasks array  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] PlanTask deserializes from JSON with id description files estimated_complexity and depends_on fields  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] PlanTask can be successfully deserialized from JSON containing id, description, files, estimated_complexity, and depends_on fields  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] PlanTask deserializes from raw JSON without markdown fence, correctly extracting id, description, estimated_complexity, and empty depends_on array  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)

### `build_commit_message()`  (happy; EXTRACTED)
- [happy] Includes Co-authored-by trailer with provided author name and email  (crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs)
- [happy] Includes Vox-Model-Id trailer with provided model identifier  (crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs)
- [happy] Includes Vox-Workspace trailer with provided workspace ID  (crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs)
- [happy] Zero-pads workspace ID to 6 digits in Vox-Workspace trailer (W-000001 format)  (crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs)
- [happy] Removes trailing whitespace from message body before trailer separator  (crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs)

### `route_backend_for_model()`  (happy, invariant; EXTRACTED)
- [invariant] Orchestrator route backend for GoogleDirect matches runtime ChatRouteBackend via route_backend_for_chat_route()  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Orchestrator route backend for OpenRouter matches runtime ChatRouteBackend via route_backend_for_chat_route()  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Orchestrator route backend for Ollama matches runtime ChatRouteBackend via route_backend_for_chat_route()  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [invariant] Orchestrator route backend for Groq/CascadeFallback matches runtime ChatRouteBackend via route_backend_for_chat_route()  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] route_backend_for_model() output converted to ChatRouteBackend telemetry labels matches mcp_provider_telemetry_labels() for the resolved model's provider type.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)

### `route_telemetry_labels()`  (happy; EXTRACTED)
- [happy] OpenRouter ChatProviderRouteKind produces telemetry labels that match mcp_provider_telemetry_labels() for ProviderType::OpenRouter.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] PopuliLocal ChatProviderRouteKind (Ollama) produces telemetry labels matching mcp_provider_telemetry_labels() for ProviderType::Ollama.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] ManualOpenAiCompatible route for Gemini produces telemetry labels matching mcp_provider_telemetry_labels() for ProviderType::GoogleDirect.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] ManualOpenAiCompatible BYOK route produces telemetry labels matching mcp_provider_telemetry_labels() for ProviderType::Groq.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] HuggingFaceRouter ChatProviderRouteKind produces telemetry labels matching mcp_provider_telemetry_labels() for ProviderType::Mistral.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)

### `ChatHistoryParams`  (happy; EXTRACTED)
- [happy] Empty JSON object deserializes with default session_id of 'default'  (crates/vox-orchestrator-mcp/src/chat_tools/params.rs)
- [happy] Empty JSON object deserializes with trace_id as None  (crates/vox-orchestrator-mcp/src/chat_tools/params.rs)
- [happy] Explicit session_id round-trips through JSON serialization/deserialization  (crates/vox-orchestrator-mcp/src/chat_tools/params.rs)
- [happy] Explicit trace_id round-trips through JSON serialization/deserialization  (crates/vox-orchestrator-mcp/src/chat_tools/params.rs)

### `PendingApprovals::resolve()`  (error, happy; EXTRACTED)
- [happy] resolve() delivers the ApprovalOutcome to a registered awaiter and wakes it from the receiver channel  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)
- [happy] resolve() removes the entry from the pending list after resolution  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)
- [error] resolve() returns false when given an unknown approval_id  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)
- [error] resolve() with ApprovalOutcome::Rejected returns an error envelope from handle_tool_call()  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)

### `clamp_http_max_output_tokens()`  (edge, happy, invariant; EXTRACTED)
- [edge] clamp_http_max_output_tokens() enforces minimum of 1 token  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] clamp_http_max_output_tokens() passes through values within bounds  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [edge] clamp_http_max_output_tokens() enforces maximum of 8192 tokens  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [invariant] clamp_http_max_output_tokens() enforces a lower bound of 1 and upper bound of 8192, allowing values within that range to pass through unchanged  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)

### `route_backend_for_chat_route()`  (happy; EXTRACTED)
- [happy] route_backend_for_chat_route() for ManualOpenAiCompatible Gemini route matches chat_lane_for_orchestrator_backend() of route_backend_for_model() for the same spec.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] route_backend_for_chat_route() for OpenRouter route matches chat_lane_for_orchestrator_backend() of route_backend_for_model() for OpenRouter spec.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] route_backend_for_chat_route() for PopuliLocal (Ollama) route matches chat_lane_for_orchestrator_backend() of route_backend_for_model() for Ollama spec.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] route_backend_for_chat_route() for ManualOpenAiCompatible Groq route produces ChatRouteBackend::CascadeFallback.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)

### `_parse_snapshot_id_value()`  (happy; EXTRACTED)
- [happy] Parses numeric JSON value (3) to SnapshotId(3)  (crates/vox-orchestrator-mcp/src/vcs_tools/parse.rs)
- [happy] Parses S-prefixed string ("S-000003") to SnapshotId(3)  (crates/vox-orchestrator-mcp/src/vcs_tools/parse.rs)
- [happy] Parses numeric string ("3") to SnapshotId(3)  (crates/vox-orchestrator-mcp/src/vcs_tools/parse.rs)

### `build_selection_request()`  (happy; EXTRACTED)
- [happy] Economy cost preference maps to COST_FIRST selection axes  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs)
- [happy] Returned request preserves task category, complexity, local preference, and cacheable workload flags  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs)
- [happy] When building a selection request with Economy cost preference, the resulting request has SelectionAxes::COST_FIRST axes.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs)

### `constant_time_eq()`  (edge, happy; EXTRACTED)
- [happy] constant_time_eq returns true for equal byte strings and false for different or different-length byte strings  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)
- [happy] Detects inequality between different-length slices without timing leaks  (crates/vox-orchestrator-mcp/src/http_gateway/mod.rs)
- [edge] Correctly handles 256-byte length boundaries without modulo overflow  (crates/vox-orchestrator-mcp/src/http_gateway/mod.rs)

### `emit_cache_miss_if_applicable()`  (edge, happy; EXTRACTED)
- [happy] emit_cache_miss_if_applicable() emits a METRIC_TYPE_ORCH_CACHE_MISS telemetry event when cache_read_input_tokens is None  (crates/vox-orchestrator-mcp/tests/telemetry_cache_miss.rs)
- [happy] emit_cache_miss_if_applicable() emits a METRIC_TYPE_ORCH_CACHE_MISS event when cache_read_input_tokens is Some(0)  (crates/vox-orchestrator-mcp/tests/telemetry_cache_miss.rs)
- [edge] emit_cache_miss_if_applicable() does not emit a cache miss event when cache_read_input_tokens contains a value greater than zero  (crates/vox-orchestrator-mcp/tests/telemetry_cache_miss.rs)

### `is_banned()`  (edge, happy; EXTRACTED)
- [happy] is_banned detects dangerous git commands including stash, reset --hard, clean -fd, restore, and checkout variants  (crates/vox-orchestrator-mcp/src/git_exec.rs)
- [happy] is_banned returns None for safe git commands: status, log, commit, and checkout with branch names  (crates/vox-orchestrator-mcp/src/git_exec.rs)
- [edge] is_banned detects banned command prefixes even when placed after git -c option  (crates/vox-orchestrator-mcp/src/git_exec.rs)

### `memory_config_for_state()`  (happy; EXTRACTED)
- [happy] Extracts log_dir and memory_md_path from ServerState's OrchestratorConfig.memory settings  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)
- [happy] memory_config_for_state() returns MemoryConfig with log_dir matching OrchestratorConfig.memory.log_dir  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)
- [happy] memory_config_for_state() returns MemoryConfig with memory_md_path matching OrchestratorConfig.memory.memory_md_path  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)

### `parse_allowed_tools_from_value()`  (error, happy; EXTRACTED)
- [happy] parse_allowed_tools_from_value with None default includes safe tool names: vox_orchestrator_status and vox_validate_file  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)
- [error] parse_allowed_tools_from_value returns error containing 'unknown tool' when given unknown tool name  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)
- [error] parse_allowed_tools_from_value() rejects unknown tool names with descriptive error containing 'unknown tool'  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `plan_result_blocked_by_adequacy_enforce()`  (happy; EXTRACTED)
- [happy] Returns true when adequacy enforce is enabled and plan is too thin  (crates/vox-orchestrator-mcp/src/chat_tools/plan.rs)
- [happy] Returns false when adequacy enforce is disabled regardless of plan thinness  (crates/vox-orchestrator-mcp/src/chat_tools/plan.rs)
- [happy] Returns false when plan is adequate even when enforce is enabled  (crates/vox-orchestrator-mcp/src/chat_tools/plan.rs)

### `resolve_existing_path_in_repository()`  (error, happy; EXTRACTED)
- [happy] resolve_existing_path_in_repository() joins a relative path with the repository root to create a canonicalized absolute path  (crates/vox-orchestrator-mcp/src/workspace_path.rs)
- [error] resolve_existing_path_in_repository() returns ResolveRepoPathError::OutsideRepository when given a relative path that escapes the repo boundary  (crates/vox-orchestrator-mcp/src/workspace_path.rs)
- [error] resolve_existing_path_in_repository() returns ResolveRepoPathError::OutsideRepository when given an absolute path outside the repo root  (crates/vox-orchestrator-mcp/src/workspace_path.rs)

### `resolve_stored_companion()`  (edge, happy; EXTRACTED)
- [happy] Selects companion with canonical storage ID when multiple companions present  (crates/vox-orchestrator-mcp/src/dei_tools/orchestrator_snapshot.rs)
- [happy] Falls back to legacy vox-dei storage ID when canonical ID not available  (crates/vox-orchestrator-mcp/src/dei_tools/orchestrator_snapshot.rs)
- [edge] Returns None when companion slice is empty  (crates/vox-orchestrator-mcp/src/dei_tools/orchestrator_snapshot.rs)

### `should_forward()`  (happy; EXTRACTED)
- [happy] should_forward returns false when topic is not in subscription set, true when topic is present, false for different topics  (crates/vox-orchestrator-mcp/src/http_gateway/scientia_feed.rs)
- [happy] should_forward returns true when subscribed to matching topic and false when subscription set is empty  (crates/vox-orchestrator-mcp/src/http_gateway/scientia_feed.rs)
- [happy] should_forward returns true when VCS_ISOLATION_CHANGED topic is in subscription set, false when subscription set is empty  (crates/vox-orchestrator-mcp/src/http_gateway/vcs_feed.rs)

### `socrates_tool_meta()`  (happy; EXTRACTED)
- [happy] socrates_tool_meta() preserves confidence_estimate parameter value  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] socrates_tool_meta() computes contradiction_ratio as half of is_contradiction when flag is true  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] socrates_tool_meta() output is valid JSON that deserializes to SocratesJsonMeta with confidence_estimate and contradiction_ratio fields accurately preserved  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)

### `tool_input_schema()`  (happy, invariant; EXTRACTED)
- [invariant] tool_input_schema() returns a non-empty schema for every tool in TOOL_REGISTRY  (crates/vox-orchestrator-mcp/src/input_schemas.rs)
- [happy] tool_input_schema('vox_submit_task') includes context_envelope_json field as optional (not required) with concrete schema shape and additionalProperties=false  (crates/vox-orchestrator-mcp/src/input_schemas.rs)
- [happy] tool_input_schema('vox_submit_task') includes harness_spec_json field with concrete schema shape and minimum length validation  (crates/vox-orchestrator-mcp/src/input_schemas.rs)

### `SocratesJsonMeta`  (happy; EXTRACTED)
- [happy] SocratesJsonMeta can deserialize from socrates_tool_meta() output  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] SocratesJsonMeta deserializes from socrates_tool_meta() output with confidence_estimate field matching the input value  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)

### `anthropic_tools_guard()`  (error, happy; EXTRACTED)
- [error] anthropic_tools_guard returns capability_gap error when tools array is present  (crates/vox-orchestrator-mcp/src/llm_bridge/provider_adapter.rs)
- [happy] anthropic_tools_guard returns Ok when neither tools nor tool_choice are present  (crates/vox-orchestrator-mcp/src/llm_bridge/provider_adapter.rs)

### `build_app()`  (happy; EXTRACTED)
- [happy] Routes /api/v2/health endpoint to 200 OK status  (crates/vox-orchestrator-mcp/src/http_gateway/mod.rs)
- [happy] Registers /api/v2/vcs/isolation route (non-404 response)  (crates/vox-orchestrator-mcp/src/http_gateway/mod.rs)

### `conflict_diff()`  (happy; EXTRACTED)
- [happy] Returns JSON response with success field set to true  (crates/vox-orchestrator-mcp/src/vcs_tools/conflicts.rs)
- [happy] Returns data object containing conflict_id, path, side_count, unique_side_hashes, all_sides_identical, resolved, and sides keys  (crates/vox-orchestrator-mcp/src/vcs_tools/conflicts.rs)

### `enforce_auth()`  (error, happy; EXTRACTED)
- [happy] enforce_auth succeeds when provided headers contain matching bearer token  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)
- [error] enforce_auth fails when headers contain bearer token that does not match expected value  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `extra_headers_for()`  (happy; EXTRACTED)
- [happy] extra_headers_for() injects HTTP-Referer header with value from VOX_OPENROUTER_HTTP_REFERER env var  (crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs)
- [happy] extra_headers_for() injects X-Title header with value from VOX_OPENROUTER_APP_TITLE env var  (crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs)

### `ghost_grounding_score()`  (happy, invariant; EXTRACTED)
- [invariant] ghost_grounding_score() scores rich context higher than minimal context  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)
- [happy] ghost_grounding_score() returns a higher score when GhostTextParams includes file_path, language, and longer prefix/suffix vs. minimal context  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)

### `handle_tool_call()`  (happy; EXTRACTED)
- [happy] Calling handle_tool_call with TOOL_REGISTRY entry names does not return 'Unknown tool' error  (crates/vox-orchestrator-mcp/src/dispatch.rs)
- [happy] handle_tool_call() for a dangerous tool registers a pending approval and parks the async call until resolved  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)

### `has_explicit_human_confirmation()`  (edge, happy; EXTRACTED)
- [happy] Detects explicit human confirmation tokens '[approval:confirm]' and '[human-approved]' in text  (crates/vox-orchestrator-mcp/src/attention_policy.rs)
- [edge] Does not detect generic phrases like 'please deploy' as explicit human confirmation  (crates/vox-orchestrator-mcp/src/attention_policy.rs)

### `http_call_tool()`  (error; EXTRACTED)
- [error] http_call_tool() returns success=false when a read-role bearer token attempts to invoke a write_only tool, with error message 'not allowed for current gateway role'  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)
- [error] http_call_tool() at router level with read-token rejects write_only tool invocations with success=false and 'not allowed for current gateway role' error  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `http_tools()`  (happy; EXTRACTED)
- [happy] http_tools() endpoint with read-role bearer token hides write_only tools from the response while exposing read-eligible tools  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)
- [happy] http_tools() at router level with read-token filters tools list to exclude write_only tools across HTTP API boundaries  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `mcp_global_llm_context_fill_ratio()`  (happy; EXTRACTED)
- [happy] Returns None when orchestrator has no context budget configured  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)
- [happy] When orchestrator has Performance cost preference without a budget configured, mcp_global_llm_context_fill_ratio() returns None.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs)

### `normalize_inbox_source()`  (happy; EXTRACTED)
- [happy] None, empty string, and whitespace-only inputs all default to A2AInboxPlane::Merged  (crates/vox-orchestrator-mcp/src/a2a_tools.rs)
- [happy] String inputs 'local', 'mesh', 'merged' map to corresponding A2AInboxPlane variants with case-insensitive matching  (crates/vox-orchestrator-mcp/src/a2a_tools.rs)

### `orch_daemon::OrchDaemonClient::call()`  (happy; EXTRACTED)
- [happy] OrchDaemonClient.call() returns a non-failure success envelope for readonly tool dispatch  (crates/vox-orchestrator-mcp/tests/daemon_extra_tests.rs)
- [happy] call() for dei_method::RESEARCH_RUN returns a fire-and-forget envelope with a positive session_id and non-failure status  (crates/vox-orchestrator-mcp/tests/daemon_extra_tests.rs)

### `parse_conflict_id_value()`  (happy; EXTRACTED)
- [happy] Parses numeric JSON value (9) to ConflictId(9)  (crates/vox-orchestrator-mcp/src/vcs_tools/parse.rs)
- [happy] Parses C-prefixed string ("C-000009") to ConflictId(9)  (crates/vox-orchestrator-mcp/src/vcs_tools/parse.rs)

### `parse_fix()`  (error, happy; EXTRACTED)
- [happy] Extracts label, replacement, and range fields from JSON and maps range.start/end to line/column tuples  (crates/vox-orchestrator-mcp/src/code_validator.rs)
- [error] Returns None when required JSON fields like replacement or range are missing  (crates/vox-orchestrator-mcp/src/code_validator.rs)

### `parse_operation_id_value()`  (happy; EXTRACTED)
- [happy] Parses numeric JSON value (7) to OperationId(7)  (crates/vox-orchestrator-mcp/src/vcs_tools/parse.rs)
- [happy] Parses OP-prefixed string ("OP-000007") to OperationId(7)  (crates/vox-orchestrator-mcp/src/vcs_tools/parse.rs)

### `post_vcs_isolation_strategy()`  (happy; EXTRACTED)
- [happy] Returns response with strategy_default in data field  (crates/vox-orchestrator-mcp/src/http_gateway/mod.rs)
- [happy] Returns response with per_agent strategy mapping keyed by agent ID  (crates/vox-orchestrator-mcp/src/http_gateway/mod.rs)

### `semantic_fs_discover()`  (happy; EXTRACTED)
- [happy] Discovers and ranks file paths by matching intent tokens against path names  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)
- [happy] semantic_fs_discover() returns results containing paths matching intent tokens from the query string  (crates/vox-orchestrator-mcp/src/memory_tools/tests.rs)

### `task_submit_signals()`  (happy; EXTRACTED)
- [happy] Sets channel field to InterruptionChannel::TaskSubmit for any task priority  (crates/vox-orchestrator-mcp/src/attention_policy.rs)
- [happy] Marks task as irreversible_or_high_risk=true when submitted  (crates/vox-orchestrator-mcp/src/attention_policy.rs)

### `tool_name_for_aci()`  (happy; EXTRACTED)
- [happy] tool_name_for_aci(vox_git_status) returns vox_git_status as canonical name  (crates/vox-orchestrator-mcp/src/aci/normalization.rs)
- [happy] tool_name_for_aci() returns the tool name unchanged for canonical names like 'vox_git_status'  (crates/vox-orchestrator-mcp/src/aci/normalization.rs)

### `ws_handle_message()`  (error, happy; EXTRACTED)
- [happy] ws_handle_message() filters returned tools to the allowed_tools set for list_tools requests  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)
- [error] ws_handle_message() returns is_error=true with error='missing tool name' when call_tool message lacks a name field  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `BranchName::parse()`  (error; EXTRACTED)
- [error] Returns error BranchNameError::IllegalChar when branch name contains space character  (crates/vox-orchestrator-mcp/src/vcs_tools/branch_tools.rs)

### `DashboardToken::generate_or_load()`  (happy; EXTRACTED)
- [happy] DashboardToken::generate_or_load generates a 43-character token and loads the same token on subsequent calls  (crates/vox-orchestrator-mcp/src/http_gateway/token.rs)

### `GitExecError`  (error; EXTRACTED)
- [error] GitExec::run rejects banned commands by returning GitExecError::Banned variant without process spawning  (crates/vox-orchestrator-mcp/src/git_exec.rs)

### `PendingApprovals::cancel()`  (happy; EXTRACTED)
- [happy] cancel() removes a pending approval entry from the list  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)

### `PendingApprovals::list()`  (happy; EXTRACTED)
- [happy] list() returns all registered pending approvals with their metadata  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)

### `PendingApprovals::register()`  (happy; EXTRACTED)
- [happy] register() creates a pending approval entry and returns an approval_id and receiver channel  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)

### `ResearchMetric`  (happy; EXTRACTED)
- [happy] ResearchMetric events include model, tool, prompt_tokens, and trace_id in their metadata_json  (crates/vox-orchestrator-mcp/tests/telemetry_cache_miss.rs)

### `SelectionAxes`  (happy; EXTRACTED)
- [happy] Economy cost preference maps to COST_FIRST SelectionAxes in model selection requests.  (crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs)

### `ServerState::new_full().with_db_initialized()`  (happy; EXTRACTED)
- [happy] with_db_initialized() attaches a VoxDb to ServerState enabling session persistence  (crates/vox-orchestrator-mcp/tests/daemon_extra_tests.rs)

### `ServerState::test_stub()`  (happy; EXTRACTED)
- [happy] ServerState::test_stub() creates a valid test server state given orchestrator config, repository context, and related components  (crates/vox-orchestrator-mcp/src/workspace_path.rs)

### `SessionManager::new()`  (happy; EXTRACTED)
- [happy] SessionManager can be created with a SessionConfig and produces a non-panicking instance  (crates/vox-orchestrator-mcp/src/workspace_path.rs)

### `TOOL_REGISTRY`  (invariant; EXTRACTED)
- [invariant] Each TOOL_REGISTRY entry has a dispatch match arm in source code  (crates/vox-orchestrator-mcp/src/dispatch.rs)

### `VoxDb::get_research_session()`  (happy; EXTRACTED)
- [happy] get_research_session() retrieves a session row from the database after research.run enqueues it  (crates/vox-orchestrator-mcp/tests/daemon_extra_tests.rs)

### `bearer_for()`  (error; EXTRACTED)
- [error] bearer_for() rejects PopuliMesh provider type by returning error containing 'not applicable to provider PopuliMesh'  (crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs)

### `code_validator::validate_file()`  (happy; EXTRACTED)
- [happy] validate_file() accepts a repo-relative path and returns a JSON response with success=true  (crates/vox-orchestrator-mcp/src/workspace_path.rs)

### `decision_label()`  (invariant; EXTRACTED)
- [invariant] Maps InterruptionDecision::ProceedAutonomously to stable label 'ProceedAutonomously'  (crates/vox-orchestrator-mcp/src/attention_policy.rs)

### `dispatch()`  (invariant; EXTRACTED)
- [invariant] Every tool in TOOL_REGISTRY has a corresponding match arm in dispatch.rs source  (crates/vox-orchestrator-mcp/src/dispatch.rs)

### `endpoint_for()`  (error; EXTRACTED)
- [error] endpoint_for() rejects PopuliMesh provider type by returning error containing 'not applicable to provider PopuliMesh'  (crates/vox-orchestrator-mcp/src/llm_bridge/provider_endpoints.rs)

### `enforce_rate_limit()`  (happy; EXTRACTED)
- [happy] enforce_rate_limit() allows calls within configured budget but rejects calls exceeding the limit  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `get_vcs_isolation()`  (happy; EXTRACTED)
- [happy] Returns JSON envelope with 'v' version field  (crates/vox-orchestrator-mcp/src/http_gateway/mod.rs)

### `http_info()`  (happy; EXTRACTED)
- [happy] http_info() endpoint exposes read_role_allowed_tools array filtered to exclude write_only tools when called with read-role bearer token  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `normalize_route()`  (happy; EXTRACTED)
- [happy] None input defaults to A2ADeliveryPlane::LocalEphemeral  (crates/vox-orchestrator-mcp/src/a2a_tools.rs)

### `orch_daemon::serve_listener_with_extra()`  (happy; EXTRACTED)
- [happy] serve_listener_with_extra() creates a daemon server that dispatches orch.tool_call method calls through an ExtraDispatch handler  (crates/vox-orchestrator-mcp/tests/daemon_extra_tests.rs)

### `plan schema`  (happy; EXTRACTED)
- [happy] Plan schema with empty tasks array is valid JSON and deserializes correctly with summary field preserved  (crates/vox-orchestrator-mcp/src/chat_tools/mod.rs)

### `questioning_policy_metric_payload()`  (happy; EXTRACTED)
- [happy] Generates metric payload that persists normalized decision via VoxDb::record_questioning_metric  (crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs)

### `read_only()`  (happy; EXTRACTED)
- [happy] read_only tools (vox_git_status) are classified with mutation_kind='read_only' by attach_aci_envelope()  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)

### `record_guardrail_deny_best_effort()`  (happy; EXTRACTED)
- [happy] GuardrailDenyDetail metrics persist to in-memory VoxDb and can be retrieved with correct metadata  (crates/vox-orchestrator-mcp/src/agentos_telemetry.rs)

### `request_identity()`  (happy; EXTRACTED)
- [happy] request_identity() extracts the first IP address from x-forwarded-for header when trust_forwarded_for is enabled  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `resolve_access_role()`  (happy; EXTRACTED)
- [happy] resolve_access_role correctly identifies read-token as AccessRole::Read  (crates/vox-orchestrator-mcp/src/http_gateway_tests.rs)

### `tool_json_envelope_is_error()`  (happy; EXTRACTED)
- [happy] tool_json_envelope_is_error() correctly identifies rejection envelopes as errors  (crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs)

### `validate_source()`  (happy; EXTRACTED)
- [happy] Returns success=true with count > 0 diagnostics when source code is syntactically invalid  (crates/vox-orchestrator-mcp/src/code_validator.rs)

### `vox_write_file`  (happy; EXTRACTED)
- [happy] vox_write_file tool is classified with mutation_kind='local_mutation' and produces 'workspace_state_change' side effect  (crates/vox-orchestrator-mcp/src/aci/envelope.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`ChatHistoryParams`** — only: _Empty JSON object deserializes with default session_id of 'default'_
- **`DashboardToken::generate_or_load()`** — only: _DashboardToken::generate_or_load generates a 43-character token and loads the same token on subsequent calls_
- **`PendingApprovals::cancel()`** — only: _cancel() removes a pending approval entry from the list_
- **`PendingApprovals::list()`** — only: _list() returns all registered pending approvals with their metadata_
- **`PendingApprovals::register()`** — only: _register() creates a pending approval entry and returns an approval_id and receiver channel_
- **`ResearchMetric`** — only: _ResearchMetric events include model, tool, prompt_tokens, and trace_id in their metadata_json_
- **`SelectionAxes`** — only: _Economy cost preference maps to COST_FIRST SelectionAxes in model selection requests._
- **`ServerState::new_full().with_db_initialized()`** — only: _with_db_initialized() attaches a VoxDb to ServerState enabling session persistence_
- **`ServerState::test_stub()`** — only: _ServerState::test_stub() creates a valid test server state given orchestrator config, repository context, and related components_
- **`SessionManager::new()`** — only: _SessionManager can be created with a SessionConfig and produces a non-panicking instance_
- **`SocratesJsonMeta`** — only: _SocratesJsonMeta can deserialize from socrates_tool_meta() output_
- **`VoxDb::get_research_session()`** — only: _get_research_session() retrieves a session row from the database after research.run enqueues it_
- **`_parse_snapshot_id_value()`** — only: _Parses numeric JSON value (3) to SnapshotId(3)_
- **`attach_aci_envelope()`** — only: _attach_aci_envelope() adds aci envelope with tool name, mutation_kind read_only, and empty side_effects for vox_git_status_
- **`build_app()`** — only: _Routes /api/v2/health endpoint to 200 OK status_
- **`build_commit_message()`** — only: _Includes Co-authored-by trailer with provided author name and email_
- **`build_selection_request()`** — only: _Economy cost preference maps to COST_FIRST selection axes_
- **`code_validator::validate_file()`** — only: _validate_file() accepts a repo-relative path and returns a JSON response with success=true_
- **`conflict_diff()`** — only: _Returns JSON response with success field set to true_
- **`enforce_rate_limit()`** — only: _enforce_rate_limit() allows calls within configured budget but rejects calls exceeding the limit_
- **`enqueue_hints_from_submit_params()`** — only: _Returns None when no task_category or campaign signals are present in SubmitTaskParams_
- **`extra_headers_for()`** — only: _extra_headers_for() injects HTTP-Referer header with value from VOX_OPENROUTER_HTTP_REFERER env var_
- **`get_vcs_isolation()`** — only: _Returns JSON envelope with 'v' version field_
- **`handle_tool_call()`** — only: _Calling handle_tool_call with TOOL_REGISTRY entry names does not return 'Unknown tool' error_
- **`http_info()`** — only: _http_info() endpoint exposes read_role_allowed_tools array filtered to exclude write_only tools when called with read-role bearer token_
- **`http_tools()`** — only: _http_tools() endpoint with read-role bearer token hides write_only tools from the response while exposing read-eligible tools_
- **`mcp_global_llm_context_fill_ratio()`** — only: _Returns None when orchestrator has no context budget configured_
- **`memory_config_for_state()`** — only: _Extracts log_dir and memory_md_path from ServerState's OrchestratorConfig.memory settings_
- **`normalize_inbox_source()`** — only: _None, empty string, and whitespace-only inputs all default to A2AInboxPlane::Merged_
- **`normalize_route()`** — only: _None input defaults to A2ADeliveryPlane::LocalEphemeral_
- **`orch_daemon::OrchDaemonClient::call()`** — only: _OrchDaemonClient.call() returns a non-failure success envelope for readonly tool dispatch_
- **`orch_daemon::serve_listener_with_extra()`** — only: _serve_listener_with_extra() creates a daemon server that dispatches orch.tool_call method calls through an ExtraDispatch handler_
- **`parse_campaign_from_description()`** — only: _Extracts campaign_id and ReconstructionBenchmarkTier from [campaign:X] and [tier:Y] bracketed tags_
- **`parse_conflict_id_value()`** — only: _Parses numeric JSON value (9) to ConflictId(9)_
- **`parse_operation_id_value()`** — only: _Parses numeric JSON value (7) to OperationId(7)_
- **`plan schema`** — only: _Plan schema with empty tasks array is valid JSON and deserializes correctly with summary field preserved_
- **`plan_result_blocked_by_adequacy_enforce()`** — only: _Returns true when adequacy enforce is enabled and plan is too thin_
- **`post_vcs_isolation_strategy()`** — only: _Returns response with strategy_default in data field_
- **`questioning_policy_metric_payload()`** — only: _Generates metric payload that persists normalized decision via VoxDb::record_questioning_metric_
- **`read_only()`** — only: _read_only tools (vox_git_status) are classified with mutation_kind='read_only' by attach_aci_envelope()_
- **`record_guardrail_deny_best_effort()`** — only: _GuardrailDenyDetail metrics persist to in-memory VoxDb and can be retrieved with correct metadata_
- **`request_identity()`** — only: _request_identity() extracts the first IP address from x-forwarded-for header when trust_forwarded_for is enabled_
- **`resolve_access_role()`** — only: _resolve_access_role correctly identifies read-token as AccessRole::Read_
- **`route_backend_for_chat_route()`** — only: _route_backend_for_chat_route() for ManualOpenAiCompatible Gemini route matches chat_lane_for_orchestrator_backend() of route_backend_for_model() for the same spec._
- **`route_policy_allows_model()`** — only: _route_policy_allows_model() returns true for OpenRouter cloud models when VOX_ROUTE_ALLOW_NET and VOX_ROUTE_ALLOW_PROVIDER_NETWORK environment variables are set to 1_
- **`route_telemetry_labels()`** — only: _OpenRouter ChatProviderRouteKind produces telemetry labels that match mcp_provider_telemetry_labels() for ProviderType::OpenRouter._
- **`run_retrieval_bundle()`** — only: _Returns evidence with used_bm25=true, used_lexical_fallback=false, and retrieval_tier='bm25'_
- **`semantic_fs_discover()`** — only: _Discovers and ranks file paths by matching intent tokens against path names_
- **`should_forward()`** — only: _should_forward returns false when topic is not in subscription set, true when topic is present, false for different topics_
- **`socrates_tool_meta()`** — only: _socrates_tool_meta() preserves confidence_estimate parameter value_
- **`task_submit_signals()`** — only: _Sets channel field to InterruptionChannel::TaskSubmit for any task priority_
- **`tool_json_envelope_is_error()`** — only: _tool_json_envelope_is_error() correctly identifies rejection envelopes as errors_
- **`tool_name_for_aci()`** — only: _tool_name_for_aci(vox_git_status) returns vox_git_status as canonical name_
- **`validate_source()`** — only: _Returns success=true with count > 0 diagnostics when source code is syntactically invalid_
- **`vox_write_file`** — only: _vox_write_file tool is classified with mutation_kind='local_mutation' and produces 'workspace_state_change' side effect_
