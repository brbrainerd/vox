# Semantic Behavior Map — `vox-code-audit`

Deterministically synthesized from 336 distinct proven-behavior claims (of 337 extracted) across 97 symbols. 5 symbols have an explicit error-path proof; **47 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `AiLazinessDetector`  (edge, happy; EXTRACTED)
- [happy] detects functions with assertion-only bodies  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] detects functions with early-return-only bodies  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] detects functions with only log statements as hollow/lazy implementations  (crates/vox-code-audit/src/detectors/ai_laziness.rs)
- [happy] detects functions with only early return statements  (crates/vox-code-audit/src/detectors/ai_laziness.rs)
- [happy] detects functions with log statement followed by early return  (crates/vox-code-audit/src/detectors/ai_laziness.rs)
- [happy] Detector fires ai-laziness/placeholder-return on functions with TODO string returns  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] Detector fires ai-laziness/implement-later-comment on functions with 'implement later' comments  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] Detector fires ai-laziness/mock-named-fn on functions named mock_* outside test directories  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [edge] Detector is silent on mock_* functions when file is in tests/ directory  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] Detector fires ai-laziness/custom-type-default-return on custom types using default() returns  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [edge] Detector is silent on builtin types (Vec) using default() or equivalent returns  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] Detector fires ai-laziness/conditional-stub on functions with if-based early returns  (crates/vox-code-audit/tests/wave_b_parity.rs)
- … +11 more claims

### `RetiredDecoratorDetector`  (edge, happy, invariant; EXTRACTED)
- [happy] bare `component` keyword is not flagged as retired  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] @endpoint(kind: server) fires with message and suggestion to @server  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] @endpoint(kind: query) fires with message and suggestion to @query  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] @endpoint(kind: mutation) fires with message and suggestion to @mutation  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] bare-form decorators @server, @query, @mutation are not flagged  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] canonical bare-form decorators @server, @query, @mutation are not flagged  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] @py.import decorator fires with message  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [edge] block comments containing decorators are not flagged  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [edge] detector does not fire on .rs Rust files  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] all three retired endpoint kinds are detected independently in same file  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] multiple different retired decorator patterns are detected in same file  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- [happy] Detector fires with one finding when encountering @component fn decorator syntax  (crates/vox-code-audit/src/detectors/retired_decorator.rs)
- … +3 more claims

### `SecretDetector`  (edge, happy; EXTRACTED)
- [happy] detects hardcoded AWS keys and emits security/hardcoded-secret/aws-key rule at correct line  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [happy] detects generic hardcoded secrets and emits security/hardcoded-secret/generic rule  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [edge] does not emit findings for example/documentation AWS key AKIAIOSFODNN7EXAMPLE  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [edge] does not emit findings for synthetic uniform AWS keys  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [edge] does not emit findings for environment variable reads  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [edge] does not emit findings for secrets in comments  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [edge] does not emit findings for secrets in trailing Rust comments  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [happy] detects AWS access key IDs (AKIA pattern with 16 hex characters) and emits findings with rule_id security/hardcoded-secret/aws-key containing 'AWS' in message  (crates/vox-code-audit/src/detectors/secrets.rs)
- [happy] detects generic hardcoded passwords via pattern matching and emits findings with rule_id security/hardcoded-secret/generic containing 'hardcoded secret'  (crates/vox-code-audit/src/detectors/secrets.rs)
- [edge] does not flag AWS example keys (AKIAIOSFODNN7EXAMPLE pattern) - excluded from detection  (crates/vox-code-audit/src/detectors/secrets.rs)
- [edge] does not flag synthetic AWS keys with uniform repeated characters (e.g., AKIAZZZZZZZZZZZZZZ) treated as test fixtures  (crates/vox-code-audit/src/detectors/secrets.rs)
- [edge] does not flag reading environment variables via std::env::var() even when passed secret-named keys  (crates/vox-code-audit/src/detectors/secrets.rs)
- … +3 more claims

### `RetiredCrateImportDetector`  (happy, invariant; EXTRACTED)
- [happy] Detector fires with one finding when Rust code imports vox_ludus crate  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Finding message identifies vox_ludus as the retired crate  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Finding message recommends vox-gamify as replacement  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Detector fires with one finding when Rust code uses vox_sherpa_transcribe  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Finding message recommends vox-tauri-stt replacement  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Detector fires with one finding when Cargo.toml declares vox-ludus dependency  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Finding message identifies vox-ludus as retired  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Detector fires with one finding when Cargo.toml declares vox-sherpa-transcribe with workspace config  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [happy] Detector does not fire when code imports canonical vox_gamify crate  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [invariant] Detector respects word boundaries and does not match ludus as substring of longer identifiers  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)
- [invariant] Finding assigns stable diagnostic_id matching catalog::RETIRED_CRATE_IMPORT  (crates/vox-code-audit/src/detectors/retired_crate_import.rs)

### `TokenMap`  (edge, happy; EXTRACTED)
- [happy] TokenMap.is_non_code_byte() returns true for line comments  (crates/vox-code-audit/src/analysis/token_map.rs)
- [happy] TokenMap.is_comment_byte() returns true for line comment positions  (crates/vox-code-audit/src/analysis/token_map.rs)
- [happy] TokenMap.is_code_byte() returns true for actual code positions  (crates/vox-code-audit/src/analysis/token_map.rs)
- [edge] TokenMap.is_string_byte() returns true for // inside string literals  (crates/vox-code-audit/src/analysis/token_map.rs)
- [edge] TokenMap.is_comment_byte() returns false for // inside string literals  (crates/vox-code-audit/src/analysis/token_map.rs)
- [edge] TokenMap correctly marks nested block comments as non-code  (crates/vox-code-audit/src/analysis/token_map.rs)
- [edge] TokenMap.is_code_byte() returns true for code after nested block comments  (crates/vox-code-audit/src/analysis/token_map.rs)
- [happy] TokenMap.is_string_byte() returns true for raw string content  (crates/vox-code-audit/src/analysis/token_map.rs)
- [edge] TokenMap.is_comment_byte() returns false for raw string content  (crates/vox-code-audit/src/analysis/token_map.rs)
- [happy] TokenMap.is_comment_byte() returns true for trailing comment text  (crates/vox-code-audit/src/analysis/token_map.rs)
- [happy] TokenMap.is_code_byte() returns true for code before trailing comment  (crates/vox-code-audit/src/analysis/token_map.rs)

### `CryptoBanDetector`  (edge, happy; EXTRACTED)
- [happy] detects ring crate imports in Rust files  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] detects aegis crate via extern crate declaration  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] detects ring dependency in Cargo.toml manifest  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] detects aws-lc-rs dependency in Cargo.toml  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] detects aegis imports in Vox source files  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] CryptoBanDetector detects md5 cryptographic imports in Rust code  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] CryptoBanDetector detects sha1 cryptographic imports in Rust code  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] CryptoBanDetector does not flag allowed chacha20poly1305 cryptographic imports  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [edge] CryptoBanDetector ignores cryptographic imports in Rust comment lines  (crates/vox-code-audit/src/detectors/crypto_ban.rs)
- [happy] CryptoBanDetector does not flag non-cryptographic dependencies in Cargo.toml  (crates/vox-code-audit/src/detectors/crypto_ban.rs)

### `HollowFnDetector`  (edge, happy; EXTRACTED)
- [happy] detects functions that return Ok(()) as hollow functions with rule_id skeleton/hollow-fn  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] detects functions that return a constant boolean value as hollow functions with rule_id skeleton/hollow-fn  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] detects functions that return Vec::new() as hollow functions with rule_id skeleton/hollow-fn  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] detects functions that return Default::default() as hollow functions with rule_id skeleton/hollow-fn  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] detects functions that return type-specific default (e.g., Response::default()) as hollow functions with rule_id skeleton/hollow-fn  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [edge] does not flag functions defined inside test modules even if they have hollow patterns  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] HollowFnDetector.detect() returns empty findings for functions with real logic (e.g., 'a + b' in add function)  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] HollowFnDetector.detect() returns empty findings for functions suppressed with toestub-ignore(skeleton) comment  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] HollowFnDetector.detect() detects TypeScript hollow functions and returns Finding with rule_id 'skeleton/hollow-fn'  (crates/vox-code-audit/src/detectors/hollow_fn.rs)
- [happy] HollowFnDetector.detect() returns empty findings for TypeScript functions with real logic (return a + b)  (crates/vox-code-audit/src/detectors/hollow_fn.rs)

### `AdrCitationDetector`  (error, happy; EXTRACTED)
- [happy] AdrCitationDetector fires on public functions without ADR citations in critical crates  (crates/vox-code-audit/src/detectors/adr_citation.rs)
- [happy] AdrCitationDetector assigns Warning severity to missing ADR citations in critical crates  (crates/vox-code-audit/src/detectors/adr_citation.rs)
- [happy] AdrCitationDetector does not fire when ADR citation is present  (crates/vox-code-audit/src/detectors/adr_citation.rs)
- [happy] AdrCitationDetector fires on functions without ADR in non-critical crates  (crates/vox-code-audit/src/detectors/adr_citation.rs)
- [happy] AdrCitationDetector assigns Info severity to missing ADR citations in non-critical crates  (crates/vox-code-audit/src/detectors/adr_citation.rs)
- [error] AdrCitationDetector fires on T-number citations instead of ADR citations  (crates/vox-code-audit/src/detectors/adr_citation.rs)
- [error] AdrCitationDetector assigns Warning severity to T-number citations  (crates/vox-code-audit/src/detectors/adr_citation.rs)
- [happy] TASK-N.M citations are not flagged as violations (findings is empty)  (crates/vox-code-audit/src/detectors/adr_citation.rs)

### `RetiredCapacitorDetector`  (happy; EXTRACTED)
- [happy] Detector fires with one finding when TypeScript imports from @capacitor/filesystem  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)
- [happy] Finding message contains suggested replacement @tauri-apps/plugin-filesystem  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)
- [happy] Detector fires with one finding when package.json contains @capacitor/camera dependency  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)
- [happy] Finding message contains suggested replacement @tauri-apps/plugin-camera  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)
- [happy] Detector fires with one finding when shell script contains npx cap sync  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)
- [happy] Finding message contains cargo tauri replacement hint  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)
- [happy] Detector fires with one finding when shell script contains npx cap run  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)
- [happy] Finding message mentions npx cap run command  (crates/vox-code-audit/src/detectors/retired_capacitor.rs)

### `DecoratorPositionDetector`  (edge, error, happy, invariant; EXTRACTED)
- [happy] DecoratorPositionDetector fires when bare durable keyword is used instead of @durable decorator  (crates/vox-code-audit/src/detectors/decorator_position.rs)
- [happy] DecoratorPositionDetector does not fire when correct @durable fn syntax is used  (crates/vox-code-audit/src/detectors/decorator_position.rs)
- [error] DecoratorPositionDetector fires on redundant @actor actor syntax  (crates/vox-code-audit/src/detectors/decorator_position.rs)
- [happy] DecoratorPositionDetector does not fire when bare actor keyword is used correctly  (crates/vox-code-audit/src/detectors/decorator_position.rs)
- [edge] DecoratorPositionDetector ignores decorator position issues in comment lines  (crates/vox-code-audit/src/detectors/decorator_position.rs)
- [invariant] DecoratorPositionDetector does not analyze Rust source files  (crates/vox-code-audit/src/detectors/decorator_position.rs)
- [happy] DecoratorPositionDetector fires when bare pure keyword is used instead of @pure decorator  (crates/vox-code-audit/src/detectors/decorator_position.rs)

### `StubDetector`  (happy; EXTRACTED)
- [happy] detects todo!() macro invocations with rule_id stub/todo at correct line number  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] detects unimplemented!() macro with rule_id stub/unimplemented  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] detects Python NotImplementedError raises with rule_id stub/not-implemented-error  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] detects Python pass statements with rule_id stub/pass  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] detects TypeScript throw new Error('not implemented') with rule_id stub/throw-not-implemented  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] does not fire on properly implemented Rust functions  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] StubDetector detects all-caps PLACEHOLDER markers as stub findings  (crates/vox-code-audit/src/detectors/stub.rs)

### `UnwrapCallDetector`  (edge, happy; EXTRACTED)
- [happy] detects .unwrap() calls in production source code with rule_id rust/unwrap-call  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [edge] does not fire on .unwrap() calls in test directories  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [edge] Unwrap calls inside #[cfg(test)] mod blocks are skipped; only production code lines are detected  (crates/vox-code-audit/src/detectors/unwrap_call.rs)
- [edge] Unwrap calls inside single-line #[cfg(test)] mod declarations are skipped  (crates/vox-code-audit/src/detectors/unwrap_call.rs)
- [edge] Modules with #[cfg(all(test, feature = ...))] are completely skipped; detect() returns zero findings  (crates/vox-code-audit/src/detectors/unwrap_call.rs)
- [happy] Code with #[cfg(not(test))] attribute still produces findings and is not treated as test code  (crates/vox-code-audit/src/detectors/unwrap_call.rs)
- [edge] Files matching pattern *_tests_body.rs are completely skipped; detect() returns zero findings  (crates/vox-code-audit/src/detectors/unwrap_call.rs)

### `LintFindingEvent`  (happy; EXTRACTED)
- [happy] LintFindingEvent has rule_id matching the detector rule and severity of 'warning' or 'error'  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [happy] LintFindingEvent.diagnostic_id is set to stable catalog ID (vox/rule-id format)  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [happy] LintFindingEvent.autofix_available is true when detector provides suggestions  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [happy] LintFindingEvent.confidence is set to 'high' by detectors  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [happy] LintFindingEvent.repository_id is populated from ToestubConfig.repository_id  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [happy] LintFindingEvent.repository_id is None when ToestubConfig does not set it  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `MagicValueDetector`  (edge, happy; EXTRACTED)
- [happy] detects hardcoded database connection strings and emits magic-value/db-conn rule  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [happy] detect() identifies database connection strings in source code via findings with rule_id 'magic-value/db-conn'  (crates/vox-code-audit/src/detectors/magic_value.rs)
- [edge] MagicValueDetector does not fire on const definitions with magic values  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [edge] MagicValueDetector skips magic values in comment lines  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [edge] MagicValueDetector does not flag ephemeral port zero (0) as a magic value  (crates/vox-code-audit/tests/wave_c_parity.rs)
- [happy] detects hardcoded port strings and assigns rule_id magic-value/port  (crates/vox-code-audit/src/detectors/magic_value.rs)

### `ScalingSurfacesDetector`  (edge, happy; EXTRACTED)
- [happy] Detects blocking filesystem read_to_string call inside async function as scaling/blocking-in-async.  (crates/vox-code-audit/src/detectors/scaling_tests.rs)
- [edge] Does not flag blocking filesystem operations inside #[cfg(test)] test modules.  (crates/vox-code-audit/src/detectors/scaling_tests.rs)
- [happy] Detects filesystem read inside loop as scaling/cache-miss-hot-read heuristic violation.  (crates/vox-code-audit/src/detectors/scaling_tests.rs)
- [happy] Detects Vec::with_capacity call with large value (250_000) as scaling/large-in-memory-accumulator.  (crates/vox-code-audit/src/detectors/scaling_tests.rs)
- [edge] Does not flag Regex::new pattern when it appears inside a string literal.  (crates/vox-code-audit/src/detectors/scaling_tests.rs)
- [happy] detects duplicate env::var() calls with identical unwrap_or() defaults using rule scaling/env-default-duplication  (crates/vox-code-audit/src/detectors/scaling_tests.rs)

### `TrainingEligibleDetector`  (happy, invariant; EXTRACTED)
- [happy] TrainingEligibleDetector flags files marked training_eligible: true that import from archive modules  (crates/vox-code-audit/src/detectors/training_eligible.rs)
- [invariant] TrainingEligibleDetector skips files without any training_eligible marker  (crates/vox-code-audit/src/detectors/training_eligible.rs)
- [invariant] TrainingEligibleDetector does not flag files marked training_eligible: false  (crates/vox-code-audit/src/detectors/training_eligible.rs)
- [happy] TrainingEligibleDetector flags files marked training_eligible: true that import from deprecated modules  (crates/vox-code-audit/src/detectors/training_eligible.rs)
- [happy] TrainingEligibleDetector flags Vox files marked training_eligible: true that import from legacy modules  (crates/vox-code-audit/src/detectors/training_eligible.rs)
- [invariant] TrainingEligibleDetector does not flag normal (non-archive/deprecated/legacy) imports even in training_eligible: true files  (crates/vox-code-audit/src/detectors/training_eligible.rs)

### `UntestedPubApiDetector`  (edge, happy; EXTRACTED)
- [happy] Flags library files containing public functions without test coverage  (crates/vox-code-audit/src/detectors/untested_pub_api.rs)
- [happy] Returns empty findings when #[cfg(test)] module is present  (crates/vox-code-audit/src/detectors/untested_pub_api.rs)
- [happy] Returns empty findings when #[tokio::test] attribute is present  (crates/vox-code-audit/src/detectors/untested_pub_api.rs)
- [edge] Skips main.rs files regardless of public function presence  (crates/vox-code-audit/src/detectors/untested_pub_api.rs)
- [edge] Skips files with fewer than 30 non-blank lines  (crates/vox-code-audit/src/detectors/untested_pub_api.rs)
- [edge] Skips files containing only private functions  (crates/vox-code-audit/src/detectors/untested_pub_api.rs)

### `EnvSecretShapeDetector`  (edge, happy; EXTRACTED)
- [happy] detects env variable names with TOKEN suffix in Vox source files  (crates/vox-code-audit/src/detectors/env_secret_shape.rs)
- [happy] detects env variable names with PASSWORD suffix in Rust source files  (crates/vox-code-audit/src/detectors/env_secret_shape.rs)
- [edge] does not flag environment variable names that lack secret-shaped suffixes (e.g., DATABASE_HOST)  (crates/vox-code-audit/src/detectors/env_secret_shape.rs)
- [edge] does not flag environment variable patterns when they appear in comment lines  (crates/vox-code-audit/src/detectors/env_secret_shape.rs)
- [happy] Detects environment variable access with secret-shaped names (API_KEY pattern) and assigns diagnostic_id vox/secret/env-get-shape  (crates/vox-code-audit/src/detectors/env_secret_shape.rs)

### `LlmProviderCallDetector`  (edge, happy; EXTRACTED)
- [happy] detects direct OpenAI API calls in Vox code with diagnostic_id vox/llm/direct-provider-call  (crates/vox-code-audit/src/detectors/llm_provider_call.rs)
- [happy] detects Anthropic LLM provider calls via reqwest HTTP client in Rust  (crates/vox-code-audit/src/detectors/llm_provider_call.rs)
- [edge] does not flag populi.complete() API calls  (crates/vox-code-audit/src/detectors/llm_provider_call.rs)
- [edge] ignores LLM provider hostnames when they appear only in comments  (crates/vox-code-audit/src/detectors/llm_provider_call.rs)
- [edge] does not flag URL strings without an associated HTTP call  (crates/vox-code-audit/src/detectors/llm_provider_call.rs)

### `PanickingBuiltinDetector`  (edge, happy; EXTRACTED)
- [happy] PanickingBuiltinDetector fires on panic! macro invocations within @activity decorated functions  (crates/vox-code-audit/src/detectors/panicking_builtin.rs)
- [happy] PanickingBuiltinDetector fires on todo! macro invocations inside actor block functions  (crates/vox-code-audit/src/detectors/panicking_builtin.rs)
- [edge] PanickingBuiltinDetector does not fire on Result propagation operator (?) in @handler functions  (crates/vox-code-audit/src/detectors/panicking_builtin.rs)
- [happy] detect() fires for .unwrap() calls within @handler-decorated functions with message containing 'unwrap'  (crates/vox-code-audit/src/detectors/panicking_builtin.rs)
- [edge] detect() produces no findings for .unwrap() calls in non-handler utility functions  (crates/vox-code-audit/src/detectors/panicking_builtin.rs)

### `PureFnImpureDetector`  (edge, happy; EXTRACTED)
- [happy] PureFnImpureDetector flags @pure functions that call http.get impure operations  (crates/vox-code-audit/src/detectors/pure_fn_impure.rs)
- [edge] PureFnImpureDetector does not fire on @pure functions containing only pure arithmetic operations  (crates/vox-code-audit/src/detectors/pure_fn_impure.rs)
- [happy] PureFnImpureDetector flags @pure functions that call impure random.int operations  (crates/vox-code-audit/src/detectors/pure_fn_impure.rs)
- [edge] PureFnImpureDetector does not fire on non-Vox source files (e.g., .rs files)  (crates/vox-code-audit/src/detectors/pure_fn_impure.rs)
- [edge] PureFnImpureDetector does not flag regular (non-@pure) functions that make impure calls  (crates/vox-code-audit/src/detectors/pure_fn_impure.rs)

### `RetiredEnvVarDetector`  (edge, happy; EXTRACTED)
- [happy] Detects retired env::var call with TURSO_URL and returns exactly one finding with High confidence.  (crates/vox-code-audit/src/detectors/retired_env_var.rs)
- [happy] Detects retired std::env::var call with VOX_TURSO_TOKEN and returns exactly one finding with High confidence.  (crates/vox-code-audit/src/detectors/retired_env_var.rs)
- [happy] Detects bare string literal containing retired env var name VOX_TURSO_URL with Medium confidence.  (crates/vox-code-audit/src/detectors/retired_env_var.rs)
- [happy] Does not emit findings when code uses canonical (non-retired) env var VOX_DB_URL.  (crates/vox-code-audit/src/detectors/retired_env_var.rs)
- [edge] Skips detection when file is not Rust or Vox source (e.g., JSON config files).  (crates/vox-code-audit/src/detectors/retired_env_var.rs)

### `Scanner`  (happy; EXTRACTED)
- [happy] Scanner filters files by language extension, returning only .rs files with Language::Rust annotation when scanning a mixed directory  (crates/vox-code-audit/src/scanner.rs)
- [happy] Scanner respects language filter parameter, returning only Language::TypeScript files when filter is set to [Language::TypeScript] in a mixed directory  (crates/vox-code-audit/src/scanner.rs)
- [happy] Scanner.scan() detects Rust files by extension and returns them with Language::Rust  (crates/vox-code-audit/src/scanner.rs)
- [happy] Scanner.scan() ignores non-code files like .txt when scanning a directory  (crates/vox-code-audit/src/scanner.rs)
- [happy] Scanner with language filter returns only files matching the specified Language  (crates/vox-code-audit/src/scanner.rs)

### `UnresolvedRefDetector::detect()`  (edge, happy; EXTRACTED)
- [edge] Returns empty findings when encountering a prelude glob import that may provide symbols  (crates/vox-code-audit/src/detectors/unresolved_ref.rs)
- [happy] Flags unknown function calls even when arbitrary module glob imports are present  (crates/vox-code-audit/src/detectors/unresolved_ref.rs)
- [edge] Returns empty findings when file path contains tests/ directory  (crates/vox-code-audit/src/detectors/unresolved_ref.rs)
- [edge] Returns empty findings when function-like tokens appear only inside double-quoted string literals  (crates/vox-code-audit/src/detectors/unresolved_ref.rs)
- [edge] Returns empty findings for module-qualified function calls like m::foo()  (crates/vox-code-audit/src/detectors/unresolved_ref.rs)

### `UnwiredModuleDetector`  (edge, happy; EXTRACTED)
- [happy] When a module is wired via `use self::module as _;`, detect() returns zero findings  (crates/vox-code-audit/src/detectors/unwired_module.rs)
- [happy] Unwired public module declarations (pub mod, pub(crate) mod) produce one finding per declaration  (crates/vox-code-audit/src/detectors/unwired_module.rs)
- [edge] Modules with #[path = "file.rs"] attribute pointing to an existing file are not flagged  (crates/vox-code-audit/src/detectors/unwired_module.rs)
- [edge] Modules backed by stem-based subdirectory pattern (mod foo; with foo/file.rs) are not flagged  (crates/vox-code-audit/src/detectors/unwired_module.rs)
- [edge] Test modules with both #[cfg(test)] and #[path] attributes are skipped  (crates/vox-code-audit/src/detectors/unwired_module.rs)

### `check_parity`  (error, happy; EXTRACTED)
- [happy] Minimal valid retirement contract with one detector surface parses cleanly and reports clean status  (crates/vox-code-audit/src/retirement_parity.rs)
- [error] Report flags detector row with unknown rule ID, stores referenced rule ID in detector_rows_missing_rule array, marks report unclean  (crates/vox-code-audit/src/retirement_parity.rs)
- [error] Report flags detector row with unknown diagnostic ID in detector_rows_missing_diagnostic_id array, marks report unclean  (crates/vox-code-audit/src/retirement_parity.rs)
- [error] Report flags deferred enforcement row lacking target_milestone in deferred_rows_missing_milestone array, marks report unclean  (crates/vox-code-audit/src/retirement_parity.rs)
- [happy] Deferred enforcement row with target_milestone passes validation, counted in deferred_rows_ok, report marked clean  (crates/vox-code-audit/src/retirement_parity.rs)

### `AnonymousErrorDetector`  (edge, error, happy, invariant; EXTRACTED)
- [happy] flags Result types with anonymous str error type  (crates/vox-code-audit/src/detectors/anonymous_error.rs)
- [error] does not flag Result with named error types  (crates/vox-code-audit/src/detectors/anonymous_error.rs)
- [edge] does not flag plain str return types (non-Result)  (crates/vox-code-audit/src/detectors/anonymous_error.rs)
- [invariant] ignores Rust files and only processes Vox source  (crates/vox-code-audit/src/detectors/anonymous_error.rs)

### `EffectNetDeclDetector`  (edge, happy; EXTRACTED)
- [happy] Flags public functions calling http.get() without @uses(net) decorator and suggests it in message  (crates/vox-code-audit/src/detectors/effect_net_decl.rs)
- [happy] Does not flag functions annotated with @uses(net) decorator  (crates/vox-code-audit/src/detectors/effect_net_decl.rs)
- [happy] Does not flag functions that have no network calls  (crates/vox-code-audit/src/detectors/effect_net_decl.rs)
- [edge] Does not analyze or flag issues in non-Vox files (e.g., Rust .rs files)  (crates/vox-code-audit/src/detectors/effect_net_decl.rs)

### `IdAtBoundaryDetector`  (happy; EXTRACTED)
- [happy] IdAtBoundaryDetector.detect() ignores typed Id parameters (Id[User]) under @query decorator  (crates/vox-code-audit/src/detectors/id_at_boundary.rs)
- [happy] IdAtBoundaryDetector.detect() ignores non-id string parameters  (crates/vox-code-audit/src/detectors/id_at_boundary.rs)
- [happy] IdAtBoundaryDetector.detect() flags bare string id parameters (order_id: str) under @activity decorator  (crates/vox-code-audit/src/detectors/id_at_boundary.rs)
- [happy] IdAtBoundaryDetector.detect() ignores Rust files and returns empty findings  (crates/vox-code-audit/src/detectors/id_at_boundary.rs)

### `NoTestForPubFnDetector`  (edge, happy, invariant; EXTRACTED)
- [happy] detect() fires with rule_id 'skeleton/no-test-for-pub-fn' for public functions lacking @test in golden vox files  (crates/vox-code-audit/src/detectors/no_test_for_pub_fn.rs)
- [happy] detect() produces no findings for functions that are called from a @test-decorated function  (crates/vox-code-audit/src/detectors/no_test_for_pub_fn.rs)
- [invariant] detect() produces no findings for @test-decorated functions themselves  (crates/vox-code-audit/src/detectors/no_test_for_pub_fn.rs)
- [edge] detect() produces no findings for vox files outside examples/golden/ directory  (crates/vox-code-audit/src/detectors/no_test_for_pub_fn.rs)

### `StringlyTypedEnumDetector`  (edge, happy; EXTRACTED)
- [happy] detects String type with enum comment in Vox code and emits stringly-typed-enum rule  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] detects String enum in Rust code  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [edge] does not emit findings for properly typed ADT variants  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [edge] does not emit findings for String type inside raw string literals  (crates/vox-code-audit/tests/wave_b_parity.rs)

### `WorkflowNondeterministicDetector`  (edge, happy; EXTRACTED)
- [happy] WorkflowNondeterministicDetector detects time.now() calls inside workflow blocks and includes 'time.now' in the finding message  (crates/vox-code-audit/src/detectors/workflow_nondeterministic.rs)
- [edge] WorkflowNondeterministicDetector does not flag time.now() calls outside workflow blocks  (crates/vox-code-audit/src/detectors/workflow_nondeterministic.rs)
- [happy] WorkflowNondeterministicDetector detects random.uuid() calls inside workflow blocks and includes 'random.uuid' in the finding message  (crates/vox-code-audit/src/detectors/workflow_nondeterministic.rs)
- [edge] WorkflowNondeterministicDetector produces no findings for non-Vox files regardless of nondeterministic patterns in content  (crates/vox-code-audit/src/detectors/workflow_nondeterministic.rs)

### `check_parity_at_paths()`  (happy; EXTRACTED)
- [happy] check_parity_at_paths() returns Report with symbols_registered > 30 from binary registrations  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] check_parity_at_paths() returns Report with symbols_documented > 5 from documentation  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] check_parity_at_paths() produces non-empty summary output in Report  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] check_parity_at_paths() produces report with mismatches that can be categorized by kind (CorpusUsesUnregistered or DocClaimsUnregistered)  (crates/vox-code-audit/src/stdlib_parity.rs)

### `detect_import_cycles_in_batch`  (edge, happy; EXTRACTED)
- [happy] detect_import_cycles_in_batch() detects mutual import cycles (a imports b and b imports a) and returns findings containing 'cycle' in message  (crates/vox-code-audit/src/detectors/import_cycles.rs)
- [happy] produces no findings for acyclic directed graph import structures  (crates/vox-code-audit/src/detectors/import_cycles.rs)
- [happy] produces no findings for diamond dependency graphs without cycles  (crates/vox-code-audit/src/detectors/import_cycles.rs)
- [edge] ignores non-Vox source files (.rs, .ts)  (crates/vox-code-audit/src/detectors/import_cycles.rs)

### `parse_binary_registrations()`  (happy; EXTRACTED)
- [happy] parse_binary_registrations() parses the vox-compiler builtins.rs file and successfully detects the 'print' global symbol  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] parse_binary_registrations() parses vox-compiler builtins.rs and includes the 'print' global symbol  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] parse_binary_registrations() extracts 'fs.read' symbol from binary registrations  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] parse_binary_registrations() detects 'regex.replace' symbol added to builtins  (crates/vox-code-audit/src/stdlib_parity.rs)

### `DeprecatedUsageDetector`  (happy; EXTRACTED)
- [happy] detects JSX className attribute and provides suggestion containing 'class='  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] emits one finding per each deprecated JSX attribute on same line  (crates/vox-code-audit/tests/wave_b_parity.rs)
- [happy] Detects JSX className usage in Vox code and returns findings with rule_id raw-jsx-leakage  (crates/vox-code-audit/src/detectors/deprecated_usage.rs)

### `ImportCyclesDetector`  (happy; EXTRACTED)
- [happy] ImportCyclesDetector.detect() detects self-imports (file importing itself) and returns Finding with 'Self-import' message and Severity::Error  (crates/vox-code-audit/src/detectors/import_cycles.rs)
- [happy] ImportCyclesDetector.detect() ignores regular imports of different files  (crates/vox-code-audit/src/detectors/import_cycles.rs)
- [happy] ImportCyclesDetector.detect() ignores commented-out import statements  (crates/vox-code-audit/src/detectors/import_cycles.rs)

### `SyntaxVersionDetector`  (edge, invariant; EXTRACTED)
- [invariant] SyntaxVersionDetector does not fire on valid syntax_version declarations in valid format  (crates/vox-code-audit/src/detectors/syntax_version.rs)
- [edge] SyntaxVersionDetector does not flag duplicate identical syntax_version declarations  (crates/vox-code-audit/src/detectors/syntax_version.rs)
- [invariant] SyntaxVersionDetector ignores .rs (Rust) files and does not analyze their content  (crates/vox-code-audit/src/detectors/syntax_version.rs)

### `VictoryClaimDetector`  (happy; EXTRACTED)
- [happy] Detects 'Done!' victory comments and produces a finding with rule_id 'victory-claim/premature'  (crates/vox-code-audit/src/detectors/victory_claim.rs)
- [happy] Detects TODO comments in non-Rust files and produces a finding with rule_id 'victory-claim/todo-leftover'  (crates/vox-code-audit/src/detectors/victory_claim.rs)
- [happy] VictoryClaimDetector detects FIXME comments and produces findings with rule_id 'victory-claim/fixme'  (crates/vox-code-audit/src/detectors/victory_claim.rs)

### `parse_binary_registrations`  (happy; EXTRACTED)
- [happy] parse_binary_registrations returns a symbol set containing 'print' when parsing builtins.rs  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] parse_binary_registrations returns a symbol set containing 'fs.read' when parsing builtins.rs  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] parse_binary_registrations returns a symbol set containing 'regex.replace' when parsing builtins.rs  (crates/vox-code-audit/src/stdlib_parity.rs)

### `DryViolationDetector`  (happy; EXTRACTED)
- [happy] Detects duplicate code blocks in Rust source  (crates/vox-code-audit/src/detectors/dry_violation.rs)
- [happy] Produces no findings for unique functions with different logic  (crates/vox-code-audit/src/detectors/dry_violation.rs)

### `DuplicatePrefixDetector`  (edge; EXTRACTED)
- [edge] Does not flag user_id identifier (single occurrence of user is acceptable)  (crates/vox-code-audit/src/detectors/duplicate_prefix.rs)
- [edge] Skips duplicate prefix patterns found in code comments  (crates/vox-code-audit/src/detectors/duplicate_prefix.rs)

### `Finding detection`  (error, happy; EXTRACTED)
- [happy] Rule detectors produce findings on gold_dataset true_positive cases at the correct line  (crates/vox-code-audit/tests/gold_dataset.rs)
- [error] Rule detectors do not produce findings on gold_dataset false_positive cases  (crates/vox-code-audit/tests/gold_dataset.rs)

### `FindingConfidence`  (happy; EXTRACTED)
- [happy] RuleConfidence::Medium converts to FindingConfidence::Medium via Into trait  (crates/vox-code-audit/src/rule_pack_bridge.rs)
- [happy] retired decorator findings have High confidence level  (crates/vox-code-audit/src/detectors/retired_decorator.rs)

### `Language`  (happy; EXTRACTED)
- [happy] RuleLanguage::Rust converts to Language::Rust via Into trait  (crates/vox-code-audit/src/rule_pack_bridge.rs)
- [happy] RuleLanguage::GDScript converts to Language::GDScript via Into trait  (crates/vox-code-audit/src/rule_pack_bridge.rs)

### `OptionCombinatorDetector`  (edge; EXTRACTED)
- [edge] detect() produces no findings for match expressions on Result types  (crates/vox-code-audit/src/detectors/option_combinator.rs)
- [edge] detect() produces no findings for multi-arm (>2) match expressions, even with Option types  (crates/vox-code-audit/src/detectors/option_combinator.rs)

### `QuestionMarkDetector`  (edge; EXTRACTED)
- [edge] QuestionMarkDetector does not fire when Result propagation already uses ? operator  (crates/vox-code-audit/src/detectors/question_mark.rs)
- [edge] QuestionMarkDetector does not fire on complex match expressions with different handling per arm  (crates/vox-code-audit/src/detectors/question_mark.rs)

### `RequireJustificationDetector`  (edge; EXTRACTED)
- [edge] RequireJustificationDetector does not fire on @require decorators with single logical operators (no justification comment required)  (crates/vox-code-audit/src/detectors/require_justification.rs)
- [edge] RequireJustificationDetector does not fire on @require decorators with exactly one operator  (crates/vox-code-audit/src/detectors/require_justification.rs)

### `RetiredMemoryApiDetector`  (edge; EXTRACTED)
- [edge] Does not flag deprecated recall API when called inside canonical memory manager implementation file.  (crates/vox-code-audit/src/detectors/retired_memory_api.rs)
- [edge] Skips detection when file is Vox source rather than Rust.  (crates/vox-code-audit/src/detectors/retired_memory_api.rs)

### `SecretSpanDetector`  (edge, happy; EXTRACTED)
- [happy] detects span.record() calls with secret-field names like 'password' and includes field name in message  (crates/vox-code-audit/src/detectors/secret_span.rs)
- [edge] does not flag tracing span records or structured logging with non-secret field names (user_id, request_path, etc.)  (crates/vox-code-audit/src/detectors/secret_span.rs)

### `Severity`  (happy; EXTRACTED)
- [happy] RuleSeverity::Warning converts to Severity::Warning via Into trait  (crates/vox-code-audit/src/rule_pack_bridge.rs)
- [happy] RuleSeverity::Critical converts to Severity::Critical via Into trait  (crates/vox-code-audit/src/rule_pack_bridge.rs)

### `TaskQueue`  (happy; EXTRACTED)
- [happy] TaskQueue serializes to JSON and deserializes back with field values preserved (total_findings field matches original)  (crates/vox-code-audit/src/task_queue.rs)
- [happy] TaskQueue serializes to JSON and deserializes back with total_findings preserved  (crates/vox-code-audit/src/task_queue.rs)

### `TaskQueue::from_findings()`  (happy; EXTRACTED)
- [happy] TaskQueue::from_findings() creates a queue with total_findings matching input  (crates/vox-code-audit/src/task_queue.rs)
- [happy] TaskQueue::from_findings() assigns Priority::Immediate to Error-severity findings and Priority::NextSession to Warning-severity findings  (crates/vox-code-audit/src/task_queue.rs)

### `TelemetryEvent`  (happy, invariant; EXTRACTED)
- [invariant] JSON serialization skips repository_id field when value is None per skip_serializing_if  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [happy] TelemetryEvent::LintFinding survives JSON serialization and deserialization without data loss  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `TelemetryEvent::LintFinding`  (happy, invariant; EXTRACTED)
- [happy] No LintFinding telemetry events are emitted when findings are empty  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [invariant] LintFinding events survive JSON serialization and deserialization round-trip without mutation  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `ToestubEngine telemetry`  (happy; EXTRACTED)
- [happy] Engine emits one TelemetryEvent::LintFinding for each Finding produced by detectors  (crates/vox-code-audit/tests/telemetry_emission_test.rs)
- [happy] Engine emits no TelemetryEvent::LintFinding events when source code has no violations  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `UnresolvedRefDetector`  (edge; EXTRACTED)
- [edge] UnresolvedRefDetector does not flag SQL function names inside SCHEMA_* string constants as unresolved references  (crates/vox-code-audit/src/detectors/unresolved_ref.rs)
- [edge] UnresolvedRefDetector does not flag function calls that may be provided by defaults::* glob imports  (crates/vox-code-audit/src/detectors/unresolved_ref.rs)

### `check_parity_at_path()`  (invariant; EXTRACTED)
- [invariant] check_parity_at_path() returns a Report where is_clean() evaluates to true for the workspace retirement contract, proving no drift in rule_ids, diagnostic_ids, or deferred milestone enforcement  (crates/vox-code-audit/src/retirement_parity.rs)
- [invariant] Report returned by check_parity_at_path() has detector_rows_ok with length >= 5, proving the workspace contract covers all 5 retired decorator detector patterns  (crates/vox-code-audit/src/retirement_parity.rs)

### `check_parity_at_paths`  (happy; EXTRACTED)
- [happy] check_parity_at_paths produces a Report with symbols_registered > 30 and symbols_documented > 5, and a non-empty summary  (crates/vox-code-audit/src/stdlib_parity.rs)
- [happy] check_parity_at_paths can successfully run against workspace (returns Ok, not a panic or error)  (crates/vox-code-audit/src/stdlib_parity.rs)

### `detectors`  (happy, invariant; EXTRACTED)
- [invariant] Precision and recall computed from gold dataset detection results are both >= 0.5 (sanity floor)  (crates/vox-code-audit/tests/gold_dataset.rs)
- [happy] Detectors can be evaluated against gold dataset for true positive, false positive, and false negative metrics  (crates/vox-code-audit/tests/gold_dataset.rs)

### `extract_vox_imports`  (edge, happy; EXTRACTED)
- [happy] extracts relative import paths with correct line numbers and paths  (crates/vox-code-audit/src/detectors/import_cycles.rs)
- [edge] filters out non-relative imports like @stdlib and standard library references  (crates/vox-code-audit/src/detectors/import_cycles.rs)

### `AiAnalyzer::endpoint_url`  (happy; EXTRACTED)
- [happy] Ollama provider constructs endpoint URL by appending '/api/generate' to configured base URL  (crates/vox-code-audit/src/ai_analyze.rs)

### `AiAnalyzer::parse_response`  (happy; EXTRACTED)
- [happy] Correctly parses pipe-delimited findings from LLM response into Finding vec with rule IDs prefixed by 'ai/', severity enum values extracted per finding  (crates/vox-code-audit/src/ai_analyze.rs)

### `Diagnostic ID catalog`  (invariant; EXTRACTED)
- [invariant] All diagnostic IDs in ALL_KNOWN_IDS start with 'vox/' and have exactly 3 slash-separated parts with non-empty category and name components  (crates/vox-code-audit/src/diagnostics/catalog.rs)

### `DryViolationDetector::similarity`  (invariant; EXTRACTED)
- [invariant] similarity() returns 1.0 (within epsilon) for identical strings and < 0.5 for dissimilar strings  (crates/vox-code-audit/src/detectors/dry_violation.rs)

### `EmptyBodyDetector`  (edge; EXTRACTED)
- [edge] Allows single-line empty trait implementations when trait provides default items  (crates/vox-code-audit/src/detectors/empty_body.rs)

### `EnforcementKind`  (happy; EXTRACTED)
- [happy] EnforcementKind::CliCheck deserializes from kebab-case 'cli-check' YAML string via serde_yaml  (crates/vox-code-audit/src/retirement_parity.rs)

### `EnvSecretShapeDetector.message`  (happy; EXTRACTED)
- [happy] finding messages contain the detected environment variable name  (crates/vox-code-audit/src/detectors/env_secret_shape.rs)

### `Finding`  (happy; INFERRED)
- [happy] For true_positive labeled gold cases, detector.detect() produces findings on the expected line; for false_positive cases, it does not  (crates/vox-code-audit/tests/gold_dataset.rs)

### `GodObjectDetector`  (invariant; EXTRACTED)
- [invariant] detect() excludes blank-only padding lines from god-object size calculations and produces no 'non-blank lines' findings for padded files  (crates/vox-code-audit/src/detectors/mod.rs)

### `HollowFnDetector.new`  (invariant; EXTRACTED)
- [invariant] HollowFnDetector can be instantiated and used to detect hollow test helper functions  (crates/vox-code-audit/src/detectors/hollow_fn.rs)

### `LintFindingEvent.repository_id`  (happy; EXTRACTED)
- [happy] repository_id field is None when ToestubConfig does not specify it  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `LongRangeCouplingDetector`  (edge; EXTRACTED)
- [edge] produces no findings for variables used within 5-line gap  (crates/vox-code-audit/src/detectors/long_range_coupling.rs)

### `OutputFormat::parse_format`  (happy; EXTRACTED)
- [happy] OutputFormat parser accepts three string variants for LLM JSON: 'llm-json', 'llm_json', 'for-llm'  (crates/vox-code-audit/src/report.rs)

### `Reporter::format(Json)`  (happy; EXTRACTED)
- [happy] JSON format output deserializes to Vec<Finding> with all input findings preserved  (crates/vox-code-audit/src/report.rs)

### `Reporter::format(LlmJson)`  (happy; EXTRACTED)
- [happy] LLM JSON format includes 'vox.lint.llm-report.v1' schema identifier, total finding count, and diagnostics array with rule_id and message fields  (crates/vox-code-audit/src/report.rs)

### `Reporter::format(Terminal)`  (happy; EXTRACTED)
- [happy] Terminal output format includes finding rule IDs and category names (TOESTUB, stub/todo, magic-value/port) as searchable strings  (crates/vox-code-audit/src/report.rs)

### `Reporter::format_run`  (happy; EXTRACTED)
- [happy] Run envelope JSON includes schema_version=1, files_scanned, rules_applied, rust_parse_failures, suppressions_applied counts, and findings array with correct length  (crates/vox-code-audit/src/report.rs)

### `RetirementContract`  (happy; EXTRACTED)
- [happy] workspace retirement contract YAML file exists at the expected path and successfully deserializes into a RetirementContract with at least one surface  (crates/vox-code-audit/src/retirement_parity.rs)

### `Rule`  (invariant; EXTRACTED)
- [invariant] every rule from all_rules() has non-empty id(), name(), and supports at least one language  (crates/vox-code-audit/src/detectors/mod.rs)

### `StateMachineUnreachableDetector`  (happy; EXTRACTED)
- [happy] detects state_machine states with no outgoing transitions and includes state name in message  (crates/vox-code-audit/src/detectors/state_machine_unreachable.rs)

### `TaskQueue::from_findings`  (happy; EXTRACTED)
- [happy] TaskQueue::from_findings creates a queue with correct total_findings count and fix_suggestions with Priority values assigned based on severity  (crates/vox-code-audit/src/task_queue.rs)

### `TelemetryEvent serialization`  (happy; EXTRACTED)
- [happy] TelemetryEvent::LintFinding serializes without repository_id field when None (skip_serializing_if)  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `ToestubConfig.repository_id`  (happy; EXTRACTED)
- [happy] repository_id from config flows into emitted LintFindingEvent payloads  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `ToestubEngine`  (happy; EXTRACTED)
- [happy] ToestubEngine.run() emits one TelemetryEvent::LintFinding event per finding, with rule_id, severity, diagnostic_id, and confidence fields populated according to the detector  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `ToestubEngine.run()`  (happy; EXTRACTED)
- [happy] Engine produces zero findings when running on clean canonical fixture  (crates/vox-code-audit/tests/telemetry_emission_test.rs)

### `UntestedPubApiDetector findings`  (happy; EXTRACTED)
- [happy] Includes function names (func_*) in the finding message  (crates/vox-code-audit/src/detectors/untested_pub_api.rs)

### `all_rules()`  (invariant; EXTRACTED)
- [invariant] all_rules() returns exactly rule_count() rules (51 in total)  (crates/vox-code-audit/src/detectors/mod.rs)

### `detectors::all_rules()`  (happy; INFERRED)
- [happy] All detector rules load from gold dataset with non-empty rules set  (crates/vox-code-audit/tests/gold_dataset.rs)

### `explain_url function`  (happy; EXTRACTED)
- [happy] explain_url() generates URLs in format https://voxlang.org/diag/{diagnostic_id} by replacing slashes with hyphens in the ID  (crates/vox-code-audit/src/diagnostics/catalog.rs)

### `is_known_id function`  (happy; EXTRACTED)
- [happy] is_known_id() returns true for known diagnostic IDs like LLM_DIRECT_PROVIDER_CALL and false for unknown IDs like 'vox/fake/nonexistent'  (crates/vox-code-audit/src/diagnostics/catalog.rs)

### `parse_documented_symbols`  (happy; EXTRACTED)
- [happy] parse_documented_symbols returns a symbol set containing 'path.join' when parsing ref-builtins-stdlib.md  (crates/vox-code-audit/src/stdlib_parity.rs)

### `parse_documented_symbols()`  (happy; EXTRACTED)
- [happy] parse_documented_symbols() extracts 'path.join' from the stdlib documentation markdown  (crates/vox-code-audit/src/stdlib_parity.rs)

### `validate_toestub_suppression_contracts`  (happy; EXTRACTED)
- [happy] validate_toestub_suppression_contracts succeeds with Ok result when passed the workspace root containing contracts/toestub/suppressions.v1.json  (crates/vox-code-audit/src/suppression.rs)

### `validate_toestub_suppression_contracts()`  (happy; EXTRACTED)
- [happy] validate_toestub_suppression_contracts() validates the suppression contract file exists and parses from repository root  (crates/vox-code-audit/src/suppression.rs)

### `victory-claim detectors`  (happy; EXTRACTED)
- [happy] victory-claim detector produces expected findings (premature, fixme, hack, todo-leftover) at baseline lines and severities  (crates/vox-code-audit/tests/victory_claim_parity.rs)

### `victory_claim detector`  (happy; EXTRACTED)
- [happy] Detector produces exactly 5 findings with specific rule IDs, lines, and severity levels matching baseline  (crates/vox-code-audit/tests/victory_claim_parity.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`AiAnalyzer::endpoint_url`** — only: _Ollama provider constructs endpoint URL by appending '/api/generate' to configured base URL_
- **`AiAnalyzer::parse_response`** — only: _Correctly parses pipe-delimited findings from LLM response into Finding vec with rule IDs prefixed by 'ai/', severity enum values extracted per finding_
- **`DeprecatedUsageDetector`** — only: _detects JSX className attribute and provides suggestion containing 'class='_
- **`DryViolationDetector`** — only: _Detects duplicate code blocks in Rust source_
- **`EnforcementKind`** — only: _EnforcementKind::CliCheck deserializes from kebab-case 'cli-check' YAML string via serde_yaml_
- **`EnvSecretShapeDetector.message`** — only: _finding messages contain the detected environment variable name_
- **`Finding`** — only: _For true_positive labeled gold cases, detector.detect() produces findings on the expected line; for false_positive cases, it does not_
- **`FindingConfidence`** — only: _RuleConfidence::Medium converts to FindingConfidence::Medium via Into trait_
- **`IdAtBoundaryDetector`** — only: _IdAtBoundaryDetector.detect() ignores typed Id parameters (Id[User]) under @query decorator_
- **`ImportCyclesDetector`** — only: _ImportCyclesDetector.detect() detects self-imports (file importing itself) and returns Finding with 'Self-import' message and Severity::Error_
- **`Language`** — only: _RuleLanguage::Rust converts to Language::Rust via Into trait_
- **`LintFindingEvent`** — only: _LintFindingEvent has rule_id matching the detector rule and severity of 'warning' or 'error'_
- **`LintFindingEvent.repository_id`** — only: _repository_id field is None when ToestubConfig does not specify it_
- **`OutputFormat::parse_format`** — only: _OutputFormat parser accepts three string variants for LLM JSON: 'llm-json', 'llm_json', 'for-llm'_
- **`Reporter::format(Json)`** — only: _JSON format output deserializes to Vec<Finding> with all input findings preserved_
- **`Reporter::format(LlmJson)`** — only: _LLM JSON format includes 'vox.lint.llm-report.v1' schema identifier, total finding count, and diagnostics array with rule_id and message fields_
- **`Reporter::format(Terminal)`** — only: _Terminal output format includes finding rule IDs and category names (TOESTUB, stub/todo, magic-value/port) as searchable strings_
- **`Reporter::format_run`** — only: _Run envelope JSON includes schema_version=1, files_scanned, rules_applied, rust_parse_failures, suppressions_applied counts, and findings array with correct length_
- **`RetiredCapacitorDetector`** — only: _Detector fires with one finding when TypeScript imports from @capacitor/filesystem_
- **`RetirementContract`** — only: _workspace retirement contract YAML file exists at the expected path and successfully deserializes into a RetirementContract with at least one surface_
- **`Scanner`** — only: _Scanner filters files by language extension, returning only .rs files with Language::Rust annotation when scanning a mixed directory_
- **`Severity`** — only: _RuleSeverity::Warning converts to Severity::Warning via Into trait_
- **`StateMachineUnreachableDetector`** — only: _detects state_machine states with no outgoing transitions and includes state name in message_
- **`StubDetector`** — only: _detects todo!() macro invocations with rule_id stub/todo at correct line number_
- **`TaskQueue`** — only: _TaskQueue serializes to JSON and deserializes back with field values preserved (total_findings field matches original)_
- **`TaskQueue::from_findings`** — only: _TaskQueue::from_findings creates a queue with correct total_findings count and fix_suggestions with Priority values assigned based on severity_
- **`TaskQueue::from_findings()`** — only: _TaskQueue::from_findings() creates a queue with total_findings matching input_
- **`TelemetryEvent serialization`** — only: _TelemetryEvent::LintFinding serializes without repository_id field when None (skip_serializing_if)_
- **`ToestubConfig.repository_id`** — only: _repository_id from config flows into emitted LintFindingEvent payloads_
- **`ToestubEngine`** — only: _ToestubEngine.run() emits one TelemetryEvent::LintFinding event per finding, with rule_id, severity, diagnostic_id, and confidence fields populated according to the detector_
- **`ToestubEngine telemetry`** — only: _Engine emits one TelemetryEvent::LintFinding for each Finding produced by detectors_
- **`ToestubEngine.run()`** — only: _Engine produces zero findings when running on clean canonical fixture_
- **`UntestedPubApiDetector findings`** — only: _Includes function names (func_*) in the finding message_
- **`VictoryClaimDetector`** — only: _Detects 'Done!' victory comments and produces a finding with rule_id 'victory-claim/premature'_
- **`check_parity_at_paths`** — only: _check_parity_at_paths produces a Report with symbols_registered > 30 and symbols_documented > 5, and a non-empty summary_
- **`check_parity_at_paths()`** — only: _check_parity_at_paths() returns Report with symbols_registered > 30 from binary registrations_
- **`detectors::all_rules()`** — only: _All detector rules load from gold dataset with non-empty rules set_
- **`explain_url function`** — only: _explain_url() generates URLs in format https://voxlang.org/diag/{diagnostic_id} by replacing slashes with hyphens in the ID_
- **`is_known_id function`** — only: _is_known_id() returns true for known diagnostic IDs like LLM_DIRECT_PROVIDER_CALL and false for unknown IDs like 'vox/fake/nonexistent'_
- **`parse_binary_registrations`** — only: _parse_binary_registrations returns a symbol set containing 'print' when parsing builtins.rs_
- **`parse_binary_registrations()`** — only: _parse_binary_registrations() parses the vox-compiler builtins.rs file and successfully detects the 'print' global symbol_
- **`parse_documented_symbols`** — only: _parse_documented_symbols returns a symbol set containing 'path.join' when parsing ref-builtins-stdlib.md_
- **`parse_documented_symbols()`** — only: _parse_documented_symbols() extracts 'path.join' from the stdlib documentation markdown_
- **`validate_toestub_suppression_contracts`** — only: _validate_toestub_suppression_contracts succeeds with Ok result when passed the workspace root containing contracts/toestub/suppressions.v1.json_
- **`validate_toestub_suppression_contracts()`** — only: _validate_toestub_suppression_contracts() validates the suppression contract file exists and parses from repository root_
- **`victory-claim detectors`** — only: _victory-claim detector produces expected findings (premature, fixme, hack, todo-leftover) at baseline lines and severities_
- **`victory_claim detector`** — only: _Detector produces exactly 5 findings with specific rule IDs, lines, and severity levels matching baseline_
