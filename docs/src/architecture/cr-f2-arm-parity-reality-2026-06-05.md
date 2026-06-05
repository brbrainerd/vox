---
title: "CR-F2 Arm-Parity Reality (2026-06-05)"
description: "Measured cross-arm parity: the interpreter runs every main()-golden, but the --mode script (codegen-rust) arm compiles none of them. A categorized codegen-rust repair backlog for v1.0."
category: "Architecture SSOTs"
status: "advisory"
training_eligible: false
---

# CR-F2 Arm-Parity Reality

> **Measured 2026-06-05** against `main @ 7a3aaf4408` with a `vox` binary
> built `--features script-execution`. This is the first behavioral
> measurement of cross-arm parity (CR-F2) and it is decisive.

## Headline

The two execution arms are **fully divergent**, not subtly drifting:

| Arm | main()-goldens that work |
|---|---|
| `--mode interp` (tree-walking interpreter) | **10 / 10** run and match their `// EXPECT:` output |
| `--mode script` (codegen-rust → `cargo` compile + run) | **3 / 10** (was 0/10 → 1 → 3; noop + while_loop_algorithms + decimal_math green as of 2026-06-05) |

The interpreter is production-shaped for these programs. The codegen-rust
arm **cannot compile even the trivial program** `fn main() to int { return 0 }`
(`examples/golden/mesh/noop.vox`). CR-F2 (all-three-arm byte parity) is
therefore **not a gate that can go green today** — it is blocked on a
substantial codegen-rust repair backlog. (The codegen-ts arm was not yet
measured; codegen-rust must work first.)

This concretely confirms the v1.0-criteria-advisory premise that "arms of
the compiler are still not fully working": one arm is real, the other does
not compile basic programs.

## The map (all 10 `main()`-goldens, `--mode script`)

```
FAIL  adt_multi_field         FAIL  json_as_typed       FAIL  range_and_indexing
FAIL  closures_hof            FAIL  noop                FAIL  regex_free_functions
FAIL  decimal_math            FAIL  process_run         FAIL  scrape_demo
FAIL  env_and_path
```

All `interp`-passing; all `script`-failing.

## Categorized codegen-rust repair backlog

Each is a distinct defect. Ordered by leverage (a fix unblocks many programs).

1. **`fn main() to <T>` return type not handled** *(highest leverage — every
   value-returning main hits it)*. **✅ FIXED 2026-06-05 (commit f982b90ac3).**
   codegen-rust now runs a non-Unit main's body in a closure and prints the
   result, mirroring interp. `mesh/noop.vox` (`fn main() to int { return 0 }`)
   compiles, runs, and prints `0` — CR-F2 ratcheted **0/10 → 1/10**. The other
   value-returning mains (`tuple_destructure`, `range_and_indexing`,
   `closures_hof`, `json_as_typed`) cleared the main-return error and now fail
   on their *separate* body bugs below (#5/#6).

2. **Missing generated `[dependencies]`** for `dec` and regex. The Native
   `Cargo.toml` template (`pipeline.rs` ~line 173) has a fixed dep set
   (tokio/serde/serde_json/tracing/vox-actor-runtime) — **no `rust_decimal`,
   no `regex`** — but codegen emits `rust_decimal::Decimal` for `dec` literals.
   `E0433 cannot find crate rust_decimal`. Fix: add deps conditionally on
   construct usage (or route decimals/regex through `vox-actor-runtime`
   re-exports). Triggers: `decimal_math`, `regex_free_functions`.

3. **`Option::None` emitted as a tuple variant** → `E0532 expected tuple
   struct or tuple variant, found unit variant None`. Triggers:
   `while_loop_algorithms`.

4. **Regex string-escape emission** → `error: unknown character escape: w`
   (a `\w` in a Vox string is emitted into Rust source without re-escaping).
   Triggers: `regex_free_functions`.

5. **Loop / binding variable scoping** → `E0425 cannot find value i`. A loop
   or destructuring variable is referenced out of the scope codegen placed it
   in. Triggers: `tuple_destructure`.

6. **`Json` type not emitted** → `E0425 cannot find type Json`. The
   `@json_as` / `json.parse` path references a `Json` type the Rust emitter
   never defines. Triggers: `json_as_typed`.

7. **String-interpolation type mismatches** → `E0308`. Interpolated
   non-string scalars aren't coerced to `String`. Triggers:
   `string_interpolation`.

(`scrape_demo`, `process_run`, `env_and_path` exercise I/O builtins and were
not individually triaged — fix the above first, then re-measure.)

## Implication for v1.0

- **CR-F2 stays red and blocked** until the codegen-rust backlog above is
  worked down. It should be implemented as a **ratcheting baseline gate**
  (record the current `0/10`, assert it only improves, require byte-parity
  where script *does* compile), feature-gated behind `script-execution` and
  opt-in (the default `vox` binary lacks the feature, and script mode compiles
  a fresh cargo crate per program — too slow for default CI; run it nightly
  or on demand).
- **The maintainer's "all three arms at parity" steer is a large body of
  work**, not a finishing touch. Recommend sequencing: codegen-rust to parity
  first (interp is the reference), then codegen-ts.
- This is the strongest single piece of evidence for the advisory's core
  thesis. The behavioral substrate (CR-F1) made it measurable; the measurement
  says the compiler's second arm is not yet a compiler.

## Reproduce

```bash
cargo build -p vox-cli --bin vox --features script-execution
# interp (works):
./target/debug/vox run --mode interp examples/golden/mesh/noop.vox      # -> 0
# script (fails to compile):
./target/debug/vox run --mode script examples/golden/mesh/noop.vox      # -> E0308
```
