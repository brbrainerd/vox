# Semantic Behavior Map — `vox-ml-cli`

Deterministically synthesized from 94 distinct proven-behavior claims (of 94 extracted) across 26 symbols. 12 symbols have an explicit error-path proof; **10 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `check_run()`  (edge, error, happy, invariant; EXTRACTED)
- [happy] check_run() returns gate results where per_context[target] passes when parse_rate >= 0.80 threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run() returns gate results where per_context[meta] passes when parse_rate >= 0.30 threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] check_run() returns gate results where per_context[target] fails when parse_rate < 0.80 threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [invariant] check_run() gates marked with block:true in policy have block=true in results  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run() returns modal_mix[voice] gate that passes when voice fraction < max_voice_fraction  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] check_run() returns modal_mix[voice] gate that fails when voice fraction > max_voice_fraction  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [invariant] check_run() gates with block:false in policy do not block even when gate fails  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run() returns mcp_tool_schema gate that passes when strict_validity_rate >= min threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] check_run() returns mcp_tool_schema gate that fails when strict_validity_rate < min threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [edge] check_run() skips mcp_tool_schema gate when min_strict_validity_rate is 0.0  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run() returns pass_at_k gate that passes when pass_rate_at_1 >= min threshold and pass_rate_at_k >= min threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run() returns anti_stub gate that passes when anti_stub_task_success >= min threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- … +6 more claims

### `EvalGatePolicy`  (happy; EXTRACTED)
- [happy] EvalGatePolicy deserializes YAML with per_context key containing target and meta context configs  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy per_context[target].min_parse_rate deserializes to 0.80 when specified  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy per_context[target].block deserializes to true when specified  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy per_context[meta].block deserializes to false when specified  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy modal_mix.max_voice_fraction deserializes to 0.30 when specified  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy modal_mix.block deserializes to false when specified  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy deserializes per_context key-value mapping from YAML with target and meta contexts  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy deserializes min_parse_rate field as f64 (0.80 for target context)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy deserializes block flag as boolean with per-context override (target=true, meta=false)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] EvalGatePolicy deserializes modal_mix configuration including max_voice_fraction and block flag  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `check_run`  (edge, happy; EXTRACTED)
- [happy] check_run returns a list of gate results when given a policy with per_context configuration  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run marks gate as blocking when policy sets block: true  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run returns gate result with matching name 'modal_mix[voice]'  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run marks warn-only gate with block: false when policy specifies non-blocking gate  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run reads mcp_tool_schema_kpi.json and validates strict_validity_rate (0.99) against min_strict_validity_rate threshold  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [edge] check_run skips mcp_tool_schema gate when min_strict_validity_rate is 0.0 (inactive)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run reads benchmark_passatk.json and validates pass_rate_at_1 and pass_rate_at_k against policy thresholds  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] check_run reads eval_local_report.json and validates anti_stub_task_success, placeholder_event_rate, trivial_placeholder_event_rate, and construct_richness_mean fields  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `anti_stub`  (error, happy; EXTRACTED)
- [happy] anti_stub gate passes when all metrics (pass_rate=0.95, placeholder_event_rate=0.03, construct_richness_mean=0.45) meet or exceed policy thresholds  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] anti_stub gate respects blocking flag from policy (block: true)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] anti_stub gate fails when anti_stub_task_success (0.80) falls below min_pass_rate threshold (0.92)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] anti_stub gate is blocking (block: true from policy) when pass_rate check fails  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] anti_stub gate fails when eval_local_report.json file is missing  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] anti_stub gate is non-blocking (warning only) when eval_local_report.json is missing and policy sets block: false  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `parse_invite_url()`  (error, happy; EXTRACTED)
- [happy] parse_invite_url() decodes vox-mesh://invite URL with percent-encoded manifest parameter and label with space decoding  (crates/vox-ml-cli/src/commands/populi_join.rs)
- [happy] parse_invite_url() accepts plain https:// URLs as direct manifest URLs  (crates/vox-ml-cli/src/commands/populi_join.rs)
- [error] parse_invite_url() rejects invalid URL schemes like ftp://  (crates/vox-ml-cli/src/commands/populi_join.rs)
- [error] parse_invite_url() rejects vox-mesh://invite URLs missing the manifest query parameter  (crates/vox-ml-cli/src/commands/populi_join.rs)
- [happy] parse_invite_url() accepts HTTPS URLs and returns an Invite with manifest_url set to the input string  (crates/vox-ml-cli/src/commands/populi_join.rs)
- [error] parse_invite_url() returns an error for unsupported URL schemes like ftp://  (crates/vox-ml-cli/src/commands/populi_join.rs)

### `resolve_workflow_run_id()`  (error, happy; EXTRACTED)
- [happy] resolve_workflow_run_id() returns the explicit run_id when provided  (crates/vox-ml-cli/src/commands/ai/workflow.rs)
- [happy] resolve_workflow_run_id() generates unique IDs prefixed with 'wf-<workflow_name>-' when not provided  (crates/vox-ml-cli/src/commands/ai/workflow.rs)
- [error] resolve_workflow_run_id() rejects blank/whitespace-only run IDs with error containing 'must not be empty'  (crates/vox-ml-cli/src/commands/ai/workflow.rs)
- [happy] resolve_workflow_run_id returns the provided explicit run_id unchanged when Some value is supplied  (crates/vox-ml-cli/src/commands/ai/workflow.rs)
- [happy] resolve_workflow_run_id generates unique run IDs with 'wf-' prefix matching the workflow name when no explicit run_id is provided  (crates/vox-ml-cli/src/commands/ai/workflow.rs)
- [error] resolve_workflow_run_id returns error containing 'must not be empty' when whitespace-only run_id is provided  (crates/vox-ml-cli/src/commands/ai/workflow.rs)

### `run_eval_gate()`  (error, happy; EXTRACTED)
- [happy] run_eval_gate() returns exit code 0 when gates pass  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] run_eval_gate() writes gate_receipt.json with schema vox_mens_gate_receipt_v1  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] run_eval_gate() writes gate_receipt.json with overall_passed=true when all gates pass  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [happy] run_eval_gate() writes gate_receipt.json with non-empty gates array  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] run_eval_gate() returns exit code 1 when gates fail  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] run_eval_gate() writes gate_receipt.json with overall_passed=false when gates fail  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `run_merge_qlora`  (error, happy; EXTRACTED)
- [error] run_merge_qlora rejects Burn binary adapter format (.bin) that is not safetensors  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)
- [error] run_merge_qlora error message mentions Candle safetensors hint and Burn retired notice when rejecting non-safetensors adapter  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)
- [happy] run_merge_qlora successfully merges LoRA adapter (with F32 dtype, rank 2, alpha 4) into base model and produces correct merged weight values with delta computation  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)
- [happy] run_merge_qlora handles v3 adapter manifest format with additional fields (adapter_method, base_quant, double_quant, provenance) and correctly merges weights  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)

### `parse_device()`  (error, happy; EXTRACTED)
- [happy] parse_device() maps case-insensitive device preference strings to DevicePref enum variants  (crates/vox-ml-cli/src/commands/quantize.rs)
- [happy] parse_device maps valid device preference strings ('auto', 'cuda', 'cpu') to corresponding DevicePref enum variants  (crates/vox-ml-cli/src/commands/quantize.rs)
- [error] parse_device rejects invalid device string ('gpu') with Err  (crates/vox-ml-cli/src/commands/quantize.rs)

### `parse_mixture()`  (error, happy; EXTRACTED)
- [happy] parse_mixture() maps case-insensitive quantization mixture strings to QuantMixture enum variants  (crates/vox-ml-cli/src/commands/quantize.rs)
- [happy] parse_mixture maps valid quantization mixture strings ('q4_k_m', 'Q8_0') to corresponding QuantMixture enum variants  (crates/vox-ml-cli/src/commands/quantize.rs)
- [error] parse_mixture rejects invalid quantization mixture strings ('bogus') with Err  (crates/vox-ml-cli/src/commands/quantize.rs)

### `prompt_for_output_mode()`  (happy; EXTRACTED)
- [happy] prompt_for_output_mode wraps prompt with strict_json format instructions including 'single valid JSON object' and 'No markdown fences' when mode is 'strict_json'  (crates/vox-ml-cli/src/commands/ai/serve/mod.rs)
- [happy] prompt_for_output_mode returns prompt unchanged when output mode is None  (crates/vox-ml-cli/src/commands/ai/serve/mod.rs)
- [happy] prompt_for_output_mode returns prompt unchanged when output mode is empty string  (crates/vox-ml-cli/src/commands/ai/serve/mod.rs)

### `mcp_tool_schema`  (error, happy; EXTRACTED)
- [happy] mcp_tool_schema gate passes when strict_validity_rate (0.99) equals min_strict_validity_rate threshold (0.99)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] mcp_tool_schema gate fails when strict_validity_rate (0.5) falls below min_strict_validity_rate threshold (0.99)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `modal_mix[voice]`  (error, happy; EXTRACTED)
- [happy] modal_mix gate passes when voice fraction (5.3%) is below max_voice_fraction ceiling (0.30)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] modal_mix gate fails when voice fraction (40%) exceeds max_voice_fraction ceiling (0.30)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `per_context[target]`  (error, happy; EXTRACTED)
- [happy] per_context gate passes when parse_rate (0.90) exceeds min_parse_rate threshold (0.80) and scope_compliance_rate meets min requirement (0.97 >= 0.95)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)
- [error] per_context gate fails when parse_rate (0.50) falls below min_parse_rate threshold (0.80)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `prepare_bench_item()`  (edge, happy; EXTRACTED)
- [happy] prepare_bench_item includes both context files and primary template file in the generated prompt with proper headers and content  (crates/vox-ml-cli/src/commands/mens/eval_local_prompt.rs)
- [edge] prepare_bench_item omits the primary file block from the prompt when the primary file does not exist  (crates/vox-ml-cli/src/commands/mens/eval_local_prompt.rs)

### `run_eval_local`  (error; EXTRACTED)
- [error] run_eval_local returns error when model file does not exist  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)
- [error] run_eval_local error message contains 'not found' or 'Model' hint when model file is missing  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)

### `run_status`  (edge; EXTRACTED)
- [edge] run_status does not error when directory does not exist  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)
- [edge] run_status succeeds with json flag on nonexistent directory without error  (crates/vox-ml-cli/src/commands/mens/populi/gpu_tests_body.rs)

### `Invite.invite_sig_b64`  (happy; EXTRACTED)
- [happy] parse_invite_url() sets invite_sig_b64 to None when parsing a direct HTTPS URL  (crates/vox-ml-cli/src/commands/populi_join.rs)

### `compression_ratio`  (happy; EXTRACTED)
- [happy] quantize report compression_ratio exceeds 1.5 when quantizing test weights  (crates/vox-ml-cli/tests/merge_quantize.rs)

### `pass_at_k`  (happy; EXTRACTED)
- [happy] pass_at_k gate passes when pass_rate_at_1 (0.71) >= min_pass_rate_at_1 (0.70) and pass_rate_at_k (0.88) >= min_pass_rate_at_k (0.85)  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `per_context[meta]`  (happy; EXTRACTED)
- [happy] per_context gate passes when parse_rate (0.35) exceeds min_parse_rate threshold (0.30) despite no scope_compliance_rate requirement for meta context  (crates/vox-ml-cli/src/commands/mens/eval_gate/tests.rs)

### `quantize()`  (happy; EXTRACTED)
- [happy] quantize() produces a quant-metadata.json artifact in the output directory  (crates/vox-ml-cli/tests/merge_quantize.rs)

### `resolve_plan_context_from_env()`  (happy; EXTRACTED)
- [happy] resolve_plan_context_from_env reads VOX_PLAN_SESSION_ID, VOX_PLAN_NODE_ID, and VOX_PLAN_VERSION environment variables and parses them into a tuple of correct types  (crates/vox-ml-cli/src/commands/ai/workflow.rs)

### `run()`  (happy; EXTRACTED)
- [happy] run() quantize command produces quant-metadata.json artifact  (crates/vox-ml-cli/tests/quantize_cli.rs)

### `run_status()`  (edge; EXTRACTED)
- [edge] run_status returns Ok result instead of panicking when given a nonexistent directory path  (crates/vox-ml-cli/src/commands/mens/status.rs)

### `src()`  (happy; EXTRACTED)
- [happy] golden Vox example files parse without panicking  (crates/vox-ml-cli/src/commands/mens/eval_local_prompt.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`EvalGatePolicy`** — only: _EvalGatePolicy deserializes YAML with per_context key containing target and meta context configs_
- **`Invite.invite_sig_b64`** — only: _parse_invite_url() sets invite_sig_b64 to None when parsing a direct HTTPS URL_
- **`compression_ratio`** — only: _quantize report compression_ratio exceeds 1.5 when quantizing test weights_
- **`pass_at_k`** — only: _pass_at_k gate passes when pass_rate_at_1 (0.71) >= min_pass_rate_at_1 (0.70) and pass_rate_at_k (0.88) >= min_pass_rate_at_k (0.85)_
- **`per_context[meta]`** — only: _per_context gate passes when parse_rate (0.35) exceeds min_parse_rate threshold (0.30) despite no scope_compliance_rate requirement for meta context_
- **`prompt_for_output_mode()`** — only: _prompt_for_output_mode wraps prompt with strict_json format instructions including 'single valid JSON object' and 'No markdown fences' when mode is 'strict_json'_
- **`quantize()`** — only: _quantize() produces a quant-metadata.json artifact in the output directory_
- **`resolve_plan_context_from_env()`** — only: _resolve_plan_context_from_env reads VOX_PLAN_SESSION_ID, VOX_PLAN_NODE_ID, and VOX_PLAN_VERSION environment variables and parses them into a tuple of correct types_
- **`run()`** — only: _run() quantize command produces quant-metadata.json artifact_
- **`src()`** — only: _golden Vox example files parse without panicking_
