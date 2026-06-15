# OS / Platform Compatibility — deterministic scan

Scanned **3453** Rust files (crates/) + **67** Vox script files (scripts/). Goal: maintain Mac/Linux/Windows parity.

Findings inside a matching `#[cfg(os)]` block are marked `[gated]` (expected); **un-gated** findings are the real portability smells.


## Summary by category

- **dynlib-ext** (high): 34 hits, **33 un-gated**
- **shell-command** (high): 14 hits, **6 un-gated**
- **path-sep-env** (high): 13 hits, **12 un-gated**
- **env-home-asym** (high): 11 hits, **11 un-gated**
- **home-tilde** (high): 6 hits, **6 un-gated**
- **process-uid-gid** (high): 4 hits, **0 un-gated**
- **win-drive-path** (high): 1 hits, **1 un-gated**
- **path-join-fmt** (medium): 154 hits, **154 un-gated**
- **os-unix-api** (medium): 41 hits, **17 un-gated**
- **tempdir-slash** (medium): 32 hits, **32 un-gated**
- **os-windows-api** (medium): 32 hits, **7 un-gated**
- **file-executable-bit** (medium): 28 hits, **9 un-gated**
- **hardcoded-newline-crlf** (medium): 2 hits, **2 un-gated**
- **unix-symlink** (medium): 1 hits, **0 un-gated**
- **locale-encoding** (low): 14 hits, **14 un-gated**
- **[vox]path-sep-env** (high): 1 hits, **1 un-gated** (Vox scripts)
- **[vox]tempdir-slash** (medium): 1 hits, **1 un-gated** (Vox scripts)

**Total un-gated portability findings: 306**

Asymmetric cfg files (handle one OS, not the other): 33


## dynlib-ext — high  (33 un-gated)

_Hardcoded dynamic-lib extension — differs per OS (.so/.dylib/.dll)._

- `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs:153` — `|| r.ends_with(".dll")`
- `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs:626` — `assert!(is_build_junk("plugin.dll"));`
- `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/groups.rs:36` — `".db", ".db-wal", ".db-shm", ".png", ".jpg", ".jpeg", ".webp", ".ico", ".dll", ".exe", ".so",`
- `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/groups.rs:38` — `".dylib", ".bin", ".svg", ".woff", ".woff2",`
- `crates/vox-cli/src/commands/review/coderabbit/stack_planner/heuristics.rs:27` — `".png", ".jpg", ".jpeg", ".webp", ".ico", ".dll", ".exe", ".so", ".dylib", ".bin", ".lock",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:111` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:122` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:133` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:144` — `"libvox_runtime_rn.so",`
- `crates/vox-cli-tests/tests/mobile_cross_compile.rs:153` — `assert_cross_compiles("vox-journal", "aarch64-linux-android", "libvox_journal.so");`
- `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs:35` — `const ARTIFACT: &str = "libvox_plugin_mens_candle_cuda.so";`
- `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs:375` — `.join("libcuda.so")`
- `crates/vox-plugin-api/tests/manifest_parsing.rs:24` — `"linux-x86_64" = "libvox_plugin_mens_candle_cuda.so"`
- `crates/vox-plugin-api/tests/manifest_parsing.rs:35` — `"libvox_plugin_mens_candle_cuda.so"`
- `crates/vox-plugin-api/tests/manifest_parsing.rs:96` — `"linux-x86_64" = "libvox_plugin_populi_mesh.so"`
- `crates/vox-plugin-host/tests/abi_mismatch.rs:18` — `format!("{}.dll", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/abi_mismatch.rs:21` — `format!("lib{}.dylib", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/abi_mismatch.rs:24` — `format!("lib{}.so", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/load_noop_code.rs:18` — `format!("{}.dll", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/load_noop_code.rs:21` — `format!("lib{}.dylib", crate_name.replace('-', "_"))`
- `crates/vox-plugin-host/tests/load_noop_code.rs:24` — `format!("lib{}.so", crate_name.replace('-', "_"))`
- `crates/vox-plugin-test-harness/src/lib.rs:17` — `//!     .artifact("linux-x86_64", "libtest.so")`
- `crates/vox-plugin-test-harness/src/lib.rs:41` — `.artifact("linux-x86_64", "libmy.so")`
- `crates/vox-plugin-test-harness/src/lib.rs:63` — `.artifact("linux-x86_64", "libtouch.so")`
- `crates/vox-plugin-test-harness/src/lib.rs:67` — `dir.touch("libtouch.so").expect("touch");`
- … +8 more

## shell-command — high  (6 un-gated)

_OS-specific shell invocation — pick per-OS or avoid the shell._

- `crates/vox-cli/src/commands/runtime/shell/backends/powershell.rs:8` — `let output = Command::new("pwsh")`
- `crates/vox-cli/src/commands/runtime/shell/backends/powershell.rs:26` — `if Command::new("pwsh").arg("-v").status().is_err() {`
- `crates/vox-integration-tests/tests/ts_emit_behavioral_test.rs:131` — `let mut c = Command::new("cmd");`
- `crates/vox-integration-tests/tests/ts_emit_behavioral_test.rs:149` — `Command::new("cmd")`
- `crates/vox-integration-tests/tests/ts_emit_typecheck_test.rs:196` — `Command::new("cmd")`
- `crates/vox-scientia/src/replay/sandbox.rs:69` — `let mut cmd = Command::new("sh");`

## path-sep-env — high  (12 un-gated)

_Splitting on ':' or ';' — PATH separator differs per OS. Use std::env::split_paths._

- `crates/vox-actor-runtime/tests/telemetry_sandbox_timeout_kill.rs:69` — `assert_eq!(kill.session_id.split(':').next(), Some("sandbox"));`
- `crates/vox-cli/src/utils/share/auth.rs:113` — `for part in cookie_str.split(';') {`
- `crates/vox-code-audit/src/detectors/stringly_typed_enum.rs:228` — `let field_name = line.trim().split(':').next().unwrap_or("field").trim();`
- `crates/vox-compiler/tests/examples_ssot_test.rs:118` — `let path_part = token.split(':').next().unwrap_or(token).trim();`
- `crates/vox-compiler/src/typeck/boilerplate_grafts.rs:415` — `let parts: Vec<&str> = q.split(':').collect();`
- `crates/vox-doc-pipeline/src/pipeline/lint.rs:522` — `let parts: Vec<&str> = include_body.split(':').collect();`
- `crates/vox-effort-route/src/bucket.rs:49` — `let path = path.split(':').next().unwrap_or(path); // strip ":line"`
- `crates/vox-orchestrator/src/planning/synthesizer.rs:104` — `for sub in s.split(';') {`
- `crates/vox-orchestrator/src/planning/synthesizer.rs:134` — `.split(';')`
- `crates/vox-populi/src/lib.rs:158` — `hostport.split(':').next().unwrap_or(hostport)`
- `crates/vox-research-shim/src/research/orchestrator/web_gather.rs:133` — `.split(':')`
- `crates/vox-speech/src/subtitle/srt.rs:109` — `let mut hms_parts = hms.split(':');`

## env-home-asym — high  (11 un-gated)

_Reads HOME (Unix) — Windows uses USERPROFILE. Use the `dirs` crate._

- `crates/vox-cli/src/commands/clean.rs:48` — `let home = std::env::var("HOME")`
- `crates/vox-cli/src/commands/publish.rs:72` — `let home = std::env::var("HOME")`
- `crates/vox-cli/src/commands/toolchain_upgrade.rs:585` — `let home = std::env::var("HOME")`
- `crates/vox-cli/src/commands/ci/mod.rs:88` — `if let Ok(h) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {`
- `crates/vox-cli-ci/src/dep_sprawl.rs:22` — `if let Ok(h) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {`
- `crates/vox-cli-core/src/artifact_policy.rs:73` — `.or_else(|_| std::env::var("HOME"))`
- `crates/vox-cli-core/src/semcov_wave22_tests.rs:137` — `.or_else(|_| std::env::var("HOME"))`
- `crates/vox-config/src/paths.rs:131` — `std::env::var("HOME")`
- `crates/vox-ml-cli/src/commands/corpus/generate.rs:514` — `.or_else(|_| std::env::var("HOME"))`
- `crates/vox-runtime/src/config.rs:94` — `if let Ok(h) = std::env::var("HOME")`
- `crates/vox-secrets/src/sources/auth_json.rs:26` — `let home = std::env::var("HOME")`

## home-tilde — high  (6 un-gated)

_Literal ~ home path — not expanded on Windows. Use the `dirs`/`home` crate._

- `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs:302` — `"~/.vox/ not writable — check permissions".to_string()`
- `crates/vox-cli-core/src/semcov_wave22_tests.rs:144` — `"~/.vox/* should be allowed"`
- `crates/vox-cli-core/src/semcov_wave22_tests.rs:154` — `"~/ sibling outside .vox must be denied"`
- `crates/vox-config/src/operator_registry.rs:831` — `defaults: "~/.cargo",`
- `crates/vox-ml-cli/src/commands/mens/pipeline.rs:187` — `let input = PathBuf::from("~/.vox/corpus/heal_pairs.jsonl");`
- `crates/vox-orchestrator/src/memory/project_file.rs:104` — `if let Some(rest) = import.strip_prefix("~/") {`

## win-drive-path — high  (1 un-gated)

_Hardcoded Windows drive path — breaks on Unix._

- `crates/vox-arch-check/src/forbidden_patterns.rs:348` — `"let p = \"C:\\\\Users\\\\Default\";",`

## path-join-fmt — medium  (154 un-gated)

_Building a path with `/` in format! — use Path::join / PathBuf for portability._

- `crates/vox-actor-runtime/src/mens.rs:181` — `.post(format!("{}/api/generate", self.config.base_url))`
- `crates/vox-actor-runtime/src/mens.rs:222` — `.post(format!("{}/api/embeddings", self.config.base_url))`
- `crates/vox-actor-runtime/src/mens.rs:266` — `.post(format!("{}/api/training/submit", self.config.base_url))`
- `crates/vox-actor-runtime/src/storage.rs:107` — `format!("{}/{}", STORAGE_URL_PREFIX, id)`
- `crates/vox-arch-check/src/main.rs:1742` — `let needle = format!("crates/{}/", name);`
- `crates/vox-audit/src/panel.rs:304` — `let url = format!("{}/v1/completions", self.base_url.trim_end_matches('/'));`
- `crates/vox-cli/src/compilerd.rs:329` — `let url = format!("http://127.0.0.1:{}/", p.port);`
- `crates/vox-cli/src/compilerd.rs:431` — `let url = format!("http://127.0.0.1:{}/", p.port);`
- `crates/vox-cli/src/v0.rs:223` — `let url = format!("{}/v1/chats", server.uri());`
- `crates/vox-cli/src/commands/generate.rs:153` — `let endpoint = format!("{}/generate", url);`
- `crates/vox-cli/src/commands/generate.rs:155` — `match client.get(format!("{}/health", url)).send().await {`
- `crates/vox-cli/src/commands/ci/coverage_gates.rs:152` — `let needle = format!("{}/", rel_dir.to_string_lossy().replace('\\', "/"));`
- `crates/vox-cli/src/commands/ci/mens_scorecard.rs:293` — `.post(format!("{}/generate", server_url.trim_end_matches('/')))`
- `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/mod.rs:117` — `g(format!("{}/README.md", concat!(".open", "code")).as_str()),`
- `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/rules.rs:299` — `format!("{}/{}", parts[0], parts[1])`
- `crates/vox-cli/src/utils/share/sse_detect.rs:12` — `let url = format!("http://127.0.0.1:{}/openapi.json", upstream_port);`
- `crates/vox-cli-ci/src/parse_check.rs:89` — `let glob = format!("{}/ok.json", tmp.path().display());`
- `crates/vox-cli-ci/src/parse_check.rs:97` — `let glob = format!("{}/bad.json", tmp.path().display());`
- `crates/vox-cli-ci/src/parse_check.rs:105` — `let glob = format!("{}/ok.yaml", tmp.path().display());`
- `crates/vox-cli-ci/src/parse_check.rs:113` — `let glob = format!("{}/bad.yaml", tmp.path().display());`
- `crates/vox-cli-ci/src/parse_check.rs:139` — `let pattern = format!("{}/nonexistent_*.xyz", tmp.path().display());`
- `crates/vox-cli-ci/src/parse_check.rs:149` — `let pattern = format!("{}/?.json", tmp.path().display());`
- `crates/vox-code-audit/src/ai_analyze.rs:236` — `AiProvider::Ollama { url, .. } => Some(format!("{}/api/generate", url)),`
- `crates/vox-code-audit/src/stdlib_parity.rs:396` — `let pattern = format!("{}/**/*.vox", root.display());`
- `crates/vox-code-audit/src/review/client.rs:168` — `let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));`
- … +129 more

## os-unix-api — medium  (17 un-gated)

_Unix-only OS API (permissions/mode). Needs a Windows path or cfg gate._

- `crates/vox-ml-cli/src/commands/mens/populi/train_arm.rs:269` — `let requested_b = memory_budget::params_b_from_model_hint(model_hint).unwrap_or(4.0);`
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
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:601` — `// ── params_b_from_model_hint ─────────────────────────────────────────────`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:606` — `assert_eq!(params_b_from_model_hint(""), None);`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:614` — `assert_eq!(params_b_from_model_hint("backend"), None);`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:615` — `assert_eq!(params_b_from_model_hint("bert-base"), None);`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:623` — `params_b_from_model_hint("Qwen/Qwen2.5-Coder-0.5B-Instruct"),`
- `crates/vox-populi/src/mens/tensor/memory_budget.rs:626` — `assert_eq!(params_b_from_model_hint("tiny-1.5b-model"), Some(1.5));`

## tempdir-slash — medium  (32 un-gated)

_Hardcoded /tmp — use std::env::temp_dir() instead._

- `crates/vox-arch-check/src/forbidden_patterns.rs:331` — `write_fixture(&dir, "crates/x/src/c.rs", "let p = \"/tmp/contracts\";");`
- `crates/vox-cli/src/commands/audit.rs:626` — `corpus: Some(std::path::PathBuf::from("/tmp/contracts")),`
- `crates/vox-cli/src/commands/audit.rs:635` — `Some(std::path::Path::new("/tmp/contracts"))`
- `crates/vox-cli/src/commands/wasm.rs:92` — `let ps = parse_preopens(&["/tmp/data".into()], &["/var/out:/out".into()]).unwrap();`
- `crates/vox-cli-core/src/artifact_policy.rs:152` — `let root = Path::new("/tmp/repo");`
- `crates/vox-cli-core/src/build_service.rs:506` — `let td = PathBuf::from("/tmp/target");`
- `crates/vox-cli-core/src/build_service.rs:528` — `let td = PathBuf::from("/tmp/target");`
- `crates/vox-cli-core/src/build_service.rs:548` — `let td = PathBuf::from("/tmp/target");`
- `crates/vox-codegen/src/codegen_rust/mod.rs:593` — `"fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }",`
- `crates/vox-compiler/src/required_capabilities.rs:416` — `"fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }",`
- `crates/vox-compiler/tests/required_capabilities_test.rs:76` — `let hir = lower_src("fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }");`
- `crates/vox-config/tests/semcov_wave36_tests.rs:289` — `let p = std::path::PathBuf::from("/tmp/__nonexistent_vox_test_manifest_semcov36__.toml");`
- `crates/vox-drift-check/src/sweep/body_hash.rs:181` — `let layers = LayersManifest::load_from_file(std::path::Path::new("/tmp/none.toml"));`
- `crates/vox-plugin-host/src/lib.rs:209` — `unsafe { std::env::set_var("VOX_PLUGINS_DIR", "/tmp/my-plugins") };`
- `crates/vox-plugin-host/src/lib.rs:213` — `assert_eq!(result, std::path::PathBuf::from("/tmp/my-plugins"));`
- `crates/vox-plugin-runtime-wasm/src/lib.rs:117` — `r#"{"artifact_path":"/tmp/nonexistent_skill.wasm","ports":[],"env":[],"volumes":[],"detach":false,"name":null,"rm":true,`
- `crates/vox-plugin-runtime-wasm/src/lib.rs:183` — `let json = run_opts_json_with_name("/tmp/not_a_real_skill.wasm", None);`
- `crates/vox-runtime/src/config.rs:162` — `let cfg = VoxConfig::mobile(PathBuf::from("/tmp/vox-mobile-test"));`
- `crates/vox-runtime/tests/profile_integration.rs:32` — `let cfg = VoxConfig::mobile(PathBuf::from("/tmp/vox-mobile-it"));`
- `crates/vox-runtime-rn/src/lib.rs:348` — `let inner = InnerConfig::mobile(std::path::PathBuf::from("/tmp/vox-rt-rn-test"));`
- `crates/vox-runtime-rn/src/lib.rs:358` — `data_dir: "/tmp/d".to_string(),`
- `crates/vox-runtime-rn/src/lib.rs:359` — `model_dir: "/tmp/m".to_string(),`
- `crates/vox-runtime-rn/src/lib.rs:366` — `assert_eq!(h.data_dir(), "/tmp/d");`
- `crates/vox-runtime-rn/src/lib.rs:368` — `assert_eq!(h.model_dir(), "/tmp/m");`
- `crates/vox-runtime-rn/src/lib.rs:376` — `data_dir: "/tmp/d".to_string(),`
- … +7 more

## os-windows-api — medium  (7 un-gated)

_Windows-only OS API. Needs a Unix path or cfg gate._

- `crates/vox-cli/src/commands/ci/runner_scale.rs:192` — `/// many times per launch/reap cycle; without `CREATE_NO_WINDOW` each child pops a`
- `crates/vox-gui/src/commands/process_util.rs:3` — `//! On Windows every `Command::new()` without `CREATE_NO_WINDOW` causes a`
- `crates/vox-gui/src/commands/process_util.rs:8` — `/// Returns a [`std::process::Command`] with `CREATE_NO_WINDOW` set on Windows,`
- `crates/vox-gui/src/commands/process_util.rs:23` — `/// Returns a [`tokio::process::Command`] with `CREATE_NO_WINDOW` set on Windows.`
- `crates/vox-orchestrator/src/process_util.rs:5` — `//! blank console window on the desktop.  The helpers here set `CREATE_NO_WINDOW``
- `crates/vox-orchestrator/src/process_util.rs:8` — `/// Returns a [`std::process::Command`] with `CREATE_NO_WINDOW` set on Windows.`
- `crates/vox-orchestrator/src/process_util.rs:21` — `/// Returns a [`tokio::process::Command`] with `CREATE_NO_WINDOW` set on Windows.`

## file-executable-bit — medium  (9 un-gated)

_Unix-only file permission API. Windows has no executable bit — needs cfg gate._

- `crates/vox-cli/src/commands/plugin_bundle/build.rs:127` — `bundle_header.set_mode(0o644);`
- `crates/vox-corpus/src/synthetic_gen/bodies/_tool_pairs_body.rs:186` — `"vox_set_model" => json!({ "agent_id": 1, "model_id": "anthropic/claude-3-5-haiku" }),`
- `crates/vox-orchestrator-mcp/src/dispatch.rs:601` — `"vox_set_model" => Ok(crate::models::set_model(state, serde_json::from_value(args)?).await),`
- `crates/vox-orchestrator-mcp/src/input_schemas.rs:345` — `"vox_set_model" => parse_obj(`
- `crates/vox-orchestrator-mcp/src/models_tools.rs:137` — `pub async fn set_model(state: &ServerState, params: SetModelParams) -> String {`
- `crates/vox-scientia/src/manuscript/latex/bundle.rs:105` — `header.set_mode(0o644);`
- `crates/vox-scientia/src/manuscript/latex/bundle.rs:114` — `header.set_mode(0o644);`
- `crates/vox-scientia/src/manuscript/latex/bundle.rs:126` — `header.set_mode(0o644);`
- `crates/vox-scientia/src/manuscript/latex/bundle.rs:135` — `header.set_mode(0o644);`

## hardcoded-newline-crlf — medium  (2 un-gated)

_Hardcoded CRLF sequence — use a platform-aware line ending or write '\n' and let git handle normalization._

- `crates/vox-code-audit/src/detectors/line_endings.rs:53` — `"# VIOLATION — file has Windows CRLF line endings (\\r\\n)\n\`
- `crates/vox-code-audit/src/detectors/line_endings.rs:54` — `# Each line ends with \\r\\n instead of \\n\n\`

## locale-encoding — low  (14 un-gated)

_Hardcoded encoding assumption — may fail on systems with different locales._

- `crates/vox-cli/tests/pm_lifecycle_integration.rs:267` — `let db = VoxDb::open(db_path.to_str().expect("utf-8"))`
- `crates/vox-cli/tests/pm_lifecycle_integration.rs:297` — `let db = VoxDb::open(db_path.to_str().expect("utf-8"))`
- `crates/vox-cli/tests/test_governance.rs:31` — `r#"<?xml version="1.0" encoding="UTF-8"?>`
- `crates/vox-cli/tests/test_runtime_report_cli.rs:12` — `r#"<?xml version="1.0" encoding="UTF-8"?>`
- `crates/vox-cli/tests/test_runtime_report_cli.rs:51` — `r#"<?xml version="1.0" encoding="UTF-8"?>`
- `crates/vox-cli/src/templates/spa.rs:43` — `<meta charset="UTF-8" />`
- `crates/vox-cli/src/templates/tanstack.rs:19` — `{ charSet: "utf-8" },`
- `crates/vox-cli/src/commands/ci/test_runtime_report.rs:527` — `const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;`
- `crates/vox-cli/src/commands/ci/tier_budget_check.rs:139` — `r#"<?xml version="1.0" encoding="UTF-8"?>`
- `crates/vox-cli/src/utils/ssg/mod.rs:81` — `<meta charset="UTF-8" />`
- `crates/vox-codegen/src/web_ir/href_emit.rs:111` — `r#"<?xml version="1.0" encoding="UTF-8"?>`
- `crates/vox-orchestrator-mcp/src/http_gateway/mod.rs:611` — `<meta charset="utf-8" />`
- `crates/vox-publisher/src/adapters/rss.rs:36` — `r#"<?xml version="1.0" encoding="UTF-8" ?>`
- `crates/vox-publisher/src/scholarly/crossref_deposit.rs:64` — `r#"<?xml version="1.0" encoding="UTF-8"?>`

## [vox]path-sep-env — high  (1 un-gated, Vox scripts)

_Splitting on ':' or ';' — PATH separator differs per OS. Use std::env::split_paths._

- `scripts/quality/audit-telemetry.vox:57` — `let name = line.split(":").get(1).unwrap().split(",").get(0).unwrap().trim().replace("\"", "");`

## [vox]tempdir-slash — medium  (1 un-gated, Vox scripts)

_Hardcoded /tmp — use std::env::temp_dir() instead._

- `scripts/show/script.vox:106` — `let mut whisper_out = "/tmp/rawnerve-transcripts"`

## Asymmetric cfg files (top 30)

_One OS handled, the other absent — likely a missing platform branch._

- `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs` — windows-only (win=7, unix=0)
- `crates/vox-actor-runtime/src/builtins/mod.rs` — unix-only (win=0, unix=4)
- `crates/vox-compiler/src/eval/builtins.rs` — unix-only (win=0, unix=4)
- `crates/vox-scientia/src/replay/sandbox.rs` — windows-only (win=4, unix=0)
- `crates/vox-telemetry/src/config.rs` — windows-only (win=4, unix=0)
- `crates/vox-ml-cli/src/commands/schola/train/spawn.rs` — windows-only (win=3, unix=0)
- `crates/vox-populi/src/mens/hardware/windows_fallback.rs` — windows-only (win=3, unix=0)
- `crates/vox-repository/src/agent_scope.rs` — windows-only (win=3, unix=0)
- `crates/voxup/src/install.rs` — unix-only (win=0, unix=3)
- `crates/vox-cli/tests/command_catalog_paths_baseline.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/tests/vox_cli_root_parsing.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/src/commands/ci/compile_matrix.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/src/commands/ci/install_hooks.rs` — unix-only (win=0, unix=2)
- `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs` — windows-only (win=2, unix=0)
- `crates/vox-cli/src/utils/share/binary_cache.rs` — unix-only (win=0, unix=2)
- `crates/vox-codegen/src/codegen_rust/manifest.rs` — windows-only (win=2, unix=0)
- `crates/vox-gui/src/commands/process_util.rs` — windows-only (win=2, unix=0)
- `crates/vox-identity/src/storage.rs` — unix-only (win=0, unix=2)
- `crates/vox-langtool/src/commands/fmt.rs` — unix-only (win=0, unix=2)
- `crates/vox-ml-cli/src/commands/populi_cli.rs` — windows-only (win=2, unix=0)
- `crates/vox-orchestrator/src/process_util.rs` — windows-only (win=2, unix=0)
- `crates/vox-populi/src/mens/hardware/linux_drm.rs` — unix-only (win=0, unix=2)
- `crates/vox-populi/src/mens/hardware/macos_metal.rs` — unix-only (win=0, unix=2)
- `crates/vox-arch-check/src/forbidden_patterns.rs` — windows-only (win=1, unix=0)
- `crates/vox-cli/src/commands/share.rs` — unix-only (win=0, unix=1)
- `crates/vox-cli/src/commands/ci/runner_scale.rs` — windows-only (win=1, unix=0)
- `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/toolchain.rs` — unix-only (win=0, unix=1)
- `crates/vox-cli/src/commands/runtime/run/backend/native.rs` — windows-only (win=1, unix=0)
- `crates/vox-ml-cli/src/commands/schola/train/run_train.rs` — unix-only (win=0, unix=1)
- `crates/vox-orchestrator/src/a2a/remote_worker.rs` — unix-only (win=0, unix=1)

## Trend since baseline

_Baseline: `graphify-out\OS_COMPATIBILITY.md.prev`_

- [vox]path-sep-env: 1 → 1 (no change)
- [vox]tempdir-slash: 1 → 1 (no change)
- dynlib-ext: 33 → 33 (no change)
- env-home-asym: 11 → 11 (no change)
- file-executable-bit: 9 → 9 (no change)
- hardcoded-newline-crlf: 2 → 2 (no change)
- home-tilde: 6 → 6 (no change)
- locale-encoding: 14 → 14 (no change)
- os-unix-api: 17 → 17 (no change)
- os-windows-api: 7 → 7 (no change)
- path-join-fmt: 154 → 154 (no change)
- path-sep-env: 12 → 12 (no change)
- process-uid-gid: 0 → 0 (no change)
- shell-command: 6 → 6 (no change)
- tempdir-slash: 32 → 32 (no change)
- unix-symlink: 0 → 0 (no change)
- win-drive-path: 1 → 1 (no change)

**Net change: 0 (stable)**
