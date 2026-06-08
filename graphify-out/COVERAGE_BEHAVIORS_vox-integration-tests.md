# Semantic Behavior Map — `vox-integration-tests`

Deterministically synthesized from 283 distinct proven-behavior claims (of 284 extracted) across 200 symbols. 24 symbols have an explicit error-path proof; **137 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `generate()`  (happy; EXTRACTED)
- [happy] Code generation for chatbot produces types.ts, vox-app-contract.json, vox-tanstack-query.tsx, and Chat.tsx but not server.ts  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Generated types.ts file for chatbot contains a snapshot-matching tagged union structure  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Generated Chat.tsx component file contains snapshot-matching useState hook usage  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Code generation does not emit server.ts file even when VOX_EMIT_EXPRESS_SERVER environment variable is set  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] JSX text content like 'Vox', 'Chatbot', and 'Send' in generated Chat.tsx appears as plain text without braces {}  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Code generation for activity construct produces activities.ts file (tombstoned test)  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Generated activities.ts file contains snapshot-matching async function for activity (tombstoned test)  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Generated activities.ts file contains snapshot-matching runtime helper functions for activity execution (tombstoned test)  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Code generation for @table type produces schema.ts file with snapshot-matching content  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Codegen produces a types.ts file from generics Option/Result definitions with snapshot validation  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)
- [happy] Codegen produces HooksDemo.tsx file with snapshot validation for React hooks  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)
- [happy] Codegen for mixed surface produces Dash.tsx component that references its own name or class  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)
- … +7 more claims

### `typecheck_module`  (error, happy; EXTRACTED)
- [happy] ADT type definitions and function definitions typecheck without errors  (crates/vox-integration-tests/tests/chatbot_integration_test.rs)
- [error] Using emit outside of a stream context produces a typecheck error diagnostic containing 'emit'  (crates/vox-integration-tests/tests/stream_emit_test.rs)
- [error] Using an undefined variable 'xyz' in a function produces an error diagnostic containing 'Undefined variable: xyz'  (crates/vox-integration-tests/tests/typeck_test.rs)
- [happy] A function with a named parameter can use that parameter without errors  (crates/vox-integration-tests/tests/typeck_test.rs)
- [happy] A variable defined with 'let' can be used in subsequent expressions without errors  (crates/vox-integration-tests/tests/typeck_test.rs)
- [error] Assigning to an immutable variable produces an error diagnostic containing 'Cannot assign to immutable variable'  (crates/vox-integration-tests/tests/typeck_test.rs)
- [happy] A variable declared with 'let mut' can be reassigned without errors  (crates/vox-integration-tests/tests/typeck_test.rs)
- [happy] A match expression that covers all variants of an enum type produces no errors  (crates/vox-integration-tests/tests/typeck_test.rs)
- [error] A match expression missing a variant produces an error diagnostic containing 'Non-exhaustive match' and the missing variant name  (crates/vox-integration-tests/tests/typeck_test.rs)
- [happy] reference example source passes typecheck with no error diagnostics  (crates/vox-integration-tests/tests/greaterfool_parity_gates_test.rs)

### `validate_web_ir`  (error, happy, invariant; EXTRACTED)
- [happy] Web IR validation passes clean with no diagnostics for Chatbot component module  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Web IR validation passes with no diagnostics for multi-route module  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Web IR validation passes clean for reactive component with state  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)
- [error] Web IR validation detects duplicate route contract IDs from multiple routes blocks with code 'web_ir_validate.route.duplicate_contract_id'  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)
- [invariant] All Web IR validation diagnostic codes use 'web_ir_validate.' prefix  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)
- [happy] validate_web_ir() returns empty diagnostics when lowering valid MIXED_SURFACE composition through lower_hir_to_web_ir()  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)
- [happy] validate_web_ir() returns empty diagnostics for well-formed MIXED_SURFACE with single routes block  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)
- [error] validate_web_ir() detects duplicate_contract_id error when multiple routes blocks define handlers for the same path  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `server_capabilities`  (happy, invariant; EXTRACTED)
- [happy] hover_provider capability is advertised in LSP server initialization  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)
- [happy] completion_provider capability is advertised in LSP server initialization  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)
- [happy] document_symbol_provider capability is advertised in LSP server initialization  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)
- [happy] code_lens_provider capability is advertised in LSP server initialization  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)
- [happy] code_action_provider capability is advertised in LSP server initialization  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)
- [happy] semantic_tokens_provider capability is advertised in LSP server initialization  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)
- [invariant] document_formatting_provider is NOT advertised (not implemented)  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)

### `lower_module HIR lowering`  (happy; EXTRACTED)
- [happy] Table declarations are correctly lowered from AST to HIR with field count preserved  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] Index declarations are correctly lowered from AST to HIR  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] @mcp.tool annotated functions are lowered to mcp_tools list, not functions list  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] MCP tool HIR preserves description, function name, and parameter count  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] Multiple @mcp.tool functions are all lowered to mcp_tools list  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] @mcp.resource declarations are lowered to mcp_resources list with URI preserved  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `parse()`  (happy; EXTRACTED)
- [happy] Parsing the chatbot example source produces exactly 5 declarations (import, type, component, server endpoint, mutation endpoint)  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)
- [happy] Parsing hooks demo with 5 imports + 1 component + 1 routes block produces exactly 7 module declarations  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)
- [happy] Parsing v0 components with 2 v0 decorators + 1 routes block produces exactly 3 module declarations  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)
- [happy] Parsing MCP tool definitions with 1 type + 2 mcp.tool functions produces exactly 3 module declarations  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Parsing pattern matching with 1 Shape ADT + 1 area function + 2 @test functions produces exactly 4 module declarations  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Parsing multi-route app with 1 import + 1 type + 3 components + 1 routes + 3 endpoint functions produces exactly 9 module declarations  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `lower_hir_to_web_ir_with_summary`  (happy; EXTRACTED)
- [happy] Web IR summary reports query function contracts >= 1 when module has query endpoints  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Web IR summary counts Path C components >= 1 when module contains reactive components  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Web IR summary reports zero classic_components_deferred for test fixture  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Chat Path C component appears in Web IR summary.components count  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] Web IR summary reports client_route_trees >= 1 when module has routes  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `vox_compiler::parser::parse`  (error, happy; EXTRACTED)
- [happy] Dashboard template with component and routes parses successfully without errors  (crates/vox-integration-tests/tests/cli_test.rs)
- [happy] API template with table and function definitions parses successfully without errors  (crates/vox-integration-tests/tests/cli_test.rs)
- [error] Tombstoned http keyword in parse input produces a parse error  (crates/vox-integration-tests/tests/cli_test.rs)
- [error] Tombstoned activity keyword produces a parse error  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [error] Tombstoned activity and workflow keywords both produce parse errors  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `generate codegen`  (happy; EXTRACTED)
- [happy] Rust codegen produces src/mcp_server.rs file when @mcp.tool declarations exist  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] Rust codegen does not produce mcp_server.rs file when no @mcp.tool/@mcp.resource declarations exist  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] Rust codegen produces mcp_server.rs when @mcp.resource declarations exist  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] Rust codegen produces Cargo.toml with [[bin]] mcp_server declaration when MCP server exists  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `parse`  (error, happy; EXTRACTED)
- [error] http bare-keyword routing syntax produces parse error  (crates/vox-integration-tests/tests/chatbot_integration_test.rs)
- [happy] actor keyword and handler declarations parse successfully  (crates/vox-integration-tests/tests/chatbot_integration_test.rs)
- [error] Parser rejects invalid Vox syntax and returns error list  (crates/vox-integration-tests/tests/cli_test.rs)
- [happy] Chatbot template source parses without errors  (crates/vox-integration-tests/tests/cli_test.rs)

### `vox_compiler::hir::lower_module`  (happy; EXTRACTED)
- [happy] Table type definition with fields is correctly lowered to HIR with exactly one table  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] Index definition is correctly lowered to HIR with exactly one index  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] MCP tool definition is lowered to HIR with exactly one tool in mcp_tools list and not in functions list  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [happy] Multiple MCP tool definitions are lowered to HIR with correct count of mcp_tools  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `MENS speech_trace schema validator`  (error, happy; EXTRACTED)
- [happy] A minimal training row with schema_version, session_id, refined_transcript, vox_code, correlation_id, compile_ok, and failure_category passes schema validation  (crates/vox-integration-tests/tests/speech_schema_parity_test.rs)
- [error] A row missing vox_code field fails schema validation  (crates/vox-integration-tests/tests/speech_schema_parity_test.rs)
- [happy] A row with diagnostics_snapshot field passes schema validation  (crates/vox-integration-tests/tests/speech_schema_parity_test.rs)

### `spawn_process mailbox loop`  (edge, happy; EXTRACTED)
- [happy] Emitted actor mailbox loop decodes Message envelopes, routes to handlers, and handler mutations persist in actor state  (crates/vox-integration-tests/tests/actor_dispatch_e2e_test.rs)
- [happy] Emitted actor mailbox loop handles Request envelopes, invokes handlers, and replies with serialized return value  (crates/vox-integration-tests/tests/actor_dispatch_e2e_test.rs)
- [edge] Unknown events in mailbox are dropped without panic and loop survives to process subsequent messages  (crates/vox-integration-tests/tests/actor_dispatch_e2e_test.rs)

### `vox init scaffold`  (happy; EXTRACTED)
- [happy] vox init scaffold creates main.vox, Cargo.toml, and .gitignore files  (crates/vox-integration-tests/tests/cli_test.rs)
- [happy] main.vox contains fn main function declaration  (crates/vox-integration-tests/tests/cli_test.rs)
- [happy] Cargo.toml contains package section  (crates/vox-integration-tests/tests/cli_test.rs)

### `Diagnostic.range.start.line`  (error, happy; EXTRACTED)
- [happy] Type mismatch error diagnostic points to line 3 or later (at/after the call site in main)  (crates/vox-integration-tests/tests/lsp_test.rs)
- [error] Parse error diagnostic appears on line 0 or 1 for incomplete input  (crates/vox-integration-tests/tests/lsp_test.rs)

### `Orchestrator.status().total_completed`  (happy; EXTRACTED)
- [happy] After draining and completing all tasks, total_completed equals 10  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)
- [happy] After submitting one task and draining, total_completed equals 1  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)

### `RetryPolicy::default().base_delay_ms`  (invariant; EXTRACTED)
- [invariant] Default RetryPolicy has base_delay_ms >= 100 to include backoff delay  (crates/vox-integration-tests/tests/parity_contracts_test.rs)
- [invariant] RetryPolicy default base_delay_ms is at least 100 for backoff delay  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `RetryPolicy::default().max_attempts`  (invariant; EXTRACTED)
- [invariant] Default RetryPolicy has max_attempts >= 3 to support multiple retry attempts  (crates/vox-integration-tests/tests/parity_contracts_test.rs)
- [invariant] RetryPolicy default max_attempts is at least 3 for multiple retries  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `attach_aci_envelope`  (happy; EXTRACTED)
- [happy] attach_aci_envelope wraps tool response JSON in ACI envelope that validates against JSON schema  (crates/vox-integration-tests/tests/agentos_aci_bench.rs)
- [happy] ACI envelope contains tool name and side_effects array fields  (crates/vox-integration-tests/tests/agentos_aci_bench.rs)

### `golden TypeScript fixtures`  (invariant; EXTRACTED)
- [invariant] node_modules directory exists in ts-noemit-scratch when running ts emit tests  (crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs)
- [invariant] At least one .vox file exists in examples/golden-ts/ directory  (crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs)

### `map_jsx_attr_name`  (happy; EXTRACTED)
- [happy] JSX attribute mapping returns 'className' for Vox 'class' attribute  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)
- [happy] JSX attribute mapping returns 'tabIndex' for Vox 'tab_index' attribute  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `validate_document`  (error, happy; EXTRACTED)
- [happy] validate_document returns type error diagnostics with severity ERROR for type mismatch in function arguments  (crates/vox-integration-tests/tests/lsp_test.rs)
- [error] validate_document returns ERROR severity diagnostics for incomplete function syntax  (crates/vox-integration-tests/tests/lsp_test.rs)

### `vox_a2a_inbox`  (happy; EXTRACTED)
- [happy] vox_a2a_inbox MCP tool returns JSON response with 'success': true and contains message payload  (crates/vox-integration-tests/tests/a2a_mcp_test.rs)
- [happy] Inbox is empty after acknowledging a message  (crates/vox-integration-tests/tests/a2a_mcp_test.rs)

### `vox_codegen::codegen_rust::generate`  (happy, invariant; EXTRACTED)
- [happy] MCP tool definition triggers generation of src/mcp_server.rs file  (crates/vox-integration-tests/tests/codegen_rust_test.rs)
- [invariant] Code generation without MCP tools does not produce src/mcp_server.rs file  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `@index on known table`  (happy; EXTRACTED)
- [happy] Index definition on an existing @table produces no typecheck errors  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `@index on missing table`  (error; EXTRACTED)
- [error] Index definition on a non-existent table produces an error containing 'unknown table'  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `@loading component lowering`  (edge; EXTRACTED)
- [edge] @loading fn component is dropped from HIR lowering (Path B removed); no Spinner.tsx emitted in codegen output  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `@server fn endpoint function presence`  (happy; EXTRACTED)
- [happy] multi-route fixture HIR contains endpoint function named get_stats  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `@table type declaration with str/bool/int fields`  (happy; EXTRACTED)
- [happy] Valid table type declarations with standard scalar fields produce no typecheck errors  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `@table/@index parsing`  (happy; EXTRACTED)
- [happy] parse() produces exactly 2 declarations when parsing @table with @index (one table + one index)  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `@v0 component codegen`  (edge; EXTRACTED)
- [edge] Test is ignored; @v0 components are dropped from HIR lowering (Path B removed), no TSX generated  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `ADR 012 interop policy section`  (invariant; EXTRACTED)
- [invariant] ADR 012 document contains 'Interop policy' section  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `ADR README doc cross-links`  (invariant; EXTRACTED)
- [invariant] ADR README.md contains references to '012-internal-web-ir-strategy.md' and 'internal-web-ir-implementation-blueprint.md'  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `ADT constructor resolution`  (happy; EXTRACTED)
- [happy] ADT constructors like Circle are resolvable in function body scope without undefined variable error  (crates/vox-integration-tests/tests/typeck_test.rs)

### `CSS file generation from codegen`  (happy; EXTRACTED)
- [happy] codegen produces files with .css extension  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `CSS import in Chat component`  (happy; EXTRACTED)
- [happy] Chat.tsx contains CSS import statement  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `CanaryPolicy deserialization`  (happy; EXTRACTED)
- [happy] The canary_policy.example.json file deserializes successfully into CanaryPolicy struct  (crates/vox-integration-tests/tests/speech_canary_test.rs)

### `Chat component file generation`  (happy; EXTRACTED)
- [happy] chatbot codegen produces Chat.tsx file  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `CompletionEngine::completions`  (happy; EXTRACTED)
- [happy] completion engine surfaces 'fn' keyword in completions at start of file  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)

### `Decl::McpTool`  (happy; EXTRACTED)
- [happy] First parsed McpTool declaration has description 'Search the knowledge base'  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `Diagnostic.message`  (error; EXTRACTED)
- [error] Parse error message contains Expected/Unexpected/expected keyword indicating syntax issue  (crates/vox-integration-tests/tests/lsp_test.rs)

### `HTTP loader contracts in Web IR`  (happy; EXTRACTED)
- [happy] multi-route module lowers to at least one HTTP loader contract or query function contract  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `KPI baseline schema validator`  (happy; EXTRACTED)
- [happy] A KPI snapshot with schema_version, captured_at_utc, wer, cer, compile_pass_at_1, and latency_ms_p95 passes schema validation  (crates/vox-integration-tests/tests/speech_schema_parity_test.rs)

### `KpiSnapshot validation and schema conformance`  (happy; EXTRACTED)
- [happy] When VOX_SPEECH_CANARY_KPI is set, the KPI snapshot file deserializes as valid JSON, matches kpi-baseline.schema.json, and deserializes into KpiSnapshot struct  (crates/vox-integration-tests/tests/speech_canary_test.rs)

### `MCP resource codegen`  (happy; EXTRACTED)
- [happy] @mcp.resource decorated function lowers to exactly 1 mcp_resources entry with correct URI; generates src/mcp_server.rs; Cargo.toml declares [[bin]] mcp_server binary  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `MCP server file generation`  (happy; EXTRACTED)
- [happy] Codegen with @mcp.tool produces src/mcp_server.rs in output files  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `MCP server multi-tool schema`  (happy; EXTRACTED)
- [happy] HIR module with 2 @mcp.tool declarations contains exactly 2 mcp_tools entries; emit_mcp_server produces schema output  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `MCP server non-generation`  (edge; EXTRACTED)
- [edge] Codegen without @mcp.tool or @mcp.resource decorators does NOT produce src/mcp_server.rs in output files  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `MCP tool HIR lowering`  (happy; EXTRACTED)
- [happy] @mcp.tool decorated function lowers to exactly 1 mcp_tools entry; does NOT appear in functions list; preserves description and parameter count  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `MCP tool list parameter schema`  (happy; EXTRACTED)
- [happy] @mcp.tool with list[str] parameter lowers to HIR and emit_mcp_server produces schema output containing list param handling  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `Orchestrator.context_store()`  (happy; EXTRACTED)
- [happy] context_store.set() and context_store.get() preserve key-value pairs across reads  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)

### `Orchestrator.status() task queue counts`  (happy; EXTRACTED)
- [happy] After submitting 10 tasks, total_queued + total_in_progress equals 10  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)

### `Orchestrator.status().total_queued + total_in_progress`  (happy; EXTRACTED)
- [happy] After submitting 10 tasks, total_queued + total_in_progress equals 10  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)

### `OrchestratorConfig memory settings`  (invariant; EXTRACTED)
- [invariant] build_repo_scoped_orchestrator and ServerState produce matching memory.log_dir and memory.memory_md_path  (crates/vox-integration-tests/tests/orchestrator_bootstrap_surface_parity_test.rs)

### `PIPELINE_SRC line count`  (invariant; EXTRACTED)
- [invariant] chatbot pipeline fixture contains at most 80 non-comment source lines  (crates/vox-integration-tests/tests/greaterfool_parity_gates_test.rs)

### `Playwright test execution`  (happy; EXTRACTED)
- [happy] Playwright test suite successfully captures route screenshots and a11y audit JSON reports from the built app  (crates/vox-integration-tests/tests/playwright_golden_route_test.rs)

### `ProcessContext yield_now preemption guard`  (invariant; EXTRACTED)
- [invariant] Emitted loop preemption guard (reduction count check + yield_now) allows scheduler to interleave concurrent tasks instead of starving  (crates/vox-integration-tests/tests/actor_gc_sandbox_test.rs)

### `React app scaffolding and npm installation`  (happy; EXTRACTED)
- [happy] scaffold_react_app() and pnpm install successfully configure and install a React application from generated code  (crates/vox-integration-tests/tests/playwright_golden_route_test.rs)

### `ServerState.orchestrator_config.memory.log_dir`  (invariant; EXTRACTED)
- [invariant] ServerState memory log_dir matches build_repo_scoped_orchestrator config log_dir  (crates/vox-integration-tests/tests/orchestrator_bootstrap_surface_parity_test.rs)

### `ServerState.orchestrator_config.memory.memory_md_path`  (invariant; EXTRACTED)
- [invariant] ServerState memory_md_path matches build_repo_scoped_orchestrator config memory_md_path  (crates/vox-integration-tests/tests/orchestrator_bootstrap_surface_parity_test.rs)

### `ServerState.repository.repository_id`  (invariant; EXTRACTED)
- [invariant] ServerState repository_id matches build_repo_scoped_orchestrator repository_id for same config  (crates/vox-integration-tests/tests/orchestrator_bootstrap_surface_parity_test.rs)

### `SpeakPanel.tsx and useVoxChat.ts`  (invariant; EXTRACTED)
- [invariant] Dashboard speak surface does not contain getUserMedia, MediaRecorder, vox_oratio, or vox_speech_to_code tokens; uses vox_chat_message instead  (crates/vox-integration-tests/tests/speech_pipeline_stage_probe_test.rs)

### `SymbolEngine::symbols`  (happy; EXTRACTED)
- [happy] symbol engine includes outline symbols for function definitions  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)

### `TypeScript scalar type mappings`  (invariant; EXTRACTED)
- [invariant] VoxScalar types map to stable TypeScript primitives: Int/Float to 'number', Str to 'string', Bool to 'boolean'  (crates/vox-integration-tests/tests/scalar_mapping_ssot_test.rs)

### `VAD near-silence detection`  (happy; EXTRACTED)
- [happy] Voice activity detection produces empty segments for near-silence audio (0.001 amplitude at 16kHz)  (crates/vox-integration-tests/tests/speech_pipeline_stage_probe_test.rs)

### `Vox.toml file`  (happy; EXTRACTED)
- [happy] vox_project_init creates Vox.toml at packages/nested_app/ when target_subdir is specified  (crates/vox-integration-tests/tests/mcp_project_init_test.rs)

### `Web IR validation on mixed surface`  (happy; EXTRACTED)
- [happy] mixed surface Web IR validates with no diagnostics  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `Web IR view_roots`  (happy; EXTRACTED)
- [happy] Chat component is present in Web IR view_roots collection  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `acoustic preprocessing`  (happy; EXTRACTED)
- [happy] Acoustic preprocessing without environment override returns original samples unchanged, reports mode='none', and does not skip due to budget  (crates/vox-integration-tests/tests/speech_pipeline_stage_probe_test.rs)

### `activity and workflow keywords (tombstoned)`  (error; EXTRACTED)
- [error] Tombstoned `activity` and `workflow` keywords each produce parse errors (test is ignored due to un-tombstoning)  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `activity and workflow keywords coexistence (ADR-041)`  (happy; EXTRACTED)
- [happy] activity and workflow keywords used together produce no E028 reservation error  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `activity and workflow keywords parsing`  (error; EXTRACTED)
- [error] Code using tombstoned 'activity' or 'workflow' keywords produces a parse error  (crates/vox-integration-tests/tests/workflow_recovery_test.rs)

### `activity keyword (tombstoned)`  (error; EXTRACTED)
- [error] Tombstoned `activity` keyword produces a parse error (test is ignored due to un-tombstoning)  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `activity keyword (tombstoned); canonical fn form`  (error; EXTRACTED)
- [error] Tombstoned `activity` keyword produces parse error; canonical form using `fn` is accepted and emits output  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `activity keyword stable status (ADR-041)`  (happy; EXTRACTED)
- [happy] activity keyword definition produces no E028 reservation error in frontend pipeline  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `activity_id alias mapping`  (happy; EXTRACTED)
- [happy] When a workflow activity is declared with an `id` option ("email-step-alias"), plan_workflow_activities() maps it to activity_id field with the specified value  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `actor handler type checking`  (happy; EXTRACTED)
- [happy] actor on receive handler with valid builtin call (print) typechecks cleanly without errors  (crates/vox-integration-tests/tests/typeck_test.rs)

### `actor handler undefined variable detection`  (error; EXTRACTED)
- [error] actor on receive handler with undefined variable produces diagnostic containing 'Undefined variable: unknown_var'  (crates/vox-integration-tests/tests/typeck_test.rs)

### `apply_context_budget provenance`  (happy; EXTRACTED)
- [happy] apply_context_budget produces provenance entry for each selected chunk  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `apply_context_budget provenance.truncated field`  (edge; EXTRACTED)
- [edge] apply_context_budget marks truncated field true when char budget is exceeded  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `apply_context_budget with max_chunks=2, max_chars=12`  (happy; EXTRACTED)
- [happy] apply_context_budget selects both chunks when under max_chunks constraint  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `apply_context_budget()`  (happy; EXTRACTED)
- [happy] apply_context_budget selects both chunks when count is below max_chunks  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `apply_context_budget() provenance`  (happy; EXTRACTED)
- [happy] apply_context_budget produces provenance entries for each selected chunk  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `apply_context_budget().provenance.truncated`  (happy; EXTRACTED)
- [happy] apply_context_budget marks provenance as truncated when character budget is exceeded  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `audit matrix CUDA compute cell presence`  (invariant; EXTRACTED)
- [invariant] audit matrix includes a SHOULD-tier CUDA compute cell for runtime decode  (crates/vox-integration-tests/tests/speech_audit_contract_test.rs)

### `audit matrix required cells coverage`  (invariant; EXTRACTED)
- [invariant] audit matrix contains required MUST-tier cells for editor-webview-mic and dashboard-speak-gap surfaces  (crates/vox-integration-tests/tests/speech_audit_contract_test.rs)

### `audit matrix schema validation`  (happy; EXTRACTED)
- [happy] audit-matrix.v1.yaml validates against audit-matrix.schema.json using jsonschema validator  (crates/vox-integration-tests/tests/speech_audit_contract_test.rs)

### `benchmark manifest Vox files`  (invariant; EXTRACTED)
- [invariant] All non-dash Vox files referenced in the benchmark manifest pass HIR validation with no ERROR-level diagnostics  (crates/vox-integration-tests/tests/speech_fixture_validate_test.rs)

### `benchmark-fixtures.manifest.txt line format`  (invariant; EXTRACTED)
- [invariant] Each non-empty manifest line contains exactly 5 tab-separated columns (audio, transcript, expected_vox_or_dash, domain, sample_rate_hz)  (crates/vox-integration-tests/tests/speech_benchmark_manifest_test.rs)

### `bind={} reactive binding codegen`  (happy; EXTRACTED)
- [happy] generate() expands bind=email syntax to value/onChange props in LoginForm.tsx output  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `build_repo_scoped_orchestrator and ServerState repository_id`  (invariant; EXTRACTED)
- [invariant] build_repo_scoped_orchestrator and ServerState::new_full produce identical repository_id for same working directory  (crates/vox-integration-tests/tests/orchestrator_bootstrap_surface_parity_test.rs)

### `builtin_hover_markdown`  (happy; EXTRACTED)
- [happy] builtin_hover_markdown returns Some for 'print' builtin function  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)

### `canary.kpi.json`  (happy; EXTRACTED)
- [happy] Committed KPI baseline JSON validates against kpi-baseline.schema.json and includes schema_version=1, WER, and CER fields  (crates/vox-integration-tests/tests/speech_audit_contract_test.rs)

### `canonicalize_prompt hash and structure preservation`  (happy; EXTRACTED)
- [happy] canonicalize_prompt() preserves original_hash and reconstructs prompt text structure with canonical sections  (crates/vox-integration-tests/tests/prompt_canonical_guardrails_test.rs)

### `classic component views in mixed surface`  (invariant; EXTRACTED)
- [invariant] mixed surface lowers zero classic component views  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `client route tree lowering`  (happy; EXTRACTED)
- [happy] multi-route module lowers to at least one client route tree in Web IR  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `code_lenses_for_module`  (happy; EXTRACTED)
- [happy] code lens engine emits vox.runTest command for @test-annotated functions  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)

### `codegen (CSS module import)`  (happy; EXTRACTED)
- [happy] Chatbot fixture generates Chat.tsx with Chat.css import statement  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `codegen (CSS)`  (happy; EXTRACTED)
- [happy] Component with style block emits CSS file containing color properties  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `codegen (TypeScript)`  (happy; EXTRACTED)
- [happy] Component with style block emits TSX file with CSS module import statement  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `codegen (Web IR)`  (happy; EXTRACTED)
- [happy] TypeScript codegen produces routes.manifest.ts file for multi-route module  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `codegen (reactive stats)`  (happy; EXTRACTED)
- [happy] Codegen reports web_ir_view_emitted >= 1 in reactive stats after Web IR view parity match  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `codegen output non-emptiness`  (invariant; EXTRACTED)
- [invariant] mixed surface codegen produces non-empty file list  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `codegen_rust`  (happy; EXTRACTED)
- [happy] Plain function definition generates valid Rust code without activity syntax  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `codegen_rust activity_id preservation`  (happy; EXTRACTED)
- [happy] Code generation preserves the `id` alias as an activity_id option in generated Rust code (with_activity_id)  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `codegen_with_options() tanstack_start flag behavior`  (happy; EXTRACTED)
- [happy] generate_with_options() with tanstack_start=true produces routes.manifest.ts but NOT VoxTanStackRouter.tsx or App.tsx  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `component Element return type`  (happy; EXTRACTED)
- [happy] component returning Element type produces no warnings  (crates/vox-integration-tests/tests/typeck_test.rs)

### `conflict detection in prompts`  (happy; EXTRACTED)
- [happy] detect_conflicts() identifies prompt conflicts between contradictory directives (never/always pairs and competing optimizations)  (crates/vox-integration-tests/tests/prompt_canonical_guardrails_test.rs)

### `context_store.get()`  (happy; EXTRACTED)
- [happy] Retrieving a key via context_store.get() returns the exact value that was set  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)

### `detect_conflicts()`  (happy; EXTRACTED)
- [happy] detect_conflicts finds at least one conflict in prompt with never/always and conflicting optimization directives  (crates/vox-integration-tests/tests/prompt_canonical_guardrails_test.rs)

### `drain_and_complete_all_bounded`  (happy; EXTRACTED)
- [happy] After draining and completing all tasks, total_completed equals 10  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)

### `dual-run legacy codegen on mixed surface`  (happy; EXTRACTED)
- [happy] legacy codegen on mixed surface produces Dash.tsx file  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `emit_api_client`  (happy; EXTRACTED)
- [happy] emit_api_client generates API client code from HIR  (crates/vox-integration-tests/tests/greaterfool_parity_gates_test.rs)

### `emit_component_view_tsx`  (happy; EXTRACTED)
- [happy] Web IR component view TSX emission produces identical whitespace-normalized output as legacy HIR emitter  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `emit_component_view_tsx()`  (happy; EXTRACTED)
- [happy] Component view TSX emission for HooksDemo contains hooks_demo class reference  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)

### `emit_mcp_server codegen`  (happy; EXTRACTED)
- [happy] @mcp.tool functions with list parameters correctly generate MCP input schema  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `encode_semantic_tokens`  (happy; EXTRACTED)
- [happy] semantic token encoding produces non-empty data for minimal Vox module  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)

### `failure_category enum`  (invariant; EXTRACTED)
- [invariant] failure_category enums match across failure-taxonomy.schema.json, speech_trace.schema.json, and speech_trace.mens.schema.json  (crates/vox-integration-tests/tests/speech_schema_parity_test.rs)

### `function argument count checking`  (error; EXTRACTED)
- [error] Calling a function with wrong number of arguments produces error containing 'Argument count mismatch'  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `function argument type checking`  (error; EXTRACTED)
- [error] Calling a function with argument type mismatch produces error containing 'Argument type mismatch'  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `generate() file emission for web routing`  (happy; EXTRACTED)
- [happy] generate() produces routes.manifest.ts and vox-client.ts when compiling web_routing_fullstack.vox example  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `generate() for @query endpoint`  (happy; EXTRACTED)
- [happy] generate() produces routes.manifest.ts and vox-client.ts when compiling blog_fullstack.vox example with @query  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `generate_rust`  (happy; EXTRACTED)
- [happy] Rust codegen produces src/main.rs file from reference example HIR  (crates/vox-integration-tests/tests/greaterfool_parity_gates_test.rs)

### `generate_rust()`  (happy; EXTRACTED)
- [happy] generate_rust produces src/main.rs file for @server fn definitions (test ignored - @server shorthand not in parser)  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `generate_with_options`  (error; EXTRACTED)
- [error] Codegen fails with web_ir_validate error when duplicate client routes exist and VOX_WEBIR_VALIDATE=1  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `generic identity function with matching types`  (happy; EXTRACTED)
- [happy] Generic function returning instantiated type parameter with matching concrete type produces no typecheck errors  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `generic type parameter type checking`  (error; EXTRACTED)
- [error] Assigning generic function result to incompatible type produces error containing 'mismatch' or 'Incompatible'  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `handle_tool_call(vox_project_init)`  (happy; EXTRACTED)
- [happy] vox_project_init tool returns success=true for nested application creation  (crates/vox-integration-tests/tests/mcp_project_init_test.rs)

### `hir.endpoint_fns`  (happy; EXTRACTED)
- [happy] HIR endpoint_fns collection contains route paths for api_todos, create_todo, and get_stats endpoints  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `http keyword (tombstoned)`  (error; EXTRACTED)
- [error] Tombstoned `http` keyword produces a parse error when used in source code  (crates/vox-integration-tests/tests/cli_test.rs)

### `index DDL emission`  (happy; EXTRACTED)
- [happy] HIR module with @index declaration contains exactly 1 index entry which can emit DDL via emit_index_ddl  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `internal-web-ir-implementation-blueprint.md documentation`  (invariant; EXTRACTED)
- [invariant] The internal web IR implementation blueprint document contains 'Acceptance gates' and either 'G4 Parity Gate' or 'Parity Gate' sections  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `lower_hir_to_web_ir`  (happy; EXTRACTED)
- [happy] lower_hir_to_web_ir() plus validate_web_ir() can complete 25 cycles on MIXED_SURFACE composition in under 10 seconds  (crates/vox-integration-tests/tests/pipeline/includes/include_04.rs)

### `lower_hir_to_web_ir()`  (invariant; EXTRACTED)
- [invariant] Web IR lowering for reactive view emits at least one component view in reactive stats  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)

### `map_jsx_attr_name (hir_emit)`  (happy; EXTRACTED)
- [happy] JSX attribute mapping returns 'htmlFor' for Vox 'for' attribute  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `map_jsx_attr_name (jsx codegen)`  (happy; EXTRACTED)
- [happy] JSX codegen attribute mapping returns 'htmlFor' for Vox 'for' attribute  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `mixed surface component lowering count`  (happy; EXTRACTED)
- [happy] mixed surface lowers to at least 2 components in Web IR  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `mixed surface typechecking correctness`  (happy; EXTRACTED)
- [happy] behavior and classic component in one module typecheck without errors  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `multi-route manifest emission`  (happy; EXTRACTED)
- [happy] multi-route codegen produces routes.manifest.ts file  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `multi-route manifest file emission`  (happy; EXTRACTED)
- [happy] multi-route codegen emits routes.manifest.ts file  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `orch.context_store().read().unwrap().get()`  (happy; EXTRACTED)
- [happy] Context store value matches the value set via context_store().write().unwrap().set()  (crates/vox-integration-tests/tests/orchestrator_e2e_test.rs)

### `p001_expected.vox`  (happy; EXTRACTED)
- [happy] The p001_expected.vox fixture produces no ERROR-level diagnostics during HIR validation  (crates/vox-integration-tests/tests/speech_fixture_validate_test.rs)

### `parse(lex(source_activity))`  (error; EXTRACTED)
- [error] Parsing source with tombstoned `activity` keyword produces a parse error  (crates/vox-integration-tests/tests/multimodal_image_gen_test.rs)

### `parse(lex(source_workflow))`  (error; EXTRACTED)
- [error] Parsing source with tombstoned `workflow` keyword produces a parse error  (crates/vox-integration-tests/tests/multimodal_image_gen_test.rs)

### `parse(lex(src)) with activity keyword`  (edge; EXTRACTED)
- [edge] Parsing source with activity keyword produces Err result (test ignored - keyword is no longer tombstoned)  (crates/vox-integration-tests/tests/multimodal_image_gen_test.rs)

### `parse/typecheck_module`  (happy; EXTRACTED)
- [happy] chatbot pipeline example parses and typechecks with no errors  (crates/vox-integration-tests/tests/greaterfool_parity_gates_test.rs)

### `plan_workflow_activities() and generate_rust() activity names`  (happy; EXTRACTED)
- [happy] Generated activity names match interpreted plan order (test ignored - activity/workflow keywords tombstoned)  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `plan_workflow_activities() id alias`  (happy; EXTRACTED)
- [happy] plan_workflow_activities maps 'id' alias in with block to activity_id field (test ignored - activity/workflow keywords tombstoned)  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `plan_workflow_activities().timeout_ms`  (happy; EXTRACTED)
- [happy] Interpreted planner converts timeout string (e.g. '10s') to milliseconds (10000)  (crates/vox-integration-tests/tests/parity_contracts_test.rs)

### `playwright test artifacts`  (happy; EXTRACTED)
- [happy] Playwright test execution produces route.png and a11y.json artifacts  (crates/vox-integration-tests/tests/playwright_golden_route_test.rs)

### `preflight_mcp_tool`  (error; EXTRACTED)
- [error] preflight_mcp_tool rejects destructive shell commands (rm -rf pattern) even with user_approval flag  (crates/vox-integration-tests/tests/agentos_aci_bench.rs)

### `quickfixes_for_diagnostics`  (happy; EXTRACTED)
- [happy] quickfixes round-trip from diagnostic JSON data into code actions  (crates/vox-integration-tests/tests/lsp_capabilities_test.rs)

### `refine_transcript confidence bounds`  (happy; EXTRACTED)
- [happy] refine_transcript normalizes whitespace and produces confidence value within configured min/max tunable bounds  (crates/vox-integration-tests/tests/speech_pipeline_stage_probe_test.rs)

### `registerOratioSpeechCommands.ts`  (invariant; EXTRACTED)
- [invariant] Editor speech commands source contains getUserMedia, sampleRate capture, encodeWavMono, and sampleRate parameter passing  (crates/vox-integration-tests/tests/speech_pipeline_stage_probe_test.rs)

### `repo_workspace_status root path matching`  (happy; EXTRACTED)
- [happy] repo_workspace_status_for_cwd() returns canonicalized root path matching the input temporary directory  (crates/vox-integration-tests/tests/repo_shared_ops_lifecycle_test.rs)

### `repository root canonicalization`  (happy; EXTRACTED)
- [happy] discover_repository_or_fallback() returns canonicalized root paths matching the temp directories  (crates/vox-integration-tests/tests/repository_ssot_test.rs)

### `repository_id uniqueness across paths`  (invariant; EXTRACTED)
- [invariant] discover_repository_or_fallback() generates distinct repository_id values for different filesystem roots  (crates/vox-integration-tests/tests/repository_ssot_test.rs)

### `route manifest file emission`  (happy; EXTRACTED)
- [happy] mixed surface codegen emits routes.manifest.ts file  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `route paths in manifest for multi-route`  (happy; EXTRACTED)
- [happy] routes.manifest.ts contains route paths like /todos or component names  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `routes codegen output`  (happy; EXTRACTED)
- [happy] generate() produces routes.manifest.ts file when compiling routes block  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `run_vox_tests`  (happy; EXTRACTED)
- [happy] all @test functions in golden .vox files pass without failures  (crates/vox-integration-tests/tests/golden_vox_test_runner.rs)

### `safety_pass acceptance of normal prompts`  (happy; EXTRACTED)
- [happy] safety_pass() accepts normal function-request prompts without injection attempts  (crates/vox-integration-tests/tests/prompt_canonical_guardrails_test.rs)

### `safety_pass rejection of injection patterns`  (error; EXTRACTED)
- [error] safety_pass() rejects prompts containing known injection attack patterns like "Ignore previous instructions"  (crates/vox-integration-tests/tests/prompt_canonical_guardrails_test.rs)

### `session directory isolation by repository_id`  (invariant; EXTRACTED)
- [invariant] mcp_sessions_dir() generates different session directory paths for different repository_id values  (crates/vox-integration-tests/tests/repository_ssot_test.rs)

### `speech audit documentation frontmatter`  (invariant; EXTRACTED)
- [invariant] All published speech audit docs (surface-inventory, audit-findings, improvement-backlog, ci-gates) exist and contain title and last_updated frontmatter  (crates/vox-integration-tests/tests/speech_audit_contract_test.rs)

### `src/main.vox file`  (happy; EXTRACTED)
- [happy] vox_project_init creates src/main.vox at the nested package location  (crates/vox-integration-tests/tests/mcp_project_init_test.rs)

### `style block lowering to Web IR and CSS codegen`  (happy; EXTRACTED)
- [happy] style blocks lower to Web IR with at least one style rule in the summary  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `table DDL emission`  (happy; EXTRACTED)
- [happy] HIR module with @table declaration contains exactly 1 table entry which can emit DDL via emit_table_ddl  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `typecheck_errors`  (happy; EXTRACTED)
- [happy] typecheck_errors detects type mismatches in variable assignment (int = string rejected)  (crates/vox-integration-tests/tests/golden_typecheck_gate.rs)

### `typecheck_module()`  (happy; EXTRACTED)
- [happy] Type-checking the chatbot example source produces no type errors  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `typecheck_module() error filtering`  (happy; EXTRACTED)
- [happy] typecheck_module() returns zero Error-severity diagnostics for valid @table/@index source  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `validate_web_ir()`  (happy; EXTRACTED)
- [happy] Web IR validation on hooks demo produces no errors  (crates/vox-integration-tests/tests/pipeline/includes/include_02.rs)

### `vox build full-stack compilation`  (happy; EXTRACTED)
- [happy] vox build can compile a full-stack minimal fixture to TypeScript and React output  (crates/vox-integration-tests/tests/playwright_golden_route_test.rs)

### `vox build output files`  (happy; EXTRACTED)
- [happy] vox build produces TypeScript output directory for scaffolding  (crates/vox-integration-tests/tests/playwright_golden_route_test.rs)

### `vox-web-stack Web IR terminology`  (invariant; EXTRACTED)
- [invariant] vox-web-stack.md contains Web IR terminology (WebIR or 'Web IR' or web_ir)  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `vox-web-stack reference doc cross-links`  (invariant; EXTRACTED)
- [invariant] vox-web-stack.md contains references to blueprint doc or Web IR terminology  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `voxRoutes export in manifest`  (happy; EXTRACTED)
- [happy] routes.manifest.ts contains voxRoutes export  (crates/vox-integration-tests/tests/pipeline/includes/blueprint_op_s_batch.rs)

### `vox_a2a_ack`  (happy; EXTRACTED)
- [happy] vox_a2a_ack MCP tool returns JSON response with 'success': true  (crates/vox-integration-tests/tests/a2a_mcp_test.rs)

### `vox_a2a_history`  (happy; EXTRACTED)
- [happy] vox_a2a_history MCP tool returns JSON response with 'success': true containing message history  (crates/vox-integration-tests/tests/a2a_mcp_test.rs)

### `vox_a2a_send`  (happy; EXTRACTED)
- [happy] vox_a2a_send MCP tool returns JSON response with 'success': true  (crates/vox-integration-tests/tests/a2a_mcp_test.rs)

### `vox_cancel_task`  (happy; EXTRACTED)
- [happy] vox_cancel_task MCP tool returns JSON response with success=true for queued task  (crates/vox-integration-tests/tests/agent_mcp_roundtrip_test.rs)

### `vox_codegen::codegen_rust::emit::emit_index_ddl`  (happy; EXTRACTED)
- [happy] Index definition generates valid DDL SQL output  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `vox_codegen::codegen_rust::emit::emit_mcp_server`  (happy; EXTRACTED)
- [happy] MCP tools generate valid input schema with parameter definitions  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `vox_codegen::codegen_rust::emit::emit_table_ddl`  (happy; EXTRACTED)
- [happy] Table definition generates valid DDL SQL output  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `vox_codegen::codegen_rust::generate()`  (happy; EXTRACTED)
- [happy] Rust codegen produces src/main.rs file with snapshot validation for multi-route Axum server  (crates/vox-integration-tests/tests/pipeline/includes/include_03.rs)

### `vox_codegen::codegen_rust::generate() file output`  (happy; EXTRACTED)
- [happy] codegen_rust() produces src/lib.rs and src/main.rs files from @table/@index HIR  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `vox_compiler::codegen output file generation`  (happy; EXTRACTED)
- [happy] generate() produces a Stats.tsx file when compiling @v0 component with prompt description  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `vox_compiler::hir::Module`  (happy; EXTRACTED)
- [happy] MCP tool preserves description and function metadata during HIR lowering  (crates/vox-integration-tests/tests/codegen_rust_test.rs)

### `vox_compiler::hir::lower_module() structure`  (happy; EXTRACTED)
- [happy] lower_module() produces HIR with tables[0].name == 'Task', 3 fields, 1 index named 'by_done' on table 'Task'  (crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)

### `vox_repo_status MCP tool response success`  (happy; EXTRACTED)
- [happy] vox_repo_status MCP tool returns success=true with non-empty repository_id in data field  (crates/vox-integration-tests/tests/repo_shared_ops_lifecycle_test.rs)

### `vox_skill_install tool success`  (happy; EXTRACTED)
- [happy] vox_skill_install MCP tool accepts parsed skill bundles and returns success=true  (crates/vox-integration-tests/tests/skill_install_test.rs)

### `vox_skill_list enumeration of installed skills`  (happy; EXTRACTED)
- [happy] vox_skill_list MCP tool lists installed skills by id and name after installation  (crates/vox-integration-tests/tests/skill_install_test.rs)

### `vox_submit_task`  (happy; EXTRACTED)
- [happy] vox_submit_task MCP tool returns JSON response with success=true and numeric task_id  (crates/vox-integration-tests/tests/agent_mcp_roundtrip_test.rs)

### `vox_task_status`  (happy; EXTRACTED)
- [happy] vox_task_status MCP tool returns JSON response with success=true for submitted task  (crates/vox-integration-tests/tests/agent_mcp_roundtrip_test.rs)

### `web build pipeline end-to-end`  (happy; EXTRACTED)
- [happy] codegen_rust() and scaffold_react_app() succeed and pnpm install/build commands exit successfully  (crates/vox-integration-tests/tests/web_vite_smoke_test.rs)

### `with operator non-record validation`  (error; EXTRACTED)
- [error] with operator applied to non-record produces error with message containing "'with' options must be a record"  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `with operator on Result values`  (happy; EXTRACTED)
- [happy] Ok(1) with { meta: "data" } typechecks without error  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `with operator retries field type checking`  (edge; EXTRACTED)
- [edge] with operator retries field with string value produces warning containing "retries" and "Int"  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `with operator unknown key detection`  (edge; EXTRACTED)
- [edge] with operator with unknown option key produces warning containing "Unknown 'with' option"  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

### `workflow keyword stable status (ADR-041)`  (happy; EXTRACTED)
- [happy] workflow keyword definition produces no E028 reservation error in frontend pipeline  (crates/vox-integration-tests/tests/workflow_integration_test.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`@index on known table`** — only: _Index definition on an existing @table produces no typecheck errors_
- **`@server fn endpoint function presence`** — only: _multi-route fixture HIR contains endpoint function named get_stats_
- **`@table type declaration with str/bool/int fields`** — only: _Valid table type declarations with standard scalar fields produce no typecheck errors_
- **`@table/@index parsing`** — only: _parse() produces exactly 2 declarations when parsing @table with @index (one table + one index)_
- **`ADT constructor resolution`** — only: _ADT constructors like Circle are resolvable in function body scope without undefined variable error_
- **`CSS file generation from codegen`** — only: _codegen produces files with .css extension_
- **`CSS import in Chat component`** — only: _Chat.tsx contains CSS import statement_
- **`CanaryPolicy deserialization`** — only: _The canary_policy.example.json file deserializes successfully into CanaryPolicy struct_
- **`Chat component file generation`** — only: _chatbot codegen produces Chat.tsx file_
- **`CompletionEngine::completions`** — only: _completion engine surfaces 'fn' keyword in completions at start of file_
- **`Decl::McpTool`** — only: _First parsed McpTool declaration has description 'Search the knowledge base'_
- **`HTTP loader contracts in Web IR`** — only: _multi-route module lowers to at least one HTTP loader contract or query function contract_
- **`KPI baseline schema validator`** — only: _A KPI snapshot with schema_version, captured_at_utc, wer, cer, compile_pass_at_1, and latency_ms_p95 passes schema validation_
- **`KpiSnapshot validation and schema conformance`** — only: _When VOX_SPEECH_CANARY_KPI is set, the KPI snapshot file deserializes as valid JSON, matches kpi-baseline.schema.json, and deserializes into KpiSnapshot struct_
- **`MCP resource codegen`** — only: _@mcp.resource decorated function lowers to exactly 1 mcp_resources entry with correct URI; generates src/mcp_server.rs; Cargo.toml declares [[bin]] mcp_server binary_
- **`MCP server file generation`** — only: _Codegen with @mcp.tool produces src/mcp_server.rs in output files_
- **`MCP server multi-tool schema`** — only: _HIR module with 2 @mcp.tool declarations contains exactly 2 mcp_tools entries; emit_mcp_server produces schema output_
- **`MCP tool HIR lowering`** — only: _@mcp.tool decorated function lowers to exactly 1 mcp_tools entry; does NOT appear in functions list; preserves description and parameter count_
- **`MCP tool list parameter schema`** — only: _@mcp.tool with list[str] parameter lowers to HIR and emit_mcp_server produces schema output containing list param handling_
- **`Orchestrator.context_store()`** — only: _context_store.set() and context_store.get() preserve key-value pairs across reads_
- **`Orchestrator.status() task queue counts`** — only: _After submitting 10 tasks, total_queued + total_in_progress equals 10_
- **`Orchestrator.status().total_completed`** — only: _After draining and completing all tasks, total_completed equals 10_
- **`Orchestrator.status().total_queued + total_in_progress`** — only: _After submitting 10 tasks, total_queued + total_in_progress equals 10_
- **`Playwright test execution`** — only: _Playwright test suite successfully captures route screenshots and a11y audit JSON reports from the built app_
- **`React app scaffolding and npm installation`** — only: _scaffold_react_app() and pnpm install successfully configure and install a React application from generated code_
- **`SymbolEngine::symbols`** — only: _symbol engine includes outline symbols for function definitions_
- **`VAD near-silence detection`** — only: _Voice activity detection produces empty segments for near-silence audio (0.001 amplitude at 16kHz)_
- **`Vox.toml file`** — only: _vox_project_init creates Vox.toml at packages/nested_app/ when target_subdir is specified_
- **`Web IR validation on mixed surface`** — only: _mixed surface Web IR validates with no diagnostics_
- **`Web IR view_roots`** — only: _Chat component is present in Web IR view_roots collection_
- **`acoustic preprocessing`** — only: _Acoustic preprocessing without environment override returns original samples unchanged, reports mode='none', and does not skip due to budget_
- **`activity and workflow keywords coexistence (ADR-041)`** — only: _activity and workflow keywords used together produce no E028 reservation error_
- **`activity keyword stable status (ADR-041)`** — only: _activity keyword definition produces no E028 reservation error in frontend pipeline_
- **`activity_id alias mapping`** — only: _When a workflow activity is declared with an `id` option ("email-step-alias"), plan_workflow_activities() maps it to activity_id field with the specified value_
- **`actor handler type checking`** — only: _actor on receive handler with valid builtin call (print) typechecks cleanly without errors_
- **`apply_context_budget provenance`** — only: _apply_context_budget produces provenance entry for each selected chunk_
- **`apply_context_budget with max_chunks=2, max_chars=12`** — only: _apply_context_budget selects both chunks when under max_chunks constraint_
- **`apply_context_budget()`** — only: _apply_context_budget selects both chunks when count is below max_chunks_
- **`apply_context_budget() provenance`** — only: _apply_context_budget produces provenance entries for each selected chunk_
- **`apply_context_budget().provenance.truncated`** — only: _apply_context_budget marks provenance as truncated when character budget is exceeded_
- **`attach_aci_envelope`** — only: _attach_aci_envelope wraps tool response JSON in ACI envelope that validates against JSON schema_
- **`audit matrix schema validation`** — only: _audit-matrix.v1.yaml validates against audit-matrix.schema.json using jsonschema validator_
- **`bind={} reactive binding codegen`** — only: _generate() expands bind=email syntax to value/onChange props in LoginForm.tsx output_
- **`builtin_hover_markdown`** — only: _builtin_hover_markdown returns Some for 'print' builtin function_
- **`canary.kpi.json`** — only: _Committed KPI baseline JSON validates against kpi-baseline.schema.json and includes schema_version=1, WER, and CER fields_
- **`canonicalize_prompt hash and structure preservation`** — only: _canonicalize_prompt() preserves original_hash and reconstructs prompt text structure with canonical sections_
- **`client route tree lowering`** — only: _multi-route module lowers to at least one client route tree in Web IR_
- **`code_lenses_for_module`** — only: _code lens engine emits vox.runTest command for @test-annotated functions_
- **`codegen (CSS module import)`** — only: _Chatbot fixture generates Chat.tsx with Chat.css import statement_
- **`codegen (CSS)`** — only: _Component with style block emits CSS file containing color properties_
- **`codegen (TypeScript)`** — only: _Component with style block emits TSX file with CSS module import statement_
- **`codegen (Web IR)`** — only: _TypeScript codegen produces routes.manifest.ts file for multi-route module_
- **`codegen (reactive stats)`** — only: _Codegen reports web_ir_view_emitted >= 1 in reactive stats after Web IR view parity match_
- **`codegen_rust`** — only: _Plain function definition generates valid Rust code without activity syntax_
- **`codegen_rust activity_id preservation`** — only: _Code generation preserves the `id` alias as an activity_id option in generated Rust code (with_activity_id)_
- **`codegen_with_options() tanstack_start flag behavior`** — only: _generate_with_options() with tanstack_start=true produces routes.manifest.ts but NOT VoxTanStackRouter.tsx or App.tsx_
- **`component Element return type`** — only: _component returning Element type produces no warnings_
- **`conflict detection in prompts`** — only: _detect_conflicts() identifies prompt conflicts between contradictory directives (never/always pairs and competing optimizations)_
- **`context_store.get()`** — only: _Retrieving a key via context_store.get() returns the exact value that was set_
- **`detect_conflicts()`** — only: _detect_conflicts finds at least one conflict in prompt with never/always and conflicting optimization directives_
- **`drain_and_complete_all_bounded`** — only: _After draining and completing all tasks, total_completed equals 10_
- **`dual-run legacy codegen on mixed surface`** — only: _legacy codegen on mixed surface produces Dash.tsx file_
- **`emit_api_client`** — only: _emit_api_client generates API client code from HIR_
- **`emit_component_view_tsx`** — only: _Web IR component view TSX emission produces identical whitespace-normalized output as legacy HIR emitter_
- **`emit_component_view_tsx()`** — only: _Component view TSX emission for HooksDemo contains hooks_demo class reference_
- **`emit_mcp_server codegen`** — only: _@mcp.tool functions with list parameters correctly generate MCP input schema_
- **`encode_semantic_tokens`** — only: _semantic token encoding produces non-empty data for minimal Vox module_
- **`generate codegen`** — only: _Rust codegen produces src/mcp_server.rs file when @mcp.tool declarations exist_
- **`generate()`** — only: _Code generation for chatbot produces types.ts, vox-app-contract.json, vox-tanstack-query.tsx, and Chat.tsx but not server.ts_
- **`generate() file emission for web routing`** — only: _generate() produces routes.manifest.ts and vox-client.ts when compiling web_routing_fullstack.vox example_
- **`generate() for @query endpoint`** — only: _generate() produces routes.manifest.ts and vox-client.ts when compiling blog_fullstack.vox example with @query_
- **`generate_rust`** — only: _Rust codegen produces src/main.rs file from reference example HIR_
- **`generate_rust()`** — only: _generate_rust produces src/main.rs file for @server fn definitions (test ignored - @server shorthand not in parser)_
- **`generic identity function with matching types`** — only: _Generic function returning instantiated type parameter with matching concrete type produces no typecheck errors_
- **`handle_tool_call(vox_project_init)`** — only: _vox_project_init tool returns success=true for nested application creation_
- **`hir.endpoint_fns`** — only: _HIR endpoint_fns collection contains route paths for api_todos, create_todo, and get_stats endpoints_
- **`index DDL emission`** — only: _HIR module with @index declaration contains exactly 1 index entry which can emit DDL via emit_index_ddl_
- **`lower_hir_to_web_ir`** — only: _lower_hir_to_web_ir() plus validate_web_ir() can complete 25 cycles on MIXED_SURFACE composition in under 10 seconds_
- **`lower_hir_to_web_ir_with_summary`** — only: _Web IR summary reports query function contracts >= 1 when module has query endpoints_
- **`lower_module HIR lowering`** — only: _Table declarations are correctly lowered from AST to HIR with field count preserved_
- **`map_jsx_attr_name`** — only: _JSX attribute mapping returns 'className' for Vox 'class' attribute_
- **`map_jsx_attr_name (hir_emit)`** — only: _JSX attribute mapping returns 'htmlFor' for Vox 'for' attribute_
- **`map_jsx_attr_name (jsx codegen)`** — only: _JSX codegen attribute mapping returns 'htmlFor' for Vox 'for' attribute_
- **`mixed surface component lowering count`** — only: _mixed surface lowers to at least 2 components in Web IR_
- **`mixed surface typechecking correctness`** — only: _behavior and classic component in one module typecheck without errors_
- **`multi-route manifest emission`** — only: _multi-route codegen produces routes.manifest.ts file_
- **`multi-route manifest file emission`** — only: _multi-route codegen emits routes.manifest.ts file_
- **`orch.context_store().read().unwrap().get()`** — only: _Context store value matches the value set via context_store().write().unwrap().set()_
- **`p001_expected.vox`** — only: _The p001_expected.vox fixture produces no ERROR-level diagnostics during HIR validation_
- **`parse()`** — only: _Parsing the chatbot example source produces exactly 5 declarations (import, type, component, server endpoint, mutation endpoint)_
- **`parse/typecheck_module`** — only: _chatbot pipeline example parses and typechecks with no errors_
- **`plan_workflow_activities() and generate_rust() activity names`** — only: _Generated activity names match interpreted plan order (test ignored - activity/workflow keywords tombstoned)_
- **`plan_workflow_activities() id alias`** — only: _plan_workflow_activities maps 'id' alias in with block to activity_id field (test ignored - activity/workflow keywords tombstoned)_
- **`plan_workflow_activities().timeout_ms`** — only: _Interpreted planner converts timeout string (e.g. '10s') to milliseconds (10000)_
- **`playwright test artifacts`** — only: _Playwright test execution produces route.png and a11y.json artifacts_
- **`quickfixes_for_diagnostics`** — only: _quickfixes round-trip from diagnostic JSON data into code actions_
- **`refine_transcript confidence bounds`** — only: _refine_transcript normalizes whitespace and produces confidence value within configured min/max tunable bounds_
- **`repo_workspace_status root path matching`** — only: _repo_workspace_status_for_cwd() returns canonicalized root path matching the input temporary directory_
- **`repository root canonicalization`** — only: _discover_repository_or_fallback() returns canonicalized root paths matching the temp directories_
- **`route manifest file emission`** — only: _mixed surface codegen emits routes.manifest.ts file_
- **`route paths in manifest for multi-route`** — only: _routes.manifest.ts contains route paths like /todos or component names_
- **`routes codegen output`** — only: _generate() produces routes.manifest.ts file when compiling routes block_
- **`run_vox_tests`** — only: _all @test functions in golden .vox files pass without failures_
- **`safety_pass acceptance of normal prompts`** — only: _safety_pass() accepts normal function-request prompts without injection attempts_
- **`src/main.vox file`** — only: _vox_project_init creates src/main.vox at the nested package location_
- **`style block lowering to Web IR and CSS codegen`** — only: _style blocks lower to Web IR with at least one style rule in the summary_
- **`table DDL emission`** — only: _HIR module with @table declaration contains exactly 1 table entry which can emit DDL via emit_table_ddl_
- **`typecheck_errors`** — only: _typecheck_errors detects type mismatches in variable assignment (int = string rejected)_
- **`typecheck_module()`** — only: _Type-checking the chatbot example source produces no type errors_
- **`typecheck_module() error filtering`** — only: _typecheck_module() returns zero Error-severity diagnostics for valid @table/@index source_
- **`validate_web_ir()`** — only: _Web IR validation on hooks demo produces no errors_
- **`vox build full-stack compilation`** — only: _vox build can compile a full-stack minimal fixture to TypeScript and React output_
- **`vox build output files`** — only: _vox build produces TypeScript output directory for scaffolding_
- **`vox init scaffold`** — only: _vox init scaffold creates main.vox, Cargo.toml, and .gitignore files_
- **`voxRoutes export in manifest`** — only: _routes.manifest.ts contains voxRoutes export_
- **`vox_a2a_ack`** — only: _vox_a2a_ack MCP tool returns JSON response with 'success': true_
- **`vox_a2a_history`** — only: _vox_a2a_history MCP tool returns JSON response with 'success': true containing message history_
- **`vox_a2a_inbox`** — only: _vox_a2a_inbox MCP tool returns JSON response with 'success': true and contains message payload_
- **`vox_a2a_send`** — only: _vox_a2a_send MCP tool returns JSON response with 'success': true_
- **`vox_cancel_task`** — only: _vox_cancel_task MCP tool returns JSON response with success=true for queued task_
- **`vox_codegen::codegen_rust::emit::emit_index_ddl`** — only: _Index definition generates valid DDL SQL output_
- **`vox_codegen::codegen_rust::emit::emit_mcp_server`** — only: _MCP tools generate valid input schema with parameter definitions_
- **`vox_codegen::codegen_rust::emit::emit_table_ddl`** — only: _Table definition generates valid DDL SQL output_
- **`vox_codegen::codegen_rust::generate()`** — only: _Rust codegen produces src/main.rs file with snapshot validation for multi-route Axum server_
- **`vox_codegen::codegen_rust::generate() file output`** — only: _codegen_rust() produces src/lib.rs and src/main.rs files from @table/@index HIR_
- **`vox_compiler::codegen output file generation`** — only: _generate() produces a Stats.tsx file when compiling @v0 component with prompt description_
- **`vox_compiler::hir::Module`** — only: _MCP tool preserves description and function metadata during HIR lowering_
- **`vox_compiler::hir::lower_module`** — only: _Table type definition with fields is correctly lowered to HIR with exactly one table_
- **`vox_compiler::hir::lower_module() structure`** — only: _lower_module() produces HIR with tables[0].name == 'Task', 3 fields, 1 index named 'by_done' on table 'Task'_
- **`vox_repo_status MCP tool response success`** — only: _vox_repo_status MCP tool returns success=true with non-empty repository_id in data field_
- **`vox_skill_install tool success`** — only: _vox_skill_install MCP tool accepts parsed skill bundles and returns success=true_
- **`vox_skill_list enumeration of installed skills`** — only: _vox_skill_list MCP tool lists installed skills by id and name after installation_
- **`vox_submit_task`** — only: _vox_submit_task MCP tool returns JSON response with success=true and numeric task_id_
- **`vox_task_status`** — only: _vox_task_status MCP tool returns JSON response with success=true for submitted task_
- **`web build pipeline end-to-end`** — only: _codegen_rust() and scaffold_react_app() succeed and pnpm install/build commands exit successfully_
- **`with operator on Result values`** — only: _Ok(1) with { meta: "data" } typechecks without error_
- **`workflow keyword stable status (ADR-041)`** — only: _workflow keyword definition produces no E028 reservation error in frontend pipeline_
