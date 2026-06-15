# Reached-but-NOT-Proven — Phase 0 (llvm-cov × proven map)

Annotated 26388 code symbols with llvm-cov `reached` status.

**reached-not-proven** = a symbol whose code EXECUTED during tests but has NO asserted behavior (`proves` edge). This is the precise set line coverage counts as 'covered' but that proves nothing — the keystone signal of this whole initiative.


**Total reached-but-unproven symbols: 5793**


| Crate | Code | Reached | Proven | Reached-not-proven |
|---|---|---|---|---|
| vox-compiler | 1760 | 947 | 392 | **699** |
| vox-orchestrator | 3564 | 783 | 641 | **666** |
| vox-code-audit | 1530 | 758 | 343 | **571** |
| vox-publisher | 1219 | 487 | 112 | **419** |
| vox-codegen | 672 | 426 | 80 | **358** |
| vox-populi | 1462 | 383 | 156 | **330** |
| vox-codegen-ts | 403 | 205 | 44 | **172** |
| vox-cli | 3463 | 219 | 650 | **153** |
| vox-corpus | 431 | 149 | 29 | **128** |
| vox-drift-check | 279 | 151 | 47 | **116** |
| vox-orchestrator-queue | 305 | 110 | 47 | **99** |
| vox-orchestrator-mcp | 1912 | 108 | 182 | **90** |
| vox-speech | 372 | 96 | 29 | **83** |
| vox-plugin-mens-candle-cuda | 405 | 87 | 34 | **71** |
| vox-db | 1694 | 88 | 237 | **67** |
| vox-rn-codegen | 92 | 73 | 6 | **67** |
| vox-search | 242 | 69 | 28 | **65** |
| vox-plugin-speech | 179 | 66 | 10 | **62** |
| vox-sql | 159 | 73 | 24 | **62** |
| vox-gamify | 744 | 63 | 69 | **57** |
| vox-secrets | 259 | 63 | 33 | **56** |
| vox-effort-route | 153 | 79 | 37 | **54** |
| vox-plugin-mens-candle-metal | 358 | 58 | 26 | **48** |
| vox-package-types | 129 | 49 | 11 | **47** |
| vox-config | 273 | 64 | 63 | **46** |
| vox-orchestrator-types | 142 | 49 | 14 | **46** |
| vox-actor-runtime | 663 | 52 | 103 | **43** |
| vox-effort-audit | 149 | 60 | 48 | **43** |
| vox-plugin-host | 129 | 54 | 16 | **43** |
| vox-plugin-populi-mesh | 307 | 47 | 14 | **43** |
| vox-workflow-runtime | 208 | 53 | 32 | **42** |
| vox-telemetry | 103 | 49 | 20 | **40** |
| vox-plugin-webhook | 144 | 44 | 12 | **38** |
| vox-constrained-gen | 82 | 42 | 13 | **33** |
| vox-mesh-types | 120 | 37 | 20 | **32** |
| vox-tensor | 57 | 34 | 5 | **32** |
| vox-openclaw-runtime | 147 | 31 | 5 | **30** |
| vox-git | 65 | 29 | 11 | **27** |
| vox-identity | 61 | 29 | 3 | **26** |
| vox-package | 84 | 26 | 6 | **26** |
| vox-repository | 166 | 30 | 17 | **25** |
| vox-skills | 110 | 30 | 11 | **24** |
| vox-test-harness | 89 | 25 | 6 | **24** |
| vox-scientia | 564 | 27 | 105 | **23** |
| vox-cli-core | 150 | 25 | 5 | **22** |
| vox-audit | 392 | 26 | 80 | **21** |
| vox-vcs | 100 | 29 | 35 | **21** |
| vox-ast | 243 | 23 | 24 | **20** |
| vox-quantize | 56 | 32 | 18 | **20** |
| vox-rule-pack | 58 | 21 | 19 | **20** |
| vox-lsp | 99 | 24 | 7 | **19** |
| vox-plugin-test-harness | 35 | 18 | 1 | **18** |
| vox-research-events | 554 | 20 | 20 | **18** |
| vox-runtime-rn | 36 | 19 | 4 | **18** |
| vox-arch-check | 89 | 31 | 20 | **17** |
| vox-scaling-policy | 61 | 18 | 3 | **17** |
| vox-grammar-export | 49 | 23 | 10 | **16** |
| vox-research-shim | 280 | 20 | 43 | **16** |
| vox-wasm-engine | 31 | 17 | 4 | **16** |
| vox-capability-registry | 51 | 17 | 3 | **15** |

## Top reached-but-unproven symbols (per worst crate)


### vox-compiler
- `fn_signature()` — crates/vox-compiler/src/app_contract.rs:L86
- `HirParam` — crates/vox-compiler/src/app_contract.rs:L86
- `project_app_contract()` — crates/vox-compiler/src/app_contract.rs:L92
- `.coverage_score()` — crates/vox-compiler/src/ast_eval.rs:L32
- `eval_module()` — crates/vox-compiler/src/ast_eval.rs:L42
- `ast_eval()` — crates/vox-compiler/src/ast_eval.rs:L58
- `count_module_constructs()` — crates/vox-compiler/src/ast_eval.rs:L78
- `builtin_entry_param_tys()` — crates/vox-compiler/src/builtin_registry.rs:L30

### vox-orchestrator
- `.sync_audit_trail()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L33
- `.new()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L42
- `.next_id()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L53
- `MessageId` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L53
- `.send()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L70
- `Into` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L70
- `.broadcast()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L117
- `.inbox()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L191

### vox-code-audit
- `.new()` — crates/vox-code-audit/src/ai_analyze.rs:L89
- `.is_available()` — crates/vox-code-audit/src/ai_analyze.rs:L94
- `.build_prompt()` — crates/vox-code-audit/src/ai_analyze.rs:L99
- `.parse_response()` — crates/vox-code-audit/src/ai_analyze.rs:L151
- `.ollama_request_body()` — crates/vox-code-audit/src/ai_analyze.rs:L204
- `.gemini_request_body()` — crates/vox-code-audit/src/ai_analyze.rs:L218
- `.endpoint_url()` — crates/vox-code-audit/src/ai_analyze.rs:L234
- `.provider_name()` — crates/vox-code-audit/src/ai_analyze.rs:L269

### vox-publisher
- `report_health()` — crates/vox-publisher/src/adapter_health.rs:L32
- `session_cache()` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `Mutex` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `legacy_post()` — crates/vox-publisher/src/adapters/bluesky.rs:L92
- `DiscordOverride` — crates/vox-publisher/src/adapters/discord.rs:L7
- `MastodonOverride` — crates/vox-publisher/src/adapters/mastodon.rs:L9
- `OpenCollectiveConfig` — crates/vox-publisher/src/adapters/opencollective.rs:L10
- `markdown_to_html()` — crates/vox-publisher/src/adapters/opencollective.rs:L79

### vox-codegen
- `.from_bundle_fragment()` — crates/vox-codegen/src/assets/mod.rs:L19
- `.validate_preflight()` — crates/vox-codegen/src/assets/mod.rs:L51
- `.stage_under()` — crates/vox-codegen/src/assets/mod.rs:L67
- `copy_path_recursive()` — crates/vox-codegen/src/assets/mod.rs:L84
- `emit_llm_function_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L7
- `HirFn` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L7
- `emit_search_memory_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L92
- `emit_search_web_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L110

### vox-populi
- `.message_bytes()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L19
- `.sign()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `SigningKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `.verify()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `VerifyingKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `.to_operation_kind()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L65
- `synthetic_weights_hash()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L76
- `.message_bytes()` — crates/vox-populi/src/distributed_training/gradient.rs:L17

### vox-codegen-ts
- `generate_types()` — crates/vox-codegen-ts/src/adt.rs:L4
- `generate_adt()` — crates/vox-codegen-ts/src/adt.rs:L17
- `HirTypeDef` — crates/vox-codegen-ts/src/adt.rs:L17
- `map_type_to_ts()` — crates/vox-codegen-ts/src/adt.rs:L97
- `HirType` — crates/vox-codegen-ts/src/adt.rs:L97
- `.standard()` — crates/vox-codegen-ts/src/builtin_registry.rs:L25
- `.lookup_method()` — crates/vox-codegen-ts/src/builtin_registry.rs:L74
- `.lookup_function()` — crates/vox-codegen-ts/src/builtin_registry.rs:L103

### vox-cli
- `cli_top_level_into_fabrica_or_self()` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `FabricaCmd` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `dispatch_cli()` — crates/vox-cli/src/cli_dispatch/mod.rs:L68
- `GlobalOpts` — crates/vox-cli/src/cli_dispatch/mod.rs:L68
- `dispatch_cli_inner()` — crates/vox-cli/src/cli_dispatch/mod.rs:L87
- `.gate_policy_id()` — crates/vox-cli/src/commands/ci/cmd_enums.rs:L970
- `.label()` — crates/vox-cli/src/commands/ci/cmd_enums.rs:L1118
- `FnMut` — crates/vox-cli/src/commands/ci/db_schema_coverage.rs:L176

### vox-corpus
- `compile_chatml_session()` — crates/vox-corpus/src/arca_replay.rs:L185
- `Value` — crates/vox-corpus/src/arca_replay.rs:L185
- `sanitize_chatml()` — crates/vox-corpus/src/arca_replay.rs:L288
- `generate_mutations()` — crates/vox-corpus/src/ast_mutator.rs:L18
- `Module` — crates/vox-corpus/src/ast_mutator.rs:L18
- `apply_mutations()` — crates/vox-corpus/src/ast_mutator.rs:L57
- `.new()` — crates/vox-corpus/src/codegen_vox/mod.rs:L25
- `.next()` — crates/vox-corpus/src/codegen_vox/mod.rs:L28

### vox-drift-check
- `parse_sev()` — crates/vox-drift-check/src/bin/vox_drift_check.rs:L33
- `.new()` — crates/vox-drift-check/src/cache.rs:L11
- `.from_workspace()` — crates/vox-drift-check/src/cache.rs:L16
- `.hash_file()` — crates/vox-drift-check/src/cache.rs:L20
- `.store()` — crates/vox-drift-check/src/cache.rs:L26
- `ExtractedFeatures` — crates/vox-drift-check/src/cache.rs:L26
- `.load()` — crates/vox-drift-check/src/cache.rs:L36
- `.default()` — crates/vox-drift-check/src/config.rs:L97

### vox-orchestrator-queue
- `.new()` — crates/vox-orchestrator-queue/src/affinity.rs:L62
- `.assign_v()` — crates/vox-orchestrator-queue/src/affinity.rs:L83
- `.lookup_v()` — crates/vox-orchestrator-queue/src/affinity.rs:L120
- `.record_experience()` — crates/vox-orchestrator-queue/src/affinity.rs:L125
- `.best_agent_for()` — crates/vox-orchestrator-queue/src/affinity.rs:L143
- `.assign()` — crates/vox-orchestrator-queue/src/affinity.rs:L176
- `.lookup()` — crates/vox-orchestrator-queue/src/affinity.rs:L181
- `.release()` — crates/vox-orchestrator-queue/src/affinity.rs:L186

### vox-orchestrator-mcp
- `a2a_message_may_surface_to_pilot()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L21
- `A2AMessageType` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L21
- `parse_msg_type()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L140
- `msg_type_wire()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L163
- `fnv1a64()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L167
- `default_idempotency_key()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L178
- `mapped_session_id()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L229
- `AgentSummary` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L229

### vox-speech
- `peak_abs()` — crates/vox-speech/src/acoustic_preprocess.rs:L28
- `effective_mode()` — crates/vox-speech/src/acoustic_preprocess.rs:L32
- `preprocess_audio_pcm_f32_reported()` — crates/vox-speech/src/acoustic_preprocess.rs:L59
- `elapsed_ms()` — crates/vox-speech/src/acoustic_preprocess.rs:L144
- `Instant` — crates/vox-speech/src/acoustic_preprocess.rs:L144
- `IdeContext` — crates/vox-speech/src/ast_mapper.rs:L18
- `merge_bias_phrases()` — crates/vox-speech/src/contextual_bias.rs:L35
- `parse_hotword_csv()` — crates/vox-speech/src/contextual_bias.rs:L59

### vox-plugin-mens-candle-cuda
- `.new()` — crates/vox-plugin-mens-candle-cuda/src/adapter_schema_v3.rs:L59
- `prune_old_checkpoints()` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/checkpoint_mid.rs:L78
- `.path_in()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L70
- `.save()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L78
- `.load()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L93
- `.delete()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L132
- `.now_utc()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L140
- `.default()` — crates/vox-plugin-mens-candle-cuda/src/config.rs:L88

### vox-db
- `.enabled_from_env()` — crates/vox-db/src/circuit_breaker.rs:L72
- `.new()` — crates/vox-db/src/circuit_breaker.rs:L78
- `.from_env()` — crates/vox-db/src/circuit_breaker.rs:L91
- `.call()` — crates/vox-db/src/circuit_breaker.rs:L134
- `F` — crates/vox-db/src/circuit_breaker.rs:L134
- `T` — crates/vox-db/src/circuit_breaker.rs:L134
- `E` — crates/vox-db/src/circuit_breaker.rs:L134
- `.local()` — crates/vox-db/src/config.rs:L90
