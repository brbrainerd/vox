# Semantic Behavior Map — `vox-ast`

7 extracted Behavior claims collapse to **3 distinct symbols** after deduplication. Coverage is overwhelmingly happy-path: only `VoxScalar::parse` exercises a failure branch, and even that is a single fabricated unknown. The two AST-shape symbols (`Span`, `TypeExpr::Named`) are construction/round-trip assertions with no invariant or edge proof. The crate's one genuine validation surface — the scalar-name parser — has its rejection contract proven for exactly one input, leaving its empty/whitespace/case/alias modes unverified.

## Span
- **Source:** `crates/vox-ast/src/lib.rs` (`test_ast_dummy_span_integrity`)
- **Proven behaviors:**
  - `Span::new(0, 0)` sets `start` to byte offset 0.
  - `Span::new(0, 0)` sets `end` to byte offset 0.
- **Error path:** none.
- **Edge / invariant:** none. (Two claims collapse to "the dummy span stores its two fields." No proof of the `start <= end` invariant or non-zero offsets.)

## TypeExpr::Named
- **Source:** `crates/vox-ast/src/lib.rs` (`test_ast_scalar_mapping_structure`)
- **Proven behaviors:**
  - Can be pattern-matched to extract the `name` field.
  - Preserves the `name` string passed at construction (round-trip).
  - `span` field stores byte offset 0 when built with `dummy_span`.
- **Error path:** none.
- **Edge / invariant:** none. (Single fabricated happy-path value; no empty-name, unicode, or non-dummy span coverage.)

## VoxScalar::parse
- **Source:** `crates/vox-ast/src/scalar_mapping.rs` (`parse_only_known_scalars`)
- **Proven behaviors:**
  - `parse("int")` returns `Some(VoxScalar::Int)` (one known scalar accepted).
  - `parse("Task")` returns `None` (one unknown ADT name rejected).
- **Error path:** YES — has a rejection proof (`None` for unknown).
- **Edge / invariant:** partial only. Acceptance proven for a single known scalar; rejection proven for a single fabricated unknown. No empty-string, whitespace, case-variant, or full known-set coverage.

## Semantic gaps

Symbols whose contract clearly has a failure/empty/conflict mode but are proven only on the happy path (or with a single token failure case):

1. **`VoxScalar::parse` (most actionable).** This is the crate's only validator/parser — an integrity surface. Its rejection contract is exercised by exactly one input (`"Task"`). Untested rejection modes that the `Option`-returning contract implies: empty string `""`, whitespace, case variants (`"Int"`, `"INT"`), and near-miss/alias names. Acceptance is proven for only `"int"`, so the remaining known scalars are unverified — a missing arm would silently regress to `None`. Add a table-driven test covering every known scalar plus a set of rejected inputs (empty, whitespace, wrong-case, unknown ADT).

2. **`TypeExpr::Named`.** Constructor/round-trip is happy-path only. No edge proof for an empty or unicode `name`, and the span claim only checks the dummy (0) case — there is no proof a real, non-zero span is preserved through construction.

3. **`Span`.** `Span::new` is proven only for the `(0, 0)` dummy. No invariant check (`start <= end`), no non-zero-offset construction, and no behavior under inverted inputs (`new(5, 2)`) — the type's positional contract is entirely unverified.