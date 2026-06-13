# Reached-but-NOT-Proven — Phase 0 (llvm-cov × proven map)

Annotated 25333 code symbols with llvm-cov `reached` status.

**reached-not-proven** = a symbol whose code EXECUTED during tests but has NO asserted behavior (`proves` edge). This is the precise set line coverage counts as 'covered' but that proves nothing — the keystone signal of this whole initiative.


**Total reached-but-unproven symbols: 7950**


| Crate | Code | Reached | Proven | Reached-not-proven |
|---|---|---|---|---|
| vox-orchestrator | 3484 | 1530 | 616 | **1278** |
| vox-compiler | 1689 | 967 | 364 | **728** |
| vox-codegen | 968 | 639 | 97 | **554** |
| vox-db | 1643 | 652 | 227 | **548** |
| vox-code-audit | 1469 | 702 | 309 | **545** |
| vox-orchestrator-mcp | 1790 | 522 | 157 | **446** |
| vox-publisher | 1103 | 433 | 96 | **374** |
| vox-cli | 3420 | 563 | 641 | **359** |
| vox-populi | 1208 | 356 | 111 | **315** |
| vox-gamify | 722 | 284 | 58 | **255** |
| vox-actor-runtime | 642 | 186 | 95 | **145** |
| vox-scientia | 514 | 180 | 96 | **135** |
| vox-corpus | 438 | 142 | 29 | **121** |
| vox-search | 241 | 128 | 27 | **113** |
| vox-drift-check | 262 | 134 | 41 | **109** |
| vox-orchestrator-queue | 303 | 127 | 46 | **105** |
| vox-research-shim | 276 | 129 | 39 | **105** |
| vox-audit | 337 | 118 | 65 | **96** |
| vox-secrets | 250 | 111 | 28 | **95** |
| vox-repository | 166 | 96 | 16 | **82** |
| vox-speech | 372 | 94 | 29 | **81** |
| vox-config | 242 | 84 | 42 | **71** |
| vox-workflow-runtime | 189 | 86 | 27 | **71** |
| vox-inference | 212 | 81 | 31 | **58** |
| vox-effort-audit | 142 | 76 | 50 | **52** |
| vox-orchestrator-types | 142 | 52 | 14 | **49** |
| vox-package-types | 129 | 48 | 11 | **46** |
| vox-plugin-mens-candle-cuda | 364 | 58 | 27 | **46** |
| vox-effort-route | 134 | 63 | 30 | **45** |
| vox-plugin-mens-candle-metal | 353 | 53 | 26 | **43** |
| vox-lsp | 94 | 45 | 6 | **40** |
| vox-telemetry | 101 | 47 | 20 | **38** |
| vox-plugin-populi-mesh | 307 | 40 | 14 | **36** |
| vox-distributed-training | 86 | 37 | 4 | **34** |
| vox-plugin-host | 118 | 45 | 15 | **34** |
| vox-constrained-gen | 82 | 41 | 13 | **32** |
| vox-mesh-types | 120 | 37 | 20 | **32** |
| vox-vcs | 100 | 39 | 35 | **30** |
| vox-plugin-webhook | 133 | 34 | 11 | **29** |
| vox-tensor | 56 | 31 | 5 | **29** |
| vox-package | 83 | 28 | 6 | **28** |
| vox-test-harness | 99 | 27 | 7 | **26** |
| vox-openclaw-runtime | 147 | 26 | 5 | **25** |
| vox-skills | 109 | 28 | 10 | **23** |
| vox-capability-registry | 51 | 23 | 3 | **21** |
| vox-git | 63 | 23 | 9 | **20** |
| vox-plugin-speech | 179 | 21 | 10 | **20** |
| vox-plugin-test-harness | 35 | 18 | 1 | **18** |
| vox-quantize | 54 | 30 | 18 | **18** |
| vox-rule-pack | 57 | 19 | 19 | **18** |
| vox-grammar-export | 49 | 23 | 10 | **16** |
| vox-runtime-rn | 35 | 17 | 4 | **16** |
| vox-wasm-engine | 31 | 17 | 4 | **16** |
| vox-cli-tests | 19 | 14 | 1 | **14** |
| vox-hf-layout | 26 | 17 | 6 | **14** |
| vox-ast | 241 | 15 | 24 | **13** |
| vox-ml-cli | 428 | 15 | 31 | **13** |
| vox-skill-runtime | 37 | 15 | 6 | **12** |
| vox-arch-check | 85 | 17 | 19 | **11** |
| vox-crypto | 39 | 24 | 13 | **11** |

## Top reached-but-unproven symbols (per worst crate)


### vox-orchestrator
- `.sync_audit_trail()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L33
- `.new()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L42
- `.next_id()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L53
- `MessageId` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L53
- `.register_agent()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L58
- `.send()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L70
- `Into` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L70
- `.broadcast()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L117

### vox-compiler
- `fn_signature()` — crates/vox-compiler/src/app_contract.rs:L86
- `HirParam` — crates/vox-compiler/src/app_contract.rs:L86
- `project_app_contract()` — crates/vox-compiler/src/app_contract.rs:L92
- `.coverage_score()` — crates/vox-compiler/src/ast_eval.rs:L32
- `eval_module()` — crates/vox-compiler/src/ast_eval.rs:L42
- `ast_eval()` — crates/vox-compiler/src/ast_eval.rs:L58
- `count_module_constructs()` — crates/vox-compiler/src/ast_eval.rs:L78
- `builtin_entry_param_tys()` — crates/vox-compiler/src/builtin_registry.rs:L30

### vox-codegen
- `.from_bundle_fragment()` — crates/vox-codegen/src/assets/mod.rs:L19
- `.validate_preflight()` — crates/vox-codegen/src/assets/mod.rs:L51
- `.stage_under()` — crates/vox-codegen/src/assets/mod.rs:L67
- `copy_path_recursive()` — crates/vox-codegen/src/assets/mod.rs:L84
- `emit_llm_function_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L7
- `HirFn` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L7
- `emit_search_memory_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L92
- `emit_subagent_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L170

### vox-db
- `.is_auto_safe()` — crates/vox-db/src/auto_migrate.rs:L86
- `.to_sql()` — crates/vox-db/src/auto_migrate.rs:L97
- `.describe()` — crates/vox-db/src/auto_migrate.rs:L116
- `.is_empty()` — crates/vox-db/src/auto_migrate.rs:L147
- `.auto_actions()` — crates/vox-db/src/auto_migrate.rs:L152
- `.describe()` — crates/vox-db/src/auto_migrate.rs:L162
- `.new()` — crates/vox-db/src/auto_migrate.rs:L188
- `.introspect_tables()` — crates/vox-db/src/auto_migrate.rs:L193

### vox-code-audit
- `.new()` — crates/vox-code-audit/src/ai_analyze.rs:L89
- `.is_available()` — crates/vox-code-audit/src/ai_analyze.rs:L94
- `.build_prompt()` — crates/vox-code-audit/src/ai_analyze.rs:L99
- `.parse_response()` — crates/vox-code-audit/src/ai_analyze.rs:L151
- `.endpoint_url()` — crates/vox-code-audit/src/ai_analyze.rs:L234
- `.parse()` — crates/vox-code-audit/src/analysis/rust_context.rs:L14
- `.from_rust_source()` — crates/vox-code-audit/src/analysis/token_map.rs:L21
- `.is_non_code_byte()` — crates/vox-code-audit/src/analysis/token_map.rs:L89

### vox-orchestrator-mcp
- `a2a_message_may_surface_to_pilot()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L21
- `A2AMessageType` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L21
- `parse_msg_type()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L140
- `msg_type_wire()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L163
- `fnv1a64()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L167
- `default_idempotency_key()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L178
- `mapped_session_id()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L229
- `AgentSummary` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L229

### vox-publisher
- `session_cache()` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `Mutex` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `legacy_post()` — crates/vox-publisher/src/adapters/bluesky.rs:L92
- `DiscordOverride` — crates/vox-publisher/src/adapters/discord.rs:L7
- `MastodonOverride` — crates/vox-publisher/src/adapters/mastodon.rs:L9
- `OpenCollectiveConfig` — crates/vox-publisher/src/adapters/opencollective.rs:L10
- `markdown_to_html()` — crates/vox-publisher/src/adapters/opencollective.rs:L79
- `refresh_access_token()` — crates/vox-publisher/src/adapters/reddit.rs:L42

### vox-cli
- `cli_top_level_into_fabrica_or_self()` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `FabricaCmd` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `dispatch_cli()` — crates/vox-cli/src/cli_dispatch/mod.rs:L68
- `GlobalOpts` — crates/vox-cli/src/cli_dispatch/mod.rs:L68
- `dispatch_cli_inner()` — crates/vox-cli/src/cli_dispatch/mod.rs:L87
- `build_catalog()` — crates/vox-cli/src/command_catalog.rs:L77
- `apply_capability_ids()` — crates/vox-cli/src/command_catalog.rs:L107
- `render_text()` — crates/vox-cli/src/command_catalog.rs:L125

### vox-populi
- `.message_bytes()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L19
- `.sign()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `SigningKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `.verify()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `VerifyingKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `.to_operation_kind()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L65
- `synthetic_weights_hash()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L76
- `.message_bytes()` — crates/vox-populi/src/distributed_training/gradient.rs:L17

### vox-gamify
- `.scaled()` — crates/vox-gamify/src/ability.rs:L76
- `default_abilities()` — crates/vox-gamify/src/ability.rs:L88
- `doubt_achievements()` — crates/vox-gamify/src/achievement/defaults/doubt.rs:L3
- `Achievement` — crates/vox-gamify/src/achievement/defaults/doubt.rs:L3
- `all()` — crates/vox-gamify/src/achievement/defaults/mod.rs:L10
- `Achievement` — crates/vox-gamify/src/achievement/defaults/mod.rs:L10
- `part_a()` — crates/vox-gamify/src/achievement/defaults/part_a.rs:L3
- `Achievement` — crates/vox-gamify/src/achievement/defaults/part_a.rs:L3

### vox-actor-runtime
- `emit_sandbox_timeout_kill()` — crates/vox-actor-runtime/src/activity.rs:L14
- `.default()` — crates/vox-actor-runtime/src/activity.rs:L58
- `.new()` — crates/vox-actor-runtime/src/activity.rs:L72
- `.with_retries()` — crates/vox-actor-runtime/src/activity.rs:L77
- `.with_timeout()` — crates/vox-actor-runtime/src/activity.rs:L83
- `.with_timeout_secs()` — crates/vox-actor-runtime/src/activity.rs:L89
- `.with_initial_backoff()` — crates/vox-actor-runtime/src/activity.rs:L95
- `.with_max_backoff()` — crates/vox-actor-runtime/src/activity.rs:L101

### vox-scientia
- `.default()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L9
- `.new()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L22
- `.decompose()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L26
- `extract_tuple()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L77
- `SciClaimTuple` — crates/vox-scientia/src/claim_extractor/atomic.rs:L77
- `fnv1a_hash()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L110
- `.mock()` — crates/vox-scientia/src/claim_extractor/minicheck.rs:L29
- `.from_env()` — crates/vox-scientia/src/claim_extractor/minicheck.rs:L45

### vox-corpus
- `.new()` — crates/vox-corpus/src/codegen_vox/mod.rs:L25
- `.next()` — crates/vox-corpus/src/codegen_vox/mod.rs:L28
- `.usize()` — crates/vox-corpus/src/codegen_vox/mod.rs:L34
- `.to_jsonl()` — crates/vox-corpus/src/codegen_vox/part_02.rs:L82
- `gen_full_stack_program()` — crates/vox-corpus/src/codegen_vox/part_03.rs:L4
- `Rng` — crates/vox-corpus/src/codegen_vox/part_03.rs:L4
- `OrganicPair` — crates/vox-corpus/src/codegen_vox/part_03.rs:L4
- `generate_organic_corpus()` — crates/vox-corpus/src/codegen_vox/part_03.rs:L43

### vox-search
- `.from_search_pass()` — crates/vox-search/src/a2a_contract.rs:L52
- `Into` — crates/vox-search/src/a2a_contract.rs:L52
- `SearchExecution` — crates/vox-search/src/a2a_contract.rs:L52
- `run_search_with_verification()` — crates/vox-search/src/bundle.rs:L22
- `SearchRuntimeContext` — crates/vox-search/src/bundle.rs:L22
- `LexicalMemoryFallback` — crates/vox-search/src/bundle.rs:L22
- `TavilySessionBudget` — crates/vox-search/src/bundle.rs:L22
- `SearchExecution` — crates/vox-search/src/bundle.rs:L22

### vox-drift-check
- `.new()` — crates/vox-drift-check/src/cache.rs:L11
- `.from_workspace()` — crates/vox-drift-check/src/cache.rs:L16
- `.hash_file()` — crates/vox-drift-check/src/cache.rs:L20
- `.store()` — crates/vox-drift-check/src/cache.rs:L26
- `ExtractedFeatures` — crates/vox-drift-check/src/cache.rs:L26
- `.load()` — crates/vox-drift-check/src/cache.rs:L36
- `.default()` — crates/vox-drift-check/src/config.rs:L97
- `.default()` — crates/vox-drift-check/src/config.rs:L108
