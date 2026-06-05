# `Result[T, E]` two-param — 2026-06-03 reconciliation addendum

**Date:** 2026-06-03
**Status:** Addendum to the canonical design [`2026-05-18-result-two-param-design.md`](2026-05-18-result-two-param-design.md), which remains the implementation plan. This note re-grounds that plan against the tree as it stands today and records what changed.
**Track:** C1 (golden-corpus & compiler-reality plan).

## Read the 2026-05-18 design first

That doc has the surface examples, the layer-by-layer table, and the migration approach. It is still correct in shape. This addendum only **updates stale references and records progress**, so the implementer doesn't trip on drift.

## What changed since 2026-05-18 (verified 2026-06-03)

1. **C2 (Option/Result match exhaustiveness) is now DONE** (commit `43e9c114dc`). The 2026-05-18 doc lists "exhaustive `match` on user-declared error ADTs across the Err arm" as a downstream goal. The exhaustiveness *machinery* now exists (`typeck/checker/match_exhaust.rs` `check_builtin_match_exhaustiveness`, Result = `Ok` + `Err|Error`). When `Ty::Result` grows its second slot, extend that helper to also check the **error ADT's** variants on the `Error(...)` arm (today it only checks Ok-vs-error-slot presence, not the error type's variants).

2. **`@endpoint` is retired.** The 2026-05-18 surface example uses `@endpoint(kind: mutation) @auth(...)`. Use `@mutation @auth(...)` in any new example/golden — `@endpoint` will not parse.

3. **`infer.rs` is dead code.** The 2026-05-18 table lists three AST→Ty lowering paths to grow a second slot: `ast_decl_lints.rs`, `infer.rs:21`, `registration.rs`. **`typeck/infer.rs` has no callers** (the active pipeline is `typecheck_hir` over HIR) — it is C5's deletion candidate. So only **two** live paths need the second slot: `registration.rs` and `ast_decl_lints.rs`. Do not spend effort threading `E` through `infer.rs`; delete it (C5) instead, or leave it untouched if deletion is deferred.

## Verified current single-param state (file:line, 2026-06-03)

| Layer | Location | Current |
|---|---|---|
| Internal `Ty` | `typeck/ty.rs` `Ty::Result(Box<Ty>)` | one slot |
| AST→Ty (Result arm drops 2nd arg) | `typeck/registration.rs:256-258` (`inner_args.into_iter().next()`) | `E` discarded |
| Err/Error pattern payload hardcoded to `Ty::Str` | `typeck/checker/expr_ops.rs:233-235` | error always typed `str` |
| Runtime value Err side is `String` | `eval/value.rs` `Result(core::result::Result<Box<VoxValue>, String>)` | unchanged |
| Exhaustiveness helper (post-C2) | `typeck/checker/match_exhaust.rs` `check_builtin_match_exhaustiveness` | checks Ok + (Err|Error) slot presence only |

## Implementation order (unchanged from 2026-05-18, with the deltas above)

1. `Ty::Result(Box<Ty>, Box<Ty>)` (ok, err) in `ty.rs`; default new `E` to a fresh var / `Ty::Str` during migration so existing `Result[T]` keeps working.
2. Thread `E` through the **two** live lowering paths (`registration.rs:256`, `ast_decl_lints.rs`), `unify.rs`, `instantiate`, `resolve`, `occurs`.
3. Bind the `Err`/`Error` pattern payload to the real `E` (`expr_ops.rs:233`).
4. Extend `match_exhaust.rs` so a `match` on `Result[T, UserErr]` checks the **`UserErr` ADT variants** on the `Error(...)` arm (reuse the named-ADT path).
5. (Optional, runtime) widen `eval/value.rs` Err side from `String` to `VoxValue` so the interpreter carries typed errors — or keep string-on-the-runtime and only enforce at typecheck (decide based on how much interp error-value richness is wanted).

## Corpus rule (binding until this lands)

**Do NOT add `Result[T, E]` training data to the golden corpus or HumanEval until the two-param representation lands.** Today `Result[T, E]` silently checks as `Result[T]` with `E` dropped, so any such golden would teach a non-feature. Train `Result[T]` + string errors only. (This is why the Track-F new goldens used `Result[T]`/`Error("...")` exclusively.) Once landed, add one golden pairing a named error enum with `to Result[T, MyErr]` and an exhaustive `match` on the error arm, and extend `nested_types.vox` with a nested two-param Result.

## Done when

- `fn f() to Result[int, MyErr] { … } match f() { Err(e) => e.field }` types `e` as `MyErr`, not `str`.
- A non-exhaustive match on a `Result[T, MyErr]` error ADT is rejected (E0301).
- `infer.rs` is deleted or confirmed irrelevant.
- One Result[T,E] golden added; corpus + HumanEval gates green.
