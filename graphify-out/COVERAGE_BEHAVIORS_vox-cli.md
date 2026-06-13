# Semantic Behavior Map — `vox-cli`

Deterministically synthesized from 129 distinct proven-behavior claims (of 129 extracted) across 75 symbols. 4 symbols have an explicit error-path proof; **37 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `levenshtein`  (happy; EXTRACTED)
- [happy] levenshtein('build', 'build') returns distance 0 for identical strings  (crates/vox-cli/src/diagnostics.rs)
- [happy] levenshtein('buid', 'build') returns distance 1 for single-character difference  (crates/vox-cli/src/diagnostics.rs)
- [happy] levenshtein('abc', 'xyz') returns distance 3 for completely different same-length strings  (crates/vox-cli/src/diagnostics.rs)
- [happy] levenshtein('hello', 'zzzzz') returns distance >= 4 for dissimilar strings  (crates/vox-cli/src/diagnostics.rs)
- [happy] levenshtein returns 0 for identical strings  (crates/vox-cli/src/diagnostics.rs)
- [happy] levenshtein returns 1 for strings differing by one character  (crates/vox-cli/src/diagnostics.rs)
- [happy] levenshtein returns 3 for 'abc' vs 'xyz', and >=4 for 'hello' vs 'zzzzz'  (crates/vox-cli/src/diagnostics.rs)

### `did_you_mean`  (edge, happy; EXTRACTED)
- [happy] did_you_mean('buid', ['build', 'bundle', 'check', 'run']) returns 'build' for single-char typo  (crates/vox-cli/src/diagnostics.rs)
- [edge] did_you_mean('xyz', ['build', 'bundle']) returns None when no candidate is sufficiently close  (crates/vox-cli/src/diagnostics.rs)
- [happy] did_you_mean('build', ['build', 'bundle']) returns 'build' for exact match in candidates  (crates/vox-cli/src/diagnostics.rs)
- [happy] did_you_mean returns 'build' for input 'buid' from candidates  (crates/vox-cli/src/diagnostics.rs)
- [happy] did_you_mean returns None when no candidates are close enough  (crates/vox-cli/src/diagnostics.rs)
- [happy] did_you_mean returns exact match when input matches a candidate  (crates/vox-cli/src/diagnostics.rs)

### `parse_build_number`  (edge, error, happy; EXTRACTED)
- [happy] parse_build_number('601') parses decimal string to Some(601)  (crates/vox-cli/src/freshness.rs)
- [happy] parse_build_number('  1917 ') strips whitespace and parses to Some(1917)  (crates/vox-cli/src/freshness.rs)
- [error] parse_build_number('dev') rejects non-numeric strings returning None  (crates/vox-cli/src/freshness.rs)
- [error] parse_build_number('') rejects empty string returning None  (crates/vox-cli/src/freshness.rs)
- [error] parse_build_number('12a') rejects alphanumeric strings with non-digits returning None  (crates/vox-cli/src/freshness.rs)
- [edge] parse_build_number accepts '601', ' 1917 ' and rejects 'dev', '', '12a'  (crates/vox-cli/src/freshness.rs)

### `classify`  (edge, happy; EXTRACTED)
- [happy] classify(Some(601), Some(1917)) returns Freshness::Stale when embedded version is older than live  (crates/vox-cli/src/freshness.rs)
- [happy] classify returns Freshness::Stale when embedded < live  (crates/vox-cli/src/freshness.rs)
- [happy] classify returns Freshness::Fresh when build numbers are equal  (crates/vox-cli/src/freshness.rs)
- [happy] classify returns Freshness::Fresh when binary is ahead of tree  (crates/vox-cli/src/freshness.rs)
- [edge] classify returns Freshness::Unknown when either build number is None  (crates/vox-cli/src/freshness.rs)

### `CorpusFeedbackJsonlSink::record()`  (happy, invariant; EXTRACTED)
- [happy] recorded CR-L8 events are persisted to a JSONL file in the sink directory  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [invariant] only CR-L8 events (lint, repair_outcome) are written to JSONL; model_call events are filtered out  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [happy] exactly 2 CR-L8 events are persisted when lint_event and repair_outcome_event are recorded  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [happy] deeply nested sink directories are created on first record call  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)

### `GitHub CI workflow ci.yml`  (invariant; EXTRACTED)
- [invariant] ci.yml workflow file contains either 'ci command-compliance' or 'ci ssot-drift' command invocation  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] ci.yml workflow contains 'ci doc-inventory verify' command to verify inventory  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] ci.yml workflow does NOT contain deprecated Python script 'verify_doc_inventory_fresh.py'  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] ci.yml does NOT invoke deprecated populi_release_gate.sh script  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `enrich_dei_daemon_error`  (error, happy; EXTRACTED)
- [error] enrich_dei_daemon_error wraps daemon spawn failures with a 'Hint:' section in the error output  (crates/vox-cli/src/dei_daemon.rs)
- [happy] enrich_dei_daemon_error passes through non-spawn-failure daemon errors unchanged  (crates/vox-cli/src/dei_daemon.rs)
- [happy] enrich_dei_daemon_error maps spawn failure (DAEMON_SPAWN_FAILED_PREFIX) to error containing 'Hint:' text  (crates/vox-cli/src/dei_daemon.rs)
- [happy] enrich_dei_daemon_error passes through non-spawn-failure errors unchanged  (crates/vox-cli/src/dei_daemon.rs)

### `render_markdown()`  (happy; EXTRACTED)
- [happy] markdown rendering removes fence delimiters from output prose  (crates/vox-cli/src/render.rs)
- [happy] markdown headings are normalized to uppercase in output  (crates/vox-cli/src/render.rs)
- [happy] markdown checked list items render with checkmark marker (✓)  (crates/vox-cli/src/render.rs)
- [happy] markdown unchecked list items render with circle marker (○)  (crates/vox-cli/src/render.rs)

### `GitHub CI workflow compile-matrix.yml`  (invariant; EXTRACTED)
- [invariant] compile-matrix.yml workflow runs from examples/compile-suite directory  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] compile-matrix.yml contains 'vox compile --workspace --target native-binary' smoke test  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] compile-matrix.yml contains 'vox compile --target desktop' smoke test for Tauri codegen  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `GitHub CI workflow ml_data_extraction.yml`  (invariant; EXTRACTED)
- [invariant] ml_data_extraction.yml contains 'ci grammar-drift' and '--emit github' to detect grammar drift  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] ml_data_extraction.yml contains 'corpus eval' and '--print-summary' to summarize evaluation results  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] ml_data_extraction.yml does NOT contain 'python3 -c' inline Python calls (enforces Vox/Rust CLI usage)  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `is_corpus_feedback_event()`  (error, happy; EXTRACTED)
- [happy] is_corpus_feedback_event returns true for lint events  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [happy] is_corpus_feedback_event returns true for repair outcome events  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [error] is_corpus_feedback_event returns false for model call events  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)

### `rollout_phase()`  (happy; EXTRACTED)
- [happy] rollout_phase returns Cold when latency is 15000ms with no p99 override  (crates/vox-cli/src/slo_gates.rs)
- [happy] rollout_phase returns Warm when latency is 5000ms with no p99 override  (crates/vox-cli/src/slo_gates.rs)
- [happy] rollout_phase returns Fast when latency is 5000ms with p99 override of 500ms  (crates/vox-cli/src/slo_gates.rs)

### `CorpusFeedbackJsonlSink`  (happy; EXTRACTED)
- [happy] Sink writes CR-L8 events (lint, repair_outcome) to JSONL file with quarter-based naming and filters model_call  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [happy] Sink creates nested directories lazily when recording first event  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)

### `GitHub CI workflow ci.yml Mens gate test deduplication`  (invariant; EXTRACTED)
- [invariant] When ci.yml contains 'ci mens-gate --profile ci_full', it does NOT duplicate qwen35_native_parity test  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] When ci.yml contains 'ci mens-gate --profile ci_full', it does NOT duplicate qwen35_linear_attention_forward_and_cache_progression tests  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `Packaging SSOT documentation`  (invariant; EXTRACTED)
- [invariant] Packaging SSOT document specifies that builds use '--target' to construct every member package, matching compile.rs workspace behavior  (crates/vox-cli/tests/ci_workflow_contract.rs)
- [invariant] Packaging SSOT document does NOT make promises about target filtering that compile.rs does not actually implement  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `SpoolSink.record`  (happy; EXTRACTED)
- [happy] SpoolSink.record creates pending/ subdirectory when no Tokio runtime is available  (crates/vox-cli/src/telemetry_sink.rs)
- [happy] SpoolSink.record spawns async task and creates pending/ subdirectory when Tokio runtime is available  (crates/vox-cli/src/telemetry_sink.rs)

### `extract_tsx_from_chat_response`  (error, happy; EXTRACTED)
- [happy] extract_tsx_from_chat_response returns content from .tsx file preferentially over .md files  (crates/vox-cli/src/v0.rs)
- [error] extract_tsx_from_chat_response returns error when ChatResponse has no files  (crates/vox-cli/src/v0.rs)

### `gate`  (happy; EXTRACTED)
- [happy] gate returns Err for Stale freshness when skip_stale=false, Ok when skip_stale=true  (crates/vox-cli/src/freshness.rs)
- [happy] gate returns Ok for Freshness::Fresh and Freshness::Unknown regardless of skip_stale  (crates/vox-cli/src/freshness.rs)

### `meets_phase1()`  (happy, invariant; EXTRACTED)
- [happy] meets_phase1 returns true for values below threshold 10000  (crates/vox-cli/src/slo_gates.rs)
- [invariant] meets_phase1 returns false for values at or above threshold 10000  (crates/vox-cli/src/slo_gates.rs)

### `meets_phase2_warm()`  (happy, invariant; EXTRACTED)
- [happy] meets_phase2_warm returns true for values below threshold 1000  (crates/vox-cli/src/slo_gates.rs)
- [invariant] meets_phase2_warm returns false for values at or above threshold 1000  (crates/vox-cli/src/slo_gates.rs)

### `parse_run_mode_from_str`  (happy; EXTRACTED)
- [happy] parse_run_mode_from_str maps default_run_mode_str() to RunMode::Auto, 'script' to RunMode::Script, and 'APP' to RunMode::App (case-insensitive)  (crates/vox-cli/src/compilerd.rs)
- [happy] parse_run_mode_from_str returns RunMode::Auto for default string, RunMode::Script for 'script', RunMode::App for 'APP'  (crates/vox-cli/src/compilerd.rs)

### `render_inline_code`  (happy; EXTRACTED)
- [happy] render_inline_code leaves plain text without backticks unchanged  (crates/vox-cli/src/render.rs)
- [happy] render_inline_code strips backticks and preserves text when color_off=false  (crates/vox-cli/src/render.rs)

### `render_inline_code()`  (happy; EXTRACTED)
- [happy] plain text without backticks is returned unchanged by render_inline_code with color off  (crates/vox-cli/src/render.rs)
- [happy] backtick pairs are stripped from inline code when color is off  (crates/vox-cli/src/render.rs)

### `render_markdown`  (happy; EXTRACTED)
- [happy] render_markdown indents code fence content and removes markdown fence delimiters  (crates/vox-cli/src/render.rs)
- [happy] render_markdown normalizes headings and renders list markers without fence leakage in prose  (crates/vox-cli/src/render.rs)

### `resolve_events_root`  (happy; EXTRACTED)
- [happy] resolve_events_root returns None when EVENTS_DIR_ENV is set to 'disabled' sentinel  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [happy] resolve_events_root returns env path when EVENTS_DIR_ENV is explicitly set  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)

### `resolve_events_root()`  (happy; EXTRACTED)
- [happy] resolve_events_root returns None when EVENTS_DIR_ENV is set to 'disabled' sentinel  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)
- [happy] explicit EVENTS_DIR_ENV path overrides default cwd-based resolution  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)

### `AI_CHECK`  (invariant; EXTRACTED)
- [invariant] AI_CHECK constant equals 'ai.check'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_CHECK constant`  (invariant; EXTRACTED)
- [invariant] AI_CHECK constant equals 'ai.check'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_FIX constant`  (invariant; EXTRACTED)
- [invariant] AI_FIX constant equals 'ai.fix'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_GENERATE constant`  (invariant; EXTRACTED)
- [invariant] AI_GENERATE constant equals 'ai.generate'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_PLAN_EXECUTE constant`  (invariant; EXTRACTED)
- [invariant] AI_PLAN_EXECUTE constant equals 'ai.plan.execute'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_PLAN_NEW`  (invariant; EXTRACTED)
- [invariant] AI_PLAN_NEW, AI_PLAN_REPLAN, AI_PLAN_STATUS, AI_PLAN_EXECUTE have correct method IDs  (crates/vox-cli/src/dei_daemon.rs)

### `AI_PLAN_NEW constant`  (invariant; EXTRACTED)
- [invariant] AI_PLAN_NEW constant equals 'ai.plan.new'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_PLAN_REPLAN constant`  (invariant; EXTRACTED)
- [invariant] AI_PLAN_REPLAN constant equals 'ai.plan.replan'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_PLAN_STATUS constant`  (invariant; EXTRACTED)
- [invariant] AI_PLAN_STATUS constant equals 'ai.plan.status'  (crates/vox-cli/src/dei_daemon.rs)

### `AI_REVIEW constant`  (invariant; EXTRACTED)
- [invariant] AI_REVIEW constant equals 'ai.review'  (crates/vox-cli/src/dei_daemon.rs)

### `CONFIG_GET constant`  (invariant; EXTRACTED)
- [invariant] CONFIG_GET constant equals 'config.get'  (crates/vox-cli/src/dei_daemon.rs)

### `CatalogTier::FeatureGated`  (invariant; EXTRACTED)
- [invariant] Feature-gated command 'mens' is marked with FeatureGated tier and feature_gate='mens-base|gpu'  (crates/vox-cli/src/command_catalog.rs)

### `CatalogTier::Recommended`  (invariant; EXTRACTED)
- [invariant] Recommended tier includes starter commands: build, check, run, test, bundle, dev, doctor, completions  (crates/vox-cli/src/command_catalog.rs)

### `Command registry configuration`  (invariant; EXTRACTED)
- [invariant] command-registry.yaml lists 'vox ci retirement-audit' command  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `CompileArgsHarness.file`  (happy; EXTRACTED)
- [happy] Parsing trailing file argument correctly captures the file path as 'foo.vox'  (crates/vox-cli/src/cli_args.rs)

### `CompileKind::Desktop`  (happy; EXTRACTED)
- [happy] Parsing --target desktop sets CompileKind to Desktop  (crates/vox-cli/src/cli_args.rs)

### `DevParams`  (happy; EXTRACTED)
- [happy] DevParams deserializes lowercase 'server' target string to BuildTargetArg::Server enum variant  (crates/vox-cli/src/compilerd.rs)

### `GitHub CI workflow Rust toolchain`  (invariant; EXTRACTED)
- [invariant] ci.yml Rust toolchain includes llvm-tools-preview for cargo-llvm-cov support  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `GitHub CI workflow ci.yml Mens gate configuration`  (invariant; EXTRACTED)
- [invariant] ci.yml contains 'ci mens-gate --profile ci_full' for unified gate profile  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `GitHub CI workflow ci.yml coverage configuration`  (invariant; EXTRACTED)
- [invariant] ci.yml contains 'cargo llvm-cov nextest --workspace' command to run workspace tests under LLVM coverage  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `GitHub CI workflow ci.yml coverage gates`  (invariant; EXTRACTED)
- [invariant] ci.yml contains 'ci coverage-gates' and '--mode enforce' to run coverage enforcement gates after llvm-cov  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `GitHub CI workflow ci.yml runner configuration`  (invariant; EXTRACTED)
- [invariant] ci.yml main test job runs on 'self-hosted, linux, x64' runner  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `GitHub CI workflow ci.yml test execution`  (invariant; EXTRACTED)
- [invariant] Linux CI executes both 'cargo llvm-cov nextest --workspace' and 'cargo nextest run --workspace' for comprehensive test coverage  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `IsolationCapabilities::detect`  (invariant; EXTRACTED)
- [invariant] IsolationCapabilities::detect always includes Permissive in supported policies  (crates/vox-cli/src/isolation.rs)

### `IsolationPolicy::from_str`  (happy; EXTRACTED)
- [happy] IsolationPolicy::from_str parses 'permissive','container','wasm','wasi','docker','gvisor','runsc','hyperv' and rejects 'fast'  (crates/vox-cli/src/isolation.rs)

### `Packaging SSOT Windows binary fallback documentation`  (invariant; EXTRACTED)
- [invariant] Packaging SSOT document explains that vox.exe is used when cargo run fails to relink on Windows  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `RunParams`  (happy; EXTRACTED)
- [happy] RunParams deserializes mode='script', args=['a','b'], and port=3000 from JSON  (crates/vox-cli/src/compilerd.rs)

### `RunParams default mode`  (happy; EXTRACTED)
- [happy] RunParams with only 'file' field deserializes with mode='auto' and empty args  (crates/vox-cli/src/compilerd.rs)

### `SpoolSink::record()`  (happy; EXTRACTED)
- [happy] record does not panic when called outside a tokio runtime and creates pending directory  (crates/vox-cli/src/telemetry_sink.rs)

### `Windows stack wrapper test guards`  (invariant; EXTRACTED)
- [invariant] Large-stack test helpers in test files remain protected with #[cfg(windows)] and #[cfg(not(windows))] guards and not consolidated into single codebase  (crates/vox-cli/tests/ci_workflow_contract.rs)

### `build_catalog`  (invariant; EXTRACTED)
- [invariant] Catalog includes required top-level commands: build, check, run, doctor, commands, ci  (crates/vox-cli/src/command_catalog.rs)

### `build_number_from_version_line`  (happy; EXTRACTED)
- [happy] build_number_from_version_line parses '601' and '1917' from version strings but rejects 'dev' variants  (crates/vox-cli/src/freshness.rs)

### `distinct_build_numbers`  (happy; EXTRACTED)
- [happy] distinct_build_numbers deduplicates and filters None values, returning sorted unique numbers  (crates/vox-cli/src/freshness.rs)

### `fetch_v0_tsx_with`  (happy; EXTRACTED)
- [happy] fetch_v0_tsx_with hits the parameterized API URL and returns expected export function  (crates/vox-cli/src/v0.rs)

### `is_cargo_build_dir`  (happy; EXTRACTED)
- [happy] is_cargo_build_dir returns true for /repo/target/* paths and false for ~/.cargo/bin, ~/.vox/bin, /usr/local/bin  (crates/vox-cli/src/freshness.rs)

### `is_corpus_feedback_event`  (happy; EXTRACTED)
- [happy] is_corpus_feedback_event returns true for lint and repair_outcome events but false for model_call  (crates/vox-cli/src/telemetry_corpus_feedback_sink.rs)

### `meets_phase1`  (happy; EXTRACTED)
- [happy] meets_phase1 returns true for values <10000 and false for >=10000  (crates/vox-cli/src/slo_gates.rs)

### `meets_phase2_warm`  (happy; EXTRACTED)
- [happy] meets_phase2_warm returns true for values <1000 and false for >=1000  (crates/vox-cli/src/slo_gates.rs)

### `render_code_block`  (happy; EXTRACTED)
- [happy] render_code_block produces output without ANSI escape sequences and with box-drawing borders and indentation  (crates/vox-cli/src/render.rs)

### `render_text`  (happy; EXTRACTED)
- [happy] Rendered catalog text includes command name 'vox build' and tier label 'recommended'  (crates/vox-cli/src/command_catalog.rs)

### `rollout_phase`  (happy; EXTRACTED)
- [happy] rollout_phase returns Cold for 15000, Warm for 5000 with no cache, Fast for 5000 with 500ms cache  (crates/vox-cli/src/slo_gates.rs)

### `search_entries`  (happy; EXTRACTED)
- [happy] Search for 'shell' returns non-empty results containing commands with 'shell' in name  (crates/vox-cli/src/command_catalog.rs)

### `search_entries with empty pattern`  (happy; EXTRACTED)
- [happy] Empty pattern returns all catalog entries  (crates/vox-cli/src/command_catalog.rs)

### `select_entries with include_nested=false`  (happy; EXTRACTED)
- [happy] Excluding nested paths returns only entries with path.len()==1  (crates/vox-cli/src/command_catalog.rs)

### `select_entries with include_nested=true`  (happy; EXTRACTED)
- [happy] Including nested paths returns at least one entry with path.len()>1  (crates/vox-cli/src/command_catalog.rs)

### `select_entries with recommended=true`  (happy; EXTRACTED)
- [happy] Recommended tier selection returns only top-level (path.len()==1) commands  (crates/vox-cli/src/command_catalog.rs)

### `vox ci pre-push --dry-run --act output format`  (happy; EXTRACTED)
- [happy] Dry-run act output contains both 'DRY-RUN:' and 'push --workflows' text markers to indicate dry-run mode  (crates/vox-cli/tests/ci_pre_push.rs)

### `vox ci pre-push --dry-run --quick --act command`  (happy; EXTRACTED)
- [happy] Command executes successfully and stdout contains workflow workflow labels from .github/workflows/ directory (docs-quality.yml, link_checker.yml, ts-emit-noemit.yml) with 'act:' prefix  (crates/vox-cli/tests/ci_pre_push.rs)

### `vox ci pre-push command with --enforce-budgets and --dry-run flags`  (happy; EXTRACTED)
- [happy] Vox CLI accepts --enforce-budgets flag with --dry-run without failing (status.success() assertion passes)  (crates/vox-cli/tests/ci_pre_push.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`CompileArgsHarness.file`** — only: _Parsing trailing file argument correctly captures the file path as 'foo.vox'_
- **`CompileKind::Desktop`** — only: _Parsing --target desktop sets CompileKind to Desktop_
- **`CorpusFeedbackJsonlSink`** — only: _Sink writes CR-L8 events (lint, repair_outcome) to JSONL file with quarter-based naming and filters model_call_
- **`DevParams`** — only: _DevParams deserializes lowercase 'server' target string to BuildTargetArg::Server enum variant_
- **`IsolationPolicy::from_str`** — only: _IsolationPolicy::from_str parses 'permissive','container','wasm','wasi','docker','gvisor','runsc','hyperv' and rejects 'fast'_
- **`RunParams`** — only: _RunParams deserializes mode='script', args=['a','b'], and port=3000 from JSON_
- **`RunParams default mode`** — only: _RunParams with only 'file' field deserializes with mode='auto' and empty args_
- **`SpoolSink.record`** — only: _SpoolSink.record creates pending/ subdirectory when no Tokio runtime is available_
- **`SpoolSink::record()`** — only: _record does not panic when called outside a tokio runtime and creates pending directory_
- **`build_number_from_version_line`** — only: _build_number_from_version_line parses '601' and '1917' from version strings but rejects 'dev' variants_
- **`distinct_build_numbers`** — only: _distinct_build_numbers deduplicates and filters None values, returning sorted unique numbers_
- **`fetch_v0_tsx_with`** — only: _fetch_v0_tsx_with hits the parameterized API URL and returns expected export function_
- **`gate`** — only: _gate returns Err for Stale freshness when skip_stale=false, Ok when skip_stale=true_
- **`is_cargo_build_dir`** — only: _is_cargo_build_dir returns true for /repo/target/* paths and false for ~/.cargo/bin, ~/.vox/bin, /usr/local/bin_
- **`is_corpus_feedback_event`** — only: _is_corpus_feedback_event returns true for lint and repair_outcome events but false for model_call_
- **`levenshtein`** — only: _levenshtein('build', 'build') returns distance 0 for identical strings_
- **`meets_phase1`** — only: _meets_phase1 returns true for values <10000 and false for >=10000_
- **`meets_phase2_warm`** — only: _meets_phase2_warm returns true for values <1000 and false for >=1000_
- **`parse_run_mode_from_str`** — only: _parse_run_mode_from_str maps default_run_mode_str() to RunMode::Auto, 'script' to RunMode::Script, and 'APP' to RunMode::App (case-insensitive)_
- **`render_code_block`** — only: _render_code_block produces output without ANSI escape sequences and with box-drawing borders and indentation_
- **`render_inline_code`** — only: _render_inline_code leaves plain text without backticks unchanged_
- **`render_inline_code()`** — only: _plain text without backticks is returned unchanged by render_inline_code with color off_
- **`render_markdown`** — only: _render_markdown indents code fence content and removes markdown fence delimiters_
- **`render_markdown()`** — only: _markdown rendering removes fence delimiters from output prose_
- **`render_text`** — only: _Rendered catalog text includes command name 'vox build' and tier label 'recommended'_
- **`resolve_events_root`** — only: _resolve_events_root returns None when EVENTS_DIR_ENV is set to 'disabled' sentinel_
- **`resolve_events_root()`** — only: _resolve_events_root returns None when EVENTS_DIR_ENV is set to 'disabled' sentinel_
- **`rollout_phase`** — only: _rollout_phase returns Cold for 15000, Warm for 5000 with no cache, Fast for 5000 with 500ms cache_
- **`rollout_phase()`** — only: _rollout_phase returns Cold when latency is 15000ms with no p99 override_
- **`search_entries`** — only: _Search for 'shell' returns non-empty results containing commands with 'shell' in name_
- **`search_entries with empty pattern`** — only: _Empty pattern returns all catalog entries_
- **`select_entries with include_nested=false`** — only: _Excluding nested paths returns only entries with path.len()==1_
- **`select_entries with include_nested=true`** — only: _Including nested paths returns at least one entry with path.len()>1_
- **`select_entries with recommended=true`** — only: _Recommended tier selection returns only top-level (path.len()==1) commands_
- **`vox ci pre-push --dry-run --act output format`** — only: _Dry-run act output contains both 'DRY-RUN:' and 'push --workflows' text markers to indicate dry-run mode_
- **`vox ci pre-push --dry-run --quick --act command`** — only: _Command executes successfully and stdout contains workflow workflow labels from .github/workflows/ directory (docs-quality.yml, link_checker.yml, ts-emit-noemit.yml) with 'act:' prefix_
- **`vox ci pre-push command with --enforce-budgets and --dry-run flags`** — only: _Vox CLI accepts --enforce-budgets flag with --dry-run without failing (status.success() assertion passes)_
