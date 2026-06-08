# Reached-but-NOT-Proven — Phase 0 (llvm-cov × proven map)

Annotated 39863 code symbols with llvm-cov `reached` status.

**reached-not-proven** = a symbol whose code EXECUTED during tests but has NO asserted behavior (`proves` edge). This is the precise set line coverage counts as 'covered' but that proves nothing — the keystone signal of this whole initiative.


**Total reached-but-unproven symbols: 16940**


| Crate | Code | Reached | Proven | Reached-not-proven |
|---|---|---|---|---|
| vox-orchestrator | 5241 | 2757 | 677 | **2447** |
| vox-compiler | 3178 | 1997 | 436 | **1694** |
| vox-cli | 5560 | 1441 | 680 | **1212** |
| vox-code-audit | 2293 | 1297 | 355 | **1095** |
| vox-db | 2399 | 1128 | 241 | **1011** |
| vox-codegen | 1557 | 1112 | 118 | **1006** |
| vox-orchestrator-mcp | 2368 | 811 | 161 | **731** |
| vox-publisher | 1643 | 753 | 104 | **687** |
| vox-populi | 1788 | 592 | 116 | **547** |
| vox-scientia | 1003 | 542 | 104 | **489** |
| vox-gamify | 1061 | 513 | 59 | **483** |
| vox-actor-runtime | 949 | 359 | 97 | **316** |
| vox-integration-tests | 467 | 328 | 35 | **303** |
| vox-corpus | 681 | 275 | 29 | **254** |
| vox-audit | 590 | 269 | 75 | **238** |
| vox-drift-check | 412 | 248 | 53 | **212** |
| vox-orchestrator-queue | 452 | 219 | 48 | **195** |
| vox-research-shim | 418 | 223 | 43 | **195** |
| vox-search | 380 | 204 | 28 | **188** |
| vox-repository | 266 | 172 | 16 | **158** |
| vox-speech | 537 | 170 | 29 | **157** |
| vox-secrets | 331 | 170 | 28 | **154** |
| vox-config | 368 | 162 | 42 | **149** |
| vox-workflow-runtime | 331 | 160 | 28 | **145** |
| vox-effort-audit | 247 | 161 | 55 | **132** |
| vox-effort-route | 258 | 155 | 38 | **130** |
| vox-inference | 299 | 137 | 39 | **106** |
| vox-plugin-mens-candle-cuda | 508 | 115 | 27 | **103** |
| vox-telemetry | 181 | 111 | 21 | **101** |
| vox-plugin-mens-candle-metal | 492 | 107 | 26 | **97** |
| vox-plugin-host | 240 | 104 | 18 | **92** |
| vox-orchestrator-types | 205 | 94 | 14 | **91** |
| vox-package-types | 195 | 92 | 11 | **90** |
| vox-lsp | 152 | 93 | 6 | **88** |
| vox-mesh-types | 222 | 98 | 28 | **85** |
| vox-constrained-gen | 123 | 75 | 13 | **66** |
| vox-plugin-webhook | 191 | 71 | 12 | **65** |
| vox-skills | 181 | 75 | 15 | **65** |
| vox-tensor | 97 | 62 | 5 | **60** |
| vox-vcs | 149 | 69 | 35 | **60** |
| vox-cli-tests | 77 | 58 | 2 | **58** |
| vox-container-types | 102 | 59 | 18 | **53** |
| vox-ml-cli | 654 | 55 | 31 | **53** |
| vox-arch-check | 172 | 58 | 21 | **51** |
| vox-package | 125 | 51 | 6 | **51** |
| vox-quantize | 96 | 62 | 18 | **50** |
| vox-distributed-training | 105 | 52 | 4 | **49** |
| vox-plugin-populi-mesh | 371 | 53 | 14 | **49** |
| vox-grammar-export | 90 | 53 | 10 | **46** |
| vox-test-harness | 139 | 45 | 7 | **44** |
| vox-git | 99 | 48 | 11 | **43** |
| vox-rule-pack | 90 | 41 | 19 | **40** |
| vox-gui | 886 | 42 | 45 | **39** |
| vox-openclaw-runtime | 193 | 40 | 5 | **39** |
| vox-runtime | 62 | 44 | 14 | **39** |
| vox-plugin-catalog | 79 | 42 | 14 | **38** |
| vox-plugin-speech | 232 | 37 | 10 | **36** |
| vox-wasm-engine | 67 | 40 | 7 | **36** |
| vox-capability-registry | 77 | 37 | 3 | **35** |
| vox-runtime-rn | 56 | 36 | 4 | **35** |

## Top reached-but-unproven symbols (per worst crate)


### vox-orchestrator
- `.sync_audit_trail()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L33
- `.new()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L42
- `Self` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L42
- `.next_id()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L53
- `MessageId` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L53
- `.register_agent()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L58
- `.send()` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L70
- `Into` — crates/vox-orchestrator/src/a2a/bus/message_bus.rs:L70

### vox-compiler
- `fn_signature()` — crates/vox-compiler/src/app_contract.rs:L86
- `HirParam` — crates/vox-compiler/src/app_contract.rs:L86
- `project_app_contract()` — crates/vox-compiler/src/app_contract.rs:L92
- `Result` — crates/vox-compiler/src/app_contract.rs:L190
- `Error` — crates/vox-compiler/src/app_contract.rs:L190
- `project_app_contract_endpoint_lists_characterization()` — crates/vox-compiler/src/app_contract.rs:L219
- `project_app_contract_mutation_no_tables_no_tx()` — crates/vox-compiler/src/app_contract.rs:L270
- `project_app_contract_interleaved_kinds_preserve_identity()` — crates/vox-compiler/src/app_contract.rs:L289

### vox-cli
- `desktop_target_and_trailing_file()` — crates/vox-cli/src/cli_args.rs:L300
- `cli_top_level_into_fabrica_or_self()` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `FabricaCmd` — crates/vox-cli/src/cli_dispatch/lanes.rs:L120
- `fabrica_reward_events_maps_known_commands()` — crates/vox-cli/src/cli_dispatch/lanes.rs:L343
- `fabrica_reward_events_skips_unrewarded_lanes()` — crates/vox-cli/src/cli_dispatch/lanes.rs:L372
- `Option` — crates/vox-cli/src/cli_dispatch/mod.rs:L31
- `dispatch_cli()` — crates/vox-cli/src/cli_dispatch/mod.rs:L68
- `GlobalOpts` — crates/vox-cli/src/cli_dispatch/mod.rs:L68

### vox-code-audit
- `.new()` — crates/vox-code-audit/src/ai_analyze.rs:L89
- `Self` — crates/vox-code-audit/src/ai_analyze.rs:L89
- `.is_available()` — crates/vox-code-audit/src/ai_analyze.rs:L94
- `.build_prompt()` — crates/vox-code-audit/src/ai_analyze.rs:L99
- `.parse_response()` — crates/vox-code-audit/src/ai_analyze.rs:L151
- `Vec` — crates/vox-code-audit/src/ai_analyze.rs:L151
- `.endpoint_url()` — crates/vox-code-audit/src/ai_analyze.rs:L234
- `Option` — crates/vox-code-audit/src/ai_analyze.rs:L234

### vox-db
- `.is_auto_safe()` — crates/vox-db/src/auto_migrate.rs:L86
- `.to_sql()` — crates/vox-db/src/auto_migrate.rs:L97
- `.describe()` — crates/vox-db/src/auto_migrate.rs:L116
- `.is_empty()` — crates/vox-db/src/auto_migrate.rs:L147
- `.auto_actions()` — crates/vox-db/src/auto_migrate.rs:L152
- `.describe()` — crates/vox-db/src/auto_migrate.rs:L162
- `.new()` — crates/vox-db/src/auto_migrate.rs:L188
- `Self` — crates/vox-db/src/auto_migrate.rs:L188

### vox-codegen
- `.from_bundle_fragment()` — crates/vox-codegen/src/assets/mod.rs:L19
- `Path` — crates/vox-codegen/src/assets/mod.rs:L19
- `Option` — crates/vox-codegen/src/assets/mod.rs:L19
- `String` — crates/vox-codegen/src/assets/mod.rs:L19
- `Self` — crates/vox-codegen/src/assets/mod.rs:L19
- `.validate_preflight()` — crates/vox-codegen/src/assets/mod.rs:L51
- `Result` — crates/vox-codegen/src/assets/mod.rs:L51
- `.stage_under()` — crates/vox-codegen/src/assets/mod.rs:L67

### vox-orchestrator-mcp
- `a2a_message_may_surface_to_pilot()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L21
- `A2AMessageType` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L21
- `parse_msg_type()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L140
- `msg_type_wire()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L163
- `fnv1a64()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L167
- `default_idempotency_key()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L178
- `Result` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L185
- `mapped_session_id()` — crates/vox-orchestrator-mcp/src/a2a_tools.rs:L229

### vox-publisher
- `session_cache()` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `Mutex` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `HashMap` — crates/vox-publisher/src/adapters/bluesky.rs:L69
- `Result` — crates/vox-publisher/src/adapters/bluesky.rs:L73
- `legacy_post()` — crates/vox-publisher/src/adapters/bluesky.rs:L92
- `Option` — crates/vox-publisher/src/adapters/discord.rs:L7
- `DiscordOverride` — crates/vox-publisher/src/adapters/discord.rs:L7
- `Result` — crates/vox-publisher/src/adapters/discord.rs:L7

### vox-populi
- `.message_bytes()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L19
- `Vec` — crates/vox-populi/src/distributed_training/checkpoint.rs:L19
- `.sign()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `SigningKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `Self` — crates/vox-populi/src/distributed_training/checkpoint.rs:L34
- `.verify()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `VerifyingKey` — crates/vox-populi/src/distributed_training/checkpoint.rs:L53
- `.to_operation_kind()` — crates/vox-populi/src/distributed_training/checkpoint.rs:L65

### vox-scientia
- `.default()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L9
- `Self` — crates/vox-scientia/src/claim_extractor/atomic.rs:L9
- `.new()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L22
- `.decompose()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L26
- `Vec` — crates/vox-scientia/src/claim_extractor/atomic.rs:L26
- `extract_tuple()` — crates/vox-scientia/src/claim_extractor/atomic.rs:L77
- `Option` — crates/vox-scientia/src/claim_extractor/atomic.rs:L77
- `SciClaimTuple` — crates/vox-scientia/src/claim_extractor/atomic.rs:L77

### vox-gamify
- `.scaled()` — crates/vox-gamify/src/ability.rs:L76
- `Self` — crates/vox-gamify/src/ability.rs:L76
- `default_abilities()` — crates/vox-gamify/src/ability.rs:L88
- `Vec` — crates/vox-gamify/src/ability.rs:L88
- `first_ability_always_unlocked()` — crates/vox-gamify/src/ability.rs:L236
- `scaling_affects_damage()` — crates/vox-gamify/src/ability.rs:L253
- `doubt_achievements()` — crates/vox-gamify/src/achievement/defaults/doubt.rs:L3
- `Vec` — crates/vox-gamify/src/achievement/defaults/doubt.rs:L3

### vox-actor-runtime
- `emit_sandbox_timeout_kill()` — crates/vox-actor-runtime/src/activity.rs:L14
- `.default()` — crates/vox-actor-runtime/src/activity.rs:L58
- `Self` — crates/vox-actor-runtime/src/activity.rs:L58
- `.new()` — crates/vox-actor-runtime/src/activity.rs:L72
- `.with_retries()` — crates/vox-actor-runtime/src/activity.rs:L77
- `.with_timeout()` — crates/vox-actor-runtime/src/activity.rs:L83
- `.with_timeout_secs()` — crates/vox-actor-runtime/src/activity.rs:L89
- `.with_initial_backoff()` — crates/vox-actor-runtime/src/activity.rs:L95

### vox-integration-tests
- `test_a2a_mcp_roundtrip()` — crates/vox-integration-tests/tests/a2a_mcp_test.rs:L7
- `Counter_add()` — crates/vox-integration-tests/tests/actor_dispatch_e2e_test.rs:L25
- `Counter_get()` — crates/vox-integration-tests/tests/actor_dispatch_e2e_test.rs:L28
- `emitted_actor_dispatch_delivers_mutates_and_replies()` — crates/vox-integration-tests/tests/actor_dispatch_e2e_test.rs:L33
- `test_infinite_loop_actor_yields_to_scheduler()` — crates/vox-integration-tests/tests/actor_gc_sandbox_test.rs:L8
- `test_agent_mcp_roundtrip()` — crates/vox-integration-tests/tests/agent_mcp_roundtrip_test.rs:L7
- `workspace_root()` — crates/vox-integration-tests/tests/agentos_aci_bench.rs:L12
- `PathBuf` — crates/vox-integration-tests/tests/agentos_aci_bench.rs:L12

### vox-corpus
- `.new()` — crates/vox-corpus/src/codegen_vox/mod.rs:L25
- `Self` — crates/vox-corpus/src/codegen_vox/mod.rs:L25
- `.next()` — crates/vox-corpus/src/codegen_vox/mod.rs:L28
- `.usize()` — crates/vox-corpus/src/codegen_vox/mod.rs:L34
- `.to_jsonl()` — crates/vox-corpus/src/codegen_vox/part_02.rs:L82
- `gen_full_stack_program()` — crates/vox-corpus/src/codegen_vox/part_03.rs:L4
- `Rng` — crates/vox-corpus/src/codegen_vox/part_03.rs:L4
- `OrganicPair` — crates/vox-corpus/src/codegen_vox/part_03.rs:L4

### vox-audit
- `.into_diagnostic_rollup()` — crates/vox-audit/src/aggregator.rs:L109
- `.observe()` — crates/vox-audit/src/aggregator.rs:L138
- `TelemetryEvent` — crates/vox-audit/src/aggregator.rs:L138
- `.finalize_outcomes()` — crates/vox-audit/src/aggregator.rs:L175
- `.top_n_diagnostics()` — crates/vox-audit/src/aggregator.rs:L183
- `event_repository_key()` — crates/vox-audit/src/aggregator.rs:L202
- `lint()` — crates/vox-audit/src/aggregator.rs:L293
- `autofix()` — crates/vox-audit/src/aggregator.rs:L306
