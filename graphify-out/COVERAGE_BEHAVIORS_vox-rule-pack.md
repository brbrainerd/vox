# Semantic Behavior Map — `vox-rule-pack`

Deterministically synthesized from 15 distinct proven-behavior claims (of 15 extracted) across 11 symbols. 1 symbols have an explicit error-path proof; **6 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `RulePack::load_from_str`  (error, happy; EXTRACTED)
- [happy] Parses YAML into RulePack with correct rule count and severity matching  (crates/vox-rule-pack/src/pack.rs)
- [error] Rejects invalid regex patterns with RulePackError::InvalidRegex  (crates/vox-rule-pack/src/pack.rs)
- [error] Rejects duplicate rule IDs with RulePackError::DuplicateId  (crates/vox-rule-pack/src/pack.rs)

### `run_bench`  (edge, happy; EXTRACTED)
- [happy] When a rule perfectly classifies all fixtures (1 TP, 0 FP), the F1 score equals 1.0  (crates/vox-rule-pack/src/bench.rs)
- [happy] When a rule has 1 TP and 1 FP, precision is 0.5  (crates/vox-rule-pack/src/bench.rs)
- [edge] When no fixtures exist for a rule, F1 score defaults to 1.0  (crates/vox-rule-pack/src/bench.rs)

### `CompiledRule::languages`  (invariant; EXTRACTED)
- [invariant] Every compiled rule in the canonical pack applies to at least one language  (crates/vox-rule-pack/tests/canonical_rules.rs)

### `CompiledRule::matches_line`  (happy; EXTRACTED)
- [happy] Line regex correctly matches patterns (foo123 matches, bar123 does not)  (crates/vox-rule-pack/src/pack.rs)

### `CompiledRule::message`  (invariant; EXTRACTED)
- [invariant] Every compiled rule in the canonical pack has a non-empty message string  (crates/vox-rule-pack/tests/canonical_rules.rs)

### `RuleConfidence`  (invariant; EXTRACTED)
- [invariant] Survives serialization and deserialization round trip through serde_yaml  (crates/vox-rule-pack/src/types.rs)

### `RuleFile`  (happy; EXTRACTED)
- [happy] Deserializes YAML with version, rules, severity, confidence, languages, and match kind  (crates/vox-rule-pack/src/schema.rs)

### `RuleLanguage`  (happy; EXTRACTED)
- [happy] Parses all 5 language variants (rust, typescript, python, vox, gdscript) from YAML list  (crates/vox-rule-pack/src/types.rs)

### `RulePack`  (happy; EXTRACTED)
- [happy] Substring match kind escapes regex metacharacters (literal dot matches but regex dot does not)  (crates/vox-rule-pack/src/pack.rs)

### `RulePack::rules_for_language`  (happy; EXTRACTED)
- [happy] Filters rules by language, returning 1 Rust rule and 0 Python rules  (crates/vox-rule-pack/src/pack.rs)

### `RuleSeverity::Warning`  (happy; EXTRACTED)
- [happy] Serializes to lowercase 'warning' string via serde_yaml  (crates/vox-rule-pack/src/types.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`CompiledRule::matches_line`** — only: _Line regex correctly matches patterns (foo123 matches, bar123 does not)_
- **`RuleFile`** — only: _Deserializes YAML with version, rules, severity, confidence, languages, and match kind_
- **`RuleLanguage`** — only: _Parses all 5 language variants (rust, typescript, python, vox, gdscript) from YAML list_
- **`RulePack`** — only: _Substring match kind escapes regex metacharacters (literal dot matches but regex dot does not)_
- **`RulePack::rules_for_language`** — only: _Filters rules by language, returning 1 Rust rule and 0 Python rules_
- **`RuleSeverity::Warning`** — only: _Serializes to lowercase 'warning' string via serde_yaml_
