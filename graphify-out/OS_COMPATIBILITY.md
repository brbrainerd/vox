# OS / Platform Compatibility — deterministic scan

Scanned 3271 Rust files across crates/. Goal: maintain Mac/Linux/Windows parity.

Findings inside a matching `#[cfg(os)]` block are marked `[gated]` (expected); **un-gated** findings are the real portability smells.


## Summary by category

- **abs-unix-path** (high): 71 hits, **64 un-gated**
- **dynlib-ext** (high): 34 hits, **33 un-gated**
- **path-sep-env** (high): 13 hits, **12 un-gated**
- **shell-command** (high): 13 hits, **5 un-gated**
- **env-home-asym** (high): 9 hits, **9 un-gated**
- **home-tilde** (high): 3 hits, **3 un-gated**
- **win-drive-path** (high): 2 hits, **2 un-gated**
- **os-unix-api** (medium): 35 hits, **11 un-gated**
- **crlf-literal** (medium): 17 hits, **17 un-gated**
- **os-windows-api** (medium): 14 hits, **1 un-gated**
- **path-join-fmt** (medium): 11 hits, **11 un-gated**
- **unix-symlink** (medium): 1 hits, **0 un-gated**

**Total un-gated portability findings: 168**

Asymmetric cfg files (handle one OS, not the other): 31


## abs-unix-path — high  (64 un-gated)

_Hardcoded absolute Unix path literal — breaks on Windows. Use std::env::temp_dir()/dirs/Path._

- `crates/vox-cli/src/freshness.rs:311` — `assert!(!is_cargo_build_dir(Path::new("/home/user/.cargo/bin")));`
- `crates/vox-cli/src/freshness.rs:312` — `assert!(!is_cargo_build_dir(Path::new("/home/user/.vox/bin")));`
- `crates/vox-cli/src/freshness.rs:313` — `assert!(!is_cargo_build_dir(Path::new("/usr/local/bin")));`
- `crates/vox-cli/src/commands/audit.rs:568` — `corpus: Some(std::path::PathBuf::from("/tmp/contracts")),`
- `crates/vox-cli/src/commands/audit.rs:576` — `Some(std::path::Path::new("/tmp/contracts"))`
- `crates/vox-cli/src/commands/deploy.rs:134` — `.unwrap_or_else(|| format!("/opt/{}", manifest.package.name));`
- `crates/vox-cli/src/commands/wasm.rs:91` — `let ps = parse_preopens(&["/tmp/data".into()], &["/var/out:/out".into()]).unwrap();`
- `crates/vox-cli/src/commands/ci/runner_scale.rs:266` — `"/var/run/docker.sock:/var/run/docker.sock",`
- `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs:260` — `let update_hint = if exe_path_str.starts_with("/usr/bin/") || exe_path_str.starts_with("/bin/")`
- `crates/vox-codegen/src/codegen_rust/mod.rs:586` — `"fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }",`
- `crates/vox-codegen/src/web_ir/validate.rs:953` — `routes: vec![make_route("route_0", "/home", Some("HomePage"))],`
- `crates/vox-compiler/src/required_capabilities.rs:415` — `"fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }",`
- `crates/vox-compiler/tests/required_capabilities_test.rs:75` — `let hir = lower_src("fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }");`
- `crates/vox-compiler/tests/required_capabilities_test.rs:85` — `return fs.read("/etc/hosts")`
- `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:1641` — `"/home",`
- `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:1660` — `"/home",`
- `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:1678` — `.push(route_tree(vec![route_contract("r1", "/home", None)]));`
- `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:1697` — `.push(route_tree(vec![route_contract("r1", "/home", None)]));`
- `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:1698` — `m.dom_nodes.push(link_element(0, "/home"));`
- `crates/vox-compiler/src/fmt/mod.rs:60` — `"nginx.conf" to "/etc/nginx/nginx.conf"`
- `crates/vox-compiler/src/fmt/mod.rs:74` — `"nginx.conf" to "/etc/nginx/nginx.conf"`
- `crates/vox-compiler/src/typeck/effect_check.rs:713` — `let diags = check(r#"fn f() uses nothing to str { fs.read("/etc/hosts") }"#);`
- `crates/vox-deploy-codegen/src/bare_metal.rs:49` — `workdir: Some("/opt/my-app".to_string()),`
- `crates/vox-deploy-codegen/src/deploy_target.rs:615` — `Path::new("/tmp"),`
- `crates/vox-deploy-codegen/src/deploy_target.rs:621` — `assert_eq!(t.context_dir, Path::new("/tmp"));`
- … +39 more

## dynlib-ext — high  (33 un-gated)

_Hardcoded dynamic-lib extension — differs per OS (.so/.dylib/.dll)._

- `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs:152` — `|| r.ends_with(".dll")`
- `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs:624` — `assert!(is_build_junk("plugin.dll"));`
- `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/groups.rs:35` — `".db", ".db-wal", ".db-shm", ".png", ".jpg", ".jpeg", ".webp", ".ico", ".dll", ".exe", ".so",`
- `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/groups.rs:36` — `".dylib", ".bin", ".svg", ".woff", ".woff2",`
- `crates/vox-cli/src/commands/review/coderabbit/stack_planner/heuristics.rs:26` — `".png", ".jpg", ".jpeg", ".webp", ".ico", ".dll", ".exe", ".so", ".dylib", ".bin", ".lock",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:110` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:120` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:130` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:140` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:148` — `assert_cross_compiles("vox-journal", "aarch64-linux-android", "libvox_journal.so");`
- `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs:34` — `const ARTIFACT: &str = "libvox_plugin_mens_candle_cuda.so";`
- `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs:372` — `.join("libcuda.so")`
- `crates/vox-plugin-api/tests/manifest_parsing.rs:23` — `"linux-x86_64" = "libvox_plugin_mens_candle_cuda.so"`
- `crates/vox-plugin-api/tests/manifest_parsing.rs:33` — `"libvox_plugin_mens_candle_cuda.so"`
- `crates/vox-plugin-api/tests/manifest_parsing.rs:93` — `"linux-x86_64" = "libvox_plugin_populi_mesh.so"`
- `crates/vox-plugin-host/tests/abi_mismatch.rs:17` — `format!("{}.dll", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/abi_mismatch.rs:19` — `format!("lib{}.dylib", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/abi_mismatch.rs:21` — `format!("lib{}.so", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/load_noop_code.rs:17` — `format!("{}.dll", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/load_noop_code.rs:19` — `format!("lib{}.dylib", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/load_noop_code.rs:21` — `format!("lib{}.so", crate_name.replace('-', "_"))`
- `crates/vox-plugin-test-harness/src/lib.rs:16` — `//!     .artifact("linux-x86_64", "libtest.so")`
- `crates/vox-plugin-test-harness/src/lib.rs:39` — `.artifact("linux-x86_64", "libmy.so")`
- `crates/vox-plugin-test-harness/src/lib.rs:60` — `.artifact("linux-x86_64", "libtouch.so")`
- `crates/vox-plugin-test-harness/src/lib.rs:63` — `dir.touch("libtouch.so").expect("touch");`
- … +8 more

## path-sep-env — high  (12 un-gated)

_Splitting on ':' or ';' — PATH separator differs per OS. Use std::env::split_paths._

- `crates/vox-actor-runtime/tests/telemetry_sandbox_timeout_kill.rs:69` — `assert_eq!(kill.session_id.split(':').next(), Some("sandbox"));`
- `crates/vox-cli/src/utils/share/auth.rs:113` — `for part in cookie_str.split(';') {`
- `crates/vox-code-audit/src/detectors/stringly_typed_enum.rs:228` — `let field_name = line.trim().split(':').next().unwrap_or("field").trim();`
- `crates/vox-compiler/tests/examples_ssot_test.rs:118` — `let path_part = token.split(':').next().unwrap_or(token).trim();`
- `crates/vox-compiler/src/typeck/boilerplate_grafts.rs:415` — `let parts: Vec<&str> = q.split(':').collect();`
- `crates/vox-doc-pipeline/src/pipeline/lint.rs:463` — `let parts: Vec<&str> = include_body.split(':').collect();`
- `crates/vox-effort-route/src/bucket.rs:49` — `let path = path.split(':').next().unwrap_or(path); // strip ":line"`
- `crates/vox-orchestrator/src/planning/synthesizer.rs:104` — `for sub in s.split(';') {`
- `crates/vox-orchestrator/src/planning/synthesizer.rs:134` — `.split(';')`
- `crates/vox-populi/src/lib.rs:158` — `hostport.split(':').next().unwrap_or(hostport)`
- `crates/vox-research-shim/src/research/orchestrator/web_gather.rs:129` — `.split(':')`
- `crates/vox-speech/src/subtitle/srt.rs:109` — `let mut hms_parts = hms.split(':');`

## shell-command — high  (5 un-gated)

_OS-specific shell invocation — pick per-OS or avoid the shell._

- `crates/vox-cli/src/commands/runtime/shell/backends/powershell.rs:7` — `let output = Command::new("pwsh")`
- `crates/vox-cli/src/commands/runtime/shell/backends/powershell.rs:24` — `if Command::new("pwsh").arg("-v").status().is_err() {`
- `crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs:195` — `Command::new("cmd")`
- `crates/vox-orchestrator/src/services/flywheel.rs:136` — `let status = tokio::process::Command::new("pwsh")`
- `crates/vox-scientia/src/replay/sandbox.rs:69` — `let mut cmd = Command::new("sh");`

## env-home-asym — high  (9 un-gated)

_Reads HOME (Unix) — Windows uses USERPROFILE. Use the `dirs` crate._

- `crates/vox-cli/src/commands/clean.rs:48` — `let home = std::env::var("HOME")`
- `crates/vox-cli/src/commands/publish.rs:72` — `let home = std::env::var("HOME")`
- `crates/vox-cli/src/commands/toolchain_upgrade.rs:585` — `let home = std::env::var("HOME")`
- `crates/vox-cli/src/commands/ci/mod.rs:103` — `if let Ok(h) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {`
- `crates/vox-cli-core/src/artifact_policy.rs:73` — `.or_else(|_| std::env::var("HOME"))`
- `crates/vox-config/src/paths.rs:110` — `std::env::var("HOME")`
- `crates/vox-ml-cli/src/commands/corpus/generate.rs:514` — `.or_else(|_| std::env::var("HOME"))`
- `crates/vox-runtime/src/config.rs:94` — `if let Ok(h) = std::env::var("HOME")`
- `crates/vox-secrets/src/sources/auth_json.rs:26` — `let home = std::env::var("HOME")`

## home-tilde — high  (3 un-gated)

_Literal ~ home path — not expanded on Windows. Use the `dirs`/`home` crate._

- `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs:301` — `"~/.vox/ not writable — check permissions".to_string()`
- `crates/vox-config/src/operator_registry.rs:831` — `defaults: "~/.cargo",`
- `crates/vox-ml-cli/src/commands/mens/pipeline.rs:187` — `let input = PathBuf::from("~/.vox/corpus/heal_pairs.jsonl");`

## win-drive-path — high  (2 un-gated)

_Hardcoded Windows drive path — breaks on Unix._

- `crates/vox-config/src/operator_registry.rs:845` — `defaults: "C:\\Users\\Default",`
- `crates/vox-container-types/src/exec_grammar/ast.rs:453` — `assert_eq!(ast.flags[1].value.as_deref(), Some("C:\\foo"));`

## os-unix-api — medium  (11 un-gated)

_Unix-only OS API (permissions/mode). Needs a Windows path or cfg gate._

- `crates/vox-ml-cli/src/commands/mens/populi/train_arm.rs:270` — `let requested_b = memory_budget::params_b_from_model_hint(model_hint).unwrap_or(4.0);`
- `crates/vox-populi/tests/hf_load_config.rs:63` — `fn detect_qwen2_from_model_type() {`
- `crates/vox-populi/tests/hf_load_config.rs:85` — `fn detect_qwen2_5_from_model_type_maps_like_qwen2() {`
- `crates/vox-populi/src/mens/hardware/registry.rs:67` — `use crate::mens::hardware::types::{ComputeBackend, vendor_from_model};`
- `crates/vox-populi/src/mens/hardware/registry.rs:74` — `vendor: vendor_from_model(model),`
- `crates/vox-populi/src/mens/hardware/types.rs:85` — `pub fn vendor_from_model(model: &str) -> GpuVendor {`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:355` — `pub fn params_b_from_model_hint(hint: &str) -> Option<f64> {`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:398` — `assert_eq!(params_b_from_model_hint("Qwen/Qwen3.5-4B"), Some(4.0));`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:399` — `assert_eq!(params_b_from_model_hint("qwen2.5-0.8b"), Some(0.8));`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:401` — `params_b_from_model_hint("meta-llama/Llama-3-70B"),`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:404` — `assert_eq!(params_b_from_model_hint("some-model"), None);`

## crlf-literal — medium  (17 un-gated)

_Hardcoded CRLF literal — line-ending assumption._

- `crates/vox-cli/src/commands/ci/agentskills_compliance.rs:64` — `.strip_prefix("\r\n")`
- `crates/vox-cli/src/commands/ci/command_sync.rs:17` — `s.replace("\r\n", "\n").replace('\r', "\n")`
- `crates/vox-cli/src/commands/ci/operations_catalog.rs:204` — `s.replace("\r\n", "\n").replace('\r', "\n")`
- `crates/vox-cli/src/commands/ci/plugin_catalog_sync.rs:187` — `s.replace("\r\n", "\n")`
- `crates/vox-cli/src/commands/ci/plugin_surface.rs:406` — `s.replace("\r\n", "\n")`
- `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:35` — `s.replace("\r\n", "\n").replace('\r', "\n")`
- `crates/vox-integration-tests/tests/golden_behavioral_gate.rs:90` — `s.replace("\r\n", "\n").trim_end().to_string()`
- `crates/vox-publisher/src/templates.rs:125` — `let crate_src = template_source(NewsTemplateId::ResearchUpdate).replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:130` — `.replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:139` — `let crate_src = template_source(NewsTemplateId::Release).replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:144` — `.replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:153` — `let crate_src = template_source(NewsTemplateId::SecurityAdvisory).replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:158` — `.replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:167` — `let crate_src = template_source(NewsTemplateId::CommunityUpdate).replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:172` — `.replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:181` — `let crate_src = template_source(NewsTemplateId::DiscordAnnouncement).replace("\r\n", "\n");`
- `crates/vox-publisher/src/templates.rs:186` — `.replace("\r\n", "\n");`

## os-windows-api — medium  (1 un-gated)

_Windows-only OS API. Needs a Unix path or cfg gate._

- `crates/vox-cli/src/commands/ci/runner_scale.rs:89` — `/// many times per launch/reap cycle; without `CREATE_NO_WINDOW` each child pops a`

## path-join-fmt — medium  (11 un-gated)

_Building a path with `/` in format! — use Path::join / PathBuf for portability._

- `crates/vox-actor-runtime/src/storage.rs:107` — `format!("{}/{}", STORAGE_URL_PREFIX, id)`
- `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/rules.rs:299` — `format!("{}/{}", parts[0], parts[1])`
- `crates/vox-orchestrator/src/catalog.rs:492` — `id: format!("mesh/{}/{}", peer.scope_id, kind_str),`
- `crates/vox-orchestrator/src/catalog.rs:493` — `canonical_slug: format!("mesh/{}/{}", peer.scope_id, kind_str),`
- `crates/vox-orchestrator/src/gate.rs:346` — `message: format!("Provider {}/{} daily limit reached", b.provider, b.model),`
- `crates/vox-orchestrator/src/models/registry.rs:182` — `.map(|p| format!("{}/{}", p, suffix));`
- `crates/vox-orchestrator/src/models/registry.rs:393` — `id: format!("mesh/{}/{}", peer.scope_id, kind_str),`
- `crates/vox-orchestrator/src/models/registry.rs:394` — `canonical_slug: format!("mesh/{}/{}", peer.scope_id, kind_str),`
- `crates/vox-package/src/workspace.rs:54` — `format!("{}/{}", root_str, pattern)`
- `crates/vox-package/src/workspace.rs:56` — `format!("{}/{}/Vox.toml", root_str, pattern)`
- `crates/vox-publisher/src/scholarly/zenodo.rs:212` — `let url = format!("{}/{}", bucket_url.trim_end_matches('/'), name);`

## Asymmetric cfg files (top 30)

_One OS handled, the other absent — likely a missing platform branch._

- `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs` — windows-only (win=7, unix=0)
- `crates/vox-actor-runtime/src/builtins/mod.rs` — unix-only (win=0, unix=4)
- `crates/vox-compiler/src/eval/builtins.rs` — unix-only (win=0, unix=4)
- `crates/vox-scientia/src/replay/sandbox.rs` — windows-only (win=4, unix=0)
- `crates/vox-telemetry/src/config.rs` — windows-only (win=4, unix=0)
- `crates/vox-ml-cli/src/commands/schola/train/spawn.rs` — windows-only (win=3, unix=0)
- `crates/vox-populi/src/mens/hardware/windows_fallback.rs` — windows-only (win=3, unix=0)
- `crates/voxup/src/install.rs` — unix-only (win=0, unix=3)
- `crates/vox-cli/tests/command_catalog_paths_baseline.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/tests/vox_cli_root_parsing.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/src/commands/fmt.rs` — unix-only (win=0, unix=2)
- `crates/vox-cli/src/commands/ci/compile_matrix.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/src/commands/ci/install_hooks.rs` — unix-only (win=0, unix=2)
- `crates/vox-cli/src/commands/ci/pre_push.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/src/utils/share/binary_cache.rs` — unix-only (win=0, unix=2)
- `crates/vox-codegen/src/codegen_rust/manifest.rs` — windows-only (win=2, unix=0)
- `crates/vox-identity/src/storage.rs` — unix-only (win=0, unix=2)
- `crates/vox-ml-cli/src/commands/populi_cli.rs` — windows-only (win=2, unix=0)
- `crates/vox-populi/src/mens/hardware/linux_drm.rs` — unix-only (win=0, unix=2)
- `crates/vox-populi/src/mens/hardware/macos_metal.rs` — unix-only (win=0, unix=2)
- `crates/vox-cli/tests/ci_workflow_contract.rs` — windows-only (win=1, unix=0)
- `crates/vox-cli/src/commands/share.rs` — unix-only (win=0, unix=1)
- `crates/vox-cli/src/commands/ci/runner_scale.rs` — windows-only (win=1, unix=0)
- `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/toolchain.rs` — unix-only (win=0, unix=1)
- `crates/vox-cli/src/commands/runtime/run/backend/native.rs` — windows-only (win=1, unix=0)
- `crates/vox-ml-cli/src/commands/schola/train/run_train.rs` — unix-only (win=0, unix=1)
- `crates/vox-orchestrator/src/a2a/remote_worker.rs` — unix-only (win=0, unix=1)
- `crates/vox-plugin-populi-mesh/src/transport/handlers/dispatch.rs` — unix-only (win=0, unix=1)
- `crates/vox-populi/src/transport/handlers/dispatch.rs` — unix-only (win=0, unix=1)
