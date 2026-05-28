---
title: "RFC: Intra-project imports (Phase J)"
description: "Cross-file `import \"./helpers.vox\"` + `pub fn` for sharing declarations within a single project. The minimum-viable module system for Vox."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-23"
training_eligible: false
training_rationale: "RFC in design phase; promote to training_eligible once status reaches 'current' after implementation lands."

schema_type: "TechArticle"
---

# RFC: Intra-project imports

> **Status:** draft (2026-05-23). Scoped narrowly: in-project file
> imports only. NOT a package manager, NOT a versioning system, NOT
> external crate FFI. Implementation tracked as audit doc §12 Phase J.
>
> **Bar to land:** approved by the council; cross-file probe scripts
> in `crates/vox-compiler/tests/imports_intra_project_test.rs`;
> `mens-corpus/walk_docs.vox` migrated back from
> `examples/aspirational/intra-project-imports/`.

## §1 — Motivation

Today every `.vox` file in `scripts/` is standalone — no way to share
`fn` declarations across files. The audit doc §Imports/Modules/FFI
audit found that 4 scripts under `examples/aspirational/intra-project-imports/`
(formerly `scripts/mens-corpus/`) want to import helpers:

```vox
// vox:skip — illustrative; the imported helpers don't exist as files.
// scripts/mens-corpus/harvest.vox (today)
// — has to re-inline walk_docs/walk_sources/jsonl_writer logic

// scripts/mens-corpus/harvest.vox (after this RFC)
import "./helpers/walk_docs.vox"     // brings `walk_docs` into scope
import "./helpers/walk_sources.vox"  // brings `walk_sources` into scope

fn main() {
    let docs = walk_docs(".")
    let sources = walk_sources(".")
    // ...
}
```

This is the **minimum viable module system** for Vox. Scope:
- In-project only — no external packages
- File path relative to importing file
- `pub fn` declarations become visible; other `fn`s stay private
- One file is one module; no nested `mod x { ... }` blocks

## §2 — Grammar

### §2.1 — Import statement

The existing `import` keyword gets a 4th path-kind:

```bnf
import-decl   := "import" import-path ("," import-path)*
import-path   := symbol-path     // existing: `import std.http`
              |  react-import    // existing: `import react X from "./X.tsx"`
              |  rust-import     // existing: `import rust:foo`
              |  local-import    // NEW: this RFC
local-import  := STRING_LIT ("as" IDENT)?
```

Examples:

```vox
import "./helpers/walk_docs.vox"               // flat — exports merge into current scope
import "./helpers/walk_sources.vox" as ws      // namespaced — call as `ws.walk_sources(...)`
import "../shared/util.vox"                    // relative path traversal allowed
```

### §2.2 — `pub` modifier on declarations

The existing `Token::Pub` keyword starts to actually mean something.
Only `pub fn`, `pub type`, `pub @table` etc. become visible to importers;
bare `fn`/`type` stay private to the file.

```vox
// helpers/walk_docs.vox
pub fn walk_docs(root: str) to list[str] {
    return collect_md(root)  // calls private helper
}

fn collect_md(root: str) to list[str] {  // NOT exported (no pub)
    // ...
}
```

## §3 — Semantics

### §3.1 — Path resolution

- Paths are **always** quoted strings (rules out the symbol-path confusion).
- `./foo.vox` and `../foo.vox` resolve relative to the importing file's directory.
- Absolute paths (`/foo.vox`, `C:\foo.vox`) are rejected at parse time —
  intra-project means relative to project structure.
- Paths MUST end in `.vox`. Other extensions are rejected.
- Resolution is **literal**: no extension-elision, no
  index.vox fallback, no module-search-paths. One path, one file.

### §3.2 — Visibility

A declaration in an imported file is visible to the importer IFF it
carries the `pub` keyword. Specifically:

| Declaration | Default visibility | With `pub` |
|---|---|---|
| `fn name(...)` | file-private | exported |
| `type X = ...` | file-private | exported |
| `@table type T` | file-private | exported |
| `@query`/`@mutation`/`@server fn` | always exported (they're endpoints) | error: redundant `pub` |
| Top-level `let` bindings | file-private | error: pub-let not supported |

### §3.3 — Scope-merge vs alias

Without `as`, exported names merge flat into the importer's top-level
scope. Two flat imports declaring the same name = **parse-time error**;
the importer must use `as` to disambiguate.

With `as alias`, exported names live under `alias.name`:

```vox
// vox:skip — illustrative; the imported helper doesn't exist as a file.
import "./helpers/walk_docs.vox" as docs

fn main() {
    let files = docs.walk_docs(".")  // namespaced
}
```

### §3.4 — Cycle detection

A → B → A is **rejected at lowering time** with a clear error naming
the cycle:

```
error[E0042]: import cycle detected
  --> a.vox:1:1
   |
1  | import "./b.vox"
   |
   = note: cycle: a.vox -> b.vox -> a.vox
   = help: extract the shared definitions into a third file that both can import.
```

### §3.5 — Re-export

NOT supported in v1.0. A file's `pub fn` is not transitively visible to
an importer's importer. If you want re-export, you write a one-line
wrapper. Keep the surface small.

### §3.6 — Side effects

Imported files are loaded + parsed + lowered. Their top-level
**non-fn-declaration** code (which today doesn't really exist outside
`fn main()`) is NOT executed during import. Only declarations contribute.

## §4 — HIR + lowering

### §4.1 — AST shape

```rust
// crates/vox-compiler/src/ast/decl/types.rs
pub enum ImportPathKind {
    SymbolPath { segments: Vec<String> },
    ReactComponent { local_name: String, module_specifier: String },
    RustCrate(RustCrateImport),
    /// NEW (this RFC). `import "./foo.vox"` and `import "./foo.vox" as alias`.
    LocalFile {
        /// Path string literal as written — kept verbatim for diagnostics.
        path: String,
        /// Optional namespace alias from `as <ident>`.
        alias: Option<String>,
    },
}
```

### §4.2 — HIR lowering with file resolver

The current `lower_module(&Module) -> HirModule` is pure — no I/O. To
load imported files we need a `FileResolver` trait threaded through:

```rust
pub trait FileResolver {
    /// Load + return the source bytes of the .vox file at `abs_path`.
    fn load(&self, abs_path: &Path) -> Result<String, ResolverError>;
}
```

The CLI passes a `FsFileResolver { project_root: PathBuf }`. Tests pass
a `MemoryFileResolver { files: HashMap<PathBuf, String> }`.

Lowering an `ImportPathKind::LocalFile`:

1. Resolve the relative path against the importing file's dir.
2. Check the cycle-set for re-entry; error if cycle.
3. `file_resolver.load(abs_path)?`
4. Lex + parse_script + lower (recursive call with the cycle-set extended).
5. Collect every `pub` declaration from the imported HirModule.
6. Add them to the importer's HirModule's top-level scope (or under an
   alias-namespace object if `as` was used).

### §4.3 — Pub-fn enforcement

Add `is_pub: bool` to `HirFnDecl` (already there as `is_pub`! verified
in `crates/vox-compiler/src/parser/descent/decl/head.rs:957`). Lowering
filter:

```rust
let exported: Vec<HirFnDecl> = imported_hir.fn_decls()
    .filter(|f| f.is_pub)
    .cloned()
    .collect();
```

## §5 — Typecheck

The exported `fn`s get added to the typecheck's `TypeEnv` like any
other module-scope `fn`. No new typeck machinery needed.

For aliased imports (`import "./x.vox" as ns`), the alias becomes a
namespace-typed binding (similar to how `fs` is typed as `FsModule`),
and method-lookup on it resolves the per-fn signatures.

## §6 — Eval

The exported `fn`s become bindings in `interp.module_scope`. Calls to
`walk_docs(...)` from the importer resolve to the closure with the
imported file's captured env.

For aliased imports, the alias is bound to an `Object` whose fields are
the exported function values — same shape as the `fs`/`process`/etc.
namespace markers in `eval/mod.rs:43`.

## §7 — Migration impact

### §7.1 — Aspirational set unblocked (landed 2026-05-23)

The original aspirational set was placeholder CommonJS pseudocode, not
working Vox. Rather than translate, we wrote real helper modules and
an entry-point pipeline:

- `scripts/mens-corpus/helpers/walk_docs.vox` — `pub fn walk_docs(root)`
- `scripts/mens-corpus/helpers/walk_sources.vox` — `pub fn walk_sources(root)`
- `scripts/mens-corpus/helpers/jsonl_writer.vox` — `pub fn write_jsonl(path, lines)`
- `scripts/mens-corpus/harvest_small.vox` — entry point that imports
  all three via `import "./helpers/<name>.vox"` and writes a
  ~3,800-entry JSONL index against the live repo.

The original `examples/aspirational/intra-project-imports/` directory
is retired. Net corpus impact: +4 PASS (32 → 38 / 51).

### §7.2 — `pub` semantics finally meaningful

Today `pub` parses but is ignored. Post-RFC, omitting `pub` makes a
declaration file-private — a meaningful enforcement that prevents
accidental coupling.

## §8 — Risks / open questions

1. **File-resolver injection.** Threading the resolver through every
   call site is invasive. Mitigation: define a thread-local
   `CURRENT_RESOLVER` during `vox check`/`vox run` startup; the lowerer
   calls it transparently. Less type-safe; cleaner call sites.

2. **Cycle detection across deep chains.** A 3-deep cycle (A→B→C→A)
   needs a HashSet on the stack. Standard implementation.

3. **`vox check foo.vox` with a missing import** — should fail with a
   clean error, not a panic. Resolver returns `Err`, lowerer surfaces
   as `EvalError::InvalidImport`. Existing-error-shape work.

4. **Imported file fails its own typecheck** — should we
   surface those errors at the import site, or only when the
   imported file is `vox check`ed directly? **Proposal**: surface BOTH —
   importer sees a single "module foo.vox has N errors" with a
   pointer; running `vox check foo.vox` gives the full per-line list.

5. **Re-export** (deferred to v0.8+). If a user really needs it,
   the workaround is a one-line `pub fn wrapper(x) { other.f(x) }`.

6. **Standard library namespacing.** Does `import "./helpers/io.vox"
   as io` clash with the built-in `io` namespace? **Proposal**: yes,
   it does — the user's alias shadows the stdlib. Lint warning, not
   error. This matches Python `import os` shadow semantics.

## §9 — Implementation plan

| Step | Scope | Cost |
|---|---|---|
| **9.1** | AST: `ImportPathKind::LocalFile { path, alias }` variant | 5 min |
| **9.2** | Parser: at `parse_import` entry, accept StringLit-starting forms | 30 min |
| **9.3** | HIR: add `local_imports: Vec<HirLocalImport>` to HirModule; lower the LocalFile variant into it | 30 min |
| **9.4** | `FileResolver` trait + `FsFileResolver` impl in vox-compiler/src/pipeline | 1 h |
| **9.5** | Lowering pass: recursive load+parse+lower, cycle detection via HashSet on stack | 1 h |
| **9.6** | Visibility filter: only `pub` declarations contribute to importer | 30 min |
| **9.7** | Typeck: imported pub fns become TypeEnv bindings | 30 min |
| **9.8** | Eval: imported pub fns become interp.module_scope bindings | 1 h |
| **9.9** | Aliased imports: `as ns` creates a namespace-Object binding | 1 h |
| **9.10** | Tests: 2-file probe, alias probe, cycle probe, missing-file probe, double-flat-import probe | 1 h |
| **9.11** | Codegen TS: import statement emission | 1 h |
| **9.12** | Codegen Rust: module declaration emission | 1.5 h |
| **9.13** | Migrate aspirational set back to scripts/mens-corpus/ | 30 min |
| **9.14** | Doc updates: ref-builtins-stdlib.md import section; AGENTS.md script policy | 30 min |

**Total: ~9 days focused work.** Could compress to 3-5 elapsed if no
codegen blockers.

## §10 — Out of scope for v0.7

- **External packages / version pinning.** Use `import rust:foo` (still
  not wired) or the planned `vox add` package manager.
- **Glob imports** (`import "./helpers/*.vox"`). Explicit per-file imports only.
- **Re-export** (see §8 risk 5).
- **Module-level constants** (`pub const X = 42`). Add when corpus demands.
- **`mod foo { ... }` nested modules.** File is the unit.

## §11 — Decisions ratified

Per the user's "best long-term interest of the language" directive:
- Single canonical syntax: `import "./path.vox" [as alias]` only.
  NO `from "./path.vox" import {a, b}` ES6 form. NO `use path::a` Rust
  form. ONE form.
- Default visibility is **file-private**. `pub` makes it visible.
  Matches Rust; protects against accidental coupling.
- Path must be a string literal. Not a symbol-path. Not an identifier.
  The quotes are a visual cue that this is a file path, not a namespace.
- Cycle detection is mandatory, not best-effort. Better diagnostic now
  than mysterious infinite-loop or stack-overflow later.
- The AST already has `RustCrateImport` and `ReactComponent`. The
  new `LocalFile` variant joins those — four flavors of imports
  remain bounded.

## Related plans

- [`closures-rfc-2026-05-23.md`](./closures-rfc-2026-05-23.md) — the
  precedent for "scoped feature RFC with health-first defaults"
- [`vox-stdlib-gap-audit-2026-05-23.md`](./vox-stdlib-gap-audit-2026-05-23.md)
  §Imports/Modules/FFI audit — the demand signal that motivated this RFC
- [`plugin-system-redesign-2026.md`](./plugin-system-redesign-2026.md)
  — separate concern (compile-time crate composition vs script-level
  import); the audit recommends NOT conflating them
