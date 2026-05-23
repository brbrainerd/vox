---
title: "How-To: Rust crate imports in Vox scripts"
description: "Syntax, compiler pipeline, Cargo.toml synthesis, diagnostics, limitations, and pragmatic ways to reduce boilerplate without over-engineering."
category: "How-To Guides"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "HowTo"
---

# How-To: Rust crate imports in Vox scripts

This page is the **SSOT for the current `import rust:…` feature**: what it does in the toolchain, what it does *not* do yet, and how to evolve it with **high leverage and low Kolmogorov complexity** (small mental model, few rules, familiar Cargo concepts).

In the bell-curve interop model, `import rust:...` is a **Tier 3 escape hatch**. See [Interop tier policy](../archive/research-2026-q1/interop-tier-policy.md).

## Syntax (what you can write today)

Rust crate imports use the reserved prefix `rust:` on an `import` entry. They can be comma-separated with ordinary symbol imports in the same `import` statement.

```vox
// vox:skip
import react.use_state
import rust:serde_json
import rust:serde_json(version: "1") as json
import rust:my_thing(path: "../crates/my_thing"), rust:other(git: "https://example.invalid/repo", rev: "main")
```

| Piece | Meaning |
| --- | --- |
| `rust:<crate_name>` | Cargo **package name** / dependency key (same string you would put in `Cargo.toml`). |
| Optional `(<meta…>)` | Source/version metadata (see below). |
| Optional `as <alias>` | Local binding name. If omitted, the binding defaults to `<crate_name>`. |

### Metadata keys (inside parentheses)

Keys are identifiers; values may be string literals or simple identifiers.

| Key | Role |
| --- | --- |
| `version` | Semver requirement string (e.g. `"1"`, `"^0.4"`). |
| `path` | Local path dependency (string). |
| `git` | Git URL (string). |
| `rev` or `branch` | Git revision / branch hint (string). |

**Compatibility rule:** Do not specify both `path` and `git` for the same import; the compiler rejects that combination.

**Same crate twice:** You may bind the same crate under two aliases **only if** the dependency tuple `(version, path, git, rev)` is identical. Otherwise you get a **lowering** diagnostic (conflicting specs).

## Architecture (end-to-end)

The feature is implemented **inside the existing compiler and codegen crates**, not as a sidecar tool.

```mermaid
flowchart LR
  A["`.vox` source"] --> B["Lexer / Parser"]
  B --> C["AST `ImportPathKind::RustCrate`"]
  C --> D["HIR `HirRustImport`"]
  D --> E["Type registration"]
  D --> F["`Cargo.toml` synthesis"]
  F --> G["`cargo build` in cache / generated crate"]
```

1. **Parse** — `rust:` is recognized only when the first segment is the identifier `rust` followed by `:`; see `crates/vox-compiler/src/parser/descent/decl/head.rs` (`parse_import_path`).
2. **AST** — `ImportPath` carries `ImportPathKind::RustCrate(RustCrateImport)` plus optional alias; see `crates/vox-compiler/src/ast/decl/types.rs`.
3. **HIR** — Lowering fills `HirModule::rust_imports` (`HirRustImport`: crate name, alias, version/path/git/rev, span); symbol-style imports still populate `HirModule::imports`; see `crates/vox-compiler/src/hir/lower/mod.rs`.
4. **Validation** — `crates/vox-compiler/src/hir/validate.rs` checks empty names, conflicting path+git, etc.
5. **Type checking** — `register_hir_module` binds the alias to an internal `Ty::Named("RustCrate::<crate>")` and reports alias clashes with other top-level names; conflicting metadata for the same crate name emits `DiagnosticCategory::Lowering`; see `crates/vox-compiler/src/typeck/registration.rs`.
6. **Code generation** — Script mode (`generate_script_with_target`) and full-server emit (`emit_cargo_toml`) append **extra `[dependencies]` lines** derived from `rust_imports`, with deduplication by crate name (first spec wins in the map). See `crates/vox-compiler/src/codegen_rust/pipeline.rs` and `crates/vox-compiler/src/codegen_rust/emit/mod.rs`.

### CLI and diagnostics

- **`vox check`** runs the same frontend (lex → parse → typecheck → HIR validate). With global **`--json`**, type/HIR diagnostics are printed as a JSON array (`category`, `severity`, `message`, `line`, `col`, `file`); see `crates/vox-cli/src/pipeline.rs` and `crates/vox-cli/src/commands/check.rs`.
- Golden coverage for a **Lowering** rust-import diagnostic lives in `crates/vox-cli/tests/golden/check_rust_import_lowering.json`.

### Relation to Vox PM (`vox.lock`)

Project dependencies for **Vox packages** still flow through `Vox.toml` / `vox.lock` / `vox sync` (see [`reference/cli.md`](../reference/cli.md)). **`import rust:…` is compile-time Cargo manifest sugar** for generated crates: it does not by itself add rows to `vox.lock`. Longer term, aligning “script deps” with the PM graph is optional hardening (see below).

## Current capabilities vs limitations

### What works

- Declaring extra Cargo dependencies for **generated script binaries** and **generated full-stack Rust** outputs.
- Deterministic **merge/dedup** of dependency lines per crate name in codegen.
- **Strict error when** the same crate name is imported with **incompatible** version/path/git/rev metadata.
- **WASI script guardrail:** native-only crates listed under `wasi_unsupported_rust_imports` in [`contracts/rust/ecosystem-support.yaml`](../../../contracts/rust/ecosystem-support.yaml) are rejected as rust imports in WASI mode; examples include `tokio` and `axum`.

### What does *not* work yet (important)

- **No automatic Rust `use` or Vox-call mapping:** Adding `import rust:serde_json` updates **Cargo.toml** only. It does **not** emit Rust that calls `serde_json` from lowered Vox code, and **does not** import items into the Vox type universe from `rustdoc` or `rustc`.
- **The alias is not a typed API surface:** Bindings use the internal marker type `RustCrate::<crate>`. **Field access** on that binding is rejected in the typechecker with a clear error (see `crates/vox-compiler/src/typeck/checker/expr_field.rs`).
- **Default version `*`:** If you omit `version` / `path` / `git`, codegen emits a loose crates.io requirement (`crate = "*"`), which is convenient for experiments but **weak for reproducibility**.
- **No linkage to `cargo vendor` / vendoring policy** in this path alone; reproducibility remains “whatever Cargo resolves” unless you tighten versions or use path/git explicitly.

**Plain language:** today’s feature is best thought of as **“make this script’s generated crate depend on these Rust packages.”** It is **not** yet **“call arbitrary Rust APIs from Vox with one line.”**

## Support-class annotations and reproducibility warnings

Rust imports now carry a support-class classification for clearer operator expectations:

- `first_class`
- `internal_runtime_only`
- `escape_hatch_only`
- `deferred`

Current compiler behavior:

- emits warnings when a crate is classified as `internal_runtime_only` or `deferred`
- emits warnings when a crate is classified as `escape_hatch_only`
- emits warnings when a crate has `planned` semantics in the support registry
- emits warnings when no `version` / `path` / `git` pin is provided (Cargo fallback `*`)
- emits warnings when import-level pins are provided for full app template-managed crates (those templates may own versions/paths)
- annotates generated `Cargo.toml` dependency lines with `# vox_rust_import support_class=...`

These annotations are guidance, not a typed interop promise.

Canonical support matrix and contract metadata:

- [Rust ecosystem support contract](../reference/rust-ecosystem-support-contract.md)

For common app capabilities, prefer:

1. builtins and `std.*` surfaces,
2. approved wrappers,
3. package-managed Vox libraries,
4. `import rust:...` only when the earlier tiers do not fit.

## Reducing K-complexity and boilerplate (without breaking compatibility)

Keep the **mental model** small:

1. **One syntax only** — Keep `import rust:…` as the single user-facing form; avoid parallel `@rust.import` or magic decorators unless they lower to the same AST (doc and tooling stay simpler).
2. **Cargo is the execution truth** — Users already understand `version` / `path` / `git`. Prefer mapping from those fields to `Cargo.toml` over inventing a third version language.
3. **Layer capabilities** — Dependency declaration (done) → optional manifest merge from project lock (next) → optional thin escape hatch or shims (later).

### High-impact, not over-engineered wins

These are ordered by **value / effort**:

1. **Implicit versions from project context (medium)**  
   If `Vox.toml` or a sibling `Cargo.toml` / lockfile already pins `serde_json`, allow `import rust:serde_json` **without** repeating `version: "…"`, by resolving from the project graph when building from a workspace package. **Compatibility:** When no pin exists, keep today’s behavior (`*` or diagnostic). **K win:** One-line imports match user expectation of “like Cargo.”

2. **`vox check` / `cargo check` parity messaging (low)**  
   When script codegen fails, surface Cargo’s error with a hint { “dependency X declared via `import rust:X` at line L.” Ties the mental model to the line they wrote.

3. **Curated `vox-*` or shims for 5–10 hot crates (medium)**  
   Instead of full `rustdoc` typing, expose **`std`-style namespaces** for e.g. JSON, time, UUID (wrappers in `vox-actor-runtime` or a small `vox-shims` crate). **K win:** Users learn one Vox API; compiler stays small. **Big win:** Works today under the existing builtin pattern.

4. **Single escape hatch: embedded Rust snippet with explicit unsafe boundary (medium–high)**  
   A block or decl that copies almost verbatim into generated `main` / module, with **scoped** `use` generated from adjacent `import rust:…`. **Compatibility:** Opt-in, clearly marked; keeps the main language pure. **K win:** Power users stop fighting the compiler; everyone else ignores it.

5. **Defer: full dynamic `rustdoc` / rustc-based typing**  
   High cost, long-term maintenance, and versioning traps. Prefer shims + escape hatch until the language stabilizes.

### Wins to defer (usually over-engineered for the current stage)

- Full ABI-stable plugin system for every crate.
- Automatic WASM component bindings for arbitrary crates.
- Replacing Cargo with a custom resolver for script deps.

Those belong behind explicit **feature gates** and product milestones, not on the default path.

## Related docs

- [Keyword: `import` syntax](../reference/ref-syntax.md)
- [CLI reference: PM vs generated `Cargo.lock`](../reference/cli.md)
- [Diagnostic taxonomy](../reference/diagnostic-taxonomy.md)
- [Vox packaging blueprint](../archive/research-2026-q1/vox-packaging-implementation-blueprint.md) (extension boundaries)

---

**Maintenance:** When you change parser, HIR, registration, or codegen behavior for rust imports, update this page and the golden JSON under `crates/vox-cli/tests/golden/` if diagnostics or spans shift.
After contract/policy edits, run `cargo run -p vox-cli --quiet -- ci rust-ecosystem-policy`.

