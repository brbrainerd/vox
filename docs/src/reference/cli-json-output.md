---
title: "Reference: `vox` CLI machine-readable `--json` output"
description: "The stdout JSON contract for vox check, build, test, run, and doctor --diag: envelope shapes, shared keys, and how many JSON lines each command emits under --json."
category: "Language Reference"
status: "current"
training_eligible: true
schema_type: "TechArticle"
---
# Reference: `vox` CLI machine-readable `--json` output

Several `vox` commands emit machine-readable JSON on **stdout** so an agent can drive
the toolchain without scraping human prose. This page is the single map of what each
command emits: the envelope shape, which keys are shared, and — critically — **how many
JSON lines** land on stdout per invocation.

Two rules hold across every command documented here:

- **stdout is reserved for the machine payload.** Human progress notes and error
  messages go to **stderr** (the CLI's tracing subscriber also writes to stderr). An
  agent parsing stdout never has to filter out log lines.
- **The build lane emits compact JSONL.** `vox build`, `vox test`, `vox run`, and
  `vox doctor --diag` each emit *compact, single-line* JSON objects — one JSON value per
  line — so multiple lines on one stream parse as [JSON Lines](https://jsonlines.org/).

## Shared envelope keys

The compact build-lane envelopes (`build` / `test` / `run` / `doctor --diag`) share three
keys so one parser handles them all:

| Key | Type | Meaning |
| --- | --- | --- |
| `envelope_version` | integer | Envelope schema version (currently `1`). |
| `command` | string | Which command produced the line (`"build"`, `"test"`, `"run"`, `"doctor"`). |
| `ok` | boolean | `true` when the command's own success condition held for that stage. |

Beyond these, each command carries a payload field appropriate to it (`diagnostics` for
the compiler lane, `checks` for doctor). The two exceptions are `vox check` (see below),
which predates this contract and has its own two older shapes.

## `vox check`

`vox check` has **two** distinct JSON shapes, selected by flag:

### `--json` / `--output-format json` (or global `--json`)

Emits a **pretty-printed JSON array** of diagnostic payloads (`VoxCompilerDiagnosticPayload`)
— *not* an envelope, and *not* compact. One multi-line array per invocation:

```json
[
  {
    "error_code": "vox/types/type-mismatch",
    "severity": "Error",
    "message": "Type mismatch in `let`: Cannot unify Str with Int",
    "file_path": "app.vox",
    "span": { "start_line": 2, "start_col": 16, "end_line": 2, "end_col": 28 },
    "explain_url": "https://vox-lang.org/diag/vox/types/type-mismatch"
  }
]
```

An empty array (`[]`) means no diagnostics. The process exit code (non-zero on errors) is
the authoritative pass/fail signal for this shape.

### `--for-llm`

Emits a **pretty-printed `CheckForLlmEnvelope` object** with an explicit `ok`/count summary,
followed by a trailing human line (`Check passed (--for-llm) with N warning(s)`) on success:

```json
{
  "envelope_version": 1,
  "file_path": "app.vox",
  "ok": true,
  "error_count": 0,
  "warning_count": 1,
  "diagnostics": [ /* VoxCompilerDiagnosticPayload… */ ]
}
```

`lint_findings` (static-analysis TOESTUB results) is included only when the `stub-check`
feature is compiled in, and omitted from the JSON when empty.

## `vox build --json`

**Always exactly one** compact `BuildLaneEnvelope` line on stdout — for a clean build, a
frontend/typecheck failure, *and* a codegen-stage failure. An agent can rely on getting
one parseable line per invocation regardless of where the build stopped.

```json
{"envelope_version":1,"command":"build","file_path":"app.vox","ok":false,"error_count":1,"warning_count":0,"diagnostics":[{"error_code":"vox/types/type-mismatch", "...":"…"}]}
```

Fields: `envelope_version`, `command` (`"build"`), `file_path`, `ok`, `error_count`,
`warning_count`, `diagnostics` (array of `VoxCompilerDiagnosticPayload`; empty for a
codegen-stage failure that produced no frontend diagnostics), and `exit_code` (omitted
when absent). On failure the envelope is printed to stdout *and* the process exits
non-zero with a human error on stderr.

## `vox test --json`

Runs a build, then `cargo test` on the generated crate. Line count depends on how far it gets:

- **Build fails** → **1 line**: the `command:"build"` envelope (with diagnostics). The test
  stage is never reached, so no `test` envelope is emitted.
- **Build succeeds** → **2 lines**: the `command:"build"` envelope (`ok:true`), then a
  `command:"test"` envelope carrying the real `cargo test` outcome:

```json
{"envelope_version":1,"command":"build","file_path":"app.vox","ok":true,"error_count":0,"warning_count":0,"diagnostics":[]}
{"envelope_version":1,"command":"test","file_path":"app.vox","ok":false,"error_count":0,"warning_count":0,"diagnostics":[],"exit_code":101}
```

The `test` envelope's `exit_code` is the actual `cargo test` process exit code (`101` on a
test panic/failure, `0` on pass).

## `vox run --mode script --json`

Compiles a script and then **executes** it, so success output is the *program's own stdout* —
which must not be corrupted by an envelope. Therefore:

- **Compile succeeds** → **0 envelope lines**: the script runs and its real stdout follows
  untouched.
- **Compile/frontend fails** → **1 line**: a `command:"run"` build-lane envelope with the
  frontend diagnostics, then a non-zero exit.

```json
{"envelope_version":1,"command":"run","file_path":"script.vox","ok":false,"error_count":1,"warning_count":0,"diagnostics":[{"error_code":"vox/types/type-mismatch","...":"…"}]}
```

## `vox doctor --diag <id> --json`

Runs the single build-health check that can produce the `[diag id=…]` tag `<id>` and emits
**one** compact `DoctorDiagEnvelope` line. It shares the `envelope_version` / `command` /
`ok` keys with the build lane, but its payload field is **`checks`** (doctor's own check
model) rather than `diagnostics`:

```json
{"envelope_version":1,"command":"doctor","diag_id":"linker.lld_missing","ok":true,"checks":[{"name":"linker: lld-link","pass":true,"detail":"present (fast Windows linker)"}]}
```

- `diag_id` echoes the requested diagnosis id.
- `ok` is `true` when the requested diagnosis did **not** fire (exit `0`); `false` and a
  non-zero exit when it did.
- `checks` is the array of `{name, pass, detail}` rows the check produced. A failing row's
  `detail` carries the machine-parseable `… | FIX: <cmd> | [diag id=<id> sev=… heal=…]` tag.

**Unknown id:** `vox doctor --diag <bogus> --json` emits **0 stdout lines** and a plain
usage error on stderr listing the known ids, with a non-zero exit. An unknown id is a
usage error (there is no check result to envelope), not a diagnostic outcome.

## Known shape inconsistencies

This contract grew incrementally, so a few shapes differ from the build-lane norm. They are
documented here rather than silently normalized:

- **`vox check --json` is a bare, pretty-printed array**, not a compact envelope — it
  predates the envelope work. `--for-llm` gives an envelope-shaped object but is
  pretty-printed and carries no `command` key.
- **`vox doctor --diag --json` uses `checks`, not `diagnostics`**, because doctor's check
  model (`{name, pass, detail}`) is not a compiler `VoxCompilerDiagnosticPayload`. The
  shared `envelope_version`/`command`/`ok` keys still let one parser dispatch on `command`.
- **`vox doctor --json` without `--diag`** (the full audit) requires the extended
  `codex`-feature build and emits a different, pretty-printed `Vec<Check>` array; only the
  `--diag` path is part of the compact-envelope contract above.
