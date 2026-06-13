# Semantic Behavior Map: vox-jsonschema-util

One-paragraph summary: This map synthesizes 3 extracted Behavior claims covering 2 symbols in `crates/vox-jsonschema-util/src/lib.rs`. The `validate` function is the best-covered surface, with both a happy path (instance satisfies schema) and an error path (rejection on a missing required property) proven. `compile_validator` is covered only on its happy path — compiling a known-valid schema — and has no proof of its failure mode (an invalid or malformed schema), making it the single actionable semantic gap.

## compile_validator

Proven behaviors:
- Successfully compiles a valid JSON Schema object into a `Validator` (happy).

Coverage flags:
- Error path: NOT proven.
- Edge/invariant: NOT proven.

## validate

Proven behaviors:
- Succeeds when a JSON instance satisfies the schema's required properties and type constraints (happy).
- Fails (returns `Err`) when a JSON instance violates the schema by missing a required property (error).

Coverage flags:
- Error path: proven (missing-required-property rejection).
- Edge/invariant: NOT proven (e.g. type-mismatch rejection, empty instance, additional-properties, nested schemas remain unproven).

## Semantic gaps

- **`compile_validator` has no rejection test.** It is a validator-construction surface that returns a fallible result, but is proven only to compile a *valid* schema. There is no proof it rejects an invalid/malformed schema (bad type keyword, non-object schema, unresolvable `$ref`). This is the most actionable gap: a validator/compiler entry point whose failure contract is entirely unverified.
- **`validate` error coverage is narrow.** The only rejection proven is the missing-required-property case. Other clear failure modes (type mismatch, constraint violations, malformed instance) and edge cases (empty object, deeply nested schema) are unproven, though the symbol is no longer purely happy-path.