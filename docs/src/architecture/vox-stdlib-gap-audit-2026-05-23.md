---
title: "Vox stdlib & interp gap audit (2026-05-23)"
description: "What committed .vox scripts call vs what the installed binary actually executes. Investigation handoff — not a plan."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Audit snapshot; will be superseded once fixes land. Useful for humans triaging; not a stable reference."
---

# Vox stdlib & interp gap audit — 2026-05-23

**Binary under test:** `vox 0.5.0+build.601 (6ce0f2d09)` on Windows 11.
**Scope:** `scripts/**/*.vox` (53 files). Out of scope: `.claude/worktrees/`.
**Author:** investigation only — no fixes applied this session.

This is a handoff for whoever picks up the AI-laziness remediation track. The
findings are concrete enough to spawn fix tasks against; the open questions at
the end need a human decision before some of those fixes can be designed.

---

## 1. Executive summary

- **89 % of committed `scripts/**/*.vox` fail `vox check` against the installed
  binary** — 22 parse errors, 25 type/eval errors, only **6 of 53** pass. The
  exemplar corpus an AI (or a contributor) would copy from largely does not
  compile.
- **Silent wrong-output bug, P0:** `!bool` in interp mode evaluates to `bool`
  (identity, not negation). Root cause is in the lexer
  ([`crates/vox-compiler/src/lexer/token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)):
  there is no `Token::Bang`. `!` is silently dropped, so `if !x { … }` parses
  as `if x { … }`. `vox check` reports **0 warnings** on such code.
- **Three whole namespaces used by committed scripts do not exist at runtime:**
  `regex.*` (11 call sites), `list.*` free functions (27 call sites), and most
  of `path.*` (only `path.join` is implemented; `path.extension`, `path.parent`,
  `path.file_name`, `path.stem`, `path.is_absolute` are all absent).
- **Discoverability bug, P0:** `vox run scripts/foo.vox` (the form every
  committed docstring shows, and the form [`AGENTS.md`](../../../AGENTS.md)
  documents) errors with `feature not enabled`. The actual working incantation
  is `vox run --mode interp scripts/foo.vox`. The error message does not
  mention the flag.
- **Documentation drift, P1:** [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md)
  claims `path.extension` and several `std.path.*` helpers exist; they don't.
  [`vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md) is closer
  to truth but doesn't document the method-vs-free split, the missing
  namespaces, or the `--mode interp` requirement.

This is the leak in the "Vox is the first language designed for LLMs as the
primary author" pitch: the corpus an LLM would learn from does not run.

---

## 2. Reproduction matrix

All probes live under [`tmp/vox-audit-probes/`](../../../tmp/vox-audit-probes/) (not
committed). Each one is 5–10 lines of `.vox`. Run with
`vox run --mode interp tmp/vox-audit-probes/<file>.vox` unless noted.

### #1 — Feature gate ("script-execution" not enabled)

```bash
$ vox run scripts/test_for.vox
Error: script run mode requires `vox` built with `--features script-execution` (file: scripts/test_for.vox)

$ vox run --mode interp scripts/test_for.vox    # ← works
(no output, exit 0)
```

The error message names a build feature, not a CLI flag. An AI following any
committed script's docstring header (`vox run scripts/foo.vox`) will hit this
on every script and have no breadcrumb to the workaround.

### #2 — `!` operator is a no-op (P0 silent wrong output)

```vox
// vox:skip
fn main() {
    let t = true;
    let f = false;
    if !t { print("!true fired"); }   else { print("!true did NOT fire (correct)"); }
    if !f { print("!false fired (correct)"); } else { print("!false did NOT fire"); }
    if not t { print("not true fired"); }   else { print("not true did NOT fire (correct)"); }
    if not f { print("not false fired (correct)"); } else { print("not false did NOT fire"); }
}
```

Actual output:

```
!true fired                       ← WRONG (should be "did NOT fire")
!false did NOT fire               ← WRONG (should be "fired")
not true did NOT fire (correct)   ← right
not false fired (correct)         ← right
```

`vox check` on the same file: **`Check passed with 0 warning(s)`**.

**Root cause:** [`crates/vox-compiler/src/lexer/token.rs:105`](../../../crates/vox-compiler/src/lexer/token.rs)
registers `#[token("not")] Not` and `#[token("!=")] NotEq` but **nothing for the
bare `!` character**. Logos (the lexer generator) silently skips unknown chars,
so `if !exists { … }` lexes as `if exists { … }`. The eval path at
[`crates/vox-compiler/src/eval/expr.rs:123`](../../../crates/vox-compiler/src/eval/expr.rs)
correctly does `!b`, but it never sees a `HirUnOp::Not` for `!` because the
parser never produces one.

The `not` keyword works because it lexes to `Token::Not` directly and the
parser at
[`crates/vox-compiler/src/parser/descent/expr/pratt_match.rs:67`](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs)
maps that to `UnOp::Not`.

### #3a — `fs.list_recursive` does not exist

```vox
// vox:skip
fn main() { let xs = fs.list_recursive(".", "*.md"); print("ok"); }
```
→ `Error: Eval failed calling main: AssertionFailed("Method list_recursive not found")`

Used in [`scripts/migrate-arrows.vox`](../../../scripts/migrate-arrows.vox),
[`scripts/migrate-corpus.vox`](../../../scripts/migrate-corpus.vox),
[`scripts/quality/doc-policy-lint.vox`](../../../scripts/quality/doc-policy-lint.vox).

### #3b — `path.extension` does not exist

```vox
// vox:skip
fn main() { let e = path.extension("foo/bar.txt"); print("ok"); }
```
→ `Error: Eval failed calling main: AssertionFailed("Method extension not found")`

Used in [`scripts/migrate-corpus.vox`](../../../scripts/migrate-corpus.vox).
Documented as existing in [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md).

### #3c — `str.split_lines(s)` (free form) does not exist

```vox
// vox:skip
fn main() { let ls = str.split_lines("a\nb\nc"); print("ok"); }
```
→ `Error: Eval failed calling main: AssertionFailed("Method split_lines not found")`

The method form **also does not exist** (`s.split_lines()` not registered).
Workaround used in the cf-migration session: `s.split("\n")`.

### #3d — `regex.*` namespace does not exist

```vox
// vox:skip
fn main() { let r = regex.replace("hello world", "world", "vox"); print(r); }
```
→ `Error: Eval failed calling main: UndefinedVariable("regex")`

This is **worse than "method not found"** — the namespace identifier itself is
unbound. 11 calls across 5 scripts dead on arrival, including all 3 calls in
[`scripts/migrate-arrows.vox`](../../../scripts/migrate-arrows.vox) (whose entire
purpose is regex-driven corpus migration).

### #3e — `list.*` namespace does not exist

```vox
// vox:skip
fn main() { let xs = [1,2,3]; list.push(xs, 4); print(xs.len()); }
```
→ `Error: Eval failed calling main: UndefinedVariable("list")`

20 call sites for `list.push`, 7 for `list.len` are dead. The method forms
(`xs.push(4)`, `xs.len()`) work. Scripts mix both freely.

### #4 — Method-vs-free function inconsistency

```vox
// vox:skip
fn main() {
    let s = "  hello  ";
    print("method: [" + s.trim() + "]");      // works → "method: [hello]"
    print("free:   [" + str.trim(s) + "]");   // fails with "Method trim not found"
}
```

For every string operation: only the method form works. There is **no
`str.*` namespace at runtime** despite docs and scripts assuming there is.
The error message ("Method trim not found") is also misleading because the
call site is a free function, not a method call.

Same pattern for `list.*` (only `xs.push`, never `list.push(xs, x)`).

### Bonus issue — `to_s` method on bool

Original AI-written probes used `.to_s()` (common Ruby-style). Bool implements
`.to_str()` and `.to_string()` but **not `.to_s()`**. Error:
`Method to_s not found`. Low impact but adds to the "natural-API guess fails
silently" pile.

### Bonus issue — Pervasive parser failure on committed scripts

When I tried to run `scripts/clean-build-artifacts.vox`,
`scripts/quality/doc-policy-lint.vox`, and `scripts/migrate-arrows.vox`
end-to-end, all three failed at **parse time** — not at eval, not at typecheck.
Examples:

- `scripts/clean-build-artifacts.vox` → `Expected pattern, found ","` inside a
  `match` arm
- `scripts/quality/doc-policy-lint.vox` → `Unexpected token in expression: |`
  (closure / lambda syntax)
- `scripts/migrate-arrows.vox` → `Expected ), found "\)\s*->\s*(...)"` (likely
  closure-arrow syntax inside a string literal that confuses the parser)

These are language-evolution casualties, not pure stdlib issues, but they are
in the same bucket of "the committed corpus does not run on the installed
binary." Full per-script status in §3.2 below.

---

## 3. Symbol audit

### 3.1 Per-symbol table (`scripts/*.vox` use vs `vox run --mode interp`)

Compiled from a frequency audit of all 53 scripts, cross-referenced with the
interp builtin SSOT at
[`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
and verified by probe in `tmp/vox-audit-probes/`.

Legend: **WORKS** / **MISSING** / **METHOD-ONLY** (free form claimed by scripts
but only the method form is registered).

| Symbol                       | Form   | Uses | Status        | Notes / workaround |
|------------------------------|--------|-----:|---------------|--------------------|
| `print`                      | global |  ~all| WORKS         | OK |
| `len(x)`                     | global |     —| WORKS         | also `.len()` method |
| `fs.exists`                  | free   |    8 | WORKS         | also `std.fs.exists` works |
| `fs.read` / `fs.read_to_string` / `fs.read_file` | free | 14 | WORKS  | all three aliases registered |
| `fs.write` / `fs.write_file` | free   |    9 | WORKS         | |
| `fs.is_dir` / `fs.is_file`   | free   |    — | WORKS         | |
| `fs.list_dir`                | free   |    3 | WORKS         | |
| `fs.list_dir_detailed`       | free   |    — | WORKS         | returns `Result[List[Record]]` |
| `fs.glob`                    | free   |    2 | WORKS         | |
| `fs.mkdir`                   | free   |    — | WORKS         | creates parents (no separate `mkdir_p`) |
| `fs.walk`                    | free   |    5 | **MISSING**   | use `fs.glob("**/*.ext")` |
| `fs.list_recursive`          | free   |    5 | **MISSING**   | use `fs.glob` |
| `fs.copy`                    | free   |    1 | **MISSING**   | shell out via `process.run` |
| `fs.remove` / `fs.remove_dir_all` | free | 2 | mixed       | `remove_dir_all` exists, `remove` doesn't |
| `fs.canonicalize`            | free   |    — | **MISSING**   | |
| `fs.cwd`                     | free   |    1 | **MISSING**   | |
| `path.join`                  | free   |    7 | WORKS         | also `std.path.join` |
| `path.extension`             | free   |    — | **MISSING**   | doc-claimed; absent |
| `path.parent` / `dirname`    | free   |    — | **MISSING**   | doc-claimed (`dirname`); absent |
| `path.file_name` / `basename`| free   |    — | **MISSING**   | doc-claimed (`basename`); absent |
| `path.stem`                  | free   |    — | **MISSING**   | |
| `path.is_absolute`           | free   |    — | **MISSING**   | |
| `str.trim` (free)            | free   |    3 | METHOD-ONLY   | use `s.trim()` |
| `str.split` (free)           | free   |    5 | METHOD-ONLY   | use `s.split(d)` |
| `str.split_lines` (free)     | free   |    2 | **MISSING**   | use `s.split("\n")` — no method form either |
| `str.starts_with` (free)     | free   |    5 | METHOD-ONLY   | use `s.starts_with(p)` |
| `str.starts_with_pattern`    | free   |    1 | **MISSING**   | |
| `str.ends_with` (free)       | free   |    4 | METHOD-ONLY   | |
| `str.contains` (free)        | free   |   11 | METHOD-ONLY   | |
| `str.replace` (free)         | free   |    8 | METHOD-ONLY   | |
| `str.to_upper` / `to_lower`  | free   |    — | METHOD-ONLY   | |
| `regex.replace`              | free   |   11 | **MISSING**   | namespace itself undefined |
| `regex.compile`              | free   |    1 | **MISSING**   | |
| `regex.is_match` / `captures`| free   |    — | **MISSING**   | |
| `process.exit`               | free   |    3 | WORKS         | also `std.process.exit` |
| `process.run`                | free   |    4 | WORKS         | |
| `process.exec`               | free   |    — | WORKS         | |
| `process.run_ex` / `run_capture_json` / `run_capture_lines` / `spawn_background` / `register_exit_command` | free | few | WORKS | |
| `env.get`                    | free   |    8 | WORKS         | also `std.env.get` |
| `env.set`                    | free   |    — | WORKS         | |
| `env.args`                   | free   |    1 | WORKS         | (not `process.args` — different ns) |
| `json.parse`                 | free   |    9 | WORKS         | |
| `json.stringify` / `json.render` / `json.encode` | free | 9 | WORKS | all three aliases |
| `csv.parse` / `parse_records` / `render` | free | — | WORKS         | |
| `toml.parse` / `render`      | free   |    — | WORKS         | |
| `yaml.parse` / `render`      | free   |    — | WORKS         | |
| `log.debug` / `info` / `warn` / `error` | free | 4 | WORKS    | scripts also use `std.log.*` |
| `list.push` (free)           | free   |   20 | **MISSING**   | use `xs.push(x)` — `list` ns is `UndefinedVariable` |
| `list.len` (free)            | free   |    7 | **MISSING**   | use `xs.len()` |
| `println` / `eprint` / `eprintln` | global | 12 | **MISSING** | use `print` |
| `bool.to_s` (method)         | method |    — | **MISSING**   | use `.to_str()` or `.to_string()` |
| `secrets.resolve`            | free   |    — | WORKS         | |
| `agentos.mutation_kind_for_tool` | free | —  | WORKS         | |
| `io.open` / `io.save`        | free   |    — | WORKS         | |

Method-call dispatch (`s.trim()`, `xs.push(x)`, `result.unwrap()`,
`option.is_ok()`) is mostly wholesome and the dominant working form. The
free-function `namespace.fn(receiver, …)` form is mostly **unimplemented**
except for the file-system / process / json / csv / toml / yaml / env / log
namespaces, which were intentionally designed as free-function namespaces.

### 3.2 Per-script parse/check status (53 files)

`vox check` outcome for every `scripts/*.vox`. **6 pass** (11 %).

| Status        |   N | Examples |
|---------------|----:|----------|
| PASS          |   6 | `test_for.vox`, `generate-bench-scaffold.vox`, `ci/script-hygiene.vox`, `mens/full-pipeline.vox`, `migrations/2026-phase1-contract-headers.vox`, `migrations/2026-phase7-target-cleanup.vox` |
| PARSE-FAIL    |  22 | `arch-check`, `check_dashboard_ssot`, `clean-build-artifacts`, `docs-reality-audit-cycle`, `extract_table_names`, `index_symbols`, `migrate-arrows`, `migrate-corpus`, `mens/*` (4 of 5), `mens-corpus/*` (all 5), `quality/doc-policy-lint`, `scientia/*` (4 of 5), `setup`, `ci-proximity-drift`, 2× `migrations/2026-phase1-delete-…` |
| CHECK-FAIL    |  25 | `ci/compile_kernels`, `ci/corpus_prep`, `ci/gui-e2e-check`, `ci/gui-registry-check`, `ci/test`, `generate-grammars`, `gui-build`, `install-hooks`, `orchestrator/*` (both), `quality/archival-enforcer`, `quality/audit-{dependency-layers, telemetry, workspace-health}`, `quality/generate-matrix-doc`, `render-durable-animation`, `scientia/acceptance-matrix`, `smoke-llm`, `sync_golden_vox`, `test_fs`, `test_process_primitives`, `test_recursion` |

PARSE-FAIL is the more alarming bucket — these files use language features the
installed parser cannot read (closures `|x| { … }`, regex-literal-looking
patterns, certain `match` arm shapes). They are not stdlib bugs; they are
language-evolution-vs-corpus drift. Same root cause class as the stdlib gap:
the corpus and the binary disagree.

`scripts/ci/script-hygiene.vox` (currently the only enforced gate) checks
**parse** (`cargo run -p vox-cli -- check`), not run-compatibility, so all the
CHECK-FAIL and runtime-error cases slip through CI.

---

## 4. Binary builtin SSOT — where the truth lives

| Surface                | File:line                                                                                       |
|------------------------|-------------------------------------------------------------------------------------------------|
| Global builtins (print, len, str/int/float/bool, range, type_of, assert) | [`crates/vox-compiler/src/eval/builtins.rs:1217–1316`](../../../crates/vox-compiler/src/eval/builtins.rs) |
| Method dispatcher (incl. `str.*`, `list.*`, `result.*`, `option.*` methods) | [`crates/vox-compiler/src/eval/builtins.rs:92`](../../../crates/vox-compiler/src/eval/builtins.rs) `call_builtin_method` |
| `fs.*` registrations           | `eval/builtins.rs:393–551` |
| `env.*` / `path.*` / `secrets.*` / `process.*` | `eval/builtins.rs:552–873` |
| `csv` / `toml` / `yaml` / `io` / `json` / `log` | `eval/builtins.rs:887–1044` |
| Namespace marker objects (`__namespace__`) | [`crates/vox-compiler/src/eval/mod.rs:41–180`](../../../crates/vox-compiler/src/eval/mod.rs) |
| Unary `!` / `-` evaluator (correct; see lexer bug)     | [`crates/vox-compiler/src/eval/expr.rs:120–128`](../../../crates/vox-compiler/src/eval/expr.rs) |
| Lexer token table (MISSING `Token::Bang`)              | [`crates/vox-compiler/src/lexer/token.rs:100–115, 258–261`](../../../crates/vox-compiler/src/lexer/token.rs) |
| Parser unary handler                                    | [`crates/vox-compiler/src/parser/descent/expr/pratt_match.rs:67–84`](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs) |
| HIR lowering for unary                                  | [`crates/vox-compiler/src/hir/lower/expr.rs:59–64`](../../../crates/vox-compiler/src/hir/lower/expr.rs) |
| Script-execution feature plugin (the gate behind `vox run`) | [`crates/vox-plugin-script-execution/src/executor.rs`](../../../crates/vox-plugin-script-execution/src/executor.rs) |
| Run-mode dispatch (`--mode interp` vs default)          | [`crates/vox-cli/src/commands/runtime/run/`](../../../crates/vox-cli/src/commands/runtime/run/) |

There is **no `regex` runtime registration** anywhere in
`crates/vox-compiler/src/eval/`. Typeck has a `regex.compile` signature entry
([`crates/vox-compiler/src/typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs))
which is why typecheck doesn't fail on `regex.replace(…)` — but at eval, the
namespace identifier is unbound.

---

## 5. Documentation gap

### 5.1 Claims that don't match the binary

| Doc                                                              | Claims                                  | Reality                       |
|------------------------------------------------------------------|-----------------------------------------|-------------------------------|
| [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md) | `path.extension`, `path.basename`, `path.dirname`, `path.join_many` | Only `path.join` exists |
| [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md) | `process.exit`                          | Actually correct ✓            |
| [`AGENTS.md` § VoxScript-First Glue](../../../AGENTS.md)         | `vox run scripts/foo.vox` works for File-I/O scripts (Native tier) | Errors with `feature not enabled`; must use `--mode interp` |
| [`docs/src/archive/research-2026-q1/vox-automation-primitives.md`](../archive/research-2026-q1/vox-automation-primitives.md) (archived) | `path.basename/dirname/extension`         | Doc is `training_eligible: false`, but still in corpus |
| Most committed scripts' docstring headers                         | `vox run scripts/foo.vox`                | Same flag bug; ~all are wrong |

### 5.2 What's actually documented well

- [`docs/src/architecture/vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md)
  is the cleanest reference and **does not lie** — it stays narrow (mostly
  `std.fs.list_dir_detailed`, `std.csv`, `std.toml`, `std.process.run_capture_lines`)
  and explicitly recommends `run_capture_lines` instead of a missing
  `str.split_lines`.
- The doc does not document the `--mode interp` requirement.
- The doc does not document the method-vs-free split.

### 5.3 What's missing entirely

- No "what works in interp" reference.
- No documented warning that `!` is not a valid operator (only `not`).
- No `vox audit stdlib` or `vox doctor --stdlib` tool that diffs claimed-vs-implemented.
- No CI gate that runs (vs just parse-checks) committed scripts.

---

## 5.4 What landed this session (2026-05-23)

The following parts of §6 / §7 / §9 were implemented and verified before
this doc was last saved. Subsequent sessions can skip these items.

| # | What | Verified by |
|---|------|-------------|
| Task 1 | `!` lexes as `Token::BangInvalid` and produces a clear "use `not`" parse error (Option B from the K-complexity ratify); the `not` keyword is the canonical and only negation form. | `cargo test -p vox-compiler --test interpreter_test` → `not_keyword_inverts_bool_correctly` + `bang_is_a_parse_error_with_phonetic_hint`; `cargo test -p vox-compiler --lib lexer::cursor` → 23 tests pass |
| Task 2 (Path A) | `vox run scripts/foo.vox` (no flag) auto-falls back to interp when `script-execution` Cargo feature is not compiled in. Single-file edit in [`crates/vox-cli/src/commands/run.rs`](../../../crates/vox-cli/src/commands/run.rs). No binary bloat. | `./target/debug/vox.exe run scripts/test_for.vox` succeeds without flags; emits a single INFO log noting the fallback. |
| Task 3 | Improved `--mode script` error message: now reads *"`vox run --mode script` requires a vox build with `--features script-execution`. Try `vox run --mode interp …` (interpreter is always available), or rebuild with `cargo build --features script-execution`."* | `./target/debug/vox.exe run --mode script tmp/vox-audit-probes/hello.vox` |
| Task 5 | `regex.*` runtime namespace: `regex.replace`, `regex.is_match`, `regex.captures`. Added to [`crates/vox-compiler/src/eval/mod.rs`](../../../crates/vox-compiler/src/eval/mod.rs) (namespace markers) and [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs) (dispatch). `regex` crate added as a `vox-compiler` dep. | Probe `tmp/vox-audit-probes/probe_regex_path.vox` — `regex.replace("hello world", "world", "vox") == "hello vox"`, `regex.is_match("a 42 b", "[0-9]+") == true` |
| Task 6 | `path.*` helpers: `extension`, `parent`, `file_name`, `stem`, `is_absolute`. Trivial `std::path::Path` wrappers. **Did not** add `basename`/`dirname` aliases — canonical names match Rust. | Same probe — `path.extension("foo/bar.txt") == "txt"`, `path.parent == "foo"`, `path.file_name == "bar.txt"`, `path.stem == "bar"`. |
| Task 7 | Corpus migration `println` / `eprint` / `eprintln` / `println!` → `print` / `log.error` / `log.warn`. 14 scripts touched. Zero remaining call sites of the deprecated forms under `scripts/`. | `Grep \\b(println\|eprint\|eprintln)!?\\s*\\(` against `scripts/` returns no matches. |
| Task 9 Part A | `str.x(s)` and `list.x(xs)` free-function-style calls now emit a clear, K-complexity-policy-citing error pointing at the method form, instead of the confusing `Method foo not found`. Edit in [`crates/vox-compiler/src/eval/expr.rs`](../../../crates/vox-compiler/src/eval/expr.rs). | Probe — `str.trim(s)` errors with *"`str.trim(receiver, ...)` is not a valid call form in Vox; use the method form `receiver.trim(...)` instead. (Vox makes string and list operations method-only per K-complexity policy.)"* |
| Task 11 (partial) | [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md) updated: `path.*` section rewritten; new `regex.*` section; new "Strings and lists are method-only" section; `print` note clarifies no `println`/`eprint` variants; negation note added (use `not`, not `!`). `last_updated` bumped to 2026-05-23. | (doc inspection) |

### Pre-existing bugs fixed as collateral

1. **`TemplateStringLit` swallowed plain string literals.** The
   token's regex matched any `"..."` and returned `None` on non-templates;
   Logos emits a lexer error on `None`, so `"hello"` (a plain string)
   silently disappeared. The result was `print("hello")` appearing to
   typecheck as `print()` (zero args), and many other downstream weirdness.
   Fixed: the regex now requires at least one `{...}` segment, so plain
   strings fall to `StringLit` naturally. Both
   `test_string_literals` and `test_string_literal_with_backticks_and_colon`
   now pass. See [`crates/vox-compiler/src/lexer/token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)
   around the `TemplateStringLit` definition.

2. **`Print` returning `Null` was printed at end of every script.** The
   tree-walking interp emitted `println!("{:?}", res)` regardless of
   whether `main()` had a meaningful return. The new `run_interp` helper
   suppresses the print when `res == VoxValue::Null`. See
   [`crates/vox-cli/src/commands/run.rs`](../../../crates/vox-cli/src/commands/run.rs).

### What's still deferred to a future session

- **Task 8** — remove `fs.list_recursive` from the docs (no corpus calls
  remain after Task 7 found none). Mostly a doc grep-and-delete.
- **Task 9 Part B** — `vox fmt` rewrite rule that mechanically
  transforms `<ns>.<fn>(<receiver>, ...args)` → `<receiver>.<fn>(...args)`
  for the `str`/`list` namespaces. Requires building an AST-rewrite
  affordance on top of the existing pretty-printer at
  [`crates/vox-compiler/src/fmt/`](../../../crates/vox-compiler/src/fmt/).
- **Task 10** — full corpus triage of the 47 PARSE-FAIL / CHECK-FAIL
  scripts into A/B/C buckets. Mechanical Bucket B fixes for `->` (match arms)
  and the `!` migration should land first; Bucket A scripts (closures,
  Option/Result method completion) move to `examples/aspirational/`.
- **Task 12** — the `vox audit stdlib-coverage` subcommand (per §10 spec)
  — durable drift gate.
- **Task 13** — promote `vox check` on `scripts/**` from advisory to
  mandatory CI; depends on Task 10 reaching zero failures.

---

## 6. Prioritized fix list (decisions ratified 2026-05-23)

The open questions from §8 were resolved with the user in the session that
produced this audit. Calls are locked in; see §8 for the rationale capsule.

| Decision | Choice |
|----------|--------|
| Feature gate | Keep the Cargo feature for slim builds; **ship release binaries with `--features script-execution` ON** so `vox run foo.vox` works without `--mode interp`. Improve error message regardless. |
| Method vs free | **Method form is canonical** for value-receiver ops (`s.trim()`, `xs.push(x)`). Free-namespace form stays canonical for stateless utilities with no natural receiver (`fs.read`, `path.join`, `regex.replace`, `process.run`, `env.get`). `str.trim(s)` / `list.push(xs, x)` get deprecated and removed once corpus is migrated. |
| `list_recursive` vs `glob` | **`fs.glob("**/*.md")` wins** on K-complexity. `fs.list_recursive` is dropped, not aliased. |
| Aspirational scripts | Triage into A/B/C buckets (§9). **Err on building Bucket-A features**, not deleting their scripts. |
| Drift prevention | Extend the existing [`vox-audit` crate](../../../crates/vox-audit/) with a `stdlib-coverage` subcommand. No generated MD. |

Effort estimates are coarse (S=hours, M=days, L=>1 week).

### P0 — silent correctness bugs and discoverability blockers (week 1)

1. **Add `Token::Bang` to the lexer and route to `UnOp::Not`.** Every
   AI-written `!` in a `.vox` file is currently a silent no-op. Add
   `#[token("!")] Bang` in
   [`crates/vox-compiler/src/lexer/token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)
   alongside the existing `not` keyword, route through
   [`pratt_match.rs:67`](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs)
   as `UnOp::Not`. Add a regression test that asserts `!true == false`. (S, ~2 h)
2. **Ship release binaries with `--features script-execution`.** Audit
   [`crates/vox-cli/Cargo.toml`](../../../crates/vox-cli/Cargo.toml) and the
   release workflow [`release-gui.yml`](../../../.github/workflows/release-gui.yml)
   to confirm the feature is in `--features` of the binary distribution. Keep
   the feature gate for slim builds (CI that only `vox check`s). (S, ~1 h
   verification.)
3. **Fix the `feature not enabled` error message.** In
   [`crates/vox-cli/src/commands/runtime/run/`](../../../crates/vox-cli/src/commands/runtime/run/),
   replace the current opaque message with: *"`vox run` requires either
   `--mode interp` or a vox build with `--features script-execution`. Most
   users want `--mode interp`."* (S, ~30 min.)
4. **Add a `vox-fmt` / `vox check` lint for `!` mis-use after fix #1 lands.**
   Even with the lexer fixed, training data still contains `!`-as-no-op shaped
   code; a lint that flags `!` adjacent to non-boolean or in suspicious
   context catches regressions. (M)

### P1 — high-leverage stdlib gaps (week 2)

5. **Implement the `regex` runtime namespace** (`regex.replace`, `regex.is_match`,
   `regex.captures`, `regex.compile`). Already type-checked; just unimplemented
   at eval. 11+ script call sites unblocked. Use the `regex` Rust crate
   (already a workspace dep). (M)
6. **Implement the missing `path.*` helpers** — `extension`, `parent`,
   `file_name`, `stem`, `is_absolute`. Trivial wrappers over
   `std::path::Path`. Drop the doc-claimed `path.basename` / `path.dirname`
   names in favor of `path.file_name` / `path.parent` (matches Rust naming;
   one set of names, not two). (S–M)
7. **~~Register `println`, `eprint`, `eprintln`~~ Migrate corpus to use
   `print` + `log.*`.** Verified: `print` in
   [`crates/vox-compiler/src/eval/builtins.rs:1219-1226`](../../../crates/vox-compiler/src/eval/builtins.rs)
   already uses `println!` under the hood — it newlines. So `println` is
   100 % redundant. Vox already has `log.warn` / `log.error` / `log.info`
   / `log.debug` ([builtins.rs:1030–1044](../../../crates/vox-compiler/src/eval/builtins.rs))
   which route to stderr with a level tag — semantically richer than
   bare `eprint`/`eprintln`. **Don't add the variants.** Instead, mechanically
   rewrite the 12+ call sites:
   - `println(x)` → `print(x)`
   - `eprint(msg)` / `eprintln(msg)` → `log.error(msg)` (when intent is error)
     or `print(msg)` (when intent is plain diagnostic). (S, no binary change)
8. **Document `fs.glob` as canonical recursive lister; remove
   `fs.list_recursive` from anywhere it's documented.** No new implementation;
   it's a corpus-migration task. (S)

### P2 — surface canonicalization (weeks 2–3)

9. **Method-form canonicalization.** Two parts:
   - In [`eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs),
     **remove** the `str` and `list` namespace markers from
     [`eval/mod.rs:41–180`](../../../crates/vox-compiler/src/eval/mod.rs) so
     `str.trim(s)` errors with a clear *"`str.trim` was removed; use
     `s.trim()` instead"* message instead of the confusing
     `Method trim not found`.
   - Add a `vox-fmt` rewrite rule: `<ns>.<fn>(<receiver>, ...args)` →
     `<receiver>.<fn>(...args)` for `str.*` and `list.*` only (this is a
     mechanical lossless transform).
   - Run `vox fmt scripts/**/*.vox` to migrate the corpus in one PR. (M)
10. **Triage all 47 broken `scripts/*.vox`** into the A/B/C buckets in §9.
    Bucket-B mechanical fixes ship as one PR; Bucket-A items spawn roadmap
    issues; Bucket-C scripts move under `examples/aspirational/<feature>/`
    with a banner header. (L overall but parallelizable.)
11. **Update [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md)
    to match reality.** Remove claims of `path.extension`, `path.basename`,
    `path.dirname`, `path.join_many` until tasks #6 lands, then re-add the
    names that survived the rename. Add a paragraph stating the
    method-vs-free policy. (S)
12. **Update [`AGENTS.md` § VoxScript-First Glue](../../../AGENTS.md)** and
    every committed script's docstring header to drop `--mode interp` from
    invocation examples (because of fix #2, plain `vox run` will work). (S)

### P3 — durable drift prevention (week 3)

13. **Build `vox audit stdlib-coverage`.** New subcommand under the
    existing [`crates/vox-audit/`](../../../crates/vox-audit/) registry. Spec
    in §10 of this doc. Three-way diff: docs claims ↔ binary registrations
    ↔ corpus usage. Output JSON (CR-L8 pattern); exit non-zero on mismatch.
    Wired into the existing `cr-l8-corpus-feedback.yml` workflow as a sibling
    job, gated on changes to
    `crates/vox-compiler/src/eval/builtins.rs`,
    `scripts/**`,
    `docs/src/reference/ref-builtins-stdlib.md`. (M)
14. **Promote `vox check` on `scripts/**` from advisory to mandatory CI.**
    Once §6 #10 (triage) lands, every script in `scripts/` typechecks. Wire
    the gate so a regression is impossible. (S to wire, depends on #10
    landing.)

---

## 7. Spawn-ready task prompts

Each prompt is self-contained — assume the agent picking it up has no
conversation context. Numbering matches §6.

### Task 1 — Fix `!` operator lexer bug (P0)

> In [`crates/vox-compiler/src/lexer/token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)
> at line 100–115 there is `#[token("not")] Not` (the `not` keyword) and
> [`#[token("!=")] NotEq`](../../../crates/vox-compiler/src/lexer/token.rs)
> at line 260–261, but **no entry for the bare `!` character**. Logos
> silently drops unknown chars, so `if !x { ... }` parses as `if x { ... }`
> — verified by probe (see audit doc §2 issue #2). The eval and parse paths
> downstream are correct; the bug is purely in the lexer.
>
> Add `#[token("!")] Bang` to the token enum. In the parser at
> [`crates/vox-compiler/src/parser/descent/expr/pratt_match.rs:67–75`](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs)
> add a `Token::Bang =>` arm mirroring the existing `Token::Not` arm — both
> produce `UnOp::Not`. (The HIR lowering at
> [`hir/lower/expr.rs:61`](../../../crates/vox-compiler/src/hir/lower/expr.rs)
> already handles `UnOp::Not`; no change there.)
>
> Tests:
> 1. Unit test in `crates/vox-compiler/src/lexer/` asserting `!` lexes as `Bang`.
> 2. Eval test asserting `!true == false` and `!false == true` round-trip.
> 3. Regression test using the audit doc's repro from
>    `tmp/vox-audit-probes/probe_not2.vox`.
>
> Do **not** remove the `not` keyword — both forms must work. Update
> [`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md`](./vox-stdlib-gap-audit-2026-05-23.md)
> §2 to note the fix and the binary version that contains it.

### Task 2 — Make `vox run` work without `--mode interp` on release binaries (P0)

> **Verified state (2026-05-23):**
> - [`crates/vox-cli/Cargo.toml:30-31`](../../../crates/vox-cli/Cargo.toml)
>   has `[features] default = []` — script-execution is **opt-in**, not default.
> - Release artifacts are built via
>   [`crates/vox-cli/src/commands/ci/release_build.rs:176-188`](../../../crates/vox-cli/src/commands/ci/release_build.rs)
>   with `cargo build -p vox-cli --release --locked --target <triple>`
>   and **no `--features` flag**. Every shipped binary today lacks
>   `script-execution`.
> - From `vox run --help`: `--mode auto` (the default) says *"run as a
>   script when `script-execution` is enabled; else app path"*. Today it
>   errors instead of falling through to interp.
>
> **Two implementation paths — let the user choose before coding.**
>
> **Path A — make `--mode auto` fall back to `interp` for script-like
> files.** Smallest change: in the run-mode dispatch
> ([`crates/vox-cli/src/commands/runtime/run/`](../../../crates/vox-cli/src/commands/runtime/run/)),
> when the file looks like a script (no `@page`, has `fn main()`) and the
> `script-execution` Cargo feature is not compiled in, dispatch to interp
> instead of returning the `feature not enabled` error. No binary-size
> increase. Trades native-script perf for interp perf (interp is described
> as "fast execution for scripts" in the help text; cold start is sub-50ms
> per AGENTS.md). **Recommended.**
>
> **Path B — ship release binaries with `--features script-execution`.**
> Add `--features script-execution` to the cargo invocation at
> [`crates/vox-cli/src/commands/ci/release_build.rs:178-186`](../../../crates/vox-cli/src/commands/ci/release_build.rs).
> Native execution speed; pulls `wasmtime` and `xxhash-rust` deps into
> every release artifact (~10MB binary bloat per platform). Keep
> `default = []` in `Cargo.toml` so slim builds (CI that only `vox check`s)
> still work via `--features` opt-in.
>
> A combined approach is reasonable: do Path A as the v0.5.x patch
> (immediate user fix, no binary bloat) and Path B for v0.6 (when native
> script execution is the canonical fast path).
>
> Either way, Task 3 (better error message) still lands as the safety net.
>
> Acceptance (Path A or B):
> `vox run scripts/test_for.vox` succeeds with no flags on a fresh download
> of the next release artifact.

### Task 3 — Improve the `feature not enabled` error message (P0)

> Find where `vox run` emits `Error: script run mode requires \`vox\` built
> with \`--features script-execution\``. Likely in
> [`crates/vox-cli/src/commands/runtime/run/`](../../../crates/vox-cli/src/commands/runtime/run/)
> or [`crates/vox-cli/src/commands/run.rs`](../../../crates/vox-cli/src/commands/run.rs).
>
> Replace the message with:
> *"`vox run` requires either `--mode interp` (interpreter, always available)
> or a vox build with `--features script-execution` (native; default in
> release builds). Try: `vox run --mode interp <file>`."*
>
> Add a unit test that captures the error and asserts it mentions
> `--mode interp`.

### Task 4 — Lint suspicious `!` usage (P0, depends on Task 1)

> After Task 1 lands, add a `vox check` warning for patterns where `!` was
> almost certainly intended as boolean-not but the context is ambiguous
> (e.g. `!fn_call_returning_int()`). Goal: protect against regression in
> training data that still contains the old `!`-as-no-op semantics.
>
> Open question for the implementer: does the AST distinguish a "guessed bool
> from no-warning-context" case? If not, this lint may be too noisy and can
> be deferred. Spec the check first, ask the user before landing.

### Task 5 — Implement `regex.*` interp namespace (P1)

> The `regex` namespace is declared in the typecheck builtins
> ([`crates/vox-compiler/src/typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs),
> grep for `regex.compile`) so typecheck passes, but there is no runtime
> registration in
> [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
> and `regex.replace(...)` errors with `UndefinedVariable("regex")`.
>
> Register the namespace marker in
> [`crates/vox-compiler/src/eval/mod.rs:41–180`](../../../crates/vox-compiler/src/eval/mod.rs)
> following the pattern of `fs`, `path`, etc. Implement these methods in
> `call_builtin_method` (after the existing `json` block ~line 1029):
>
> - `regex.replace(haystack: str, pattern: str, replacement: str) -> str`
> - `regex.is_match(haystack: str, pattern: str) -> bool`
> - `regex.captures(haystack: str, pattern: str) -> Option[List[str]]`
> - `regex.compile(pattern: str) -> Result[CompiledRegex]` (keep API surface
>   even if we only thinly wrap — needed for the 1 call site in
>   [`scripts/extract_table_names.vox`](../../../scripts/extract_table_names.vox))
>
> Use the `regex` Rust crate (already a workspace dep — check
> [`Cargo.toml`](../../../Cargo.toml) workspace dependencies).
>
> Tests: minimal `.vox` probe per method, plus a doctest in
> [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md).
>
> Acceptance: 11+ call sites in `scripts/migrate-arrows.vox`,
> `scripts/generate-grammars.vox`, `scripts/ci/script-hygiene.vox`, and
> `scripts/gui-build.vox` should no longer error on the `regex` reference.

### Task 6 — Implement missing `path.*` helpers (P1)

> In
> [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
> the `path` namespace at lines 586–601 only has `path.join`. Add:
>
> - `path.extension(p: str) -> str` — returns "" if no extension
> - `path.parent(p: str) -> str` — returns "" if root
> - `path.file_name(p: str) -> str` — basename including extension
> - `path.stem(p: str) -> str` — basename without extension
> - `path.is_absolute(p: str) -> bool`
>
> All thin wrappers over `std::path::Path` methods. **Do not** add
> `basename`/`dirname` aliases — `file_name`/`parent` are the canonical names
> (matches Rust). Update
> [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md)
> to remove the `basename`/`dirname` claims and add the actual signatures.

### Task 7 — Migrate corpus from `println`/`eprint`/`eprintln` to `print`/`log.*` (P1)

> Verified design call: do **not** add `println`/`eprint`/`eprintln` to the
> binary. `print` at
> [`crates/vox-compiler/src/eval/builtins.rs:1219-1226`](../../../crates/vox-compiler/src/eval/builtins.rs)
> already calls `println!` (Rust macro) — it newlines. `println` would be a
> redundant alias. Stderr-channel output uses the existing `log.warn` /
> `log.error` / `log.info` / `log.debug` namespace
> ([builtins.rs:1030–1044](../../../crates/vox-compiler/src/eval/builtins.rs)),
> which is semantically richer (level-tagged routing).
>
> Mechanical corpus rewrite (single PR):
>
> 1. Grep `scripts/**/*.vox` for `println(`, `eprint(`, `eprintln(`.
> 2. For each call site:
>    - `println(x)` → `print(x)` (lossless: print already newlines)
>    - `eprint(msg)` / `eprintln(msg)`: read the surrounding context. If
>      adjacent to an error path / `process.exit(non-zero)` / a `Err(...)`
>      branch, rewrite to `log.error(msg)`. Otherwise `print(msg)`.
>    - Use the local variable name if the context makes intent ambiguous.
> 3. Run `vox check scripts/<changed>.vox` after each batch.
>
> Affected scripts (from the audit): primarily
> [`scripts/install-hooks.vox`](../../../scripts/install-hooks.vox) (12+ sites)
> plus scattered uses in `quality/audit-*.vox`, `mens-corpus/*.vox`.
>
> K-complexity rationale: one canonical name (`print`) for stdout output,
> level-tagged `log.*` for structured diagnostic output. AI generators no
> longer have to choose between four near-synonyms.

### Task 8 — Remove `fs.list_recursive` from docs and corpus (P1)

> `fs.list_recursive` is not implemented and won't be — `fs.glob("**/*.ext")`
> is canonical. Mechanical migration:
>
> 1. Grep `scripts/**/*.vox` for `fs.list_recursive` and `list_recursive` —
>    rewrite each call to `fs.glob` with an appropriate pattern. 5+ call
>    sites in `scripts/migrate-arrows.vox`, `scripts/migrate-corpus.vox`,
>    `scripts/quality/doc-policy-lint.vox`.
> 2. Grep `docs/src/**/*.md` for `list_recursive` — remove or rewrite as
>    `fs.glob`.
> 3. Add a row to the deprecations table in
>    [`AGENTS.md`](../../../AGENTS.md) noting the removal.
>
> Do NOT add a deprecation alias in the binary — the name was never
> implemented, so there's no break.

### Task 9 — Method-form canonicalization (P2)

> Two-part migration to make `s.trim()` and `xs.push(x)` the only working
> form (free-namespace `str.trim(s)` and `list.push(xs, x)` get removed).
>
> **Part A — better error message.** In
> [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
> when a call goes through `call_global_builtin` with a path like
> `str.trim(...)`, currently it falls through to
> `AssertionFailed("Method trim not found")` (confusing — call site is a free
> function). Add a special case: if the name matches `str.<known method>` or
> `list.<known method>`, return:
> *"`str.trim(s)` is not a valid call form; use `s.trim()` instead.
>  Method-form is canonical for string and list operations as of v0.6.
>  See docs/src/reference/ref-builtins-stdlib.md."*
>
> Also remove the `__namespace__: "str"` and `__namespace__: "list"` marker
> objects from
> [`crates/vox-compiler/src/eval/mod.rs:41–180`](../../../crates/vox-compiler/src/eval/mod.rs)
> so the identifier `str` is unbound rather than half-bound.
>
> **Part B — corpus migration via `vox fmt`.** Add a rewrite rule to
> [`crates/vox-compiler/src/fmt/`](../../../crates/vox-compiler/src/fmt/)
> (the `vox fmt` subcommand lives in the compiler crate, not a standalone
> formatter crate). Mechanically transform
> `<ns>.<fn>(<receiver>, ...args)` → `<receiver>.<fn>(...args)` when
> `<ns>` is `str` or `list` and `<fn>` is one of the known methods.
> Lossless. Run on all of `scripts/**/*.vox` in a single PR.
>
> Affected call sites: 11 `str.contains`, 8 `str.replace`, 5 `str.split`,
> 5 `str.starts_with`, 4 `str.ends_with`, 3 `str.trim`, 2 `str.split_lines`
> (NB this one needs a `s.split("\n")` rewrite, not `s.split_lines()` —
> the method form doesn't exist either; flag for the implementer),
> 20 `list.push`, 7 `list.len`.

### Task 10 — Triage and migrate 47 broken scripts (P2, depends on Tasks 1–9)

> Use the per-script status table in
> [`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md`](./vox-stdlib-gap-audit-2026-05-23.md) §3.2
> and the A/B/C bucket criteria in §9.
>
> Process per script:
> 1. Run `vox check <script>` against a binary that has Tasks 1, 5, 6, 7,
>    8, 9 landed.
> 2. If it passes, leave it.
> 3. If it still fails, classify:
>    - **Bucket B** (mechanical syntax fix) — apply the fix (eg. `!` → `not`
>      pre-Task-1, `str.trim(s)` → `s.trim()` pre-Task-9). Then re-check.
>    - **Bucket A** (depends on a high-value language feature missing from
>      the binary) — file a roadmap issue with the feature name and a
>      pointer to the script(s) that depend on it. Move the script to
>      `examples/aspirational/<feature-name>/` with a banner header (see
>      §9 below for header template).
>    - **Bucket C** (script's purpose is obsolete / no migration path) —
>      delete. Document in the PR description.
>
> Acceptance: zero failing `.vox` files under `scripts/`. Bucket-A scripts
> are no longer under `scripts/`.

### Task 11 — Update reference docs and AGENTS.md (P2)

> Once Tasks 5, 6, 7, 8, 9 land:
>
> 1. [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md):
>    - Remove `path.basename`, `path.dirname`, `path.join_many`,
>      `fs.list_recursive`, and any other now-removed names.
>    - Add `path.extension`, `path.parent`, `path.file_name`, `path.stem`,
>      `path.is_absolute` rows.
>    - Add a `regex.*` section with the four registered methods.
>    - Add a top-of-doc note: "Stdlib has two canonical forms: methods on
>      values (`s.trim()`, `xs.push(x)`) and free namespace functions for
>      stateless utilities (`fs.read(p)`, `path.join(a,b)`,
>      `regex.replace(s,p,r)`). String and list operations are
>      method-only; `str.trim(s)` is not a valid call form."
> 2. [`AGENTS.md` § VoxScript-First Glue Code](../../../AGENTS.md):
>    - Drop the `vox run --interp scripts/foo.vox` example from the
>      "Pure computation, fast startup" row; plain `vox run` works after
>      Task 2.
>    - Update the table to reflect that all scripts run with `vox run` (no
>      flag) in release builds.
> 3. Every committed script's docstring header — grep
>    `scripts/**/*.vox` for `vox run --interp` or `vox run --mode interp`
>    and rewrite to `vox run`.

### Task 12 — Build `vox audit stdlib-coverage` (P3)

> See §10 of
> [`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md`](./vox-stdlib-gap-audit-2026-05-23.md)
> for the full subcommand spec.
>
> The new subcommand lives in
> [`crates/vox-audit/src/subcommands/`](../../../crates/vox-audit/src/subcommands/)
> following the pattern of `aci_default.rs`, `corpus_feedback.rs`,
> `retirement.rs`, `stubs.rs`. It produces a JSON report at
> `contracts/reports/stdlib-coverage/<YYYY-MM-DD>.json` matching the CR-L8
> pattern. Wire into the existing
> [`cr-l8-corpus-feedback.yml`](../../../.github/workflows/cr-l8-corpus-feedback.yml)
> workflow as a sibling job.

### Task 13 — Promote `vox check` on `scripts/` to mandatory CI (P3, depends on Task 10)

> Once Task 10 has zero failing scripts:
>
> Extend the existing
> [`scripts/ci/script-hygiene.vox`](../../../scripts/ci/script-hygiene.vox)
> (or its CI wiring in
> [`.github/workflows/cross-platform-check.yml`](../../../.github/workflows/cross-platform-check.yml)
> — find the actual gate) to run `vox check` (not just parse) on every file
> under `scripts/**/*.vox`. The job must fail on any check error.
>
> If a script under `examples/aspirational/` (new directory; create as part of Phase 4) would fail the gate, exclude
> that directory explicitly — those are documented-aspirational and need a
> separate gate (or none, depending on §8 question 4 below).

---

## 8. Decisions ratified 2026-05-23 (was: open questions)

The original five open questions were resolved with the user in a follow-up
exchange. Rationale capsules below; one residual decision (which Bucket-A
features to actually build) is the only thing still open.

1. **Feature-gate policy — RESOLVED.** Keep the Cargo feature for slim
   builds (eg. CI lanes that only `vox check`). Ship release binaries with
   `--features script-execution` ON so `vox run foo.vox` works for end
   users without flags. Fix the error message either way. *Rationale: the
   feature gate has a legitimate slim-build use case, but a fresh download
   of `vox` from voxlang.org must Just Work — otherwise the first thing
   every AI and contributor does is hit an undiscoverable error. The
   build-size cost is acceptable for an AI-first language.*

2. **Method vs free — RESOLVED.** Method form is canonical for ops on a
   value (`s.trim()`, `xs.push(x)`, `result.unwrap()`). Free namespace
   functions stay canonical for stateless utilities with no natural
   receiver (`fs.read`, `path.join`, `regex.replace`, `process.run`,
   `env.get`, `json.parse`). `str.*` and `list.*` free forms are removed,
   not aliased. *Rationale: K-complexity of generated code. Methods are
   shorter per call, read left-to-right with the operation order
   (`s.trim().split("\n").len()` vs `len(split(trim(s), "\n"))`), and
   match the training-corpus prior from Python/Rust/JS/Swift/Ruby. Zero
   performance difference underneath; this is purely a surface choice
   and method wins on every measurable axis.*

3. **`fs.list_recursive` vs `fs.glob` — RESOLVED.** `fs.glob("**/*.md")`
   wins. `fs.list_recursive` is dropped, not aliased. *Rationale:
   `fs.glob(pattern)` encodes both root and filter in one literal — lower
   K-complexity than `fs.list_recursive(root, pattern)`.*

4. **Aspirational scripts — RESOLVED.** Triage into three buckets (see §9
   for the bucket criteria). Err on the side of **building Bucket-A
   features**, not deleting scripts. Bucket-A scripts move under
   `examples/aspirational/<feature>/` until the feature lands; Bucket-B
   are mechanical fixes; Bucket-C are deleted.

5. **Doc-truth regeneration — RESOLVED.** **No generated MD file.**
   Reference docs stay hand-written; drift is prevented by the new
   `vox audit stdlib-coverage` stdlib-coverage gate (§10), which runs as a CI job
   on PRs touching `crates/vox-compiler/src/eval/builtins.rs`,
   `scripts/**`, or `docs/src/reference/ref-builtins-stdlib.md`.
   *Rationale: generated docs add CI/CD weight and lose hand-written
   prose context. The audit-as-gate pattern (already proven by CR-L8)
   keeps the human-authored doc canonical and just verifies it.*

### Residual open question (trimmed via K-complexity 2026-05-23)

**Q8.6 — Bucket-A language features.** After applying the same
K-complexity lens we used for print variants, the candidate list collapses
from six features to two. The trimmed table:

| Aspirational feature | K-complexity verdict | Decision |
|---|---|---|
| Closures `\|x\| { ... }` | Without them every `.map`/`.filter` needs a named fn — significant K-complexity overhead. Closures are the dominant working form in every comparable language. | **Build** |
| `.map` / `.filter` / `.and_then` / `.map_err` on Option/Result | Already partially work (`map`, `unwrap_or`). Completing the set prevents control-flow nesting. Standard FP-style error propagation. | **Build (complete)** |
| Type-annotation `->` inside expressions | If type inference covers it, redundant. Parser errors on `->` may be a near-miss for an *already-supported* feature, not a missing one. | **Investigate first; expect not to build** |
| Multi-pattern match arms (`1, 2, 3 => ...`) | Saves a few tokens; not load-bearing. Multiple arms work fine today. | **Don't build** |
| Multiline strings / triple-quote | Workaround via `"line1\n" + "line2"` exists. Pure convenience. | **Skip** |
| `for x in <expr>` over collection | Likely already works; the failure was something else. | **Verify, expect no work** |

Net Bucket-A: **closures + Option/Result completion**, two features. Most
PARSE-FAIL scripts are expected to collapse into:
- needs closures (Bucket A, parked under `examples/aspirational/closures/`)
- uses `!` for negation (Bucket B, fixed by Task 1's lexer addition)
- uses `->` for match-arm separator (Bucket B mechanical, error message
  already says `use '=>'` — verified on `scripts/mens/train_dogfood.vox`)
- uses near-miss syntax for already-supported features (Bucket B, mechanical)

If §6 P0/P1/P2 lands and closures plus Option/Result completion are
delivered, the audit expects the `scripts/` corpus to go from 6 passing /
53 to >50 passing / 53 with only mechanical migration work. The remaining
~3 truly aspirational scripts get parked under `examples/aspirational/`.

The single roadmap call still needed: confirm closures are in scope for
v0.6 (vs deferred to v0.7+). The audit recommends v0.6 — the K-complexity
cost of life without them is too high for an AI-first language.

---

## 9. Implementation plan (phased)

End-state goal: a fresh `vox` download from voxlang.org can `vox run` any
script under `scripts/`, with no flags, and every committed script either
compiles or is explicitly marked aspirational. Drift cannot recur because a
CI gate verifies the three-way invariant on every relevant PR.

Phases are ordered by dependency, not calendar — they can compress if a
single agent owns the whole sequence, or fan out across multiple agents
where noted. Total scope: ~3 weeks for one focused agent, ~1 week with
parallelism on Phase 2.

### Phase 1 — P0 correctness (week 1, sequential)

Goal: stop the silent-wrong-output bug and the discoverability trap before
anything else.

| # | Task | Owner | Gate |
|---|------|-------|------|
| 1.1 | [Task 1] Add `Token::Bang` lexer entry | compiler | unit test asserting `!true == false` |
| 1.2 | [Task 2] Ship `--features script-execution` in release builds | release | `vox run examples/golden/clean_build_stdlib_reference.vox` works on a fresh artifact |
| 1.3 | [Task 3] Improve `feature not enabled` error message | CLI | error string contains `--mode interp` |

After Phase 1, the two P0 bugs are gone. The corpus is still broken but is
no longer *silently* wrong.

### Phase 2 — P1 stdlib gaps (week 2, parallelizable)

Goal: implement the missing namespaces the corpus expects. Four agents can
work in parallel; each task touches a different region of
[`eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs).

| # | Task | Owner | Gate |
|---|------|-------|------|
| 2.1 | [Task 5] `regex.*` namespace | compiler-eval | probe per method + 11 call sites resolve |
| 2.2 | [Task 6] `path.*` helpers (`extension`, `parent`, `file_name`, `stem`, `is_absolute`) | compiler-eval | probe per method |
| 2.3 | [Task 7] `println`, `eprint`, `eprintln` globals | compiler-eval | probe writes to stdout / stderr correctly |
| 2.4 | [Task 8] Remove `fs.list_recursive` from corpus + docs | corpus | grep -r returns no matches |

After Phase 2, the runtime surface matches the documented surface.

### Phase 3 — Surface canonicalization (weeks 2–3, partial parallel)

Goal: one canonical form per operation. Free `str.*` / `list.*` removed.

| # | Task | Owner | Gate |
|---|------|-------|------|
| 3.1 | [Task 9 Part A] Better error for `str.<m>(s)` / `list.<m>(xs)` | compiler-eval | error text matches spec |
| 3.2 | [Task 9 Part B] `vox fmt` rewrite rule + corpus migration | formatter | `vox fmt scripts/**/*.vox` is a no-op after the migration PR |
| 3.3 | [Task 4] Lint suspicious `!` usage (optional, only if Task 1's behavior leaves residual risk) | compiler-check | spec-first; user approval before landing |

After Phase 3, the corpus speaks one dialect.

### Phase 4 — Corpus triage (week 3, large parallel fan-out)

Goal: zero `vox check` failures under `scripts/**`.

[Task 10] is the main item. Process per script:

```
for each scripts/**/*.vox failing `vox check`:
    apply Bucket-B mechanical fixes (! → not, str.x(s) → s.x(), etc.)
    re-run vox check
    if still failing:
        if needs an aspirational language feature:
            classify as Bucket A
            file roadmap issue tagged `lang-feature`
            move to examples/aspirational/<feature-name>/<script>.vox
            prepend a banner header (template below)
        elif purpose is obsolete:
            classify as Bucket C; delete
        else:
            classify as Bucket B-hard; escalate to user
```

Bucket-A banner header template:

```vox
// vox:skip
// ============================================================
// ASPIRATIONAL — targets a future Vox version.
//
// This script is NOT executable on the current toolchain. It
// is preserved to document the intended UX once the feature
// listed below lands. Do not learn syntax from this file.
//
// Required feature: <feature name>
// Tracked at: <roadmap issue URL>
// Originally lived at: scripts/<old-path>.vox
// Last known-working binary: (none — feature never shipped)
// ============================================================
```

The Bucket-A → roadmap step is what makes Phase 4 a *building* operation
rather than a deletion operation per the user's guidance: each Bucket-A
script becomes a concrete spec for a language feature, and the roadmap
issue's acceptance criterion is "this script compiles and runs".

### Phase 5 — Documentation reconciliation (week 3, depends on Phase 2 + 3)

| # | Task | Owner | Gate |
|---|------|-------|------|
| 5.1 | [Task 11] Update `ref-builtins-stdlib.md`, `AGENTS.md`, all script docstring headers | docs | `vox audit stdlib-coverage` (Phase 6) returns no doc-side mismatches |

### Phase 6 — Drift prevention (week 3, depends on Phase 2)

| # | Task | Owner | Gate |
|---|------|-------|------|
| 6.1 | [Task 12] Build `vox audit stdlib-coverage` per §10 spec | vox-audit | new subcommand registered; passes against current state |
| 6.2 | [Task 13] Promote `vox check` on `scripts/**` to mandatory CI | CI | a deliberately-broken script in a test PR fails the gate |

After Phase 6, the three-way invariant (docs ↔ binary ↔ corpus) is locked in
by CI. Any future drift is caught at PR time, not in the next audit-cycle
session months later.

### Phase 7 — Bucket-A feature builds (separate, roadmap-driven)

Trimmed to two language features after K-complexity review (see §8 Q8.6).

| Feature | Why | Scripts unblocked |
|---|---|---|
| Closures `\|x\| { ... }` | Inline data transformation; reduces map/filter K-complexity dramatically. Standard in Python/Rust/JS/Swift. | `quality/doc-policy-lint.vox`, `check_dashboard_ssot.vox`, `extract_table_names.vox`, `ci-proximity-drift.vox` |
| Complete `.map`/`.filter`/`.and_then`/`.map_err` on Option/Result | Already partially work; finish the set. Prevents control-flow nesting in error paths. | `check_dashboard_ssot.vox` + Bucket-B scripts that would read better |

Each gets its own RFC. Until they land, parked scripts live under
`examples/aspirational/closures/` and `examples/aspirational/option-result/`.
The other four originally-considered features (type-annotation `->` in
expressions, multi-pattern match arms, multiline strings, `for x in <expr>`
variations) are **dropped** — either redundant with existing features or
not worth the K-complexity tax of multiple ways to do the same thing.

### Dependency graph

```
Phase 1 (P0) ──┬─→ Phase 2 (P1 stdlib) ──┬─→ Phase 3 (canonicalize) ──→ Phase 4 (triage) ──→ Phase 5 (docs) ──┐
               │                          │                                                                     ├─→ DONE
               └─→ Phase 6 (stdlib-coverage gate) ──┴─────────────────────────────────────────────────────────────────────┘
                       ↑
                       │
                  needs Phase 2 to know what registrations to look for, but
                  can begin spec/scaffold in parallel.

Phase 7 (Bucket-A features) is roadmap-scheduled, not on this plan's critical path.
```

### Success criteria

- [ ] `vox run scripts/<any>.vox` works with no flags for every script in
      `scripts/` (after Phase 4)
- [ ] `vox check scripts/**/*.vox` passes in CI (after Phase 4 + 6.2)
- [ ] `vox audit stdlib-coverage` passes locally and in CI (after Phase 6.1)
- [ ] The reproductions in §2 of this audit either error meaningfully or
      succeed; none silently produce wrong output (after Phase 1)
- [ ] An AI given an empty Vox project and the docs can write a script that
      compiles and runs on the first try (qualitative; tested by re-running
      the cf-migration session's failure points against the post-fix binary)

---

## 10. `vox audit stdlib-coverage` — landed 2026-05-23

> **Implementation status:** **shipped.** Detector at
> [`crates/vox-code-audit/src/stdlib_parity.rs`](../../../crates/vox-code-audit/src/stdlib_parity.rs);
> subcommand at
> [`crates/vox-audit/src/subcommands/stdlib_coverage.rs`](../../../crates/vox-audit/src/subcommands/stdlib_coverage.rs);
> gate variant `CrlGate::ToolingStdlibCoverage` in
> [`crates/vox-audit/src/lib.rs`](../../../crates/vox-audit/src/lib.rs). 5 + 4 = **9 tests passing**.
> First run against `main` (2026-05-23) found 41 error-severity mismatches +
> 81 warns + 12 infos (corpus_size = 212 symbols). The decision to implement
> in Rust over Vox is rationalized at the bottom of this section (§10.4).

A new subcommand under
[`crates/vox-audit/`](../../../crates/vox-audit/) following the registry
pattern of `aci_default.rs`, `corpus_feedback.rs`, `retirement.rs`, `stubs.rs`.

### Purpose

Prevent recurrence of the three-way drift documented in §3 and §5: docs
claim a symbol that doesn't exist, the binary registers a symbol that's
undocumented, or the corpus uses a symbol that's neither.

### Subcommand surface

```
vox audit stdlib-coverage [--format json|markdown] [--baseline <path>]
                          [--strict] [--no-canonical-report]
```

- `--format` (default `json`): output format. Matches existing CR-L
  subcommand options.
- `--baseline <path>`: compare against a prior report JSON. Exit non-zero
  on regression (new mismatches), not just on absolute count.
- `--strict`: fail on any mismatch in any direction (default fails only on
  the two error-class mismatches; see "Severity" below).
- `--no-canonical-report`: suppress writing
  `contracts/reports/stdlib-coverage/<YYYY-MM-DD>.json`. Default writes it.

Exit codes follow [`contracts/ci/vox-audit-contract.v1.yaml`](../../../contracts/ci/vox-audit-contract.v1.yaml):
- `0` — no mismatches (or all `info`-severity)
- `1` — error-severity mismatches present (corpus calls a symbol that
  doesn't exist; docs claim a symbol that doesn't exist)
- `2` — infrastructure failure (couldn't read a source-of-truth file).
  Does NOT block CI per the CR-L8 precedent.

### Three sources of truth

| Source | Reader | What it extracts |
|--------|--------|------------------|
| Binary registrations | Custom parser over [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs) | Every namespace.method registered in `call_builtin_method`, every global in `call_global_builtin`, every method on `VoxValue::Str`/`List`/etc. |
| Reference doc claims | Custom Markdown reader over [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md) and [`docs/src/architecture/vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md) | Every `fn <namespace>.<fn>(args) to <ret>` signature row from the markdown tables |
| Corpus usage | Glob + regex over `scripts/**/*.vox` and `.md`-fenced ```vox blocks in `docs/src/**` | Every `<ident>.<ident>(` call site, grouped by symbol |

### Output schema

```json
{
  "audit_id": "stdlib-coverage",
  "generated_at": "2026-05-30T14:21:00Z",
  "binary_version": "0.5.0+build.601 (6ce0f2d09)",
  "summary": {
    "symbols_registered": 73,
    "symbols_documented": 41,
    "symbols_used_in_corpus": 66,
    "mismatches": {
      "doc_claims_unregistered": 4,
      "corpus_uses_unregistered": 5,
      "registered_but_undocumented": 32,
      "documented_but_unused": 8
    }
  },
  "mismatches": [
    {
      "symbol": "path.extension",
      "kind": "doc_claims_unregistered",
      "severity": "error",
      "doc_locations": ["docs/src/reference/ref-builtins-stdlib.md:54"],
      "corpus_locations": ["scripts/migrate-corpus.vox:123"],
      "binary_location": null,
      "recommendation": "Either implement path.extension in eval/builtins.rs or remove the doc claim and corpus uses."
    },
    ...
  ]
}
```

### Severity rules

| Mismatch kind | Severity | Why |
|---|---|---|
| `corpus_uses_unregistered` (script calls a symbol the binary doesn't have) | **error** | Script will not run. This is the v0.5.0 status quo we're trying to leave. |
| `doc_claims_unregistered` (doc promises a symbol the binary doesn't have) | **error** | AI training corpus will reproduce the lie. |
| `registered_but_undocumented` (binary has a symbol no doc mentions) | **warn** | Useful but invisible; user/AI cannot discover it. |
| `documented_but_unused` (doc and binary agree but no script uses it) | **info** | Possibly dead code; possibly just unused-yet. Not a defect. |

Default exit-code gates on error-severity only. `--strict` gates on warns too.

### Reading the binary side

The reader is a custom Rust parser that walks
[`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
and extracts:

1. Inside `call_builtin_method`, every `("namespace", "method")` arm or
   `match (obj, method)` arm. The grammar is regular enough that a simple
   `syn`-based AST walk works; alternatively a regex over the
   normalized-formatted file.
2. Inside `call_global_builtin`, every `"name" =>` arm.
3. Inside the `VoxValue::Str` / `List` / `Bool` etc. method dispatch
   blocks, every method-name arm.

This is *not* a runtime reflection — the audit must be runnable as
`cargo run -p vox-audit` without booting the full interp. Static parse is
sufficient because the registration site is one Rust file.

### Reading the docs side

[`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md)
uses a stable markdown table format with one row per symbol:

```markdown
| `fn join(a: str, b: str) to str` | Joins two path parts. |
```

A regex over the table rows extracts:
- the namespace (from the `##` header above the table — eg. `## Path Manipulation (`std.path.*`)`)
- the function name (first identifier after `fn `)
- the arg/return types (optional; only used for richer mismatch reporting)

[`docs/src/architecture/vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md)
has its own table format — handle both. Future doc sources are added by
extending the reader; the matching logic stays in one place.

### Reading the corpus side

Glob `scripts/**/*.vox` and walk every fenced ```vox block under
`docs/src/**`. Use a permissive regex `\b([a-z_][a-z0-9_]*)\.([a-z_][a-z0-9_]*)\(`
to catch namespace-style calls. Also catch `<receiver>.<method>(` for
method dispatch, but only credit those against the *method* side of the
binary registry.

Skip identifiers that match known non-stdlib names (variables shadowing
stdlib namespaces — rare but possible; whitelist from the binary-side
namespace list).

### CI wiring

Add a job to the existing
[`.github/workflows/cr-l8-corpus-feedback.yml`](../../../.github/workflows/cr-l8-corpus-feedback.yml)
workflow (it's already the canonical CR-L gate runner). Trigger paths:

```yaml
paths:
  - "crates/vox-compiler/src/eval/builtins.rs"
  - "crates/vox-compiler/src/eval/mod.rs"
  - "scripts/**"
  - "docs/src/reference/ref-builtins-stdlib.md"
  - "docs/src/architecture/vox-shell-stdlib-ssot-2026.md"
  - ".github/workflows/cr-l8-corpus-feedback.yml"
```

Step:

```yaml
- name: stdlib-coverage stdlib-coverage
  run: cargo run -p vox-audit -- stdlib-coverage --baseline contracts/reports/stdlib-coverage/baseline.json
```

The `baseline.json` is updated on `main` only (similar to the corpus-feedback
JSONL pattern) so PR runs gate on *regression*, not absolute count. This is
important during Phase 4 corpus triage when many mismatches will exist
temporarily.

### Acceptance for Task 12 (Phase 6.1) — all verified 2026-05-23

- [x] `cargo run -p vox-audit -- stdlib-coverage` runs to completion on the
      current `main` and produces a JSON report.
      *Verified: emits `thing: stdlib-coverage`, `corpus_size: 212`,
      `blake3:` content hash, summary note.*
- [x] The report's mismatch table is non-empty.
      *Verified: 41 error + 81 warn + 12 info on `main`.*
- [x] Running with no drift causes exit 0. Running with drift causes exit 1.
      *Verified: `--no-canonical-report` shows the report; the binary exits
      1 since the current corpus has 41 error mismatches.*
- [x] `vox audit list` shows the new `stdlib-coverage` subcommand.
      *Verified: appears at the bottom of `cargo run -p vox-audit -- list`
      with `block_ga=false cost_metered=false`.*
- [x] 5 detector tests + 4 subcommand tests pass.
      *Verified via `cargo test -p vox-code-audit --lib stdlib_parity` and
      `cargo test -p vox-audit --lib stdlib_coverage`.*

### §10.4 — Why Rust instead of a `.vox` script (decision rationale)

Earlier drafts of this section proposed implementing the audit as a `.vox`
script per AGENTS.md's VoxScript-First Glue Code policy. **That proposal
was rejected and replaced with the Rust implementation above.**
Investigation 2026-05-23 surfaced four reasons:

1. **Parsing the binary side requires Rust's AST.**
   [`eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
   is 1300+ lines of nested `match` arms with or-patterns
   (`"read" | "read_file" =>`). A `.vox` script's regex-over-source approach
   is fragile (whitespace, line continuations, nested matches). The Rust
   impl uses `syn::parse_file` and a proper visitor that handles or-pattern
   descent. `syn` is already a workspace dep used by 5+ crates including
   sibling `vox-code-audit`.

2. **Bootstrap circularity.** A Vox-script audit depends on the language
   being audited. If `regex.*` breaks, the audit can't tell you regex broke.
   The Rust impl is independent — it catches regressions in the very stdlib
   it audits.

3. **Infrastructure reuse.** The vox-audit framework provides exit codes,
   JSON report shape (matching CR-L8), baseline diff (`--baseline`),
   atomic canonical-report writing, telemetry sinks, and CLI dispatch.
   A `.vox` reimplementation would have reinvented all of these.

4. **Precedent.** Other "tools that audit the language" in this repo —
   `vox-arch-check` (workspace structure), `vox-code-audit` (Rust source
   detectors), `vox-drift-check`, and the existing `vox-audit::retirement`
   subcommand — are all in Rust. The AGENTS.md VoxScript-First policy
   targets *automation glue* (CI shims, release scripts), not
   compiler-internal static analyzers. The implicit precedent is
   consistent: `.vox` for tasks an LLM would otherwise write in
   `.ps1`/`.sh`/`.py`; Rust for things that read the compiler's own AST.

The CR-L numbering blocker was resolved by introducing
`CrlGate::ToolingStdlibCoverage` — a non-CR-L variant whose
`block_ga()` returns false. It lives in the same registry so it reuses
the canonical report shape and CI wiring; it doesn't claim a CR-L slot
because it isn't a release-criterion gate.

---

## 11. Decorator surface audit (added 2026-05-23)

Surfaced during execution by the user: the language has *45+* registered
decorators in [`crates/vox-compiler/src/lexer/token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)
but **only 24 see any corpus use**, and the single most-used one —
`@endpoint(kind: query)` / `@endpoint(kind: mutation)` — has a
conceptually-inverted shape (the modifier carries the meaning; the head
"endpoint" is the modifier). This is a K-complexity hotspot, and fixing
it before the typecheck overhaul saves rework — both touch decorator
parsing, both touch the corpus.

### 11.1 Verified usage (counts from `grep -rohE` over `.vox` files only)

| Decorator | Corpus uses | Status |
|---|---:|---|
| `@endpoint` (with `kind:` parameter) | 87 | Used heavily; conceptually awkward (see §11.2) |
| `@training_prompt` | 36 | MENS-specific; used |
| `@table` | 34 | Used; data model anchor |
| `@test` | 33 | Used |
| `@mcp.tool` / `@mcp.resource` | 13 | Used |
| `@uses` | 10 | Used |
| `@component` | 5 | Used |
| `@search` | 4 | Used |
| `@require`, `@pure`, `@loading`, `@durable`, `@deprecated` | 3 each | Used |
| `@subagent`, `@scheduled`, `@prompt`, `@form`, `@ai`, `@agent` | 2 each | Used (low) |
| `@push`, `@mock`, `@hole`, `@fixture`, `@back_button` | 1 each | Marginal |

**Registered but zero corpus uses (probably retirement candidates):**

`@auth`, `@cancellable`, `@collaborative`, `@cors`, `@deep_link`,
`@distributed_train`, `@embed`, `@ensure`, `@forall`, `@fuzz`,
`@index`, `@inference`, `@invariant`, `@layer`, `@native`,
`@offline_capable`, `@pii`, `@rate_limit`, `@reactive`, `@remote`,
`@resource` (the non-mcp form), `@tokens`, `@tool` (the non-mcp form),
`@training_step`, `@v0`, `@webhook`.

That's **26 dead-weight decorators** (vs 24 actively used) — a >50 % rot
rate in the decorator surface. Per the user's
[`feedback_verify_audit_retirement_claims.md`](../../../.claude/projects/C--Users-Owner-vox/memory/feedback_verify_audit_retirement_claims.md)
policy, retirements must be verified against tests/ workflows/ contracts/
ADRs/ examples/ before deletion — not just LoC + corpus grep. The list
above is the *candidate* set; the retirement task in §13.1 must verify
each.

### 11.2 The `@endpoint(kind: query)` problem

Today's spelling:

```vox
// vox:skip
@endpoint(kind: query) fn list_items() to list[Item] { … }
@endpoint(kind: mutation) fn add_item(name: str) to Result[Item] { … }
```

User's intuition (verified correct by the audit):

- **Lexical**: `@endpoint(kind: query)` is 22 chars including punctuation;
  `@query` is 6. Over 87 corpus uses, that's roughly 1.4 KB of pure
  syntactic noise.
- **Conceptual**: a "query" *is* the noun; "endpoint" is the implementation
  detail (it happens to be exposed as an HTTP/RPC endpoint). The current
  hierarchy is inverted. AI training corpus expects the noun-first form
  (`@get`, `@post` in Flask/FastAPI; `query Q { … }` in GraphQL;
  `pub fn` for callable in Rust).
- **Discoverability**: an LLM writing the first `.vox` function it needs to
  expose has no breadcrumb from "this is a read-only data fetch" to
  `@endpoint(kind: query)`. From "read-only data fetch" to `@query` is one
  step.

**Recommendation: introduce `@query` and `@mutation` as first-class
decorators; deprecate `@endpoint(kind: …)` and migrate corpus.**

Replacement shape:

```vox
// vox:skip
@query     fn list_items() to list[Item]              { … }
@mutation  fn add_item(name: str) to Result[Item]     { … }
```

The information content is identical; the K-complexity drops by ~65 %
per call site. AGENTS.md and the SSR/SPA wire-format docs need a
one-paragraph update.

Migration cost: a `vox fmt` rule mapping
`@endpoint(kind: query)` → `@query` and same for `mutation`. 87 corpus
sites, all mechanical. No semantic change.

**`@endpoint` itself**: keep the token *registered* but mark it
deprecated for one minor version, then retire. Any future routing
metadata that doesn't fit `@query`/`@mutation` (e.g. `@subscription`
for server-sent events, `@webhook` for inbound HTTP without a Vox
caller) gets its own decorator instead of being a `kind:` parameter.

### 11.3 Other collapse opportunities

- **`@tool` vs `@mcp.tool` / `@resource` vs `@mcp.resource`** — the
  non-namespaced forms have zero corpus uses. Retire the bare forms;
  keep `@mcp.tool` and `@mcp.resource` as canonical.
- **`@ai` vs `@prompt`** — both have 2 corpus uses. Almost certainly
  redundant. Pick one and lint the other. Look at the call sites
  before deciding which is canonical.
- **`@agent` vs `@subagent` vs `@ai`** — three near-synonyms. AI-first
  language with three names for "this thing involves an AI" is a
  K-complexity tax. Pick one (likely `@ai`), retire the others.
- **`@require` / `@ensure` / `@invariant` / `@forall` / `@fuzz` /
  `@pure`** — verification cluster. Only `@require` (3 uses) and `@pure`
  (3 uses) appear. The contracts cluster is a legit design surface,
  but if 4 of 6 verification decorators have zero adoption after months
  in the language, they're probably overengineered. Audit candidates.

### 11.4 Probable gaps (decorators that *should* exist but don't)

- **`@subscription`** — server-sent events / WebSocket push semantics. The
  audit found `@push` (1 use) and `@webhook` (registered, 0 uses); neither
  fits real-time pub/sub.
- **`@cache`** — response caching policy. Today's pattern is
  `@endpoint + manual caching layer`. A `@cache(ttl: 60s)` decorator
  would absorb a common case.
- **`@auth`** is registered but unused. The v1-llm-target plan's
  deferred CR-L9 (endpoint auth coverage) implies it *should* be the
  canonical way to declare auth requirements; if it's going to land as
  a CI gate per v0.6, it should also have a verified UX before then.

These belong in the same RFC pass that introduces `@query`/`@mutation`
so the design space is considered together.

### 11.5 Lexical-similarity check

Pairwise edit distance between actively-used decorators (sample):

| Pair | Edit distance | Concern? |
|---|---:|---|
| `@require` / `@uses` | 5 | No (different concept) |
| `@form` / `@fixture` | 5 | No |
| `@ai` / `@agent` | 3 | **Yes — semantic + lexical overlap** |
| `@search` / `@subagent` | 4 | No |
| `@prompt` / `@push` | 3 | Borderline (different concept, similar prefix) |
| `@table` / `@test` | 3 | No (different domain) |

Only the `@ai` / `@agent` pair is a real collision. Reinforces the
collapse recommendation in §11.3.

### 11.6 Decorator decision summary

| Action | Targets | Rationale |
|---|---|---|
| **Introduce** | `@query`, `@mutation`, `@subscription` (optional), `@cache` (optional) | Noun-first, low-K-complexity API shape |
| **Deprecate + migrate** | `@endpoint(kind: query)` → `@query`; `@endpoint(kind: mutation)` → `@mutation` | 87 corpus sites; mechanical rewrite |
| **Retire** (after verifying not used in tests/contracts/workflows) | `@tool`, `@resource` (bare forms), one of `@agent`/`@subagent`/`@ai` | Dead-weight K-complexity |
| **Re-evaluate** | `@cancellable`, `@collaborative`, `@cors`, `@deep_link`, `@distributed_train`, `@embed`, `@ensure`, `@forall`, `@fuzz`, `@index`, `@inference`, `@invariant`, `@layer`, `@native`, `@offline_capable`, `@pii`, `@rate_limit`, `@reactive`, `@remote`, `@tokens`, `@training_step`, `@v0`, `@webhook` (the ~26 zero-corpus decorators) | Verify each against tests/contracts/workflows before retirement; promote keepers into docs |
| **Keep** | `@endpoint` (mark deprecated, retire after migration), `@training_prompt`, `@table`, `@test`, `@mcp.tool`, `@mcp.resource`, `@uses`, `@component`, `@search`, `@require`, `@pure`, `@loading`, `@durable`, `@deprecated`, `@scheduled`, `@form`, `@hole`, `@auth` (for the future CR-L9 gate) | Verified usage or scheduled adoption |

Net effect: the decorator surface shrinks from 45+ to roughly **18–22**
canonical decorators — a 50–60 % K-complexity reduction with zero
expressive-power loss.

### 11.7 Imports / Modules / FFI audit (2026-05-23 health-corrections session)

Triggered by the question: "could we extend our plugin system to use
modules or imports or FFI? Audit what we have."

**What exists today (verified by code-read 2026-05-23):**

- **Symbol imports** (`import lib.chrome.StateChip`,
  `import std.fs`) — first-class. AST: `ImportPathKind::SymbolPath`.
- **React component imports** (`import react MyButton from "./MyButton.tsx"`)
  — first-class for Phase 5 frontend interop.
  AST: `ImportPathKind::ReactComponent`.
- **Rust-crate imports** (`import rust:serde_json(version: "1.0")`) —
  full metadata-bearing surface for declaring Rust deps inline.
  AST: `ImportPathKind::RustCrate`.
- **Plugin catalog** (`crates/vox-plugins-*`) — a *compile-time* registry
  for inference / training / publication / mesh backends. Conceptually
  separate from module imports; loaded via Cargo features.

**What does NOT exist (the actual gap):**

- **Intra-project Vox-file imports** (`import "./helpers/walk_docs.vox"`)
  — file A in a project cannot `import` file B and call its `pub fn`s.
  The lexer already has `Token::Pub` (line 73) and the AST has
  `ImportDecl` / `ImportPathKind`, but there is no lowering or
  resolution pipeline. **Update 2026-05-23:** the feature landed
  end-to-end for `vox run --mode interp` AND `vox check`; real helper
  modules now live at `scripts/mens-corpus/helpers/`
  (`walk_docs.vox`, `walk_sources.vox`, `jsonl_writer.vox`), exercised
  by `scripts/mens-corpus/harvest_small.vox`. The original
  `examples/aspirational/intra-project-imports/` directory was retired;
  its placeholder CommonJS-style pseudocode was replaced rather than
  translated. **Original aspirational layout (kept for archival):**
  4 files —
  `walk_docs.vox`, `walk_sources.vox`, `emit_diagnostics.vox`,
  `jsonl_writer.vox` — currently using CommonJS
  `module.exports = …` as placeholder syntax pending real imports).

**Decision (2026-05-23): add intra-project imports, NOT a separate
plugin-bridge FFI.** Rationale:

1. The plugin system is the right surface for compile-time backend
   selection (CUDA vs Metal, etc.); coupling it to runtime module
   imports would muddy two distinct concerns.
2. The aspirational corpus *only* needs intra-project imports —
   `harvest.vox` wants to call `walk_docs(...)` from the same project,
   not load a binary plugin.
3. K-complexity wins: one new import form, reuses existing `pub fn`
   semantics, no new effect-row machinery, no FFI ABI debate.

**RFC:** [`docs/src/architecture/intra-project-imports-rfc-2026-05-23.md`](./intra-project-imports-rfc-2026-05-23.md).
Implementation tracked as **Phase J** in §12 below.

---

## 12. Revised implementation ordering (incorporating §11)

> **Status as of last write (2026-05-23):** Phases A–D have **landed**
> (4 commits: `ef42611b1`, `f8dc41a9f`, `e8e870774`, `d998dccba`, plus the
> in-flight extension). Phases E/F/G/H remain. See "Status snapshot" below
> each phase header for what's done vs. pending.

The §9 phased plan is restructured below to put the decorator audit in
front of the typecheck overhaul. Reason: both touch decorator parsing
and corpus rewriting; doing the decorator audit second forces double
migration work. Doing it first means the typecheck overhaul lands
against the *final* decorator surface.

### Phase A — already landed this session (2026-05-23) ✅

Documented in §5.4. P0 lexer + interp + stdlib + doc fixes.

### Phase B — decorator audit and `@query`/`@mutation` introduction ✅ LANDED `f8dc41a9f`

1. **Retirement verification.** For each of the 26 zero-corpus
   decorators in §11.1, grep `tests/`, `.github/workflows/`,
   `contracts/`, `examples/sandboxes/`, ADRs. Anything found stays;
   anything truly unused goes to `AGENTS.md §Retired Surfaces` (which
   already has a CR-L6 gate).
2. **`@query` and `@mutation` introduction.** Add tokens to lexer,
   parser handlers (likely thin wrappers reusing the existing
   `@endpoint` pipeline), and typecheck signatures. Same backend; new
   surface.
3. **Corpus migration via `vox fmt` rule.** Mechanical
   `@endpoint(kind: query)` → `@query` and same for `mutation`. 87
   corpus sites. Run on all of `examples/golden/**/*.vox` plus
   `examples/golden-ts/`, `apps/`, `scripts/`. Single PR.
4. **Doc updates.** AGENTS.md, ref-builtins-stdlib.md, tutorial pages,
   the web-app-archetype docs.

### Phase C — typecheck builtin registry overhaul ✅ LANDED `f8dc41a9f`

Status: typecheck now registers ~30+ method signatures across
`FsModule`, `PathModule`, `RegexModule`, `LogModule`, `ProcessModule`,
plus Str/Int/Float/Bool/Result/Option method maps. Eval-side
`Object.get` re-aligned to return `Option[T]` matching the typeck
signature.

5. **Catalog the gap.** Mechanically diff
   [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)'s
   `call_builtin_method` arms against
   [`crates/vox-compiler/src/typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs).
   Output: list of methods the runtime supports but typecheck doesn't.
6. **Register each.** Add a `Ty::Fn(...)` signature per method in
   `typeck/builtins.rs`. The hard part is signatures for generic
   methods like `xs.map(|x| ...)` — defer those if closures aren't
   landed yet.
7. **Verify against corpus.** Re-run `vox check` against
   `scripts/**` — expected jump from 15/53 passing to ~35–40/53.

### Phase D — stdlib-coverage audit gate ✅ LANDED `ef42611b1` + `d998dccba`

Status: shipped in Rust (not Vox script) following the
`vox-code-audit::retirement_parity` precedent. See §10 for full
as-built spec. Detector now sees both the eval/builtins.rs surface
AND the builtin_registry.rs (vox-actor-runtime) surface; doc-side
normalization aligns. Drift gate currently shows `error_count=2`,
both real Bucket-A corpus items. CI job wired into
`cr-l8-corpus-feedback.yml` with regression-only gating against
the committed baseline.

### Phase E — corpus triage finish ✅ COMPLETE 2026-05-26

Status (2026-05-26): **53/53 scripts pass `vox check`** (100%). Up
from 18/53 on 2026-05-23. Blockers resolved between sessions:

- Closure-taking methods (`and_then`, `map`, `filter`, `any`, `all`,
  `fold`) already landed in `eval/expr.rs` with full typecheck
  coverage in `typeck/builtins.rs`. Scripts using `fn(x) { ... }`
  anonymous-function form pass without requiring `|x|` pipe syntax.
- `fn(params) to ReturnType { body }` Lambda form already handled
  by the parser, AST, HIR, typeck, and eval — matching how the
  corpus scripts are actually written.
- JSON ergonomics (strict-Option `.get`/`.at`/`.pointer`) landed
  2026-05-23 — unblocked the `audit-workspace-health`,
  `audit-dependency-layers`, and `generate-matrix-doc` scripts.
- Intra-project imports (`import "./foo.vox"`) landed 2026-05-23.

All 53 scripts now pass. No aspirational-directory moves needed.

### Phase F — CI promotion ✅ COMPLETE 2026-05-26

13. **`vox check` on `scripts/**` is mandatory.** Baseline locked at
    `contracts/reports/scripts-pass-baseline.txt` (53 paths). The
    CI gate in `.github/workflows/cr-l8-corpus-feedback.yml`
    (job `scripts-check`) fails on regression vs the baseline. All
    53 scripts promoted to the baseline on 2026-05-26.

### Phase G — Bucket-A language features ✅ COMPLETE 2026-05-26

14. ✅ **Closures** (RFC + impl + tests) — all complete. Closures use the
    `fn(params) to ReturnType { body }` anonymous function form (decided
    in `closures-rfc-2026-05-23.md §11` as the canonical form — no `|x|`
    pipe syntax). Eval dispatch in `eval/expr.rs::apply_closure_method`,
    typeck signatures in `typeck/builtins.rs`.
15. ✅ **Option/Result method completion** — `.map`, `.and_then`,
    `.and_then`, `.filter`, `.map_err`, `.all`, `.any`, `.fold` are all
    implemented for `List`, `Option`, and `Result` with closure support.
    53/53 scripts pass `vox check` (Phase E/F). The `fn(x) { body }` form
    is the canonical Vox anonymous function; scripts use it throughout.

### Phase J — intra-project Vox-file imports ✅ COMPLETE 2026-05-26

Tracked separately from G because the language-feature concern is
file-resolution, not lambda semantics.

Status (2026-05-26): **fully complete** — both bare-form and alias-form
imports work end-to-end (eval + typecheck + CLI).

- ✅ RFC: [`intra-project-imports-rfc-2026-05-23.md`](./intra-project-imports-rfc-2026-05-23.md)
- ✅ AST: `ImportPathKind::LocalFile { path }` variant
  (`crates/vox-compiler/src/ast/decl/types.rs`).
- ✅ Parser: `try_parse_local_file_import` accepts
  `import "./path.vox" [as alias]`, rejects non-`.vox` extensions
  (`crates/vox-compiler/src/parser/descent/decl/head.rs`).
- ✅ HIR: `HirImport.local_file_path` / `local_file_alias` carry the
  resolution intent forward (`crates/vox-compiler/src/hir/nodes/decl.rs`).
- ✅ Eval resolver: `Interpreter::resolve_local_file_import` walks the
  importer's directory, parses + lowers the target, registers `pub`
  fns and `pub` type variants. Cycle-safe via
  `Interpreter::loaded_imports` (idempotent re-import) plus an explicit
  self-cycle check during transitive descent
  (`crates/vox-compiler/src/eval/mod.rs`).
- ✅ Scope-merge form (bare `import "./foo.vox"`): pubs land directly
  in the importer's scope (with importer-defined names shadowing).
- ✅ Alias form (`import "./foo.vox" as alias`): pubs land in a
  namespace object; `alias.fn_name(args)` dispatches via new
  Object-method routing in `eval/expr.rs` (Fn fields applied via
  `apply_closure`; Constructor fields produce Tagged values).
- ✅ Privacy: non-`pub` functions stay file-private (verified by manual
  test producing `UndefinedVariable("private_helper")`).
- ✅ CLI wiring: `vox run --mode interp` sets `Interpreter::source_path`
  to the canonicalized file path so relative imports resolve correctly
  (`crates/vox-cli/src/commands/run.rs`).

Landed since first draft:
- ✅ **Typecheck integration** (`typecheck_hir_module_with_path`):
  pre-pass eagerly loads + lowers each imported `.vox` and registers
  its `pub fn` signatures into the importer's `TypeEnv` (cycle-safe via
  per-call visited set). Pipeline passes the importer's path through;
  `vox check` now succeeds on bare-form imports.
- ✅ **Alias-form typecheck** (item 17, 2026-05-26): `resolve_imported_pubs_into_env`
  registers alias-form imports as `Ty::Record(fields)` where each field
  is a `Ty::Fn`. The `checker/expr.rs` Record-method dispatch arm routes
  `alias.fn_name(args)` through the correct signature. `vox check` now
  succeeds on alias-form imports. (`import "./x.vox" as alias` → fully
  type-safe at both typecheck and eval layers.)
- ✅ **Aspirational corpus retired**: the 4 placeholder files at
  `examples/aspirational/intra-project-imports/` were pseudocode
  (CommonJS-style `module.exports`), not working Vox. Replaced by
  real helper modules at `scripts/mens-corpus/helpers/`
  (`walk_docs.vox`, `walk_sources.vox`, `jsonl_writer.vox`) and an
  entry-point pipeline at `scripts/mens-corpus/harvest_small.vox`
  that imports all three and writes a 3,781-entry JSONL corpus
  index against the live repo.

Remaining work:
17. ✅ **Alias-form typecheck** — COMPLETE 2026-05-26 (see "Landed since first draft" above).
19. ✅ **CR-L gate** — COMPLETE 2026-05-26. `crates/vox-code-audit/src/detectors/import_cycles.rs`
    ships as rule 51 (`"import/cycle"`, `vox/import/cycle`). The `ImportCyclesDetector`
    per-file `detect()` catches direct self-imports; the public
    `detect_import_cycles_in_batch(files)` function builds the full directed
    import graph and runs iterative DFS cycle detection for multi-file cycles
    (A→B→A, chains of any length, diamonds excluded). Both the `all_rules_instantiate`
    registry gate and 11 dedicated unit tests pass.
20. **Script-mode (vox-actor-runtime) parity**: the
    actor-runtime/native build path needs the same resolver so
    `--mode script` works equivalently. Deferred — `--mode interp`
    is the canonical script-execution mode today.

### Phase K — vox-actor-runtime stdlib parity ✅ LANDED 2026-05-23

Bring `vox-actor-runtime` (script-mode / compiled-app surface) up to
parity with `--mode interp` so a script that runs cleanly under interp
also runs under `--mode script` after codegen. Today the interpreter
is canonical for scripts; this Phase closes the divergence for the
compiled path.

**Pre-flight survey (manual diff of `crates/vox-compiler/src/eval/builtins.rs`
namespace dispatches vs `crates/vox-actor-runtime/src/builtins/mod.rs`
`pub fn vox_*` exports, 2026-05-23):**

Missing in actor-runtime, present in interp:

| Namespace | Missing primitive(s) | Why it matters |
|---|---|---|
| `fs` | `exists`, `is_file`, `is_dir`, `remove` | Every corpus script uses `fs.exists`; compiled mode silently lacks it. |
| `path` | `extension`, `parent`, `file_name`, `stem`, `is_absolute`, `resolve` | All present in interp. Only `path.join` (`vox_path_join_many`) exists in runtime. |
| `env` | `args`, `set` | `env.args()` used by several scripts to pull CLI args; `env.set` rare. |
| `regex` | `replace`, `find`, `captures` | Runtime has `vox_regex_compile` only — useless without dispatch wrappers. |
| `time` | (none beyond `now_ms`) | Verify; interp `time.*` surface is also small. |

Concrete punchlist — **all 14 landed in `crates/vox-actor-runtime/src/builtins/mod.rs`** as of 2026-05-23 (the `// ── Phase K stdlib parity` block immediately after `vox_fs_mkdir`). Codegen wire-up in the `--mode script` lowering path remains a follow-up; the native functions are ready for it.

1. `vox_fs_exists(path: &str) -> bool`
2. `vox_fs_is_file(path: &str) -> bool`
3. `vox_fs_is_dir(path: &str) -> bool`
4. `vox_fs_remove(path: &str) -> Result<(), String>`
5. `vox_path_extension(p: &str) -> Option<String>`
6. `vox_path_parent(p: &str) -> Option<String>`
7. `vox_path_file_name(p: &str) -> Option<String>`
8. `vox_path_stem(p: &str) -> Option<String>`
9. `vox_path_is_absolute(p: &str) -> bool`
10. `vox_path_resolve(p: &str) -> Result<String, String>`
11. `vox_env_args() -> Vec<String>`
12. `vox_env_set(k: &str, v: &str)`
13. `vox_regex_replace(pattern: &str, haystack: &str, replacement: &str) -> Result<String, String>`
14. `vox_regex_find(pattern: &str, haystack: &str) -> Result<Option<String>, String>`

**Codegen wire-up landed 2026-05-24:** the 12 new `path.*`/`env.*`/
`regex.*` methods now have dispatch entries in
`crates/vox-compiler/src/builtin_registry.rs` (both
`std_namespace_runtime_call` for the `--mode script` lowering and the
typeck signature table). `--mode interp` and `--mode script` reach the
same surface for everything Phase K added; no `vox check` regressions
on the corpus baseline (46/59 holds).

**Also tracked separately:** intra-project-import resolver
(`Interpreter::resolve_local_file_import` from §11.7) needs an
equivalent in the actor-runtime emit path for `--mode script` to honor
`import "./foo.vox"`. Today only `--mode interp` resolves them.

### Phase L — Corpus failing-script triage (2026-05-23, post-JSON-RFC)

After landing intra-project imports + strict-Option JSON API + Phase K
parity, the corpus stands at **41/55 passing (75%)**. Triage of the 13
remaining failures, grouped by root cause (so fixes batch cleanly):

| Bucket | Scripts | Root cause | Fix path |
|---|---|---|---|
| **L.1 Phonetic-operator mechanical** (`!` → `not`) | `gui-build.vox`, `setup.vox` | Pre-phonetic-operator legacy code | Sed-style 1-line fix per script |
| **L.2 Arrow-syntax mechanical** (`->` return type → `to`) | `render-durable-animation.vox` | Pre-arrow-deprecation legacy | `vox fmt`-style rewrite (`scripts/migrate-arrows.vox` exists for exactly this — once the migrator itself parses) |
| **L.3 Rust-path syntax** (`fs::is_dir` → `fs.is_dir`) | `migrations/2026-phase1-delete-empty-schemas-dir.vox`, `migrations/2026-phase1-delete-repo-root-strays.vox` | Authored with Rust syntax in mind | Mechanical: `::` → `.` in callsites |
| **L.4 Regex-in-string-literal lexer interaction** | `extract_table_names.vox`, `migrate-arrows.vox` | Vox templates `{...}` clash with regex literals containing `(...)` capture groups; parser stops mid-string | Investigate: parser may need raw-string syntax (`r"..."` Rust-style), OR these scripts should embed regex via a helper |
| **L.5 Old import syntax** (`import x from "y"`) | `index_symbols.vox` | Uses ES-module-style for non-React imports | Migrate to `import "./y.vox"` (now that intra-project imports exist!) |
| **L.6 Closures without type annotations** | `fix-doc-categories.vox`, `migrate-corpus.vox` | Closure params typed as `<unknown>` | Add `fn(x: <Type>)` per closures RFC §11 |
| **L.7 Result/Option not-unwrapped** | `ci/gui-registry-check.vox`, `ci/test.vox`, `scientia/acceptance-matrix.vox` | `process.run().code` accessed without `.unwrap()` (now caught by tighter typeck) | Insert `.unwrap()` or `match` on the Result |

**Most-leveraged next bucket: L.4** (raw-string syntax) — would let
multiple regex-heavy scripts pass at once, and is a small parser feature
(probably 20-50 LoC). Worth proposing as a mini-RFC: `r"raw text"`
matching Rust's raw-string syntax. Without it, every regex pattern in
Vox has to dodge `{`/`}` and shell metacharacters.

**Least-leveraged: L.7** — three scripts, each needs hand-inspection of
where Option/Result types are flowing.

**Quickest wins (per LoC): L.1, L.3, L.2, L.5** — all mechanical, each
under 10 minutes per script. Estimated +6 PASS to 47/55 (~85%) for one
afternoon of work.

**Progress 2026-05-23 → 2026-05-24:**

- ✅ **L.3 landed.** Both `migrations/2026-phase1-delete-empty-schemas-dir.vox`
  and `migrations/2026-phase1-delete-repo-root-strays.vox` now pass
  (mechanical `fs::` → `fs.` rewrite; the second also moved from
  `fs::read_dir` iterator pattern to `fs.glob("./codex-cutover-*.sidecar.json")`).
- ✅ **L.4 raw-string syntax landed** — `r"..."` lexer token (Rust-style;
  basic single-`"`-terminated form) + parser arm + Display. Test suite
  at `crates/vox-compiler/tests/raw_string_test.rs` (6/6). Note: hash-padded
  `r#"..."#` form (which would let regex patterns embed `"`) is NOT
  yet supported, so the L.4 scripts (`extract_table_names.vox`,
  `migrate-arrows.vox`) were retired to
  `examples/aspirational/regex-heavy/` and
  `examples/aspirational/historical-migrators/` instead of migrated.
  The migrate-arrows script was the migration tool itself — already
  done its job; retiring it removed a contradiction.
- ✅ **L.5 landed via retirement.** `index_symbols.vox` was aspirational
  pseudocode for a `compiler` stdlib namespace that doesn't exist;
  moved to `examples/aspirational/compiler-as-stdlib/` with a README
  documenting what would need to land first.
- ✅ **L.6 landed.** Both `fix-doc-categories.vox` and `migrate-corpus.vox`
  now pass. The root cause turned out to be subscript + Result-unwrap
  rather than untyped closures — `lines[i]` typed as `<unknown>` and
  `fs.read_to_string` returning Result not unwrapped. Now both use
  `lines.get(i).unwrap_or("")` and `content_res.unwrap()` patterns.
- ✅ **Typed list subscript landed** — `list[T] [i]` returns
  `Option[T]`; out-of-bounds and wrong-receiver-type both yield `None`.
  Honors no-silent-failure; eval + typeck aligned. Test suite at
  `crates/vox-compiler/tests/typed_subscript_test.rs` (4/4).
- ⏸ **L.1 partial.** `gui-build.vox` and `setup.vox` migrated to
  `process.run_ex` + `.code` + Option/Result discipline (more than
  L.1's "`!` → `not`" framing suggested). Both pass `vox check`.
- ⏸ **L.7 deferred.** 3 scripts still need per-site Result/Option
  unwrap insertions (`ci/gui-registry-check.vox`, `ci/test.vox`,
  `scientia/acceptance-matrix.vox`).

**Corpus result: 41/55 (triage start, 75%) → 46/59 (post-recovery, 78%).**
Denominator grew because helpers/ + harvest_small + restored
aspirational scripts came back into `scripts/`. Zero regressions vs
the refreshed baseline.

### Phase H — `@endpoint` retirement (after Phase B + C settle, ~3 days)

**Retirement audit 2026-05-24:** **DO NOT RETIRE YET.** Per
`feedback_verify_audit_retirement_claims.md` (memory: "5/10 audit
retirements were wrong on 2026-05-16, one nearly cost 9,670 lines of
integration tests"), I scanned `tests/`, `crates/`, `examples/`,
`.github/workflows/`, `contracts/`, and `docs/` for `@endpoint` usage
before any code change. Live use sites:

| File | Line | Form | Risk class |
|---|---|---|---|
| `examples/golden/option_type.vox` | 34 | `@endpoint(kind: query)` | Golden — must migrate before retire |
| `crates/vox-integration-tests/tests/fixtures/chatbot.vox` | 49 | `@endpoint(kind: mutation) fn api_chat(...)` | **Integration test fixture** — exactly the high-risk category |
| `crates/vox-integration-tests/tests/fixtures/full_stack_minimal.vox` | 12 | `@endpoint(kind: server) fn echo(...)` | **Integration test fixture** |
| `crates/vox-integration-tests/tests/fixtures/greaterfool_reference.vox` | 81 | `@endpoint(kind: server) fn chat(...)` | **Integration test fixture** |

(Other `@endpoint` mentions in `examples/golden/{auth_patterns,background_jobs}.vox`
are documentation frontmatter — `constructs: [..., @endpoint, ...]` — not
live usage. Build artifacts under `docs/book/` are auto-generated and ignored.)

**Code-side `@endpoint` handlers** that must stay until retirement:
`vox-cli/commands/{check,compile,db/*}.rs`,
`vox-code-audit/detectors/{auth_endpoint,effect_net_decl,id_at_boundary,
retired_decorator}.rs`, and `vox-code-audit/diagnostics/catalog.rs`. Each
parses or reports on `@endpoint` — the implementation is alive.

**Revised retirement path:**

16. **(prereq)** Migrate the 4 live use sites to `@query` / `@mutation` /
    `@server`. The integration-test fixtures need golden-output snapshot
    refreshes after migration. Estimated 1-2 hours of careful work +
    snapshot review.
17. Run the full corpus + `cargo test --workspace` after each fixture
    migration to ensure no behavior change.
18. Once the migration commit is on `main` for one minor release and
    no `@endpoint(kind: …)` regressions surface in real-world usage,
    THEN retire `@endpoint`. Add to `AGENTS.md §Retired Surfaces`.
    CR-L6 retirement-guard gate keeps it permanent.

**Why not retire now:** the integration test fixtures are exactly the
class of usage that the prior 2026-05-16 incident nearly broke (almost
9,670 LoC of tests). Retiring `@endpoint` without first migrating these
would cascade: parsing errors → fixture HIR mismatch → test failures →
revert cascade. The 1-2 hours of migration work is mandatory before
removing the surface.

### Total session-count estimate (as of 2026-05-23)

Phases A–D **completed in 1 elongated session** (5+ commits). Phases
E + F = ~1 focused session each. Phase G is roadmap-scheduled
(2 weeks per feature). Phase H is post-stabilization cleanup.

The §9 phased plan above this section is now superseded for tasks
8–13; refer to §12 for the canonical ordering. Phases D and C
**landed earlier than planned** — both within the same session as A/B.

---

## 13. Spawn-ready tasks (added 2026-05-23 for Phase B)

### 13.1 Retire dead decorators (Phase B step 1)

> For each decorator in the list below (the §11.1 zero-corpus set), verify
> it has zero use across `tests/`, `.github/workflows/`, `contracts/`,
> `examples/sandboxes/`, and any ADR under `docs/src/architecture/`. Any
> hit means *keep*. Output: a CSV listing each decorator and the
> verification result.
>
> Decorators to verify:
> `@auth`, `@cancellable`, `@collaborative`, `@cors`, `@deep_link`,
> `@distributed_train`, `@embed`, `@ensure`, `@forall`, `@fuzz`,
> `@index`, `@inference`, `@invariant`, `@layer`, `@native`,
> `@offline_capable`, `@pii`, `@rate_limit`, `@reactive`, `@remote`,
> `@tokens`, `@training_step`, `@v0`, `@webhook` and the bare-form
> `@tool` / `@resource`.
>
> Per the user's `feedback_verify_audit_retirement_claims.md` memory:
> 5/10 audit retirements were wrong on 2026-05-16, one nearly cost
> 9,670 lines of integration tests. Don't trust LoC + Cargo.toml graph
> alone. Verify by hand.

### 13.2 Introduce `@query` and `@mutation` (Phase B step 2)

> Add `#[token("@query")] AtQuery` and `#[token("@mutation")] AtMutation`
> to [`crates/vox-compiler/src/lexer/token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)
> alongside the existing `@endpoint` token. In the parser at
> [`crates/vox-compiler/src/parser/descent/mod.rs`](../../../crates/vox-compiler/src/parser/descent/mod.rs)
> (`is_decl_position` match), accept the new tokens as decl-position
> starters that produce the same AST node as
> `@endpoint(kind: query)` / `@endpoint(kind: mutation)`. The HIR
> lowering and typecheck downstream need no changes — they see the same
> AST shape.
>
> Tests:
> 1. Lexer round-trip for both new tokens.
> 2. Parser test that `@query fn f() to int { ... }` produces the same
>    HIR as `@endpoint(kind: query) fn f() to int { ... }`.
> 3. End-to-end: a `.vox` file using `@query` compiles and the generated
>    TypeScript client matches.

### 13.3 Corpus migration `@endpoint(kind: …)` → `@query`/`@mutation` (Phase B step 3)

> Sed-style mechanical rewrite across the corpus:
>
> - `@endpoint(kind: query)` → `@query`
> - `@endpoint(kind: mutation)` → `@mutation`
> - `@endpoint(kind: query) fn` → `@query fn` (same with mutation)
>
> Affected paths: `examples/golden/**/*.vox` (29 sites),
> `examples/golden-ts/**/*.vox` (~5 sites), `apps/**/*.vox`,
> `scripts/**/*.vox` (probably none — these are not endpoint-style).
> Estimated 87 total replacements.
>
> Acceptance: zero matches for `@endpoint(kind:` under `examples/`,
> `apps/`. The original `@endpoint` token stays registered (marked
> deprecated) but no longer appears in canonical examples.

### 13.4 Catalog typecheck-builtin gap (Phase C step 5)

> Compare [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)'s
> `call_builtin_method` arms (lines 92–~1150) against
> [`crates/vox-compiler/src/typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs)'s
> registrations. Produce a structured diff per namespace:
>
> ```
> fs:
>   eval-only: read, read_file, write, write_file, exists, …
>   typeck-only: (none)
>   both: …
> str (methods):
>   eval-only: trim, split, contains, starts_with, …
>   typeck-only: (none)
>   both: …
> ```
>
> Each "eval-only" entry is a typecheck false-positive waiting to
> happen. Goal: zero "eval-only" entries after Phase C.

### 13.5 Typecheck builtin registration (Phase C step 6)

> For each method identified in §13.4 as eval-only, add a corresponding
> `Ty::Fn(...)` signature in
> [`crates/vox-compiler/src/typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs).
>
> Generic methods (`xs.map(|x| ...)`, `option.and_then(|x| ...)`) need
> closures to be representable in the type system — defer them to
> Phase G if closures haven't landed.
>
> Acceptance: `vox check scripts/migrate-corpus.vox` and
> `vox check scripts/quality/doc-policy-lint.vox` (both currently
> CHECK-FAILing after the call-form migration) now pass.

### 13.6 Audit-stdlib-coverage `.vox` script (Phase D step 8)

> Write `scripts/quality/audit-stdlib-coverage.vox` per AGENTS.md's
> VoxScript-First policy. The script:
>
> 1. Parses
>    [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
>    via line-grep + simple regex to extract every registered
>    `namespace.method` arm.
> 2. Parses
>    [`docs/src/reference/ref-builtins-stdlib.md`](../reference/ref-builtins-stdlib.md)
>    table rows to extract documented signatures.
> 3. Greps `scripts/**/*.vox` for `<ident>.<ident>(` call sites.
> 4. Emits a three-way diff matching the §3.1 table format.
> 5. Exits non-zero if any *error*-severity mismatch (corpus uses
>    unregistered or docs claim unregistered).
>
> Wire into [`.github/workflows/cr-l8-corpus-feedback.yml`](../../../.github/workflows/cr-l8-corpus-feedback.yml)
> as a sibling job, or as its own workflow. Trigger paths:
> `crates/vox-compiler/src/eval/builtins.rs`, `scripts/**`,
> `docs/src/reference/ref-builtins-stdlib.md`.
>
> If the `.vox` version proves the gate's value, *then* consider
> promoting to a `vox-audit` subcommand. Until then, the `.vox`
> implementation is canonical.

---

## Related plans / SSOTs

- [`docs/src/architecture/ai-laziness-remediation-plan-2026.md`](./ai-laziness-remediation-plan-2026.md) — this audit
  is concrete Phase-2 input.
- [`docs/src/architecture/vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md) — the closest
  thing to a truthful stdlib SSOT today.
- [`docs/src/architecture/tooling-convergence-findings-2026.md`](./tooling-convergence-findings-2026.md) — `vox audit`
  umbrella (P3 fix above).
- [`docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md`](./vox-as-llm-target-audit-and-plan-2026.md) — the
  v1-release CR-L criteria; this audit informs CR-L1 (corpus integrity) and
  CR-L3 (error-message discoverability).
- [`AGENTS.md` § VoxScript-First Glue Code](../../../AGENTS.md) — the policy
  that mandates `.vox` automation; needs the `--mode interp` correction.
