# Duplicate Code / Split-Brain / Wiring — deterministic scan

Scanned 3271 Rust files (0 unreadable).

- **Exact body duplicates:** 435 clusters (identical normalized body, >= 120 chars)
- **Structural near-duplicates:** 21 clusters (same shape, renamed identifiers, >=3 sites, >= 240 chars)
- **Split-brain name candidates:** 270 names defined in >=4 crates
- **Zero-inbound-edge nodes (ADVISORY/noisy):** 3607 — cross-check with vox-arch-check


## Exact body duplicates (top 40)


**2× `preflight_native_qlora` (~5530 chars)**
  - `preflight_native_qlora` — crates/vox-plugin-mens-candle-cuda/src/qlora_preflight.rs:321
  - `preflight_native_qlora` — crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs:321

**2× `finalize_training_run` (~4641 chars)**
  - `finalize_training_run` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/finalize.rs:64
  - `finalize_training_run` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/finalize.rs:64

**2× `deliver_a2a` (~4416 chars)**
  - `deliver_a2a` — crates/vox-plugin-populi-mesh/src/transport/handlers/a2a.rs:120
  - `deliver_a2a` — crates/vox-populi/src/transport/handlers/a2a.rs:120

**2× `apply_timestamp_rules` (~3888 chars)**
  - `apply_timestamp_rules` — crates/vox-plugin-speech/src/backends/candle_engine.rs:370
  - `apply_timestamp_rules` — crates/vox-speech/src/backends/candle_engine.rs:370

**2× `populi_http_app_with_auth` (~3746 chars)**
  - `populi_http_app_with_auth` — crates/vox-plugin-populi-mesh/src/transport/router.rs:101
  - `populi_http_app_with_auth` — crates/vox-populi/src/transport/router.rs:106

**2× `a2a_inbox` (~3366 chars)**
  - `a2a_inbox` — crates/vox-plugin-populi-mesh/src/transport/handlers/a2a.rs:431
  - `a2a_inbox` — crates/vox-populi/src/transport/handlers/a2a.rs:431

**2× `decode` (~3335 chars)**
  - `decode` — crates/vox-plugin-speech/src/backends/candle_engine.rs:186
  - `decode` — crates/vox-speech/src/backends/candle_engine.rs:186

**2× `spawn_training_db_writer` (~3095 chars)**
  - `spawn_training_db_writer` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/db_thread.rs:8
  - `spawn_training_db_writer` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/db_thread.rs:8

**2× `run_validation_pass` (~2879 chars)**
  - `run_validation_pass` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/training_loop/validation.rs:108
  - `run_validation_pass` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/training_loop/validation.rs:108

**2× `all_row_types_implement_serde` (~2871 chars)**
  - `all_row_types_implement_serde` — crates/vox-db-types/tests/serde_uniformity.rs:8
  - `all_row_types_implement_serde` — crates/vox-db/tests/serde_uniformity.rs:9

**2× `merge_qlora_into_base_subset` (~2736 chars)**
  - `merge_qlora_into_base_subset` — crates/vox-plugin-mens-candle-cuda/src/merge.rs:102
  - `merge_qlora_into_base_subset` — crates/vox-plugin-mens-candle-metal/src/merge.rs:102

**2× `merge_v2_applies_lm_head_delta` (~2654 chars)**
  - `merge_v2_applies_lm_head_delta` — crates/vox-plugin-mens-candle-cuda/src/merge.rs:255
  - `merge_v2_applies_lm_head_delta` — crates/vox-plugin-mens-candle-metal/src/merge.rs:255

**2× `start_federation_gossip` (~2601 chars)**
  - `start_federation_gossip` — crates/vox-plugin-populi-mesh/src/transport/mod.rs:522
  - `start_federation_gossip` — crates/vox-populi/src/transport/mod.rs:675

**3× `build_dir` (~2480 chars)**
  - `build_dir` — crates/vox-inference/src/backends/candle_cpu.rs:174
  - `build_dir` — crates/vox-inference/src/backends/candle_cuda.rs:167
  - `build_dir` — crates/vox-inference/src/backends/candle_metal.rs:172

**2× `pcm_decode` (~2316 chars)**
  - `pcm_decode` — crates/vox-plugin-speech/src/backends/audio_io.rs:16
  - `pcm_decode` — crates/vox-speech/src/backends/audio_io.rs:16

**2× `forward_masked_ce` (~2249 chars)**
  - `forward_masked_ce` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/training_loop/forward.rs:10
  - `forward_masked_ce` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/training_loop/forward.rs:10

**2× `merge_file` (~2219 chars)**
  - `merge_file` — crates/vox-plugin-speech/src/oratio_internals/runtime_config.rs:230
  - `merge_file` — crates/vox-speech/src/runtime_config.rs:230

**2× `build_logit_processor` (~2172 chars)**
  - `build_logit_processor` — crates/vox-plugin-speech/src/backends/logit_processors.rs:321
  - `build_logit_processor` — crates/vox-speech/src/backends/logit_processors.rs:330

**2× `apply_checkpoint_resume` (~2048 chars)**
  - `apply_checkpoint_resume` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/training_loop/checkpoint.rs:14
  - `apply_checkpoint_resume` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/training_loop/checkpoint.rs:14

**2× `validate_qwen35_linear_shapes` (~2038 chars)**
  - `validate_qwen35_linear_shapes` — crates/vox-plugin-mens-candle-cuda/src/qlora_preflight.rs:243
  - `validate_qwen35_linear_shapes` — crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs:243

**2× `exec_lease_grant` (~2034 chars)**
  - `exec_lease_grant` — crates/vox-plugin-populi-mesh/src/transport/handlers/leases.rs:22
  - `exec_lease_grant` — crates/vox-populi/src/transport/handlers/leases.rs:22

**2× `a2a_ack` (~1985 chars)**
  - `a2a_ack` — crates/vox-plugin-populi-mesh/src/transport/handlers/a2a.rs:567
  - `a2a_ack` — crates/vox-populi/src/transport/handlers/a2a.rs:567

**2× `emit_durable_boot_prelude` (~1872 chars)**
  - `emit_durable_boot_prelude` — crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:173
  - `main` — crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:160

**2× `run_streaming` (~1719 chars)**
  - `run_streaming` — crates/vox-plugin-speech/src/backends/candle_engine.rs:589
  - `run_streaming` — crates/vox-speech/src/backends/candle_engine.rs:589

**2× `preprocess_audio_pcm_f32_reported` (~1696 chars)**
  - `preprocess_audio_pcm_f32_reported` — crates/vox-plugin-speech/src/oratio_internals/acoustic_preprocess.rs:59
  - `preprocess_audio_pcm_f32_reported` — crates/vox-speech/src/acoustic_preprocess.rs:59

**2× `try_encode_training_step` (~1657 chars)**
  - `try_encode_training_step` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/training_loop/encoding.rs:12
  - `try_encode_training_step` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/training_loop/encoding.rs:12

**2× `a2a_lease_renew` (~1634 chars)**
  - `a2a_lease_renew` — crates/vox-plugin-populi-mesh/src/transport/handlers/a2a.rs:365
  - `a2a_lease_renew` — crates/vox-populi/src/transport/handlers/a2a.rs:365

**2× `finish_epoch` (~1603 chars)**
  - `finish_epoch` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/epoch_boundary.rs:14
  - `finish_epoch` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/epoch_boundary.rs:14

**2× `enforce_deliver_attestation` (~1541 chars)**
  - `enforce_deliver_attestation` — crates/vox-plugin-populi-mesh/src/transport/result_attestation.rs:21
  - `enforce_deliver_attestation` — crates/vox-populi/src/transport/result_attestation.rs:49

**2× `apply_rotary_emb` (~1523 chars)**
  - `apply_rotary_emb` — crates/vox-plugin-mens-candle-cuda/src/model.rs:179
  - `apply_rotary_emb` — crates/vox-plugin-mens-candle-metal/src/model.rs:158

**2× `build_train_step_payload` (~1458 chars)**
  - `build_train_step_payload` — crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/training_loop/telem_helpers.rs:10
  - `build_train_step_payload` — crates/vox-plugin-mens-candle-metal/src/candle_qlora_train/training_loop/telem_helpers.rs:10

**2× `new` (~1413 chars)**
  - `new` — crates/vox-plugin-speech/src/backends/candle_engine.rs:126
  - `new` — crates/vox-speech/src/backends/candle_engine.rs:126

**2× `federation_announce` (~1393 chars)**
  - `federation_announce` — crates/vox-plugin-populi-mesh/src/transport/handlers/federation.rs:30
  - `federation_announce` — crates/vox-populi/src/transport/handlers/federation.rs:30

**2× `router` (~1347 chars)**
  - `router` — crates/vox-plugin-populi-mesh/src/transport/router.rs:64
  - `router` — crates/vox-populi/src/transport/router.rs:69

**2× `claim_policy_allows_worker` (~1279 chars)**
  - `claim_policy_allows_worker` — crates/vox-plugin-populi-mesh/src/transport/handlers/a2a.rs:28
  - `claim_policy_allows_worker` — crates/vox-populi/src/transport/handlers/a2a.rs:28

**2× `merge_optional_node_fields` (~1269 chars)**
  - `merge_optional_node_fields` — crates/vox-plugin-populi-mesh/src/transport/handlers/nodes.rs:132
  - `merge_optional_node_fields` — crates/vox-populi/src/transport/handlers/nodes.rs:166

**2× `load_session` (~1269 chars)**
  - `load_session` — crates/vox-plugin-speech/src/backends/candle_whisper.rs:138
  - `load_session` — crates/vox-speech/src/backends/candle_whisper.rs:144

**2× `default` (~1255 chars)**
  - `default` — crates/vox-plugin-mens-candle-cuda/src/config.rs:185
  - `default` — crates/vox-populi/src/mens/tensor/training_config.rs:180

**2× `bootstrap_exchange_round_trip` (~1253 chars)**
  - `bootstrap_exchange_round_trip` — crates/vox-plugin-populi-mesh/src/transport/router.rs:261
  - `bootstrap_exchange_round_trip` — crates/vox-populi/src/transport/router.rs:296

**2× `decode_with_fallback` (~1242 chars)**
  - `decode_with_fallback` — crates/vox-plugin-speech/src/backends/candle_engine.rs:314
  - `decode_with_fallback` — crates/vox-speech/src/backends/candle_engine.rs:314

## Structural near-duplicates (top 30)


**5 sites, names: docs_mirror_community_update_template_matches_crate_template, docs_mirror_discord_announcement_template_matches_crate_template, docs_mirror_release_template_matches_crate_template, docs_mirror_research_template_matches_crate_template, docs_mirror_security_advisory_template_matches_crate_template**
  - `docs_mirror_community_update_template_matches_crate_template` — crates/vox-publisher/src/templates.rs:166
  - `docs_mirror_discord_announcement_template_matches_crate_template` — crates/vox-publisher/src/templates.rs:180
  - `docs_mirror_release_template_matches_crate_template` — crates/vox-publisher/src/templates.rs:138
  - `docs_mirror_research_template_matches_crate_template` — crates/vox-publisher/src/templates.rs:124
  - `docs_mirror_security_advisory_template_matches_crate_template` — crates/vox-publisher/src/templates.rs:152

**5 sites, names: create_deposition_draft, get_deposition, get_note, post_note_edit, publish_deposition**
  - `create_deposition_draft` — crates/vox-publisher/src/scholarly/zenodo.rs:130
  - `get_deposition` — crates/vox-publisher/src/scholarly/zenodo.rs:165
  - `get_note` — crates/vox-publisher/src/scholarly/openreview.rs:264
  - `post_note_edit` — crates/vox-publisher/src/scholarly/openreview.rs:309
  - `publish_deposition` — crates/vox-publisher/src/scholarly/zenodo.rs:273

**4 sites, names: error, lowering, runtime_contract, warning**
  - `error` — crates/vox-compiler/src/typeck/diagnostics.rs:562
  - `lowering` — crates/vox-compiler/src/typeck/diagnostics.rs:627
  - `runtime_contract` — crates/vox-compiler/src/typeck/diagnostics.rs:647
  - `warning` — crates/vox-compiler/src/typeck/diagnostics.rs:582

**4 sites, names: build_target_defaults_to_fullstack_when_build_section_absent, reads_build_target_fullstack_from_vox_toml, reads_build_target_server_from_vox_toml, reads_web_run_mode_from_vox_toml**
  - `build_target_defaults_to_fullstack_when_build_section_absent` — crates/vox-config/src/config/impl_ops.rs:543
  - `reads_build_target_fullstack_from_vox_toml` — crates/vox-config/src/config/impl_ops.rs:533
  - `reads_build_target_server_from_vox_toml` — crates/vox-config/src/config/impl_ops.rs:523
  - `reads_web_run_mode_from_vox_toml` — crates/vox-config/src/config/impl_ops.rs:435

**4 sites, names: version**
  - `version` — crates/vox-container/src/docker.rs:36
  - `version` — crates/vox-container/src/podman.rs:37
  - `version` — crates/vox-plugin-runtime-container/src/docker.rs:36
  - `version` — crates/vox-plugin-runtime-container/src/podman.rs:37

**4 sites, names: run**
  - `run` — crates/vox-container/src/docker.rs:75
  - `run` — crates/vox-container/src/podman.rs:78
  - `run` — crates/vox-plugin-runtime-container/src/docker.rs:75
  - `run` — crates/vox-plugin-runtime-container/src/podman.rs:78

**4 sites, names: push**
  - `push` — crates/vox-container/src/docker.rs:112
  - `push` — crates/vox-container/src/podman.rs:115
  - `push` — crates/vox-plugin-runtime-container/src/docker.rs:112
  - `push` — crates/vox-plugin-runtime-container/src/podman.rs:115

**4 sites, names: tag**
  - `tag` — crates/vox-container/src/docker.rs:126
  - `tag` — crates/vox-container/src/podman.rs:129
  - `tag` — crates/vox-plugin-runtime-container/src/docker.rs:126
  - `tag` — crates/vox-plugin-runtime-container/src/podman.rs:129

**4 sites, names: login**
  - `login` — crates/vox-container/src/docker.rs:141
  - `login` — crates/vox-container/src/podman.rs:144
  - `login` — crates/vox-plugin-runtime-container/src/docker.rs:141
  - `login` — crates/vox-plugin-runtime-container/src/podman.rs:144

**4 sites, names: codegen_component_has_use_state, codegen_types_has_tagged_union, pipeline_generics_option_codegen, pipeline_pattern_matching_codegen**
  - `codegen_component_has_use_state` — crates/vox-integration-tests/tests/pipeline/includes/include_01.rs:73
  - `codegen_types_has_tagged_union` — crates/vox-integration-tests/tests/pipeline/includes/include_01.rs:62
  - `pipeline_generics_option_codegen` — crates/vox-integration-tests/tests/pipeline/includes/include_02.rs:43
  - `pipeline_pattern_matching_codegen` — crates/vox-integration-tests/tests/pipeline/includes/include_03.rs:63

**4 sites, names: auth_allows_deliver, auth_allows_worker_plane**
  - `auth_allows_deliver` — crates/vox-plugin-populi-mesh/src/transport/auth.rs:46
  - `auth_allows_deliver` — crates/vox-populi/src/transport/auth.rs:46
  - `auth_allows_worker_plane` — crates/vox-plugin-populi-mesh/src/transport/auth.rs:33
  - `auth_allows_worker_plane` — crates/vox-populi/src/transport/auth.rs:33

**3 sites, names: vox_browser_html, vox_browser_screenshot, vox_browser_text**
  - `vox_browser_html` — crates/vox-actor-runtime/src/builtins/mod.rs:1464
  - `vox_browser_screenshot` — crates/vox-actor-runtime/src/builtins/mod.rs:1478
  - `vox_browser_text` — crates/vox-actor-runtime/src/builtins/mod.rs:1450

**3 sites, names: query_all_guard_all_ok, sql_surface_guard_all_ok, turso_import_guard_all_ok**
  - `query_all_guard_all_ok` — crates/vox-cli/tests/query_all_guard_integration.rs:15
  - `sql_surface_guard_all_ok` — crates/vox-cli/tests/sql_surface_guard_integration.rs:15
  - `turso_import_guard_all_ok` — crates/vox-cli/tests/turso_import_guard_integration.rs:15

**3 sites, names: bind_name, set_account_config, set_user_preference**
  - `bind_name` — crates/vox-db/src/store/ops_cas.rs:56
  - `set_account_config` — crates/vox-db/src/store/ops_learning.rs:316
  - `set_user_preference` — crates/vox-db/src/store/ops_learning.rs:270

**3 sites, names: list_repository_reliability_worst_first, list_skill_reliability_worst_first, list_workflow_reliability_worst_first**
  - `list_repository_reliability_worst_first` — crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs:494
  - `list_skill_reliability_worst_first` — crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs:442
  - `list_workflow_reliability_worst_first` — crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs:468

**3 sites, names: bundle_load_is_unsupported_cas_gap**
  - `bundle_load_is_unsupported_cas_gap` — crates/vox-inference/src/backends/candle_cpu.rs:354
  - `bundle_load_is_unsupported_cas_gap` — crates/vox-inference/src/backends/candle_cuda.rs:295
  - `bundle_load_is_unsupported_cas_gap` — crates/vox-inference/src/backends/candle_metal.rs:300

**3 sites, names: with_express_server_enabled, with_web_ir_validate_enabled**
  - `with_express_server_enabled` — crates/vox-integration-tests/tests/pipeline_env_test.rs:14
  - `with_express_server_enabled` — crates/vox-integration-tests/tests/pipeline_test.rs:23
  - `with_web_ir_validate_enabled` — crates/vox-integration-tests/tests/pipeline_test.rs:70

**3 sites, names: browser_html, browser_screenshot, browser_text**
  - `browser_html` — crates/vox-orchestrator-mcp/src/browser_tools.rs:211
  - `browser_screenshot` — crates/vox-orchestrator-mcp/src/browser_tools.rs:231
  - `browser_text` — crates/vox-orchestrator-mcp/src/browser_tools.rs:191

**3 sites, names: repo_query_file, repo_query_history, repo_query_text**
  - `repo_query_file` — crates/vox-orchestrator-mcp/src/repo_catalog_tools.rs:186
  - `repo_query_history` — crates/vox-orchestrator-mcp/src/repo_catalog_tools.rs:213
  - `repo_query_text` — crates/vox-orchestrator-mcp/src/repo_catalog_tools.rs:159

**3 sites, names: dispatch, heartbeat, join**
  - `dispatch` — crates/vox-populi/src/http_client.rs:594
  - `heartbeat` — crates/vox-populi/src/http_client.rs:314
  - `join` — crates/vox-populi/src/http_client.rs:298

**3 sites, names: admin_maintenance, admin_quarantine, relay_a2a**
  - `admin_maintenance` — crates/vox-populi/src/http_client.rs:564
  - `admin_quarantine` — crates/vox-populi/src/http_client.rs:549
  - `relay_a2a` — crates/vox-populi/src/http_client.rs:377

## Split-brain candidates (names defined in >=4 crates, top 40)

_Same symbol name across many crates — candidate divergent implementations of one concept. Verify by reading bodies._

- **`lib.rs`** — 103 crates: vox-actor-runtime, vox-arch-check, vox-ast, vox-audit, vox-bounded-fs, vox-build-meta, vox-capability-registry, vox-cli, vox-cli-core, vox-cli-tests
- **`String`** — 100 crates: vox-actor-runtime, vox-arch-check, vox-ast, vox-audit, vox-bounded-fs, vox-build-meta, vox-capability-registry, vox-cli, vox-cli-core, vox-cli-tests
- **`Result`** — 92 crates: vox-actor-runtime, vox-arch-check, vox-ast, vox-audit, vox-bounded-fs, vox-capability-registry, vox-cli, vox-cli-core, vox-code-audit, vox-codegen
- **`Vec`** — 89 crates: vox-actor-runtime, vox-arch-check, vox-ast, vox-audit, vox-capability-registry, vox-cli, vox-cli-core, vox-cli-tests, vox-code-audit, vox-codegen
- **`Option`** — 87 crates: vox-actor-runtime, vox-arch-check, vox-ast, vox-audit, vox-bounded-fs, vox-build-meta, vox-capability-registry, vox-cli, vox-cli-core, vox-cli-tests
- **`Self`** — 80 crates: vox-actor-runtime, vox-arch-check, vox-ast, vox-audit, vox-capability-registry, vox-cli, vox-cli-core, vox-cli-tests, vox-code-audit, vox-codegen
- **`PathBuf`** — 63 crates: vox-actor-runtime, vox-arch-check, vox-audit, vox-bounded-fs, vox-cli, vox-cli-core, vox-cli-tests, vox-code-audit, vox-codegen, vox-compiler
- **`Path`** — 63 crates: vox-arch-check, vox-audit, vox-bounded-fs, vox-capability-registry, vox-cli, vox-cli-core, vox-cli-tests, vox-code-audit, vox-codegen, vox-compiler
- **`Default`** — 52 crates: vox-actor-runtime, vox-audit, vox-cli, vox-code-audit, vox-compiler, vox-config, vox-constrained-gen, vox-container-types, vox-corpus, vox-db
- **`HashMap`** — 52 crates: vox-actor-runtime, vox-arch-check, vox-audit, vox-cli, vox-code-audit, vox-codegen, vox-compiler, vox-config, vox-constrained-gen, vox-container-types
- **`Value`** — 50 crates: vox-actor-runtime, vox-arch-check, vox-audit, vox-capability-registry, vox-cli, vox-cli-core, vox-code-audit, vox-codegen, vox-compiler, vox-config
- **`mod.rs`** — 48 crates: vox-actor-runtime, vox-arch-check, vox-ast, vox-audit, vox-cli, vox-cli-core, vox-code-audit, vox-codegen, vox-compiler, vox-config
- **`Send`** — 37 crates: vox-actor-runtime, vox-audit, vox-cli, vox-code-audit, vox-compiler, vox-constrained-gen, vox-container-types, vox-db, vox-distributed-training, vox-drift-check
- **`Into`** — 35 crates: vox-actor-runtime, vox-audit, vox-cli, vox-cli-core, vox-code-audit, vox-compiler, vox-db, vox-forge, vox-gamify, vox-git
- **`Display`** — 34 crates: vox-actor-runtime, vox-ast, vox-cli, vox-code-audit, vox-codegen, vox-compiler, vox-config, vox-corpus, vox-db, vox-distributed-training
- **`Formatter`** — 34 crates: vox-actor-runtime, vox-ast, vox-cli, vox-code-audit, vox-codegen, vox-compiler, vox-config, vox-corpus, vox-crypto, vox-db
- **`Mutex`** — 32 crates: vox-actor-runtime, vox-audit, vox-cli, vox-code-audit, vox-compiler, vox-config, vox-constrained-gen, vox-distributed-training, vox-gamify, vox-gui
- **`Sync`** — 31 crates: vox-actor-runtime, vox-audit, vox-cli, vox-code-audit, vox-compiler, vox-constrained-gen, vox-container-types, vox-drift-check, vox-effort-audit, vox-effort-route
- **`Error`** — 31 crates: vox-actor-runtime, vox-audit, vox-cli, vox-code-audit, vox-codegen, vox-compiler, vox-config, vox-corpus, vox-db, vox-mesh-types
- **`Arc`** — 30 crates: vox-actor-runtime, vox-cli, vox-code-audit, vox-config, vox-db, vox-distributed-training, vox-gui, vox-inference, vox-integration-tests, vox-lsp
- **`main`** — 29 crates: vox-arch-check, vox-audit, vox-cli, vox-code-audit, vox-compiler, vox-corpus, vox-db, vox-doc-inventory, vox-doc-pipeline, vox-drift-check
- **`Box`** — 26 crates: vox-actor-runtime, vox-ast, vox-audit, vox-cli, vox-code-audit, vox-codegen, vox-compiler, vox-constrained-gen, vox-container, vox-drift-check
- **`types.rs`** — 26 crates: vox-actor-runtime, vox-ast, vox-capability-registry, vox-cli, vox-code-audit, vox-codegen, vox-compiler, vox-doc-inventory, vox-doc-pipeline, vox-forge
- **`HashSet`** — 23 crates: vox-arch-check, vox-cli, vox-code-audit, vox-codegen, vox-compiler, vox-constrained-gen, vox-db, vox-doc-inventory, vox-drift-check, vox-gamify
- **`Duration`** — 20 crates: vox-actor-runtime, vox-audit, vox-cli, vox-config, vox-db, vox-effort-audit, vox-effort-route, vox-foundation, vox-integration-tests, vox-orchestrator
- **`Client`** — 19 crates: vox-actor-runtime, vox-audit, vox-cli, vox-code-audit, vox-forge, vox-gamify, vox-lsp, vox-openclaw-runtime, vox-orchestrator, vox-orchestrator-mcp
- **`T`** — 18 crates: vox-actor-runtime, vox-cli, vox-compiler, vox-corpus, vox-db, vox-integration-tests, vox-orchestrator, vox-orchestrator-mcp, vox-orchestrator-queue, vox-orchestrator-test-helpers
- **`VoxDb`** — 17 crates: vox-actor-runtime, vox-cli, vox-corpus, vox-db, vox-gui, vox-lsp, vox-ml-cli, vox-openclaw-runtime, vox-orchestrator, vox-orchestrator-mcp
- **`From`** — 17 crates: vox-cli, vox-code-audit, vox-compiler, vox-db, vox-gui, vox-hf-layout, vox-ml-cli, vox-openclaw-runtime, vox-orchestrator, vox-orchestrator-mcp
- **`Instant`** — 16 crates: vox-actor-runtime, vox-audit, vox-cli, vox-cli-core, vox-db, vox-gamify, vox-orchestrator, vox-orchestrator-mcp, vox-orchestrator-queue, vox-plugin-mens-candle-cuda
- **`F`** — 16 crates: vox-actor-runtime, vox-cli, vox-codegen, vox-compiler, vox-db, vox-integration-tests, vox-orchestrator, vox-orchestrator-mcp, vox-orchestrator-queue, vox-orchestrator-test-helpers
- **`Item`** — 16 crates: vox-actor-runtime, vox-audit, vox-capability-registry, vox-cli, vox-code-audit, vox-compiler, vox-gamify, vox-ml-cli, vox-orchestrator, vox-orchestrator-mcp
- **`config.rs`** — 16 crates: vox-ast, vox-cli, vox-db, vox-drift-check, vox-effort-audit, vox-effort-route, vox-ml-cli, vox-orchestrator, vox-orchestrator-mcp, vox-plugin-mens-candle-cuda
- **`build.rs`** — 15 crates: vox-arch-check, vox-cli, vox-compiler, vox-corpus, vox-db, vox-db-types, vox-gui, vox-mcp-registry, vox-orchestrator, vox-orchestrator-mcp
- **`RString`** — 14 crates: vox-plugin-api, vox-plugin-browser, vox-plugin-cloud, vox-plugin-host, vox-plugin-mens-candle-cuda, vox-plugin-mens-candle-metal, vox-plugin-nvml-probe, vox-plugin-populi-mesh, vox-plugin-publication, vox-plugin-runtime-container
- **`RResult`** — 14 crates: vox-plugin-api, vox-plugin-browser, vox-plugin-cloud, vox-plugin-host, vox-plugin-mens-candle-cuda, vox-plugin-mens-candle-metal, vox-plugin-nvml-probe, vox-plugin-populi-mesh, vox-plugin-publication, vox-plugin-runtime-container
- **`RBoxError`** — 14 crates: vox-plugin-api, vox-plugin-browser, vox-plugin-cloud, vox-plugin-host, vox-plugin-mens-candle-cuda, vox-plugin-mens-candle-metal, vox-plugin-nvml-probe, vox-plugin-populi-mesh, vox-plugin-publication, vox-plugin-runtime-container
- **`VoxPlugin`** — 14 crates: vox-plugin-api, vox-plugin-browser, vox-plugin-cloud, vox-plugin-host, vox-plugin-mens-candle-cuda, vox-plugin-mens-candle-metal, vox-plugin-nvml-probe, vox-plugin-populi-mesh, vox-plugin-publication, vox-plugin-runtime-container
- **`README.md`** — 13 crates: vox-actor-runtime, vox-cli, vox-code-audit, vox-db, vox-doc-pipeline, vox-effort-audit, vox-effort-route, vox-gamify, vox-integration-tests, vox-lsp
- **`Sender`** — 13 crates: vox-actor-runtime, vox-cli, vox-cli-core, vox-db, vox-gui, vox-ml-cli, vox-orchestrator, vox-orchestrator-mcp, vox-plugin-populi-mesh, vox-plugin-webhook
