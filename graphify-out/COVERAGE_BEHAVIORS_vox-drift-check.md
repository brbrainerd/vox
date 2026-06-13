# Semantic Behavior Map — `vox-drift-check`

Deterministically synthesized from 47 distinct proven-behavior claims (of 47 extracted) across 28 symbols. 0 symbols have an explicit error-path proof; **24 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `parse_drift_allow_comments()`  (happy; EXTRACTED)
- [happy] Detects trailing '// drift-allow(rule-id)' comments and includes the same line in coverage  (crates/vox-drift-check/src/extractor.rs)
- [happy] Covers both the comment line and the following line when annotation is placed above code  (crates/vox-drift-check/src/extractor.rs)
- [happy] Parses comma-separated rule IDs from a single drift-allow annotation  (crates/vox-drift-check/src/extractor.rs)
- [happy] Accepts full slash-prefixed rule IDs like 'drift/timeout-literal' verbatim  (crates/vox-drift-check/src/extractor.rs)

### `DriftEngine`  (happy, invariant; EXTRACTED)
- [happy] DriftEngine.run_all() detects planted reqwest::Client::new() calls outside SSOT crate and reports drift/reqwest-bypass finding  (crates/vox-drift-check/tests/integration.rs)
- [happy] DriftEngine.run_all() detects duplicate string literals across files and reports sweep/duplicate-string-literal finding  (crates/vox-drift-check/tests/integration.rs)
- [invariant] Multiple sequential calls to DriftEngine.run_all() produce findings vectors of equal length  (crates/vox-drift-check/tests/integration.rs)

### `RustExtractor`  (happy; EXTRACTED)
- [happy] RustExtractor.extract() correctly identifies Duration::from_secs() calls and extracts the numeric literal with value 30.0  (crates/vox-drift-check/src/extractors/rust.rs)
- [happy] RustExtractor.extract() identifies Duration::from_millis() calls and extracts numeric literals  (crates/vox-drift-check/src/extractors/rust.rs)
- [happy] RustExtractor.extract() correctly extracts use statements and populates imports collection  (crates/vox-drift-check/src/extractors/rust.rs)

### `TimeoutLiteralRule`  (edge, happy; EXTRACTED)
- [happy] TimeoutLiteralRule::check flags Duration::from_secs() numeric literals when not in const context  (crates/vox-drift-check/src/rules/timeout_literal.rs)
- [edge] TimeoutLiteralRule::check skips numeric literals marked with in_const=true  (crates/vox-drift-check/src/rules/timeout_literal.rs)
- [happy] TimeoutLiteralRule::check respects per-line drift-allow annotations in allowed_lines map  (crates/vox-drift-check/src/rules/timeout_literal.rs)

### `VoxPathLiteralRule`  (edge, happy; EXTRACTED)
- [happy] VoxPathLiteralRule::check flags vox path string literals outside vox-config crate  (crates/vox-drift-check/src/rules/vox_path_literal.rs)
- [edge] VoxPathLiteralRule::check allows vox path literals in ConstDecl context  (crates/vox-drift-check/src/rules/vox_path_literal.rs)
- [happy] VoxPathLiteralRule::check respects per-line drift-allow annotations for vox-path-literal  (crates/vox-drift-check/src/rules/vox_path_literal.rs)

### `Finding`  (happy; EXTRACTED)
- [happy] Finding struct contains rule_id field that can be matched against 'drift/reqwest-bypass'  (crates/vox-drift-check/tests/integration.rs)
- [happy] Finding with rule_id 'sweep/duplicate-string-literal' is emitted for repeated constant string values  (crates/vox-drift-check/tests/integration.rs)

### `LiteralDedupRule::sweep`  (edge, happy; EXTRACTED)
- [edge] The rule returns no findings for duplicate string literals that appear fewer times than the configured threshold  (crates/vox-drift-check/src/sweep/literal_dedup.rs)
- [happy] String literals in ConstDecl context are excluded when counting duplicates toward the threshold  (crates/vox-drift-check/src/sweep/literal_dedup.rs)

### `NumericDedupRule::sweep`  (happy; EXTRACTED)
- [happy] Numeric literals marked with in_const=true are excluded from duplicate occurrence counting  (crates/vox-drift-check/src/sweep/numeric_dedup.rs)
- [happy] Per-line drift-allow annotations registered in allowed_lines suppress counted occurrences from reaching the duplicate threshold  (crates/vox-drift-check/src/sweep/numeric_dedup.rs)

### `ReqwestBypassRule`  (happy; EXTRACTED)
- [happy] ReqwestBypassRule::check allows reqwest::Client::new() in vox-http-client crate  (crates/vox-drift-check/src/rules/reqwest_bypass.rs)
- [happy] ReqwestBypassRule::check flags reqwest::Client::builder() in non-SSOT crates  (crates/vox-drift-check/src/rules/reqwest_bypass.rs)

### `TypeScriptExtractor`  (happy; EXTRACTED)
- [happy] TypeScriptExtractor.extract() correctly identifies string literals in TypeScript source code  (crates/vox-drift-check/src/extractors/typescript.rs)
- [happy] TypeScriptExtractor.extract() parses import statements and populates imports collection  (crates/vox-drift-check/src/extractors/typescript.rs)

### `UnitHint`  (happy; EXTRACTED)
- [happy] UnitHint::Seconds is assigned to numeric literals from Duration::from_secs() calls  (crates/vox-drift-check/src/extractors/rust.rs)
- [happy] UnitHint::Millis is assigned to numeric literals from Duration::from_millis() calls  (crates/vox-drift-check/src/extractors/rust.rs)

### `crate_name_from_path()`  (happy; EXTRACTED)
- [happy] Extracts crate name from canonical 'crates/<name>/src/...' paths  (crates/vox-drift-check/src/extractor.rs)
- [happy] Returns None for paths not under 'crates/' directory  (crates/vox-drift-check/src/extractor.rs)

### `imports`  (happy; EXTRACTED)
- [happy] Import paths are decomposed into segments: std::collections::HashMap becomes vec!["std", "collections", "HashMap"]  (crates/vox-drift-check/src/extractors/rust.rs)
- [happy] Import symbols are extracted from import declarations (e.g., 'foo' from 'import { foo } from "some-module"')  (crates/vox-drift-check/src/extractors/typescript.rs)

### `Any`  (happy; EXTRACTED)
- [happy] String literal values are preserved exactly in TypeScriptExtractor output  (crates/vox-drift-check/src/extractors/typescript.rs)

### `BearerHeaderRule`  (happy; EXTRACTED)
- [happy] BearerHeaderRule::check detects Bearer token string literals in code context  (crates/vox-drift-check/src/rules/bearer_header.rs)

### `BodyHashRule`  (happy; EXTRACTED)
- [happy] BodyHashRule::sweep flags duplicate function bodies within a single crate  (crates/vox-drift-check/src/sweep/body_hash.rs)

### `BodyHashRule::sweep`  (happy; EXTRACTED)
- [happy] The sweep rule returns no findings when duplicate function bodies span only crates declared as siblings in LayersManifest  (crates/vox-drift-check/src/sweep/body_hash.rs)

### `DriftConfig`  (happy; EXTRACTED)
- [happy] DriftConfig deserializes from TOML and field values are parsed correctly  (crates/vox-drift-check/src/config.rs)

### `DriftEngine.extract_workspace()`  (happy; EXTRACTED)
- [happy] Finds Rust source files in directory tree and extracts string literals from them  (crates/vox-drift-check/src/engine.rs)

### `FeatureCache.store() and .load()`  (happy; EXTRACTED)
- [happy] Features with string_literals are stored and loaded from disk without data loss  (crates/vox-drift-check/src/cache.rs)

### `LiteralContext`  (happy; EXTRACTED)
- [happy] LiteralContext enum variant (Code) survives serde round-trip  (crates/vox-drift-check/src/features.rs)

### `LiteralLoc`  (happy; EXTRACTED)
- [happy] LiteralLoc serializes to JSON and deserializes back with all fields intact (value, line, col, context)  (crates/vox-drift-check/src/features.rs)

### `LiteralLoc, Loc, LiteralContext`  (happy; EXTRACTED)
- [happy] LiteralLoc, Loc, and LiteralContext serialize to JSON and deserialize back without data loss  (crates/vox-drift-check/src/features.rs)

### `Loc`  (happy; EXTRACTED)
- [happy] Loc struct survives serde round-trip with correct line and col values  (crates/vox-drift-check/src/features.rs)

### `SerdeDefaultDupRule`  (happy; EXTRACTED)
- [happy] SerdeDefaultDupRule::check flags default_true function definition outside vox-config crate  (crates/vox-drift-check/src/rules/serde_default_dup.rs)

### `VersionStringRule`  (happy; EXTRACTED)
- [happy] VersionStringRule::check flags string literals matching workspace_version  (crates/vox-drift-check/src/rules/version_string.rs)

### `is_allowed_at()`  (happy; EXTRACTED)
- [happy] Accepts both full (prefixed) and short (unprefixed) forms of rule IDs when checking line allowance  (crates/vox-drift-check/src/extractor.rs)

### `short_rule_id()`  (happy; EXTRACTED)
- [happy] Strips 'drift/' and 'sweep/' prefixes from rule IDs, leaving unprefixed IDs unchanged  (crates/vox-drift-check/src/extractor.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Any`** — only: _String literal values are preserved exactly in TypeScriptExtractor output_
- **`BearerHeaderRule`** — only: _BearerHeaderRule::check detects Bearer token string literals in code context_
- **`BodyHashRule`** — only: _BodyHashRule::sweep flags duplicate function bodies within a single crate_
- **`BodyHashRule::sweep`** — only: _The sweep rule returns no findings when duplicate function bodies span only crates declared as siblings in LayersManifest_
- **`DriftConfig`** — only: _DriftConfig deserializes from TOML and field values are parsed correctly_
- **`DriftEngine.extract_workspace()`** — only: _Finds Rust source files in directory tree and extracts string literals from them_
- **`FeatureCache.store() and .load()`** — only: _Features with string_literals are stored and loaded from disk without data loss_
- **`Finding`** — only: _Finding struct contains rule_id field that can be matched against 'drift/reqwest-bypass'_
- **`LiteralContext`** — only: _LiteralContext enum variant (Code) survives serde round-trip_
- **`LiteralLoc`** — only: _LiteralLoc serializes to JSON and deserializes back with all fields intact (value, line, col, context)_
- **`LiteralLoc, Loc, LiteralContext`** — only: _LiteralLoc, Loc, and LiteralContext serialize to JSON and deserialize back without data loss_
- **`Loc`** — only: _Loc struct survives serde round-trip with correct line and col values_
- **`NumericDedupRule::sweep`** — only: _Numeric literals marked with in_const=true are excluded from duplicate occurrence counting_
- **`ReqwestBypassRule`** — only: _ReqwestBypassRule::check allows reqwest::Client::new() in vox-http-client crate_
- **`RustExtractor`** — only: _RustExtractor.extract() correctly identifies Duration::from_secs() calls and extracts the numeric literal with value 30.0_
- **`SerdeDefaultDupRule`** — only: _SerdeDefaultDupRule::check flags default_true function definition outside vox-config crate_
- **`TypeScriptExtractor`** — only: _TypeScriptExtractor.extract() correctly identifies string literals in TypeScript source code_
- **`UnitHint`** — only: _UnitHint::Seconds is assigned to numeric literals from Duration::from_secs() calls_
- **`VersionStringRule`** — only: _VersionStringRule::check flags string literals matching workspace_version_
- **`crate_name_from_path()`** — only: _Extracts crate name from canonical 'crates/<name>/src/...' paths_
- **`imports`** — only: _Import paths are decomposed into segments: std::collections::HashMap becomes vec!["std", "collections", "HashMap"]_
- **`is_allowed_at()`** — only: _Accepts both full (prefixed) and short (unprefixed) forms of rule IDs when checking line allowance_
- **`parse_drift_allow_comments()`** — only: _Detects trailing '// drift-allow(rule-id)' comments and includes the same line in coverage_
- **`short_rule_id()`** — only: _Strips 'drift/' and 'sweep/' prefixes from rule IDs, leaving unprefixed IDs unchanged_
