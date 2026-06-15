# Reached-but-NOT-Proven — Phase 0 (llvm-cov × proven map)

Annotated 26387 code symbols with llvm-cov `reached` status.

**reached-not-proven** = a symbol whose code EXECUTED during tests but has NO asserted behavior (`proves` edge). This is the precise set line coverage counts as 'covered' but that proves nothing — the keystone signal of this whole initiative.


**Total reached-but-unproven symbols: 5268**


| Crate | Code | Reached | Proven | Reached-not-proven |
|---|---|---|---|---|
| vox-compiler | 1760 | 947 | 437 | **664** |
| vox-orchestrator | 3564 | 783 | 836 | **590** |
| vox-code-audit | 1529 | 758 | 362 | **553** |
| vox-publisher | 1219 | 487 | 159 | **386** |
| vox-codegen | 672 | 426 | 135 | **318** |
| vox-populi | 1462 | 383 | 209 | **304** |
| vox-codegen-ts | 403 | 205 | 57 | **165** |
| vox-cli | 3463 | 219 | 663 | **151** |
| vox-corpus | 431 | 149 | 35 | **122** |
| vox-drift-check | 279 | 151 | 53 | **110** |
| vox-speech | 372 | 96 | 32 | **81** |
| vox-orchestrator-mcp | 1912 | 108 | 199 | **80** |
| vox-orchestrator-queue | 305 | 110 | 82 | **79** |
| vox-plugin-mens-candle-cuda | 405 | 87 | 35 | **70** |
| vox-rn-codegen | 92 | 73 | 9 | **64** |
| vox-search | 242 | 69 | 36 | **63** |
| vox-db | 1694 | 88 | 398 | **61** |
| vox-plugin-speech | 179 | 66 | 11 | **61** |
| vox-sql | 159 | 73 | 30 | **56** |
| vox-gamify | 744 | 63 | 116 | **48** |
| vox-effort-route | 153 | 79 | 44 | **47** |
| vox-plugin-mens-candle-metal | 358 | 58 | 27 | **47** |
| vox-secrets | 259 | 63 | 58 | **47** |
| vox-config | 273 | 64 | 75 | **43** |
| vox-plugin-populi-mesh | 307 | 47 | 25 | **42** |
| vox-effort-audit | 149 | 60 | 52 | **41** |
| vox-actor-runtime | 663 | 52 | 133 | **40** |
| vox-workflow-runtime | 208 | 53 | 51 | **39** |
| vox-orchestrator-types | 142 | 49 | 24 | **38** |
| vox-plugin-host | 129 | 54 | 23 | **36** |
| vox-package-types | 129 | 49 | 25 | **34** |
| vox-constrained-gen | 82 | 42 | 14 | **33** |
| vox-plugin-webhook | 144 | 44 | 19 | **33** |
| vox-telemetry | 103 | 49 | 38 | **29** |
| vox-mesh-types | 120 | 37 | 35 | **28** |
| vox-tensor | 57 | 34 | 9 | **28** |
| vox-openclaw-runtime | 147 | 31 | 18 | **26** |
| vox-repository | 166 | 30 | 17 | **25** |
| vox-identity | 61 | 29 | 8 | **23** |
| vox-audit | 392 | 26 | 90 | **21** |
| vox-vcs | 100 | 29 | 38 | **20** |
| vox-scientia | 564 | 27 | 123 | **19** |
| vox-package | 84 | 26 | 15 | **18** |
| vox-skills | 110 | 30 | 21 | **17** |
| vox-test-harness | 89 | 25 | 15 | **17** |
| vox-arch-check | 89 | 31 | 21 | **16** |
| vox-git | 65 | 29 | 22 | **16** |
| vox-grammar-export | 49 | 23 | 11 | **16** |
| vox-lsp | 99 | 24 | 13 | **16** |
| vox-rule-pack | 58 | 21 | 23 | **16** |
| vox-ast | 243 | 23 | 60 | **15** |
| vox-plugin-test-harness | 35 | 18 | 4 | **15** |
| vox-quantize | 56 | 32 | 23 | **15** |
| vox-research-events | 554 | 20 | 23 | **15** |
| vox-research-shim | 280 | 20 | 47 | **15** |
| vox-capability-registry | 51 | 17 | 4 | **14** |
| vox-cli-core | 150 | 25 | 18 | **14** |
| vox-cli-tests | 19 | 15 | 3 | **14** |
| vox-scaling-policy | 61 | 18 | 12 | **14** |
| vox-hf-layout | 26 | 17 | 7 | **13** |

## Top reached-but-unproven symbols (per worst crate)


### vox-compiler
- `HirParam` — crates/vox-compiler/src/app_contract.rs:L86
- `project_app_contract()` — crates/vox-compiler/src/app_contract.rs:L92
- `eval_module()` — crates/vox-compiler/src/ast_eval.rs:L42
- `ast_eval()` — crates/vox-compiler/src/ast_eval.rs:L58
- `count_module_constructs()` — crates/vox-compiler/src/ast_eval.rs:L78
- `builtin_entry_param_tys()` — crates/vox-compiler/src/builtin_registry.rs:L30
- `Ty` — crates/vox-compiler/src/builtin_registry.rs:L30
- `builtin_entry_result_ty()` — crates/vox-compiler/src/builtin_registry.rs:L58

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
- `.build_prompt()` — crates/vox-code-audit/src/ai_analyze.rs:L99
- `.parse_response()` — crates/vox-code-audit/src/ai_analyze.rs:L151
- `.ollama_request_body()` — crates/vox-code-audit/src/ai_analyze.rs:L204
- `.gemini_request_body()` — crates/vox-code-audit/src/ai_analyze.rs:L218
- `.parse()` — crates/vox-code-audit/src/analysis/rust_context.rs:L14
- `.from_rust_source()` — crates/vox-code-audit/src/analysis/token_map.rs:L21
- `merge_spans_kind()` — crates/vox-code-audit/src/analysis/token_map.rs:L118

### vox-publisher
- `session_cache()` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `Mutex` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `legacy_post()` — crates/vox-publisher/src/adapters/bluesky.rs:L92
- `DiscordOverride` — crates/vox-publisher/src/adapters/discord.rs:L7
- `MastodonOverride` — crates/vox-publisher/src/adapters/mastodon.rs:L9
- `OpenCollectiveConfig` — crates/vox-publisher/src/adapters/opencollective.rs:L10
- `markdown_to_html()` — crates/vox-publisher/src/adapters/opencollective.rs:L79
- `refresh_access_token()` — crates/vox-publisher/src/adapters/reddit.rs:L42

### vox-codegen
- `.from_bundle_fragment()` — crates/vox-codegen/src/assets/mod.rs:L19
- `emit_llm_function_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L7
- `HirFn` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L7
- `emit_search_memory_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L92
- `emit_search_web_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L110
- `emit_search_docs_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L142
- `emit_subagent_body()` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L170
- `HirSubagentFixture` — crates/vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs:L170

### vox-populi
- `.message_bytes()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L19
- `.sign()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `SigningKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `.verify()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `VerifyingKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `synthetic_weights_hash()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L76
- `.message_bytes()` — crates/vox-populi/src/distributed_training/gradient.rs:L17
- `.sign()` — crates/vox-populi/src/distributed_training/gradient.rs:L32

### vox-codegen-ts
- `generate_types()` — crates/vox-codegen-ts/src/adt.rs:L4
- `generate_adt()` — crates/vox-codegen-ts/src/adt.rs:L17
- `HirTypeDef` — crates/vox-codegen-ts/src/adt.rs:L17
- `map_type_to_ts()` — crates/vox-codegen-ts/src/adt.rs:L97
- `HirType` — crates/vox-codegen-ts/src/adt.rs:L97
- `.standard()` — crates/vox-codegen-ts/src/builtin_registry.rs:L25
- `.lookup_method()` — crates/vox-codegen-ts/src/builtin_registry.rs:L74
- `no_emit_entry_from_env()` — crates/vox-codegen-ts/src/emitter.rs:L88

### vox-cli
- `cli_top_level_into_fabrica_or_self()` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `FabricaCmd` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `dispatch_cli()` — crates/vox-cli/src/cli_dispatch/mod.rs:L68
- `GlobalOpts` — crates/vox-cli/src/cli_dispatch/mod.rs:L68
- `dispatch_cli_inner()` — crates/vox-cli/src/cli_dispatch/mod.rs:L87
- `.label()` — crates/vox-cli/src/commands/ci/cmd_enums.rs:L1118
- `FnMut` — crates/vox-cli/src/commands/ci/db_schema_coverage.rs:L176
- `crate_of()` — crates/vox-cli/src/commands/ci/db_schema_coverage.rs:L200

### vox-corpus
- `Value` — crates/vox-corpus/src/arca_replay.rs:L185
- `generate_mutations()` — crates/vox-corpus/src/ast_mutator.rs:L18
- `Module` — crates/vox-corpus/src/ast_mutator.rs:L18
- `.new()` — crates/vox-corpus/src/codegen_vox/mod.rs:L25
- `.next()` — crates/vox-corpus/src/codegen_vox/mod.rs:L28
- `.usize()` — crates/vox-corpus/src/codegen_vox/mod.rs:L34
- `.to_jsonl()` — crates/vox-corpus/src/codegen_vox/part_02.rs:L82
- `gen_full_stack_program()` — crates/vox-corpus/src/codegen_vox/part_03.rs:L4

### vox-drift-check
- `.new()` — crates/vox-drift-check/src/cache.rs:L11
- `.from_workspace()` — crates/vox-drift-check/src/cache.rs:L16
- `.hash_file()` — crates/vox-drift-check/src/cache.rs:L20
- `ExtractedFeatures` — crates/vox-drift-check/src/cache.rs:L26
- `.load()` — crates/vox-drift-check/src/cache.rs:L36
- `.default()` — crates/vox-drift-check/src/config.rs:L97
- `.default()` — crates/vox-drift-check/src/config.rs:L108
- `.default()` — crates/vox-drift-check/src/config.rs:L117

### vox-speech
- `peak_abs()` — crates/vox-speech/src/acoustic_preprocess.rs:L28
- `effective_mode()` — crates/vox-speech/src/acoustic_preprocess.rs:L32
- `preprocess_audio_pcm_f32_reported()` — crates/vox-speech/src/acoustic_preprocess.rs:L59
- `elapsed_ms()` — crates/vox-speech/src/acoustic_preprocess.rs:L144
- `Instant` — crates/vox-speech/src/acoustic_preprocess.rs:L144
- `IdeContext` — crates/vox-speech/src/ast_mapper.rs:L18
- `merge_bias_phrases()` — crates/vox-speech/src/contextual_bias.rs:L35
- `parse_hotword_csv()` — crates/vox-speech/src/contextual_bias.rs:L59

### vox-orchestrator-mcp
- `mapped_session_id()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L229
- `AgentSummary` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L229
- `normalize_sender_session_id()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L237
- `resolve_sender_binding()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L243
- `extend_binding_fields()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L270
- `Map` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L270
- `Value` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L270
- `a2a_send()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L300

### vox-orchestrator-queue
- `.new()` — crates/vox-orchestrator-queue/src/affinity.rs:L62
- `.assign_v()` — crates/vox-orchestrator-queue/src/affinity.rs:L83
- `.record_experience()` — crates/vox-orchestrator-queue/src/affinity.rs:L125
- `.assign()` — crates/vox-orchestrator-queue/src/affinity.rs:L176
- `.lookup()` — crates/vox-orchestrator-queue/src/affinity.rs:L181
- `.release()` — crates/vox-orchestrator-queue/src/affinity.rs:L186
- `.release_all()` — crates/vox-orchestrator-queue/src/affinity.rs:L191
- `.owner_or_assign()` — crates/vox-orchestrator-queue/src/affinity.rs:L197

### vox-plugin-mens-candle-cuda
- `.new()` — crates/vox-plugin-mens-candle-cuda/src/adapter_schema_v3.rs:L59
- `prune_old_checkpoints()` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/checkpoint_mid.rs:L78
- `.path_in()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L70
- `.load()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L93
- `.delete()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L132
- `.now_utc()` — crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:L140
- `.default()` — crates/vox-plugin-mens-candle-cuda/src/config.rs:L88
- `.default()` — crates/vox-plugin-mens-candle-cuda/src/config.rs:L192

### vox-rn-codegen
- `class_string_to_style_key()` — crates/vox-rn-codegen/src/component.rs:L101
- `tailwind_tokens_to_rn_props()` — crates/vox-rn-codegen/src/component.rs:L149
- `resolve_tw_color()` — crates/vox-rn-codegen/src/component.rs:L166
- `kwargs_to_rn_props()` — crates/vox-rn-codegen/src/component.rs:L278
- `HirJsxAttr` — crates/vox-rn-codegen/src/component.rs:L278
- `rn_props_to_object_body()` — crates/vox-rn-codegen/src/component.rs:L344
- `emit_hir_expr_inline_with_state()` — crates/vox-rn-codegen/src/component.rs:L389
- `HashSet` — crates/vox-rn-codegen/src/component.rs:L389
