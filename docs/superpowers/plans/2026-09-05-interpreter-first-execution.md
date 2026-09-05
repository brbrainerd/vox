# Interpreter-First Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Revision 2** — rewritten after a seven-track critique verified revision 1 against the source. Revision 1 had three code blocks that did not compile, a task that could not execute (missing crate edge), a filesystem method list wrong in both directions, an ungated `import`, a payload frame that rejected every payload, and a default-flip predicate that would have routed `table`/`routes`/`server` programs to the interpreter. Each correction below cites the finding.

**Goal:** Make the HIR interpreter the default and sandboxing execution tier for VoxScripts — locally and over the mesh — so `cargo`, `rustc`, and a Vox checkout stop being end-user requirements for running pure Vox.

**Architecture:** Capabilities become receiver-imposed, non-optional and fatal; memory, recursion depth and output join steps as hard bounds; every side-effecting entry point (including `import`) is gated, so the interpreter is the sandbox for Vox code. The mesh executes `VoxScript` jobs by spawning `vox run --mode interp` as a bounded child in its own process group from `InterpExecutor` in `vox-mesh-transport` (no new crate edge), and never ships native code. A differential gate with a grown golden corpus proves the interpreter and native lanes agree. The wasi script lane, the HTTP native-bundle lane, `secret_gate`, `ProbeOnlyExecutor`, and the parsed-then-rejected isolation tiers are deleted and *retired* (contract rows), not merely removed.

**Tech Stack:** Rust 1.96 (edition 2024, let-chains), `vox-compiler::eval`, `vox-mesh-transport` on `iroh 1.1` (postcard frames — positional, not self-describing), `tokio::process`, clap 4.5.

**Spec:** [`docs/superpowers/specs/2026-09-05-interpreter-first-execution-design.md`](../specs/2026-09-05-interpreter-first-execution-design.md) (revision 2). Evidence: [`docs/src/architecture/voxscript-portability-substrate-research-2026.md`](../../src/architecture/voxscript-portability-substrate-research-2026.md).

## Global Constraints

- **Test-first.** Every new `pub fn` gets a test in the same file. Write the failing test, watch it fail, then implement.
- **Mutation-verify every guard** (spec §4): break it once, confirm the test fails, restore, `grep -c` the restoration, record it in the commit body.
- **Formatting:** `cargo fmt -p <crate>`. **Never `cargo fmt --all`.**
- **Clippy before every commit:** `cargo clippy -p <touched-crate> --all-targets -- -D warnings`. Use let-chains (`if let … && cond {}`), not nested `if let` — clippy 1.96 rejects the nested form. Nested `unsafe {}` inside `unsafe fn` is **required** under edition 2024.
- **Never `presets::N0`, `N0DisableRelay`, or `into_0rtt()`** (detector `vox/mesh/unsafe-iroh-pattern`, Error).
- **Crate edges:** this plan adds **none**. The `vox-compiler → vox-crypto` edge that `crypto.*` parity needs is a maintainer decision recorded in spec §3.3 — do not take it; Task 2 records `crypto` as a known asymmetry instead. Never add a `vox-cli` dev-dependency to `vox-mesh-transport` (upward edge). Removing `script-wasi` removes `vox-cli → vox-wasm-engine`; tighten with `cargo run -q -p vox-cli -- ci crate-edges --tighten`. **Never** add an `exceptions` entry.
- **`--features populi` goes on `-p vox-ml-cli`, never `-p vox-cli`.**
- **Do not regenerate `docs/agents/doc-inventory.json`** — stale on the base, owned by `ssot-autoregen`. If pre-push stops there, run the remaining gates directly and say so.
- **Contract regeneration is part of the task that causes it**, in this order when a command is deleted: `contracts/operations/catalog.v1.yaml` → `contracts/cli/command-registry.yaml` → `contracts/capability/{capability-registry.yaml,model-manifest.generated.json}` → `docs/src/reference/cli-command-surface.generated.md` → `contracts/reports/gui-surface-{registry,coverage}.v1.json` → `UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog`. `vox ci command-sync` alone is not sufficient. When a SecretId is deleted: `vox ci secrets-contracts` **before** `vox ci secrets-parity`.
- **Doc frontmatter** on every new `.md` under `docs/src/`; lint with `cargo run -q -p vox-doc-pipeline -- --lint-only --paths <file>`.
- **Commit messages:** imperative subject < 72 chars, body explains why, ending with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Do not push unless asked.
- **Fresh worktree gotcha:** build `crates/vox-gui/ui/dist` once (`pnpm install && pnpm build` in `crates/vox-gui/ui`) or workspace clippy fails. Do not commit anything to "fix" it.
- **`PROTO` 1 → 2 in Task 8 is incompatible by design.** A PROTO-1 peer (BLAPTOP04 until rebuilt) cannot talk to a Task-8 node. Task 16's cross-machine smoke requires rebuilding both ends.
- **Line numbers** in this plan were verified on `mesh-phase3-plan` @ `7124010b5`; re-`rg` before editing if the file has moved.

---

## File Structure

**Created**

| Path | Responsibility |
|---|---|
| `crates/vox-compiler/src/eval/caps.rs` | `CapabilitySet`: grammar, legacy-directive shim, typed `from_roots` constructor, `allows_namespace`/`allows_path`, `frozen_time_ms`, `random_seed`. Roots canonicalised at parse. No other I/O. |
| `crates/vox-cli/src/mem_limit.rs` | Counting `#[global_allocator]`, armed at runtime, aborts via `_exit`/`TerminateProcess`. **Declared in `lib.rs`.** |
| `crates/vox-mesh-transport/src/interp_executor.rs` | `InterpExecutor`: bounded child in its own process group, per-node semaphore, `(peer, job_id)`-keyed cancel, exit-code + stderr-marker mapping. |
| `crates/vox-mesh-transport/tests/common/mod.rs` | Shared live-endpoint helpers moved out of `security.rs` (`start_server_with`, `client_endpoint`, `send_run_on`, `loopback_addr_of`). |
| `crates/vox-mesh-transport/tests/interp_executor.rs` | Executor tests, `#[ignore]`d slow (build + spawn `vox`), registered in the full tier. |
| `crates/vox-integration-tests/tests/golden_differential_gate.rs` | Both-tier stdout diff; `// EXPECT-EXIT: nonzero-both` support; builds `vox` itself. |
| `examples/golden/{display_composites,float_formatting,object_field_order,glob_and_listdir_order,int_overflow_boundary,division_by_zero,argv_shape,crypto_hash_parity}.vox` | The eight goldens that give the gate a corpus (Task 1b). |
| `docs/src/reference/isolation.md` | The page `script.rs` already tells users to read. |
| `docs/src/adr/048-interpreter-is-the-execution-and-sandbox-tier.md` | The decision record. |

**Modified (major)** — `eval/{mod,builtins,expr,value,env}.rs`; `vox-cli/src/{lib,cli_args}.rs`, `commands/run.rs`, `commands/runtime/run/{script,sandbox}.rs`, `backend/{native,mod,tests}.rs`, `cli_dispatch/{mod,lanes}.rs`, `compilerd.rs`; `vox-langtool/src/commands/run.rs`; `vox-mesh-transport/src/{protocol,endpoint,directory,lib}.rs` + `tests/{security,mailbox}.rs`; `vox-orchestrator/src/a2a/{remote_worker,envelope}.rs`; `vox-workflow-runtime/src/workflow/populi.rs`; `vox-actor-runtime/src/builtins/mod.rs`; `vox-compiler/src/builtin_registry.rs`; `vox-codegen/src/codegen_rust/{pipeline.rs,emit/*}`; contracts and docs per task.

**Deleted** — `vox-cli/src/commands/wasm.rs`, `commands/runtime/run/backend/wasi.rs`, `src/isolation.rs`; `vox-skill-runtime/src/microvm.rs`, `tests/microvm_tier.rs`; `vox-orchestrator/src/a2a/secret_gate.rs` (+ `secret_bag.rs` if orphaned); `vox-mesh-transport::ProbeOnlyExecutor`.

---

## Task 0: Measure — by executing, not only checking

**Files:** Create `scripts/bench-script-tiers.vox`, `docs/src/architecture/script-tier-timings-2026-09.md`.

Finding (parity track): revision 1's script did not run — `fs.glob` returns `Result[list[str], str]`, `process.run_capture` returns `Result[{exit, stdout, stderr}]` (field is `exit`, not `exit_code`), and `vox check` cannot see runtime-only asymmetry, which is exactly what the flip creates.

- [ ] **Step 1: Write the script**

```vox
// Time every scripts/**/*.vox under `vox check` AND execute the side-effect-free ones
// under `--mode interp`, so a runtime-only asymmetry surfaces before the default flips.
// Native timings are taken by hand (Step 3): a cold native compile is ~275 s each.
pub fn main() {
  let files = match fs.glob("scripts/**/*.vox") { Ok(fs) => fs.sorted(), Error(e) => [] }
  let vox = match process.which("vox") { Some(p) => p, None => "target/debug/vox" }
  let mut rows = []
  for f in files {
    let t0 = time.now_ms()
    let check = match process.run_capture(vox, ["check", f]) {
      Ok(r) => if r.exit is 0 { "ok" } else { "FAIL" }
      Error(e) => "SPAWN-FAIL"
    }
    let dt = time.now_ms() - t0
    // `--help` after `--` exits early in scripts that mutate; scripts that ignore it run fully.
    let t1 = time.now_ms()
    let run = match process.run_capture(vox, ["run", "--mode", "interp", f, "--", "--help"]) {
      Ok(r) => if r.exit is 0 { "ok" } else { "FAIL(" + str(r.exit) + ")" }
      Error(e) => "SPAWN-FAIL"
    }
    let dr = time.now_ms() - t1
    rows = rows.push("| " + f + " | " + str(dt) + " ms | " + check + " | " + str(dr) + " ms | " + run + " |")
  }
  print("| script | `vox check` | status | `run --mode interp` | status |")
  print("|---|---|---|---|---|")
  for r in rows { print(r) }
}
```

- [ ] **Step 2: Run it** — `cargo build -q -p vox-cli --bin vox && cargo run -q -p vox-cli -- run --mode interp scripts/bench-script-tiers.vox > /tmp/tiers.md; grep -c FAIL /tmp/tiers.md`. Every `FAIL(n)` in the run column is a script the interpreter cannot execute today; list each with the first stderr line. **Any FAIL in `scripts/fmt.vox`, `scripts/install-hooks.vox`, `scripts/setup.vox`, or `scripts/arch-check.vox` blocks Task 7** — those are the lefthook/CI entry points (`lefthook.yml:18`, `.github/workflows/setup-e2e.yml:70,82,84`, `ci.yml:993`).

- [ ] **Step 3: Native timings by hand** for the same four scripts, cold (`rm -rf ~/.vox/script-cache`) and warm, with `/usr/bin/time -p cargo run -q -p vox-cli -- run --mode script <f> -- --help 2>&1 | grep real`.

- [ ] **Step 4: Write the doc** with frontmatter (`title: "Script tier timings (2026-09)"`, `description`, `category: "Architecture SSOTs"`, `status: "current"`), the machine line from `system_profiler SPHardwareDataType | grep Chip`, the native table, the pasted `/tmp/tiers.md`, and the FAIL list. **No placeholder cells may remain in the committed file.**

- [ ] **Step 5: Lint and commit** — `cargo run -q -p vox-doc-pipeline -- --lint-only --paths docs/src/architecture/script-tier-timings-2026-09.md` → `no hard errors`. `git commit -m "chore(scripts): measure script tiers by execution before flipping the default"`.

---

## Task 1: The differential gate

**Files:** Create `crates/vox-integration-tests/tests/golden_differential_gate.rs`; modify `crates/vox-integration-tests/Cargo.toml` (`which` is **not** a dev-dep there — add `which = { workspace = true }`); modify `crates/vox-cli/src/commands/ci/pre_push.rs:1551-1568` (`step_nextest_slow`); modify `docs/src/contributors/local-ci-pre-push.md`.

Findings: the `--include-slow` set is a **hardcoded nextest `-E` filter in Rust** (`pre_push.rs:1551-1568`), not the doc list; `cargo test -p vox-integration-tests` does not build the `vox` binary; exit-code faults cannot be `// EXPECT:` goldens; no golden calls `crypto.*`, so "must diverge today" was wrong.

- [ ] **Step 1: Write the test**

```rust
//! Differential gate (spec §3.3): a golden that declares `// EXPECT:` prints the same bytes
//! under `vox run --mode interp` AND `vox run --mode script`; one that declares
//! `// EXPECT-EXIT: nonzero-both` exits non-zero on both and its EXPECT lines are a prefix
//! of each tier's stdout. This is the only test in the repository that proves two execution
//! tiers agree. Slow (native compile per golden); run via the full tier's nextest filter.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// Builds `vox` itself if absent: `cargo test -p vox-integration-tests` does not, and a
/// gate that silently skips because a binary is missing is a decoy.
fn vox_binary() -> PathBuf {
    if let Ok(p) = std::env::var("VOX_BIN") {
        return PathBuf::from(p);
    }
    let root = repo_root();
    let exe = root.join("target/debug").join(if cfg!(windows) { "vox.exe" } else { "vox" });
    if !exe.exists() {
        let st = Command::new(env!("CARGO"))
            .current_dir(&root)
            .args(["build", "-q", "-p", "vox-cli", "--bin", "vox"])
            .status()
            .expect("spawn cargo");
        assert!(st.success(), "could not build the vox binary the gate spawns");
    }
    exe
}

fn collect_vox_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_vox_recursive(&p, out);
            } else if p.extension().is_some_and(|e| e == "vox") {
                out.push(p);
            }
        }
    }
}

fn directive_lines(src: &str, key: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| l.trim_start().strip_prefix(key).map(|r| r.strip_prefix(' ').unwrap_or(r).to_string()))
        .collect()
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").trim_end().to_string()
}

struct Run { code: Option<i32>, stdout: String, stderr: String }

fn run_mode(vox: &Path, mode: &str, file: &Path) -> Run {
    let out = Command::new(vox).args(["run", "--mode", mode]).arg(file).output()
        .unwrap_or_else(|e| panic!("spawn `{}` failed: {e}", vox.display()));
    Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

#[test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: native lane compiles a crate per golden; run via the full tier"]
fn golden_expect_blocks_match_on_both_tiers() {
    let root = repo_root();
    let vox = vox_binary();
    assert!(which::which("cargo").is_ok(), "the native half needs cargo on PATH; this gate must not pass silently");

    let mut files = Vec::new();
    collect_vox_recursive(&root.join("examples/golden"), &mut files);
    files.sort();

    let mut checked = 0usize;
    let mut failures = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        let expect = directive_lines(&src, "// EXPECT:");
        let expect_exit = directive_lines(&src, "// EXPECT-EXIT:");
        if expect.is_empty() && expect_exit.is_empty() {
            continue;
        }
        checked += 1;
        let expected = normalize(&expect.join("\n"));
        let i = run_mode(&vox, "interp", f);
        let n = run_mode(&vox, "script", f);
        let (io, no) = (normalize(&i.stdout), normalize(&n.stdout));
        let fault_expected = expect_exit.iter().any(|d| d == "nonzero-both");
        let ok = if fault_expected {
            i.code != Some(0) && n.code != Some(0) && io.starts_with(&expected) && no.starts_with(&expected)
        } else {
            i.code == Some(0) && n.code == Some(0) && io == expected && no == expected
        };
        if !ok {
            failures.push(format!(
                "{}\n  expected: {expected:?} (fault_expected={fault_expected})\n  interp:   code={:?} {io:?}\n    stderr: {}\n  native:   code={:?} {no:?}\n    stderr: {}",
                f.display(), i.code, i.stderr.trim(), n.code, n.stderr.trim()
            ));
        }
    }
    assert!(checked > 0, "no golden declares EXPECT / EXPECT-EXIT — the gate has no corpus");
    assert!(failures.is_empty(), "tiers disagree on {} golden(s):\n\n{}", failures.len(), failures.join("\n\n"));
}
```

- [ ] **Step 2: Run it once.** `cargo test -q -p vox-integration-tests --test golden_differential_gate -- --ignored --nocapture 2>&1 | tail -40`. **Expected: it may well pass** — the current 11 EXPECT goldens mostly `return "ok"`. A green run here is evidence the corpus is inadequate, not that the tiers agree. Record whatever it reports in the commit body; Task 1b gives it teeth.

- [ ] **Step 3: Register it in the full tier — in Rust, not only the doc.** In `pre_push.rs:1551-1568` the `-E` filter is a `concat!`; append `or test(golden_expect_blocks_match_on_both_tiers)`. Then add the row to `docs/src/contributors/local-ci-pre-push.md`'s `--include-slow` list. Verify the ignore string passes governance: `cargo run -q -p vox-cli -- ci ignored-test-age --mode enforce` (the check at `crates/vox-cli-ci/src/test_inventory.rs:204-224` accepts `owner:`/`sunset`/ISO date — the string above matches on `owner:`).

- [ ] **Step 4: Commit** — `git commit -m "test(gate): differential gate — interp and native must print the same bytes"`.

---

## Task 1b: Give the gate a corpus — eight goldens

**Files:** Create the eight `examples/golden/*.vox` files below. Each follows the existing comment-frontmatter style; fill `last_validated` per the golden conventions in `examples/golden/README.md` if present.

Finding (parity): 79 goldens, 11 with EXPECT, 9 of those trivial; zero coverage of overflow, div-by-zero, composite display, float formatting, object order, directory order, argv, or `crypto`. Goldens 5 and 6 need `// EXPECT-EXIT:`; golden 7 cannot compare paths so it asserts shape. Several of these **fail today by design** — they encode the decisions Task 2 implements. Commit them with the gate expected red, then Task 2 turns them green.

- [ ] **Step 1: `display_composites.vox`** (decision: `vox_value_display` spacing is the Vox surface form)

```vox
// ---
// title: "Display of composite values"
// description: "print/str of list, object, tuple, Option and Result produce the same text under the interpreter and the native lane."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, print, str, list, object, tuple, Option, Result]
// training_eligible: true
// difficulty: beginner
// ---
// EXPECT: [1, 2, 3]
// EXPECT: {a: 1, b: two}
// EXPECT: (1, two)
// EXPECT: Some(3)
// EXPECT: None
// EXPECT: Ok(1)
// EXPECT: [1, 2, 3]|{a: 1, b: two}
// EXPECT: ok
fn main() to str {
    print([1, 2, 3])
    print({a: 1, b: "two"})
    print((1, "two"))
    print(Some(3))
    let nothing: Option[int] = None
    print(nothing)
    let good: Result[int, str] = Ok(1)
    print(good)
    print(str([1, 2, 3]) + "|" + str({a: 1, b: "two"}))
    return "ok"
}
```

- [ ] **Step 2: `float_formatting.vox`**

```vox
// ---
// title: "Float formatting parity"
// description: "Whole floats, repeating fractions and IEEE-754 round-off print identically under the interpreter and the native lane."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, print, str, float]
// training_eligible: true
// difficulty: beginner
// ---
// EXPECT: 1
// EXPECT: 0.30000000000000004
// EXPECT: 0.3333333333333333
// EXPECT: -0.5
// EXPECT: 1
// EXPECT: 2.5
// EXPECT: ok
fn main() to str {
    print(1.0)
    print(0.1 + 0.2)
    print(1.0 / 3.0)
    print(0.0 - 0.5)
    print(str(1.0))
    print(str(2.5))
    return "ok"
}
```

- [ ] **Step 3: `object_field_order.vox`** — encodes **insertion order**. This is the maintainer decision recorded in spec §3.3(b); if the decision goes to key-sorted, reorder the EXPECT lines to `alpha/middle/zeta` and make Task 2 sort `Object` under interp instead of enabling `preserve_order` natively.

```vox
// ---
// title: "Object field iteration order"
// description: "Iterating an object literal yields its fields in a single defined order on both execution tiers."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, for, object, print]
// training_eligible: true
// difficulty: beginner
// ---
// EXPECT: zeta=1
// EXPECT: alpha=2
// EXPECT: middle=3
// EXPECT: ok
fn main() to str {
    let o = {zeta: 1, alpha: 2, middle: 3}
    for k, v in o { print(k + "=" + str(v)) }
    return "ok"
}
```

- [ ] **Step 4: `glob_and_listdir_order.vox`** (also exercises `.sorted()`, which has no codegen arm until Task 2)

```vox
// ---
// title: "Directory enumeration order"
// description: "fs.glob and fs.list_dir return entries in sorted order, identically on both execution tiers and across filesystems."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, fs, match, for, print]
// training_eligible: true
// difficulty: intermediate
// ---
// vox:caps fs
// EXPECT: sorted_glob: true
// EXPECT: sorted_list_dir: true
// EXPECT: ok
fn main() to str {
    let g = match fs.glob("examples/golden/*.vox") { Ok(v) => v, Error(e) => [] }
    print("sorted_glob: " + str(g is g.sorted()))
    let d = match fs.list_dir("examples/golden") { Ok(v) => v, Error(e) => [] }
    print("sorted_list_dir: " + str(d is d.sorted()))
    return "ok"
}
```

- [ ] **Step 5: `int_overflow_boundary.vox`** (needs Task 2's `overflow-checks = true`)

```vox
// ---
// title: "Integer overflow halts"
// description: "i64 overflow is a fatal error on both execution tiers rather than silently wrapping on one of them."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, int, arithmetic]
// training_eligible: true
// difficulty: intermediate
// ---
// EXPECT-EXIT: nonzero-both
// EXPECT: near_max: 9223372036854775806
fn main() to str {
    let near_max = 9223372036854775806
    print("near_max: " + str(near_max))
    let overflowed = near_max + 10
    print("UNREACHABLE: " + str(overflowed))
    return "ok"
}
```

- [ ] **Step 6: `division_by_zero.vox`**

```vox
// ---
// title: "Integer division by zero halts"
// description: "Dividing by zero terminates the run on both execution tiers; neither returns a value."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, int, arithmetic]
// training_eligible: true
// difficulty: beginner
// ---
// EXPECT-EXIT: nonzero-both
// EXPECT: before
fn main() to str {
    print("before")
    let zero = 0
    let boom = 10 / zero
    print("UNREACHABLE: " + str(boom))
    return "ok"
}
```

- [ ] **Step 7: `argv_shape.vox`** (shape, not value — `env.args()[0]` is a path)

```vox
// ---
// title: "Script argv shape"
// description: "env.args() is the script's own argument vector — program name at index 0, no interpreter flags — on both execution tiers."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, env, @test, assert]
// training_eligible: true
// difficulty: intermediate
// ---
// EXPECT: argc: 1
// EXPECT: ok
@test
fn test_argv_has_no_interpreter_flags() to Unit {
    let argv = env.args()
    assert(len(argv) >= 1)
    assert(argv.contains("--mode") is false)
    assert(argv.contains("run") is false)
}

fn main() to str {
    print("argc: " + str(len(env.args())))
    return "ok"
}
```

- [ ] **Step 8: `crypto_hash_parity.vox`** — matches the **native** shapes (XXH3-128 → 32 hex; `vox-<16hex>-<16hex>` → 37 chars), since that is what parity has to reach. Stays red until the `vox-compiler → vox-crypto` edge is authorised (spec §3.3(a)); it is the golden that forces the decision.

```vox
// ---
// title: "Crypto hash and id parity"
// description: "crypto.hash_fast and crypto.hash_secure produce identical hex on both execution tiers; crypto.uuid produces the vox id shape."
// syntax_version: "0.5.0"
// status: golden
// category: example
// constructs: [fn, crypto, print, str]
// training_eligible: true
// difficulty: beginner
// ---
// EXPECT: fast_len: 32
// EXPECT: secure_len: 64
// EXPECT: id_len: 37
// EXPECT: ok
fn main() to str {
    print("fast_len: " + str(len(crypto.hash_fast("abc"))))
    print("secure_len: " + str(len(crypto.hash_secure("abc"))))
    print("id_len: " + str(len(crypto.uuid())))
    return "ok"
}
```

- [ ] **Step 9: Run the gate, record, commit.** `cargo test -q -p vox-integration-tests --test golden_differential_gate -- --ignored 2>&1 | tail -60` — expected **red** on 1, 3, 4, 5, 6, 7, 8 today. Paste the failure list into the commit body: it is the measured drift. `git commit -m "test(golden): eight goldens that make the differential gate mean something"`.

---

## Task 2: Close the measured drift (parity fixes)

**Files:** `crates/vox-cli/src/commands/runtime/run/backend/native.rs:67-73` (profile); `crates/vox-compiler/src/eval/builtins.rs` (`vox_value_display` ~2591-2617, `print` ~2363, `fs.glob` 1154-1175, `fs.list_dir` 1138-1152, `env.args` 1261-1264, `time` 1237-1251); `crates/vox-actor-runtime/src/builtins/mod.rs` (`vox_display`, `vox_fs_list_dir` ~1725, `vox_process_cwd`, `vox_secrets_resolve`); `crates/vox-compiler/src/builtin_registry.rs:848+`; `crates/vox-codegen/src/codegen_rust/emit/{stmt_expr.rs:1192,method_emit.rs:815-830,workflow.rs:39-52}`, `pipeline.rs:338-397`; `crates/vox-compiler/tests/eval_typeck_parity_test.rs`.

Findings: `vox_crypto::hash` does not exist and `vox-compiler` has no `vox-crypto` edge (executability #1) — **`crypto` is deferred**; `vox_hash_fast` is XXH3-128 and `vox_uuid` is `vox-…` (#2); `vox_secrets_resolve` must mirror `builtins.rs:1411-1430` exactly (#3); `vox_json_render` returns `Result<String,String>` and takes `&VoxJson` (#4); plus the parity list in spec §3.3.

- [ ] **Step 1: Write the failing tests** — append to `eval_typeck_parity_test.rs`:

```rust
/// Spec §3.3: every entry is a script that works on one tier and fails on the other.
/// Add an entry only with a reason. `crypto` waits on the vox-compiler -> vox-crypto edge
/// (maintainer decision, spec §3.3(a)); `object_iteration_order` waits on §3.3(b).
const KNOWN_TIER_ASYMMETRIES: &[(&str, &str)] = &[
    ("crypto.*", "needs vox-compiler -> vox-crypto edge; native is XXH3-128 / vox-<hex> id, not BLAKE3 / UUIDv4"),
    ("object_iteration_order", "interp = insertion, native = serde_json key-sorted; decision pending, golden object_field_order.vox forces it"),
    ("log.*", "generated script binary installs no tracing subscriber; interp inherits the CLI's"),
];

#[test]
fn every_known_asymmetry_has_a_reason() {
    for (what, why) in KNOWN_TIER_ASYMMETRIES {
        assert!(!why.trim().is_empty(), "{what} has no reason");
    }
}

#[test]
fn registry_emits_every_eval_only_method() {
    use vox_compiler::builtin_registry::std_namespace_runtime_call;
    for (ns, m, args) in [
        ("time", "now", vec![]),
        ("json", "encode", vec!["x".to_string()]),
        ("json", "stringify", vec!["x".to_string()]),
        ("process", "cwd", vec![]),
        ("secrets", "resolve", vec!["\"K\"".to_string()]),
    ] {
        assert!(std_namespace_runtime_call(ns, m, &args).is_some(), "codegen has no emit for {ns}.{m}");
    }
}

#[test]
fn display_of_composites_matches_the_surface_form() {
    let v = run_probe(r#"pub fn main() { return str([1, 2]) + "|" + str({a: 1}) + "|" + str(Some(3)) + "|" + str(Ok(1)) }"#).unwrap();
    assert!(matches!(v, VoxValue::Str(ref s) if s == "[1, 2]|{a: 1}|Some(3)|Ok(1)"), "{v:?}");
}

#[test]
fn glob_is_sorted_and_propagates_errors() {
    let v = run_probe(r#"pub fn main() { return match fs.glob("examples/golden/*.vox") { Ok(xs) => xs is xs.sorted(), Error(e) => false } }"#).unwrap();
    assert!(matches!(v, VoxValue::Bool(true)), "{v:?}");
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p vox-compiler --test eval_typeck_parity_test 2>&1 | tail -20`. Expected: `registry_emits…` fails on `time.now`; `display_of_composites…` fails on `Some(3)` (prints `Option(Some(Int(3)))`); `glob_is_sorted…` may fail (order) or pass by luck — keep it.

- [ ] **Step 3: Native profile halts on overflow** — `native.rs:67-73`: in the generated `script-dev` profile set `overflow-checks = true`; do the same for the `VOX_SCRIPT_RELEASE` path. One line each. This makes goldens 5/6 pass natively without touching codegen.

- [ ] **Step 4: One display for both tiers.** In `eval/builtins.rs::vox_value_display` add arms: `Option(Some(v)) → "Some(" + display(v) + ")"`, `Option(None) → "None"`, `Result(Ok(v)) → "Ok(…)"`, `Result(Err(e)) → "Err(…)"`, `Tagged { name, fields } → name + "(" + fields joined ", " + ")"`; replace the `_ => format!("{v:?}")` catch-all with an explicit list so a new variant is a compile error. In `vox-actor-runtime/src/builtins/mod.rs` add `pub fn vox_display(v: &serde_json::Value) -> String` reproducing exactly that spacing (`[1, 2]`, `{a: 1, b: two}`, `(1, two)`) with a unit test per shape; route codegen's `("str", 1)` (`workflow.rs:39-52 as_string`) and `("print", n)` (`stmt_expr.rs:1192`) through it, and make `print` accept n args joined by a space to match `builtins.rs:2363-2372`. `as_string`'s `.expect("serde_json::to_value failed")` becomes a formatted `inf`/`nan` so `str(1.0/0.0)` does not panic natively.

- [ ] **Step 5: Registry arms** in `builtin_registry.rs::std_namespace_runtime_call`, matching the surrounding style (no leading `::`):

```rust
        ("time", "now") => Some("vox_actor_runtime::builtins::vox_now_ms()".to_string()),
        ("json", "encode" | "stringify") if args.len() == 1 => Some(format!(
            "vox_actor_runtime::builtins::vox_json_render(&({})).unwrap_or_default()", args[0]
        )),
        ("process", "cwd") => Some("vox_actor_runtime::builtins::vox_process_cwd()".to_string()),
        ("secrets", "resolve") if args.len() == 1 => Some(format!(
            "vox_actor_runtime::builtins::vox_secrets_resolve(({}).as_str())", args[0]
        )),
```

and in `vox-actor-runtime/src/builtins/mod.rs` (each with a test):

```rust
/// Parity with eval `process.cwd`.
pub fn vox_process_cwd() -> String {
    std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default()
}

/// Parity with eval `secrets.resolve` (eval/builtins.rs:1411-1430): FromStr on the name,
/// then `resolve_secret_with_context(id, "script")`. Never `env::var`.
pub fn vox_secrets_resolve(key: &str) -> Option<String> {
    let id: vox_secrets::SecretId = std::str::FromStr::from_str(key).ok()?;
    let resolved = vox_secrets::resolve_secret_with_context(id, "script");
    resolved.value.map(|v| v.expose_secret().to_string())
}
```

(Check `vox-actor-runtime` already depends on `vox-secrets` — `grep -n vox-secrets crates/vox-actor-runtime/Cargo.toml`; if not, this arm is deferred with a reason, not an edge.)

- [ ] **Step 6: Sorted, propagating enumeration on both tiers.** `eval/builtins.rs:1154-1175` (`glob`): collect into a `Vec`, `sort()`, and turn the `.ok()` swallow into `Result::Err` like `vox_fs_glob` (`mod.rs:325-337`). `list_dir` / `list_dir_detailed` in **both** `eval/builtins.rs:1138-1152` and `mod.rs:1725-1734`: `sort()`. Add a `.sorted()` arm to `emit/method_emit.rs` beside `sorted_by_key` (`:815-830`).

- [ ] **Step 7: `env.args` parity** — `eval/builtins.rs:1261-1264` returns `std::env::args()` of the `vox` process. Add `pub script_args: Vec<String>` to `Interpreter` (set by `run_interp` in Task 6) and make `env.args` return `[source_path] ++ script_args`, matching the native binary's argv shape (`backend/native.rs:114-116`).

- [ ] **Step 8: Faults exit 1 natively.** In `pipeline.rs:338-397` wrap the generated `main` body in `std::panic::catch_unwind`, print the panic payload to stderr, and `std::process::exit(1)` — so assert/unwrap/div-by-zero exit 1 on both tiers (interp already does via `EvalError` → anyhow). Also: `pipeline.rs:344-346` forces `non_unit_ret = None` for `async fn main` — add a typecheck error "async main cannot return a value" so both tiers refuse it rather than one dropping the value.

- [ ] **Step 9: Run everything** — `cargo test -q -p vox-compiler --test eval_typeck_parity_test && cargo test -q -p vox-actor-runtime builtins && cargo build -q -p vox-cli --bin vox && cargo test -q -p vox-integration-tests --test golden_differential_gate -- --ignored 2>&1 | tail -30`. Expected: goldens 1, 2, 4, 5, 6 green; 3, 7, 8 red **with the reasons in `KNOWN_TIER_ASYMMETRIES`** (7 turns green in Task 6 when args are threaded). Anything else red is new drift — add it to the list with a reason or fix it here.

- [ ] **Step 10: Commit** — `cargo fmt -p vox-compiler -p vox-actor-runtime -p vox-codegen -p vox-cli`; `git commit -m "fix(parity): close the measured drift between the interpreter and the native lane"`.

---

## Task 3: `CapabilitySet` — non-optional, canonical roots, one value per token

**Files:** Create `crates/vox-compiler/src/eval/caps.rs`; modify `eval/mod.rs:37,247` (field), `eval/builtins.rs:110-115,952-966` (param type + gate, behaviour unchanged this task), `eval/expr.rs:620`; **`crates/vox-cli/src/commands/run.rs:44-66` and `crates/vox-langtool/src/commands/run.rs:36`** (both assign a `HashSet` today — converting them here keeps the workspace building; revision 1 deferred them and left a three-commit broken window).

Findings (fs track #1, #5, #6, #9, #11; escape #10; executability #7, #8): roots must be canonicalised at parse; `|` is a shell pipe; `io:` grants nothing; negative `frozen=`; `caps: None` still a bypass; legacy `net` word must map to `http`; `db`/`repo` are pure; `crypto` needs a `random` axis; Windows verbatim prefixes and case.

- [ ] **Step 1: Write the failing tests** (create `caps.rs` with the test module first)

```rust
//! Receiver-imposed capabilities for the interpreter (spec §3.2). The grammar is a public
//! CLI surface once shipped; keep it small. Roots are canonicalised here so `allows_path`
//! compares like with like — on macOS `/tmp` is `/private/tmp`, on Windows `canonicalize`
//! yields `\\?\C:\…`, and comparing a canonical path against a raw root denies everything.

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn unmentioned_namespaces_are_denied() {
        let c = CapabilitySet::parse("env:ro").unwrap();
        assert!(c.allows_namespace("env"));
        for ns in ["fs", "process", "http", "secrets", "agentos", "crypto"] {
            assert!(!c.allows_namespace(ns), "{ns} must be denied");
        }
    }

    #[test]
    fn pure_namespaces_are_always_allowed() {
        let c = CapabilitySet::parse("").unwrap();
        for ns in ["path", "json", "csv", "toml", "yaml", "regex", "log", "db", "repo"] {
            assert!(c.allows_namespace(ns), "{ns} is pure and must not be gated");
        }
    }

    #[test]
    fn roots_are_canonicalised_at_parse_time() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.txt"), "A").unwrap();
        let c = CapabilitySet::parse(&format!("fs:rw={}", d.path().display())).unwrap();
        let canon = std::fs::canonicalize(d.path().join("a.txt")).unwrap();
        assert!(c.allows_path(&canon, true), "root not canonicalised: {c:?}");
    }

    #[test]
    fn fs_roots_scope_reads_and_writes_separately_and_repeat_the_token() {
        let ro = tempfile::tempdir().unwrap();
        let ro2 = tempfile::tempdir().unwrap();
        let rw = tempfile::tempdir().unwrap();
        let c = CapabilitySet::parse(&format!("fs:ro={},fs:ro={},fs:rw={}", ro.path().display(), ro2.path().display(), rw.path().display())).unwrap();
        let cro = std::fs::canonicalize(ro.path()).unwrap();
        let crw = std::fs::canonicalize(rw.path()).unwrap();
        assert!(c.allows_path(&cro.join("x"), false));
        assert!(!c.allows_path(&cro.join("x"), true));
        assert!(c.allows_path(&crw.join("x"), true));
        assert!(c.allows_path(&crw.join("x"), false), "rw roots satisfy reads");
        // Component boundary: /tmp/job must not admit /tmp/jobx.
        let sibling = crw.parent().unwrap().join(format!("{}x", crw.file_name().unwrap().to_string_lossy()));
        assert!(!c.allows_path(&sibling.join("y"), true));
    }

    #[test]
    fn a_missing_root_is_a_receiver_error_not_a_silent_deny() {
        assert!(CapabilitySet::parse("fs:ro=/definitely/not/here").is_err());
    }

    #[test]
    fn pipe_separator_is_gone_and_io_token_is_rejected() {
        assert!(CapabilitySet::parse("fs:ro=/a|/b").is_err());
        assert!(CapabilitySet::parse("io:allow").is_err());
    }

    #[test]
    fn env_has_read_and_write_levels() {
        let ro = CapabilitySet::parse("env:ro").unwrap();
        assert!(ro.allows_namespace("env") && !ro.allows_env_write());
        let rw = CapabilitySet::parse("env:rw").unwrap();
        assert!(rw.allows_env_write());
    }

    #[test]
    fn frozen_time_and_seeded_random() {
        let c = CapabilitySet::parse("time:frozen=1700000000000,random:seed=7").unwrap();
        assert_eq!(c.frozen_time_ms(), Some(1_700_000_000_000));
        assert_eq!(c.random_seed(), Some(7));
        assert!(c.allows_namespace("crypto"));
        assert!(CapabilitySet::parse("time:frozen=-1").is_err());
        let d = CapabilitySet::parse("random:deny").unwrap();
        assert!(!d.allows_namespace("crypto"));
    }

    #[test]
    fn deterministic_shorthand_expands() {
        let c = CapabilitySet::parse("deterministic").unwrap();
        assert_eq!(c.frozen_time_ms(), Some(0));
        assert_eq!(c.random_seed(), Some(0));
        assert!(!c.allows_namespace("process") && !c.allows_namespace("http") && !c.allows_namespace("env"));
    }

    #[test]
    fn net_and_http_are_one_namespace() {
        assert!(CapabilitySet::parse("net:allow").unwrap().allows_namespace("http"));
        assert!(CapabilitySet::parse("http:allow").unwrap().allows_namespace("http"));
    }

    #[test]
    fn legacy_directive_maps_words_and_is_unscoped() {
        let c = CapabilitySet::from_legacy_directive(&["fs".into(), "subprocess".into(), "net".into()]);
        assert!(c.allows_namespace("fs") && c.allows_namespace("io"));
        assert!(c.allows_namespace("process"));
        assert!(c.allows_namespace("http"), "`net` was the legacy word for http (vox-langtool fixture)");
        assert!(!c.allows_namespace("env"));
        assert!(c.allows_path(Path::new("/anything"), true));
    }

    #[test]
    fn developer_default_allows_everything() {
        let c = CapabilitySet::developer_default();
        for ns in ["fs", "io", "process", "env", "secrets", "http", "time", "agentos", "crypto"] {
            assert!(c.allows_namespace(ns));
        }
        assert!(c.allows_env_write() && c.allows_path(Path::new("/"), true));
    }

    #[test]
    fn from_roots_refuses_separator_characters() {
        let d = tempfile::tempdir().unwrap();
        assert!(CapabilitySet::from_roots(vec![], vec![d.path().to_path_buf()], &[]).is_ok());
        assert!(CapabilitySet::from_roots(vec![], vec![PathBuf::from("/tmp/a,b")], &[]).is_err());
    }

    #[test]
    fn bad_specs_name_the_offending_token() {
        for bad in ["fs", "fs:banana", "net:maybe", "time:frozen=abc", "nosuch:allow", "env:allow"] {
            let e = CapabilitySet::parse(bad).unwrap_err();
            assert!(e.to_string().contains(bad.split('=').next().unwrap()), "{bad} → {e}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_drive_letters_survive_the_ns_separator_and_case() {
        let d = tempfile::tempdir().unwrap();
        let c = CapabilitySet::parse(&format!("fs:rw={}", d.path().display())).unwrap();
        let canon = std::fs::canonicalize(d.path()).unwrap();
        assert!(c.allows_path(&canon.join("a.txt"), true));
        let upper = PathBuf::from(canon.to_string_lossy().to_ascii_uppercase());
        assert!(c.allows_path(&upper.join("a.txt"), true), "NTFS is case-insensitive");
    }
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p vox-compiler --lib eval::caps 2>&1 | tail -5` → compile error.

- [ ] **Step 3: Implement** (prepend to `caps.rs`; `thiserror` and `tempfile` are already deps of `vox-compiler` — `Cargo.toml:24,65`)

```rust
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

/// Never gated. `path.resolve` is the one method in a pure namespace that touches disk;
/// the fs arm gates it as a read (Task 5). `db`/`repo` are in-memory stores.
pub const PURE: &[&str] = &["path", "json", "csv", "toml", "yaml", "regex", "log", "db", "repo"];
/// Gated. `io` is here for classification only — its authority comes from `fs:`.
/// `crypto` is gated by the `random` axis.
pub const GATED: &[&str] = &["fs", "io", "process", "env", "secrets", "http", "time", "agentos", "crypto"];

pub fn is_classified(ns: &str) -> bool {
    PURE.contains(&ns) || GATED.contains(&ns)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    allowed: BTreeSet<String>,
    env_write: bool,
    /// `None` = unscoped (developer default / legacy directive).
    fs_ro: Option<Vec<PathBuf>>,
    fs_rw: Option<Vec<PathBuf>>,
    frozen_time_ms: Option<i64>,
    random_seed: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid --caps token {token:?}: {why}")]
pub struct CapsParseError {
    pub token: String,
    pub why: &'static str,
}

impl CapabilitySet {
    fn restrictive() -> Self {
        Self { allowed: BTreeSet::new(), env_write: false, fs_ro: Some(Vec::new()), fs_rw: Some(Vec::new()), frozen_time_ms: None, random_seed: None }
    }

    pub fn parse(spec: &str) -> Result<Self, CapsParseError> {
        let mut out = Self::restrictive();
        for tok in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let err = |why| CapsParseError { token: tok.to_string(), why };
            if tok == "deterministic" {
                out.frozen_time_ms = Some(0);
                out.random_seed = Some(0);
                out.allowed.insert("time".into());
                out.allowed.insert("crypto".into());
                continue;
            }
            let (ns, rest) = tok.split_once(':').ok_or(err("expected ns:value"))?;
            match (ns, rest) {
                ("fs", r) if r.starts_with("ro=") || r.starts_with("rw=") => {
                    let raw = &r[3..];
                    if raw.is_empty() { return Err(err("fs needs a directory")); }
                    if raw.contains('|') { return Err(err("one directory per token; repeat fs:ro=/fs:rw=")); }
                    let root = std::fs::canonicalize(raw).map_err(|_| err("fs root does not exist or is unreadable"))?;
                    let target = if r.starts_with("ro=") { &mut out.fs_ro } else { &mut out.fs_rw };
                    target.get_or_insert_with(Vec::new).push(root);
                    out.allowed.insert("fs".into());
                    out.allowed.insert("io".into());
                }
                ("fs", _) => return Err(err("fs takes ro=<dir> or rw=<dir>")),
                ("io", _) => return Err(err("io is covered by fs:ro= / fs:rw=")),
                ("net" | "http", "allow") => { out.allowed.insert("http".into()); }
                ("net" | "http", "none") => {}
                ("net" | "http", _) => return Err(err("net takes none|allow")),
                ("env", "ro") => { out.allowed.insert("env".into()); }
                ("env", "rw") => { out.allowed.insert("env".into()); out.env_write = true; }
                ("env", "none") => {}
                ("env", _) => return Err(err("env takes none|ro|rw")),
                ("time", "real") => { out.allowed.insert("time".into()); }
                ("time", r) if r.starts_with("frozen=") => {
                    let ms: i64 = r[7..].parse().map_err(|_| err("frozen= needs integer ms"))?;
                    if ms < 0 { return Err(err("frozen= must be non-negative")); }
                    out.frozen_time_ms = Some(ms);
                    out.allowed.insert("time".into());
                }
                ("time", _) => return Err(err("time takes real|frozen=<ms>")),
                ("random", r) if r.starts_with("seed=") => {
                    out.random_seed = Some(r[5..].parse().map_err(|_| err("seed= needs a u64"))?);
                    out.allowed.insert("crypto".into());
                }
                ("random", "deny") => {}
                ("random", _) => return Err(err("random takes seed=<u64>|deny")),
                ("process" | "secrets" | "agentos", "allow") => { out.allowed.insert(ns.into()); }
                ("process" | "secrets" | "agentos", "none") => {}
                ("process" | "secrets" | "agentos", _) => return Err(err("expected none|allow")),
                _ => return Err(err("unknown namespace")),
            }
        }
        Ok(out)
    }

    /// The mesh executor's constructor: roots are typed, so a directory containing a
    /// grammar separator is refused rather than encoded into a string.
    pub fn from_roots(ro: Vec<PathBuf>, rw: Vec<PathBuf>, extra: &[&str]) -> Result<Self, CapsParseError> {
        let mut spec = Vec::new();
        for (kind, dirs) in [("ro", &ro), ("rw", &rw)] {
            for d in dirs {
                let s = d.display().to_string();
                if s.contains(',') || s.contains('=') || s.contains('|') {
                    return Err(CapsParseError { token: s, why: "directory contains a caps separator" });
                }
                spec.push(format!("fs:{kind}={s}"));
            }
        }
        spec.extend(extra.iter().map(|s| s.to_string()));
        Self::parse(&spec.join(","))
    }

    /// Serialise for the child's `--caps`. Round-trips through `parse`.
    pub fn to_spec(&self) -> String {
        let mut parts = Vec::new();
        for d in self.fs_ro.iter().flatten() { parts.push(format!("fs:ro={}", d.display())); }
        for d in self.fs_rw.iter().flatten() { parts.push(format!("fs:rw={}", d.display())); }
        if self.allowed.contains("http") { parts.push("net:allow".into()); }
        if self.allowed.contains("process") { parts.push("process:allow".into()); }
        if self.allowed.contains("env") { parts.push(if self.env_write { "env:rw".into() } else { "env:ro".into() }); }
        if self.allowed.contains("secrets") { parts.push("secrets:allow".into()); }
        if self.allowed.contains("agentos") { parts.push("agentos:allow".into()); }
        match self.frozen_time_ms { Some(ms) => parts.push(format!("time:frozen={ms}")), None if self.allowed.contains("time") => parts.push("time:real".into()), None => {} }
        match self.random_seed { Some(s) => parts.push(format!("random:seed={s}")), None => {} }
        parts.join(",")
    }

    pub fn from_legacy_directive(words: &[String]) -> Self {
        let mut allowed = BTreeSet::new();
        for w in words {
            match w.as_str() {
                "fs" => { allowed.insert("fs".into()); allowed.insert("io".into()); }
                "process" | "subprocess" => { allowed.insert("process".into()); }
                "net" | "http" => { allowed.insert("http".into()); }
                other => { allowed.insert(other.into()); }
            }
        }
        let env_write = allowed.contains("env");
        Self { allowed, env_write, fs_ro: None, fs_rw: None, frozen_time_ms: None, random_seed: None }
    }

    pub fn developer_default() -> Self {
        Self { allowed: GATED.iter().map(|s| s.to_string()).collect(), env_write: true, fs_ro: None, fs_rw: None, frozen_time_ms: None, random_seed: None }
    }

    pub fn allows_namespace(&self, ns: &str) -> bool {
        PURE.contains(&ns) || self.allowed.contains(ns)
    }

    pub fn allows_env_write(&self) -> bool {
        self.env_write
    }

    /// `path` MUST be canonical (see `fs_resolve_allowed`, Task 5). rw roots satisfy reads.
    pub fn allows_path(&self, path: &Path, write: bool) -> bool {
        if !self.allows_namespace("fs") {
            return false;
        }
        let roots = if write { &self.fs_rw } else { &self.fs_ro };
        match roots {
            None => true,
            Some(rs) => {
                let extra = if write { None } else { self.fs_rw.as_deref() };
                rs.iter().chain(extra.into_iter().flatten()).any(|r| is_under(path, r))
            }
        }
    }

    pub fn frozen_time_ms(&self) -> Option<i64> { self.frozen_time_ms }
    pub fn random_seed(&self) -> Option<u64> { self.random_seed }
}

fn is_under(path: &Path, root: &Path) -> bool {
    let p: Vec<Component> = path.components().collect();
    let r: Vec<Component> = root.components().collect();
    p.len() >= r.len() && p[..r.len()].iter().zip(&r).all(|(a, b)| component_eq(a, b))
}

/// Both sides are canonical, so only case can differ — and NTFS/APFS-default are insensitive.
fn component_eq(a: &Component, b: &Component) -> bool {
    #[cfg(windows)]
    {
        if let (Component::Normal(x), Component::Normal(y)) = (a, b) {
            return x.to_string_lossy().eq_ignore_ascii_case(&y.to_string_lossy());
        }
    }
    a == b
}
```

- [ ] **Step 4: Make `caps` non-optional and convert both call sites.** `eval/mod.rs:37` → `pub caps: caps::CapabilitySet,`; `:247` → `caps: caps::CapabilitySet::developer_default(),`; add `pub mod caps;`. `builtins.rs:110-115` → `caps: &crate::eval::caps::CapabilitySet`; gate at `:952-966` becomes `if let Some(ns_str) = ns && !caps.allows_namespace(ns_str) { println!(…); return Some(VoxValue::Null); }` (fatal denial is Task 4). `expr.rs:620` → `&interp.caps`. `vox-cli/src/commands/run.rs:44-66`: keep the first-line parse, then `interpreter.caps = if has_caps_directive { CapabilitySet::from_legacy_directive(&legacy_words) } else { CapabilitySet::developer_default() };`. Same at `vox-langtool/src/commands/run.rs:36`. Grep for any other `.caps = Some(` — there are exactly these two.

- [ ] **Step 5: Run** — `cargo test -q -p vox-compiler --lib eval::caps && cargo test -q -p vox-compiler && cargo build -q -p vox-cli -p vox-langtool && cargo test -q -p vox-langtool --test integration run_caps_directive 2>&1 | tail -5`. The `vox-langtool` fixture (`tests/integration.rs:107-117`, `// vox:caps net fs`) must still exit 0.

- [ ] **Step 6: Commit** — `cargo fmt -p vox-compiler -p vox-cli -p vox-langtool`; `git commit -m "feat(eval): CapabilitySet — non-optional, canonical roots, one value per token"`.

---

## Task 4: Denial is fatal and every side-effecting entry point is gated

**Files:** `eval/value.rs:82` (sentinel); `eval/mod.rs` (`EvalError::CapabilityDenied`, `eval_depth`, `exit_commands` field, import gate at `:377-392`); `eval/builtins.rs:952-966` (gate), `:1396-1409` (`path.resolve`), `:1625-1650` (`register_exit_command`), `:16-52` (global static → field); `eval/expr.rs:618-632`; `eval/env.rs` (`Scope::bindings`); create `crates/vox-compiler/tests/caps_enforcement_test.rs`.

Findings (escape #2, #8, #9, #11; fs #8, #10, #11): `import` is an ungated read+execute; `db`/`repo` are pure (test dropped, gate not added); `path.resolve` hits disk; `http` lives only under `std.http`; `repo.status` does not exist (`snapshot`/`changes`/`undo`); the source-parsing namespace test is rustfmt-fragile; `register_exit_command` queues past the gate and the queue is a process-global static.

- [ ] **Step 1: Write the failing tests**

```rust
//! Spec §3.2 items 2–4: denial is fatal; every side-effecting entry point is gated.
use vox_compiler::eval::caps::CapabilitySet;
use vox_compiler::eval::value::VoxValue;
use vox_compiler::eval::{EvalError, Interpreter};
use vox_compiler::{hir, lexer, parser};

fn run_with(caps: CapabilitySet, src: &str) -> Result<VoxValue, EvalError> {
    let tokens = lexer::lex(src);
    let module = parser::parse_script(tokens).expect("parse");
    let lowered = hir::lower::lower_module(&module);
    let mut interp = Interpreter::new(1_000_000);
    interp.caps = caps;
    interp.run_module(&lowered)?;
    interp.call("main", vec![])
}

fn denied(r: &Result<VoxValue, EvalError>, ns: &str) -> bool {
    matches!(r, Err(EvalError::CapabilityDenied { ns: n, .. }) if n == ns)
}

#[test]
fn denied_fs_read_is_fatal_and_nothing_after_it_runs() {
    let r = run_with(CapabilitySet::parse("env:ro").unwrap(),
        r#"pub fn main() { let s = fs.read("/etc/hosts"); print("MUST NOT PRINT"); return 1 }"#);
    match r {
        Err(EvalError::CapabilityDenied { ns, method }) => { assert_eq!(ns, "fs"); assert_eq!(method, "read"); }
        other => panic!("expected CapabilityDenied, got {other:?}"),
    }
}

#[test]
fn http_is_gated_under_std() {
    let r = run_with(CapabilitySet::parse("").unwrap(), r#"pub fn main() { return std.http.get_text("http://127.0.0.1:9/") }"#);
    assert!(denied(&r, "http"), "{r:?}");
}

#[test]
fn repo_is_pure_and_allowed_without_caps() {
    let r = run_with(CapabilitySet::parse("").unwrap(), r#"pub fn main() { return repo.snapshot() }"#);
    assert!(r.is_ok(), "repo is an in-memory store: {r:?}");
}

#[test]
fn path_resolve_is_an_fs_read() {
    let r = run_with(CapabilitySet::parse("").unwrap(), r#"pub fn main() { return path.resolve("/etc/hosts") }"#);
    assert!(denied(&r, "fs"), "{r:?}");
}

#[test]
fn env_set_needs_rw() {
    let r = run_with(CapabilitySet::parse("env:ro").unwrap(), r#"pub fn main() { return env.set("X", "1") }"#);
    assert!(denied(&r, "env"), "{r:?}");
}

#[test]
fn register_exit_command_is_gated_at_queue_time() {
    let r = run_with(CapabilitySet::parse("").unwrap(), r#"pub fn main() { return process.register_exit_command("true", []) }"#);
    assert!(denied(&r, "process"), "{r:?}");
}

#[test]
fn import_outside_the_fs_roots_is_denied_before_main_runs() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("lib.vox"), "pub fn leak() { return 42 }").unwrap();
    let inside = tempfile::tempdir().unwrap();
    let main = inside.path().join("main.vox");
    std::fs::write(&main, format!("import \"{}\" as l\npub fn main() {{ return l.leak() }}", outside.path().join("lib.vox").display())).unwrap();
    let src = std::fs::read_to_string(&main).unwrap();
    let tokens = lexer::lex(&src);
    let module = parser::parse_script(tokens).unwrap();
    let lowered = hir::lower::lower_module(&module);
    let mut interp = Interpreter::new(1_000_000);
    interp.caps = CapabilitySet::parse(&format!("fs:ro={}", inside.path().display())).unwrap();
    interp.set_source_path(std::fs::canonicalize(&main).unwrap());
    let r = interp.run_module(&lowered);
    assert!(matches!(r, Err(EvalError::CapabilityDenied { ref ns, ref method }) if ns == "fs" && method == "import"), "{r:?}");
}

#[test]
fn a_default_interpreter_is_developer_default_not_ungated() {
    let i = Interpreter::new(1000);
    assert_eq!(i.caps, CapabilitySet::developer_default());
}

#[test]
fn every_seeded_namespace_is_classified() {
    fn walk(v: &VoxValue, out: &mut std::collections::BTreeSet<String>) {
        if let VoxValue::Object(fields) = v {
            for (k, fv) in fields.iter() {
                if k == "__namespace__" {
                    if let VoxValue::Str(ns) = fv { out.insert(ns.clone()); }
                } else {
                    walk(fv, out);
                }
            }
        }
    }
    let i = Interpreter::new(1000);
    let mut names = std::collections::BTreeSet::new();
    for (_, v) in i.scope.bindings() { walk(v, &mut names); }
    assert!(names.len() >= 16, "only found {names:?}");
    for ns in &names {
        assert!(vox_compiler::eval::caps::is_classified(ns), "namespace `{ns}` is seeded but in neither caps::GATED nor caps::PURE");
    }
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p vox-compiler --test caps_enforcement_test 2>&1 | tail -20`.

- [ ] **Step 3: Implement.**
  - `value.rs:82`: add `_Denied(String)` after `_Panic`; treat like `_Panic` in every `match` the compiler flags.
  - `mod.rs`: `EvalError::CapabilityDenied { ns: String, method: String }`.
  - `builtins.rs:952-966` gate → `if let Some(ns_str) = ns && !caps.allows_namespace(ns_str) { return Some(VoxValue::_Denied(format!("{ns_str}.{method}"))); }` (no `println!`, no method allowlist). `env.set` arm additionally checks `caps.allows_env_write()`.
  - `expr.rs:618-632`: `_Denied(what)` → split on `.` → `Err(EvalError::CapabilityDenied { ns, method })`.
  - `builtins.rs:1396-1409` `path.resolve`: guard via Task 5's `fs_resolve_allowed(caps, &raw, false)`; denial names `fs.resolve`. Add a comment next to `PURE` in `caps.rs` naming this exception.
  - `builtins.rs:1625` `register_exit_command`: `if !caps.allows_namespace("process") { return Some(VoxValue::_Denied("process.register_exit_command".into())); }`.
  - **Exit commands off the global static.** Move the `OnceLock<Mutex<Vec<…>>>` at `builtins.rs:16` to `pub exit_commands: Vec<(String, Vec<String>)>` on `Interpreter`; `vox_flush_exit_commands()` becomes `Interpreter::flush_exit_commands(&mut self)`; `ensure_signal_handler` (`:21-52`) is installed only by `run_interp` (Task 6), never by an embedder. Update `run.rs:88`.
  - **Import gate**, `mod.rs::resolve_local_file_import` after the `canonicalize` at `:377`:
    ```rust
    if !self.caps.allows_path(&canonical, false) {
        return Err(EvalError::CapabilityDenied { ns: "fs".into(), method: "import".into() });
    }
    ```
  - `env.rs`: `pub fn bindings(&self) -> impl Iterator<Item = (&String, &VoxValue)> { self.frames.iter().rev().flat_map(|f| f.iter()) }` (adapt to the frame type at `env.rs:13`).
  - Delete the `mod.rs:568-569` comment about repo not consulting caps (repo is pure now; the comment is moot).

- [ ] **Step 4: Run** — `cargo test -q -p vox-compiler --test caps_enforcement_test && cargo test -q -p vox-compiler 2>&1 | tail -5`.

- [ ] **Step 5: Mutation-verify twice.** (a) Gate: `perl -0pi -e 's/return Some\(VoxValue::_Denied\(format!\("\{ns_str\}\.\{method\}"\)\)\);/let _ = (ns_str, method);/' crates/vox-compiler/src/eval/builtins.rs` → `denied_fs_read_is_fatal…` MUST FAIL; `git checkout -- …`; `grep -c '_Denied(format' …` → 1. (b) Import gate: comment out the `allows_path` check in `resolve_local_file_import` → `import_outside_the_fs_roots…` MUST FAIL; restore; grep. Record both.

- [ ] **Step 6: Commit** — `git commit -m "feat(eval): capability denial is fatal; import, path.resolve and exit commands are gated"`.

---

## Task 5: Filesystem scoping, frozen time, seeded random, depth bound

**Files:** `eval/builtins.rs` (`fs` arm 970-1236, `io` 1897-1924, `time` 1237-1251); `eval/mod.rs` (`eval_depth`), `eval/expr.rs:28`, the parser's expression descent (`crates/vox-compiler/src/parser/descent/` — find the recursive `parse_expr`); `crates/vox-compiler/tests/caps_enforcement_test.rs` (append).

Findings (fs #2, #3, #7; escape #1, #3): the check must return the resolved path and the syscall must use it; `""`/`.`/`..` must be denied not degraded; the real fs surface is 19 methods and revision 1's list named two that do not exist and missed six; no recursion bound anywhere.

- [ ] **Step 1: Write the failing tests** (append)

```rust
#[test]
fn every_fs_method_that_takes_a_path_is_scoped() {
    let d = tempfile::tempdir().unwrap();
    let inside = d.path().join("in"); std::fs::create_dir_all(&inside).unwrap();
    let outside = d.path().join("out"); std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("s.txt"), "S").unwrap();
    let caps = CapabilitySet::parse(&format!("fs:rw={}", inside.display())).unwrap();
    let f = outside.join("s.txt");
    for (m, arg) in [
        ("read", &f), ("read_file", &f), ("read_to_string", &f), ("read_bytes", &f), ("canonicalize", &f),
        ("exists", &f), ("is_file", &f), ("is_dir", &outside), ("stat", &f),
        ("list_dir", &outside), ("list_dir_detailed", &outside), ("walk", &outside), ("list_recursive", &outside),
        ("remove", &f), ("remove_dir_all", &outside), ("mkdir", &outside.join("new")),
    ] {
        let r = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.{m}("{}") }}"#, arg.display()));
        assert!(denied(&r, "fs"), "fs.{m} ungated: {r:?}");
    }
    let r = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.write("{}", "x") }}"#, f.display()));
    assert!(denied(&r, "fs"), "fs.write ungated: {r:?}");
    let r = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.copy("{}", "{}") }}"#, f.display(), inside.join("c").display()));
    assert!(denied(&r, "fs"), "fs.copy source ungated: {r:?}");
    let r = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.glob("{}/*") }}"#, outside.display()));
    assert!(denied(&r, "fs"), "fs.glob prefix ungated: {r:?}");
    for m in ["open", "save"] {
        let r = run_with(caps.clone(), &format!(r#"pub fn main() {{ return io.{m}("{}") }}"#, f.display()));
        assert!(denied(&r, "fs") || denied(&r, "io"), "io.{m} ungated: {r:?}");
    }
    assert!(outside.join("s.txt").exists(), "a write escaped the sandbox");
}

#[test]
fn symlink_escape_is_denied_and_the_op_uses_the_checked_path() {
    let d = tempfile::tempdir().unwrap();
    let allowed = d.path().join("ok"); std::fs::create_dir_all(&allowed).unwrap();
    std::fs::write(allowed.join("a.txt"), "A").unwrap();
    std::fs::write(d.path().join("secret.txt"), "S").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(d.path().join("secret.txt"), allowed.join("link.txt")).unwrap();
    let caps = CapabilitySet::parse(&format!("fs:ro={}", allowed.display())).unwrap();
    let ok = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.read("{}") }}"#, allowed.join("a.txt").display()));
    assert!(matches!(ok, Ok(VoxValue::Str(ref s)) if s == "A"), "{ok:?}");
    #[cfg(unix)]
    {
        let via_link = run_with(caps, &format!(r#"pub fn main() {{ return fs.read("{}") }}"#, allowed.join("link.txt").display()));
        assert!(denied(&via_link, "fs"), "symlink escape: {via_link:?}");
    }
}

#[test]
fn degenerate_paths_are_denied_not_degraded_to_the_parent() {
    let d = tempfile::tempdir().unwrap();
    let caps = CapabilitySet::parse(&format!("fs:rw={}", d.path().display())).unwrap();
    for raw in ["", "..", "."] {
        let r = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.write("{raw}", "x") }}"#));
        assert!(denied(&r, "fs"), "{raw:?}: {r:?}");
    }
}

#[test]
fn frozen_time_and_seeded_random_are_what_the_receiver_said() {
    let r = run_with(CapabilitySet::parse("time:frozen=42").unwrap(), r#"pub fn main() { return time.now_ms() }"#);
    assert!(matches!(r, Ok(VoxValue::Int(42))), "{r:?}");
}

#[test]
fn deep_recursion_is_a_limit_error_not_a_crash() {
    let r = run_with(CapabilitySet::developer_default(), r#"fn f(n: int) to int { return f(n + 1) } pub fn main() { return f(0) }"#);
    assert!(matches!(r, Err(EvalError::RecursionLimitExceeded)), "{r:?}");
}

#[test]
fn deeply_nested_source_is_a_parse_error_not_a_crash() {
    let src = format!("pub fn main() {{ return {}1{} }}", "(".repeat(200_000), ")".repeat(200_000));
    let tokens = lexer::lex(&src);
    assert!(parser::parse_script(tokens).is_err(), "parser must bound nesting");
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p vox-compiler --test caps_enforcement_test 2>&1 | tail -15` (the nesting test may SIGSEGV the test binary today — that *is* the failure).

- [ ] **Step 3: One resolver, used by every arm**

```rust
/// Resolve `raw` the way the OS will (symlinks included) and check it against the caller's
/// fs roots. Returns the resolved path: callers MUST use it for the syscall so the check and
/// the operation name the same file. Non-existent targets resolve through the parent and
/// need a real file name — `""`, `.` and `..` are denied, not degraded.
///
/// Residual: not `openat`-based; directory components resolve twice. A script holding
/// `process:allow` can race the final component (spec §3.2 item 5).
fn fs_resolve_allowed(caps: &crate::eval::caps::CapabilitySet, raw: &str, write: bool) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(raw);
    let canon = match std::fs::canonicalize(p) {
        Ok(cp) => cp,
        Err(_) => {
            let name = p.file_name()?;
            if name == "." || name == ".." { return None; }
            let parent = p.parent().unwrap_or(std::path::Path::new("."));
            std::fs::canonicalize(parent).ok()?.join(name)
        }
    };
    caps.allows_path(&canon, write).then_some(canon)
}

/// The fs surface, verified against the arm at builtins.rs:970-1236. Unknown methods are
/// denied by the guard below, so a new method cannot ship ungated.
const FS_PATH_METHODS: &[(&str, bool /* write */)] = &[
    ("read", false), ("read_file", false), ("read_to_string", false), ("read_bytes", false),
    ("canonicalize", false), ("exists", false), ("is_file", false), ("is_dir", false), ("stat", false),
    ("list_dir", false), ("list_dir_detailed", false), ("walk", false), ("list_recursive", false), ("glob", false),
    ("write", true), ("write_file", true), ("write_to_file", true), ("remove", true),
    ("remove_dir_all", true), ("mkdir", true),
];
```

At the **head** of the `Some("fs")` arm, before `match method`:

```rust
            if !caps.allows_namespace("fs") {
                return Some(VoxValue::_Denied(format!("fs.{method}")));
            }
            // Path-taking methods: resolve once, deny unknown methods, and shadow `args` so
            // every arm below operates on the checked path.
            let args = match method {
                "cwd" => args,                                   // takes no path; namespace check above suffices
                "copy" => {                                      // src read, dst write
                    let mut it = args.into_iter();
                    let (Some(VoxValue::Str(src)), Some(VoxValue::Str(dst))) = (it.next(), it.next()) else { return None };
                    let (Some(s), Some(d)) = (fs_resolve_allowed(caps, &src, false), fs_resolve_allowed(caps, &dst, true)) else {
                        return Some(VoxValue::_Denied("fs.copy".into()));
                    };
                    vec![VoxValue::Str(s.to_string_lossy().into()), VoxValue::Str(d.to_string_lossy().into())]
                }
                "glob" => {                                      // check the non-wildcard prefix as a read
                    let Some(VoxValue::Str(pat)) = args.first() else { return None };
                    let prefix = pat.split(['*', '?', '[']).next().unwrap_or("");
                    let dir = if prefix.is_empty() { "." } else { prefix.rsplit_once('/').map(|(d, _)| d).unwrap_or(".") };
                    if fs_resolve_allowed(caps, dir, false).is_none() {
                        return Some(VoxValue::_Denied("fs.glob".into()));
                    }
                    args
                }
                m => match FS_PATH_METHODS.iter().find(|(n, _)| *n == m) {
                    Some((_, write)) => {
                        let mut it = args.into_iter();
                        let Some(VoxValue::Str(raw)) = it.next() else { return None };
                        let Some(p) = fs_resolve_allowed(caps, &raw, *write) else {
                            return Some(VoxValue::_Denied(format!("fs.{method}")));
                        };
                        std::iter::once(VoxValue::Str(p.to_string_lossy().into())).chain(it).collect()
                    }
                    None => return Some(VoxValue::_Denied(format!("fs.{method}"))),
                },
            };
```

`io.open`/`io.save` (`:1898/1908`): same shape with `false`/`true`. `walk` builds `format!("{root}/**/*")` — it now receives the resolved root.

- [ ] **Step 4: Frozen time and seeded random.** `time` arm: `if let Some(ms) = caps.frozen_time_ms() { return Some(VoxValue::Int(ms)); }` before the `SystemTime::now()` body (`builtins.rs:1243-1249`). Add `pub rng: Option<rand::rngs::StdRng>` to `Interpreter`, seeded from `caps.random_seed()` in `Interpreter::new`; any future randomness builtin draws from it when `Some` (nothing draws today — `crypto` is deferred).

- [ ] **Step 5: Depth bound.** `mod.rs`: `pub eval_depth: usize` and `pub const MAX_EVAL_DEPTH: usize = 1024;`; `EvalError::RecursionLimitExceeded`. In `expr.rs:28` after `track_step`: `interp.eval_depth += 1; if interp.eval_depth > MAX_EVAL_DEPTH { interp.eval_depth -= 1; return Err(EvalError::RecursionLimitExceeded); }` and decrement on every exit path (wrap the body in a closure or a guard struct). Parser: find the recursive expression descent in `crates/vox-compiler/src/parser/descent/` and add a `depth: usize` to the parser state with a 4 096 limit that returns a parse error.

- [ ] **Step 6: Run, then mutate the symlink check** — `cargo test -q -p vox-compiler --test caps_enforcement_test`; change `std::fs::canonicalize(p)` in `fs_resolve_allowed` to `Ok::<_, std::io::Error>(p.to_path_buf())` → `symlink_escape…` MUST FAIL; restore; grep → 1.

- [ ] **Step 7: Commit** — `git commit -m "feat(eval): scope every fs method after symlink resolution; frozen time; depth bound"`.

---

## Task 6: `vox run` flags, exit codes, memory ceiling — in the library crate

**Files:** Create `crates/vox-cli/src/mem_limit.rs`; modify `crates/vox-cli/src/lib.rs` (module + `#[global_allocator]`), `Cargo.toml` (`windows-sys` features — `libc` is already under `[target.'cfg(unix)'.dependencies]` at `:330`; do **not** add an unconditional one), `cli_args.rs:143-163`, `commands/run.rs:38-146` (both `run_interp` call sites `:92,:139`), **`cli_dispatch/lanes.rs:269` and `compilerd.rs:336`** (callers of `run()`); create `crates/vox-cli/tests/run_interp_limits.rs`.

Findings (limits #1–#6, #15; executability #9, #23): allocator must live in `lib.rs`; `std::process::exit` inside `alloc` deadlocks — use `_exit`/`TerminateProcess`; override `realloc`/`alloc_zeroed` for cost; `used()` racy under parallel tests; 77/78/79 free, 101 is a Rust panic; `run()` has two callers; `_args` was ignored.

- [ ] **Step 1: Write the failing integration test** (`tests/run_interp_limits.rs`; add `argv_is_the_scripts_own` alongside the four from revision 1)

```rust
use std::process::Command;
fn vox() -> String { env!("CARGO_BIN_EXE_vox").to_string() }
fn write(name: &str, src: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("vox-limits-{name}-{}.vox", std::process::id()));
    std::fs::write(&p, src).unwrap();
    p
}

#[test]
fn capability_denial_exits_77_with_a_marker_on_stderr_and_nothing_on_stdout() {
    let f = write("caps", r#"pub fn main() { let s = fs.read("/etc/hosts"); print("LEAK") }"#);
    let out = Command::new(vox()).args(["run", "--mode", "interp", "--caps", "env:ro"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(77), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("LEAK"));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("vox: capability denied: fs.read"));
}

#[test]
fn caps_flag_overrides_script_directive() {
    let f = write("override", "// vox:caps fs\npub fn main() { let s = fs.read(\"/etc/hosts\"); print(\"LEAK\") }");
    let out = Command::new(vox()).args(["run", "--mode", "interp", "--caps", "env:ro"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(77));
}

#[test]
fn step_and_depth_limits_exit_78() {
    let f = write("steps", "pub fn main() { let mut i = 0; while true { i = i + 1 } }");
    let out = Command::new(vox()).args(["run", "--mode", "interp", "--max-steps", "10000"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(78), "{}", String::from_utf8_lossy(&out.stderr));
    let g = write("depth", "fn f(n: int) to int { return f(n + 1) } pub fn main() { return f(0) }");
    let out = Command::new(vox()).args(["run", "--mode", "interp"]).arg(&g).output().unwrap();
    assert_eq!(out.status.code(), Some(78), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn memory_limit_exits_79() {
    let f = write("mem", r#"pub fn main() { let mut xs = []; while true { xs = xs.push("0123456789012345678901234567890123456789") } }"#);
    let out = Command::new(vox()).args(["run", "--mode", "interp", "--max-memory", "67108864", "--max-steps", "100000000"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(79), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("memory limit exceeded"));
}

#[test]
fn argv_is_the_scripts_own() {
    let f = write("argv", r#"pub fn main() { let a = env.args(); print(str(len(a))); print(a[1]) }"#);
    let out = Command::new(vox()).args(["run", "--mode", "interp"]).arg(&f).args(["--", "hello"]).output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.starts_with("2\nhello"), "{s:?} stderr={}", String::from_utf8_lossy(&out.stderr));
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p vox-cli --test run_interp_limits 2>&1 | tail -10`.

- [ ] **Step 3: The allocator** — `crates/vox-cli/src/mem_limit.rs`:

```rust
//! Counting allocator with a runtime-armed ceiling (spec §3.2 item 6).
//!
//! **This module belongs to the library crate.** `commands::run::run` calls [`arm`], and
//! `crate::` there is `vox_cli`. Declaring the module in both the bin and the lib compiles
//! two copies of `USED`/`LIMIT`: the allocator consults one and `arm` writes the other, and
//! the ceiling silently never fires. `lib.rs` therefore carries both `pub mod mem_limit;`
//! and the `#[global_allocator]` static; every binary linking `vox-cli` inherits it.
//!
//! Disarmed cost: one relaxed atomic add per alloc, one sub per dealloc.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Counting;

static USED: AtomicUsize = AtomicUsize::new(0);
static LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);

pub const EXIT_MEMORY_LIMIT: i32 = 79;

/// Arm the ceiling. Everything allocated so far is counted but was not bounded; call it as
/// early in `run()` as the clap parse allows. Counts Rust heap in this process only — not
/// child processes, not `mmap` by C deps, not thread stacks, not bytes written to disk.
pub fn arm(bytes: usize) {
    LIMIT.store(bytes, Ordering::Relaxed);
}

pub fn used() -> usize { USED.load(Ordering::Relaxed) }

#[inline]
fn charge(size: usize) {
    let new = USED.fetch_add(size, Ordering::Relaxed).wrapping_add(size);
    if new > LIMIT.load(Ordering::Relaxed) {
        die(new);
    }
}

#[inline]
fn refund(size: usize) { USED.fetch_sub(size, Ordering::Relaxed); }

/// Report and terminate **without running any exit handler**. `std::process::exit` calls
/// libc `exit`, which takes `__exit_funcs_lock` and runs atexit handlers; a handler that
/// allocates re-enters `alloc` over the ceiling — recursive `exit` under that lock, which
/// glibc treats as undefined and which deadlocks in practice. `abort()` is out too: it dies
/// by signal, so the parent sees `None`, never 79. Buffered stdout is lost by intent.
#[cold]
#[inline(never)]
fn die(used_now: usize) -> ! {
    let mut buf = [0u8; 128];
    let n = format_message(&mut buf, used_now, LIMIT.load(Ordering::Relaxed));
    #[cfg(unix)]
    unsafe {
        libc::write(2, buf.as_ptr().cast(), n);
        libc::_exit(EXIT_MEMORY_LIMIT);
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Storage::FileSystem::WriteFile;
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE};
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        // TerminateProcess, not ExitProcess: the latter runs DLL_PROCESS_DETACH — same hazard.
        let mut written = 0u32;
        WriteFile(GetStdHandle(STD_ERROR_HANDLE), buf.as_ptr(), n as u32, &mut written, std::ptr::null_mut());
        TerminateProcess(GetCurrentProcess(), EXIT_MEMORY_LIMIT as u32);
        std::hint::unreachable_unchecked()
    }
    #[cfg(not(any(unix, windows)))]
    std::process::abort()
}

/// `vox: memory limit exceeded (<used> > <limit> bytes); exit 79\n` into a stack buffer —
/// `format!` would allocate, and we are inside the allocator.
fn format_message(buf: &mut [u8; 128], used: usize, limit: usize) -> usize {
    struct Cur<'a> { buf: &'a mut [u8; 128], n: usize }
    impl Cur<'_> {
        fn s(&mut self, bytes: &[u8]) { for &b in bytes { if self.n < self.buf.len() { self.buf[self.n] = b; self.n += 1; } } }
        fn u(&mut self, mut v: usize) {
            let mut d = [0u8; 20]; let mut i = d.len();
            loop { i -= 1; d[i] = b'0' + (v % 10) as u8; v /= 10; if v == 0 { break; } }
            self.s(&d[i..]);
        }
    }
    let mut c = Cur { buf, n: 0 };
    c.s(b"vox: memory limit exceeded ("); c.u(used); c.s(b" > "); c.u(limit); c.s(b" bytes); exit 79\n");
    c.n
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        let p = unsafe { System.alloc(layout) };
        if p.is_null() { refund(layout.size()); }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // Overridden for cost, not accounting: keeps calloc's zero-page path.
        charge(layout.size());
        let p = unsafe { System.alloc_zeroed(layout) };
        if p.is_null() { refund(layout.size()); }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        refund(layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // The provided default is alloc+copy+dealloc, which forfeits realloc(3)'s in-place
        // growth for every Vec in the compiler, armed or not. Charge only the delta.
        let old = layout.size();
        if new_size > old { charge(new_size - old); }
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if p.is_null() { if new_size > old { refund(new_size - old); } }
        else if new_size < old { refund(old - new_size); }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_message_is_built_without_allocating() {
        let mut b = [0u8; 128];
        let n = format_message(&mut b, 100, 64);
        assert_eq!(std::str::from_utf8(&b[..n]).unwrap(), "vox: memory limit exceeded (100 > 64 bytes); exit 79\n");
    }
    #[test]
    fn a_long_message_is_truncated_not_panicking() {
        let mut b = [0u8; 128];
        assert!(format_message(&mut b, usize::MAX, usize::MAX) <= 128);
    }
    // `used()` is a live shared counter under cargo test's thread pool; assert direction only.
    #[test]
    fn charging_moves_the_counter() {
        let before = used();
        charge(1 << 20);
        assert!(used() >= before + (1 << 20));
        refund(1 << 20);
    }
}
```

In `lib.rs`: `pub mod mem_limit;` and `#[global_allocator] static GLOBAL: mem_limit::Counting = mem_limit::Counting;`. `main.rs` unchanged. `Cargo.toml` `[target.'cfg(windows)'.dependencies]`: `windows-sys = { workspace = true, features = ["Win32_Foundation", "Win32_Storage_FileSystem", "Win32_System_Console", "Win32_System_Threading"] }`.

- [ ] **Step 4: Flags, args, exit codes.** `cli_args.rs` `RunArgs`: add `caps: Option<String>`, `max_steps: Option<usize>`, `max_memory: Option<usize>` (doc strings pointing at `docs/src/reference/isolation.md`). `run.rs`: `run_interp(file, args, caps, max_steps)`; set `interpreter.script_args = args.to_vec()` (Task 2's field) and `interpreter.caps` = `parse(spec)?` / `from_legacy_directive` / `developer_default()`; install the signal handler here only. Map outcomes:

```rust
    let outcome = interpreter.run_module(&lowered).and_then(|_| interpreter.call("main", vec![]));
    let res = match outcome {
        Ok(v) => v,
        Err(vox_compiler::eval::EvalError::CapabilityDenied { ns, method }) => {
            eprintln!("vox: capability denied: {ns}.{method}");  // the executor keys on this prefix
            std::process::exit(77);                              // deliberately skips exit commands
        }
        Err(vox_compiler::eval::EvalError::StepLimitExceeded) | Err(vox_compiler::eval::EvalError::RecursionLimitExceeded) => {
            eprintln!("vox: execution budget exceeded; pass --max-steps or use --mode script for compute-heavy work");
            std::process::exit(78);
        }
        Err(e) => anyhow::bail!("Eval failed: {e:?}"),
    };
```

`run()` gains `caps: Option<&str>, max_steps: Option<usize>, max_memory: Option<usize>`; arm the allocator first; thread from `cli_dispatch/lanes.rs:269` (from `RunArgs`) and `compilerd.rs:336` (`None, None, None`). Both `run_interp` call sites (`:92`, `:139`) updated.

- [ ] **Step 5: Run; mutate** — `cargo test -q -p vox-cli --test run_interp_limits && cargo test -q -p vox-cli --lib mem_limit`. Mutation: `if new > LIMIT.load(…)` → `if false` in `charge` → `memory_limit_exits_79` MUST FAIL (exits 78); restore; `grep -c 'new > LIMIT' crates/vox-cli/src/mem_limit.rs` → 1.

- [ ] **Step 6: Commit** — `cargo fmt -p vox-cli`; `git commit -m "feat(run): --caps/--max-steps/--max-memory; allocator ceiling in the library; args reach the script"`.

---

## Task 7: The interpreter becomes the default for script-shaped files

**Files:** `crates/vox-cli/src/commands/runtime/run/run.rs:13-23` (predicate), `commands/run.rs:92-146`, `cli_args.rs:149-157`, `docs/src/reference/cli.md:144`, `lefthook.yml:18`, `crates/vox-cli/tests/run_mode_dispatch.rs` (its `#[ignore]`d `run_mode_auto_matches_script_for_script_shaped_file` at `:68` asserts the *old* routing — retarget it).

Findings (parity, Task 7 note; deletions item 5): `!head.contains("@page")` routes `table`/`routes`/`server`/`actor`/`workflow` programs to the interpreter; verify the pre-commit script **before** flipping.

- [ ] **Step 1: Verify the automation scripts first.** From Task 0's table, `scripts/fmt.vox`, `install-hooks.vox`, `setup.vox`, `arch-check.vox` must show `ok` in the run column. If any does not, fix the asymmetry (Task 2 class) before continuing — a failure here breaks every commit for every contributor.

- [ ] **Step 2: Write the failing tests** (append to `run_mode_dispatch.rs`; retarget the ignored one to assert interp routing)

```rust
#[test]
fn auto_mode_runs_script_shaped_files_under_the_interpreter() {
    let f = std::env::temp_dir().join(format!("vox-auto-{}.vox", std::process::id()));
    std::fs::write(&f, r#"pub fn main() { print("AUTO_INTERP_OK") }"#).unwrap();
    let t0 = std::time::Instant::now();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox")).args(["run"]).arg(&f).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("AUTO_INTERP_OK"));
    assert!(t0.elapsed() < std::time::Duration::from_secs(5), "took {:?}: auto mode is still compiling", t0.elapsed());
}

#[test]
fn service_shaped_files_do_not_go_to_the_interpreter() {
    use vox_cli::commands::runtime::run::run::is_interpreter_shaped;
    for src in [
        "table T { id: Id[T] }\npub fn main() {}",
        "routes { }\npub fn main() {}",
        "server hello() to str { return \"x\" }\npub fn main() {}",
        "workflow w() { }\npub fn main() {}",
        "actor A { }\npub fn main() {}",
        "@page fn home() {}",
    ] {
        assert!(!is_interpreter_shaped(src), "must keep the native/app lane: {src:?}");
    }
    assert!(is_interpreter_shaped("pub fn main() { print(1) }"));
}
```

- [ ] **Step 3: Implement.** In `run.rs` (the `runtime/run/run.rs` one) add:

```rust
/// Script-shaped: declares `fn main()` and none of the surfaces the native lane boots.
/// Scans the first 8 KiB like the `@page` heuristic; false positives route to the native
/// lane, which is the safe direction.
pub fn is_interpreter_shaped(head: &str) -> bool {
    let has_main = head.contains("fn main(");
    let service = ["@page", "\nroutes", "\ntable ", "\nserver ", "\nquery ", "\nmutation ", "\nactor ", "\nworkflow ", "\nactivity "];
    let h = format!("\n{head}");
    has_main && !service.iter().any(|s| h.contains(s))
}
```

In `commands/run.rs::run`: when `mode == Auto` and `web_mode != WebRunMode::Script` and `is_interpreter_shaped(&head)` → `run_interp(...)`; otherwise the existing lanes, and when the file is script-shaped but a service surface was found, `eprintln!("vox: this program declares a service surface; running it on the native lane (use --mode script explicitly to silence this)")`. Update the `RunMode::Auto` doc, `cli_args.rs:149`, `cli.md:144`, and add the lefthook comment.

- [ ] **Step 4: Run and commit** — `cargo test -q -p vox-cli --test run_mode_dispatch && cargo test -q -p vox-cli --test run_interp_limits`; `git commit -m "feat(run): the interpreter is the default for script-shaped files; cargo is opt-in"`.

---

## Task 8: Protocol — job ids on the wire, honest version refusal, payload framing

**Files:** `crates/vox-mesh-transport/src/protocol.rs` (`:15-16` PROTO, `:19` `JobId`, `:49-52` `Run`, `:73-79` `QueueStats` doc, `:88-97` `Isolation`, `:100-118` `Probed`, `:120-140` `JobLimits`, `:160-169` `check_hello`, `:283-284,304` tests); `endpoint.rs:35-40` (`ReceivedJob`), `:191-196` (close codes), `:221-262` (`handle`); `tests/security.rs` (`:186-200`, `:254-268` send `Run` with no payload frame and no `job_id`; `:206` `Isolation::Wasm`); `tests/mailbox.rs:12,62` (uses `ProbeOnlyExecutor` — leave until Task 10 but note).

Findings (mesh #1–#4, #6; limits #7–#9): `check_hello` error never reaches the peer; `#[serde(default)]` is decorative under postcard; frame max off by the varint; sender never learns a payload-hash id; `JobId` is a type alias; 1 GiB cap; `JobLimits` must carry every bound.

- [ ] **Step 1: Write the failing tests** (append to `security.rs`, using the helpers you will move to `tests/common/mod.rs` in Task 9 — for now keep them local)

```rust
#[tokio::test]
async fn a_payload_of_exactly_the_declared_size_is_accepted_and_reaches_the_executor() {
    let server = start_server().await;
    server.trust.trust(&client_id(), None).unwrap();
    let payload = b"pub fn main() { print(\"hi\") }".to_vec();
    let resp = send_run_on(&server, JobId(1), TaskKind::VoxScript, &payload).await;
    assert!(matches!(resp, JobResponse::Output(_)), "{resp:?}");
    assert_eq!(server.exec.last_payload(), payload);
}

#[tokio::test]
async fn a_payload_shorter_than_its_claim_is_refused_not_truncated() {
    let server = start_server().await;
    server.trust.trust(&client_id(), None).unwrap();
    let resp = send_run_with_claim(&server, JobId(2), TaskKind::VoxScript, b"short", 999).await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if m.contains("claim")), "{resp:?}");
}

#[tokio::test]
async fn a_voxscript_payload_over_four_mib_is_refused_before_transfer() {
    let server = start_server().await;
    server.trust.trust(&client_id(), None).unwrap();
    let resp = send_run_with_claim(&server, JobId(3), TaskKind::VoxScript, b"", 5 * 1024 * 1024).await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if m.contains("exceeds")), "{resp:?}");
    assert_eq!(server.exec.invocations(), 0);
}

#[tokio::test]
async fn a_v1_peer_is_told_which_machine_to_upgrade() {
    let server = start_server().await;
    server.trust.trust(&client_id(), None).unwrap();
    let (resp, close) = send_raw_hello_on(&server, Hello { proto: 1, ..Hello::current() }).await;
    assert!(matches!(resp, Some(JobResponse::Failed(ref m)) if m.contains("v1") && m.contains("v2")), "{resp:?}");
    assert_eq!(close, Some(REFUSED_PROTO));
}

#[test]
fn a_new_trailing_field_is_not_readable_from_an_old_sender() {
    // postcard is positional: encode a Probed WITHOUT `engines`, decode with it — must fail.
    #[derive(serde::Serialize)]
    enum OldResp { #[allow(dead_code)] A, Probed { host_triple: String, vox: String, task_kinds: Vec<TaskKind> } }
    let bytes = postcard::to_allocvec(&OldResp::Probed { host_triple: "t".into(), vox: "v".into(), task_kinds: vec![] }).unwrap();
    assert!(postcard::from_bytes::<JobResponse>(&bytes).is_err(), "#[serde(default)] does not buy wire compatibility under postcard; PROTO does");
}

#[test]
fn isolation_default_is_the_interpreter_and_there_is_no_third_tier() {
    assert_eq!(Isolation::DEFAULT_FOR_MESH, Isolation::Interpreter);
    for v in [Isolation::Interpreter, Isolation::Native] { match v { Isolation::Interpreter | Isolation::Native => {} } }
}

#[test]
fn proto_is_two_and_limits_carry_every_bound() {
    assert_eq!(vox_mesh_transport::protocol::PROTO, 2);
    let l = JobLimits::default();
    assert_eq!(l.max_payload_for(TaskKind::VoxScript), 4 * 1024 * 1024);
    assert!(l.max_memory_bytes > 0 && l.max_steps > 0);
}
```

Extend `SpyExecutor` with `last_payload()` / `invocations()`; write `send_run_on(server, job_id, kind, payload)` and `send_run_with_claim(server, job_id, kind, payload, claim)` that write `Hello`, then `JobRequest::Run { job_id, kind, payload_bytes: claim }`, then `write_frame(&mut send, &payload.to_vec())`; and `send_raw_hello_on` returning `(Option<JobResponse>, Option<u32 close code>)`. Update the two existing `Run` senders at `:186-200` and `:254-268` to send a `job_id` and a payload frame (they currently `finish()` right after the request — that would now fail at *runtime* on `read_exact`).

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p vox-mesh-transport --test security 2>&1 | tail -10` → compile errors.

- [ ] **Step 3: Implement `protocol.rs`.**

```rust
pub const PROTO: u16 = 2;

/// Assigned by the SENDER and echoed in every response; scoped by the receiver to the
/// sending peer, so ids need only be unique per sender.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub u64);

pub enum JobRequest {
    Probe,
    Run { job_id: JobId, kind: TaskKind, payload_bytes: u64 },
    Cancel { job_id: JobId },
    QueueStats,
}

/// postcard is positional: `#[serde(default)]` does NOT make a new trailing field readable
/// from an older sender (postcard-1.1.3 de/deserializer.rs:151 — SeqAccess bounds on the
/// reader's field count, so a short buffer is DeserializeUnexpectedEnd, never Ok(None)).
/// Compatibility comes from PROTO + check_hello, nothing else. ANY field added to any frame
/// below is a PROTO bump. The existing `#[serde(default)]`s are kept for JSON debug dumps only.
pub enum Isolation { Interpreter, Native }
impl Isolation { pub const DEFAULT_FOR_MESH: Self = Self::Interpreter; }

// JobResponse::Probed gains `pub engines: Vec<String>` (see the comment above).

pub struct JobLimits {
    pub wall_clock: Duration,
    pub max_output_bytes: usize,
    /// General cap, sized for declarative ML job JSON. VoxScript has its own (below).
    pub max_payload_bytes: u64,        // default 16 MiB, was 1 GiB for the deleted bundle lane
    pub max_memory_bytes: usize,       // default 512 MiB
    pub max_steps: u64,                // default 50_000_000
    pub isolation: Isolation,
}
impl JobLimits {
    /// VoxScript is *source*: 4 MiB is already an absurd script. Checked before the frame is
    /// read, so an oversized claim costs a frame rather than the allocation.
    pub fn max_payload_for(&self, kind: TaskKind) -> u64 {
        match kind { TaskKind::VoxScript => 4 * 1024 * 1024, _ => self.max_payload_bytes }
    }
}
```

`endpoint.rs`: add `pub const REFUSED_PROTO: u32 = 4005;` (check the existing `REFUSED_*` constants — 4001/4003/4004 are taken — and pick the next free). In `handle()`, replace `protocol::check_hello(&hello)?` with: on `Err(e)` write `JobResponse::Failed(e.to_string())`, `send.finish()?`, `conn.close(REFUSED_PROTO.into(), b"proto mismatch")`, `conn.closed().await`, `return Ok(())`. Then:

```rust
    let payload = match &request {
        JobRequest::Run { kind, payload_bytes, .. } => {
            let cap = limits.max_payload_for(*kind);
            if *payload_bytes > cap {
                let msg = format!("payload of {payload_bytes} bytes exceeds the {cap} byte cap for {kind}");
                protocol::write_frame(&mut send, &JobResponse::Failed(msg)).await?;
                send.finish()?; conn.closed().await; return Ok(());
            }
            // +8: the frame is the postcard Vec<u8> = varint(len) ++ bytes.
            let max = usize::try_from(*payload_bytes).unwrap_or(usize::MAX).saturating_add(8);
            let p: Vec<u8> = protocol::read_frame(&mut recv, max).await?;
            if p.len() as u64 != *payload_bytes {
                let msg = format!("payload claim of {payload_bytes} bytes but {} arrived", p.len());
                protocol::write_frame(&mut send, &JobResponse::Failed(msg)).await?;
                send.finish()?; conn.closed().await; return Ok(());
            }
            p
        }
        _ => Vec::new(),
    };
```

`ReceivedJob` gains `pub payload: Vec<u8>`. Fix every `Isolation::Wasm`/`Container` and every `JobRequest::Cancel { job_id: 42 }` (`protocol.rs:232`) the compiler reports, plus the `protocol.rs:283-284` test asserting `DEFAULT_FOR_MESH == Wasm`.

- [ ] **Step 4: Run** — `cargo test -q -p vox-mesh-transport && cargo clippy -q -p vox-mesh-transport --all-targets -- -D warnings`. (`tests/mailbox.rs` still compiles: it uses `ProbeOnlyExecutor`, which Task 10 deletes.)

- [ ] **Step 5: Commit** — `git commit -m "feat(mesh): PROTO 2 — sender job ids, honest version refusal, payload framing that works"`.

---

## Task 9: `InterpExecutor` — the mesh runs VoxScript

**Files:** Create `crates/vox-mesh-transport/src/interp_executor.rs`, `tests/common/mod.rs`, `tests/interp_executor.rs`; modify `src/lib.rs`, `Cargo.toml` (`tokio` features `process, io-util, time, sync, fs, macros`; `tempfile`; `libc` under `cfg(unix)`); `pre_push.rs:1551-1568` (register the slow tests).

Findings (limits #4, #9–#12, #17, #18; mesh #4, #5, #7–#9, #18; executability #12, #13, #19–#21): `select!` borrow conflict; `Arc<dyn>` coercion; cross-peer cancel; no concurrency cap; grandchild pipe hang; Windows env; `HOME` on the job dir; caps-string injection; truncation marker missing; test numbers; `vox` binary not built by `cargo test`.

- [ ] **Step 1: Move the helpers.** Create `tests/common/mod.rs` holding `Server`, `SpyExecutor`, `start_server_with(make: impl FnOnce(Arc<MeshTrust>) -> Arc<dyn JobExecutor>)`, `start_server()` (wrapping `SpyExecutor`), `client_endpoint`, `client_id`, `send_run_on`, `send_run_with_claim`, `loopback_addr_of`. `security.rs` declares `mod common;` and imports from it; behaviour unchanged. Run `cargo test -q -p vox-mesh-transport --test security` → still green.

- [ ] **Step 2: Write the failing tests** — `tests/interp_executor.rs`; every live test carries `#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]`:

```rust
mod common;
use std::path::PathBuf;
use std::sync::Arc;
use vox_mesh_transport::endpoint::JobExecutor;
use vox_mesh_transport::protocol::{JobId, JobLimits, JobResponse};
use vox_mesh_transport::trust::TrustLevel;
use vox_mesh_transport::{InterpExecutor, MeshTrust};
use vox_mesh_types::TaskKind;

/// Builds `vox` if absent: `cargo test -p vox-mesh-transport` does not, and a dev-dep on
/// `vox-cli` would be an upward crate edge.
fn vox_bin() -> PathBuf {
    if let Ok(p) = std::env::var("VOX_BIN") { return p.into(); }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let exe = root.join("target/debug").join(if cfg!(windows) { "vox.exe" } else { "vox" });
    if !exe.exists() {
        let st = std::process::Command::new(env!("CARGO")).current_dir(&root)
            .args(["build", "-q", "-p", "vox-cli", "--bin", "vox"]).status().expect("spawn cargo");
        assert!(st.success(), "could not build the vox binary the executor spawns");
    }
    exe
}

fn exec(trust: Arc<MeshTrust>) -> Arc<dyn JobExecutor> {
    Arc::new(InterpExecutor::new(trust, vox_bin(), JobLimits::default()))
}

#[test]
fn caps_mapping_never_grants_more_than_the_trust_level() {
    let d = tempfile::tempdir().unwrap();
    let s = InterpExecutor::caps_for(TrustLevel::Sandboxed, d.path()).unwrap().to_spec();
    assert!(s.contains("fs:rw=") && s.contains("time:real"));
    for forbidden in ["net:allow", "process:allow", "env:", "secrets"] { assert!(!s.contains(forbidden), "{s}"); }
    let n = InterpExecutor::caps_for(TrustLevel::Native, d.path()).unwrap().to_spec();
    assert!(n.contains("net:allow") && n.contains("process:allow") && n.contains("env:ro"));
    assert!(!n.contains("secrets"), "secrets are never granted by trust level");
}

#[test]
fn a_job_dir_containing_a_separator_is_refused_not_encoded() {
    assert!(InterpExecutor::caps_for(TrustLevel::Sandboxed, std::path::Path::new("/tmp/a,net:allow")).is_err());
}

#[tokio::test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]
async fn a_voxscript_job_runs_and_returns_its_output() {
    let server = common::start_server_with(exec).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let resp = common::send_run_on(&server, JobId(1), TaskKind::VoxScript, b"pub fn main() { print(\"MESH_RAN\") }").await;
    assert!(matches!(resp, JobResponse::Output(ref b) if String::from_utf8_lossy(b).contains("MESH_RAN")), "{resp:?}");
}

#[tokio::test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]
async fn a_sandboxed_peer_cannot_read_the_host_filesystem() {
    let server = common::start_server_with(exec).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let resp = common::send_run_on(&server, JobId(2), TaskKind::VoxScript, b"pub fn main() { let s = fs.read(\"/etc/hosts\"); print(\"LEAK\") }").await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if m.contains("capability denied") && m.contains("fs.read")), "{resp:?}");
}

#[tokio::test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]
async fn a_forged_exit_77_is_not_reported_as_a_denial() {
    let server = common::start_server_with(exec).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let resp = common::send_run_on(&server, JobId(3), TaskKind::VoxScript, b"pub fn main() { process.exit(77) }").await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if !m.contains("capability denied")), "{resp:?}");
}

#[tokio::test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]
async fn a_runaway_allocation_is_killed_and_reported() {
    let mut limits = JobLimits::default();
    limits.max_memory_bytes = 64 * 1024 * 1024;
    limits.max_steps = 100_000_000;   // headroom so the step budget does not win the race
    let server = common::start_server_with(|t| Arc::new(InterpExecutor::new(t, vox_bin(), limits)) as Arc<dyn JobExecutor>).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let resp = common::send_run_on(&server, JobId(4), TaskKind::VoxScript, b"pub fn main() { let mut xs = []; while true { xs = xs.push(\"0123456789012345678901234567890123456789\") } }").await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if m.contains("memory limit")), "{resp:?}");
}

#[tokio::test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]
async fn output_is_capped_and_marked() {
    let mut limits = JobLimits::default();
    limits.max_output_bytes = 64 * 1024;
    let server = common::start_server_with(|t| Arc::new(InterpExecutor::new(t, vox_bin(), limits)) as Arc<dyn JobExecutor>).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let resp = common::send_run_on(&server, JobId(5), TaskKind::VoxScript, b"pub fn main() { let mut i = 0; while i < 10000 { print(\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"); i = i + 1 } }").await;
    match resp {
        JobResponse::Output(b) => { assert!(b.len() <= 64 * 1024 + 128); assert!(String::from_utf8_lossy(&b).contains("[vox: output truncated")); }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]
async fn one_peer_cannot_cancel_another_peers_job() {
    let server = common::start_server_with(exec).await;
    let (a, b) = (common::client_endpoint(common::client_sk_a()).await, common::client_endpoint(common::client_sk_b()).await);
    server.trust.trust(&a.id(), None).unwrap();
    server.trust.trust(&b.id(), None).unwrap();
    // A starts a slow job (JobId(9)); B cancels JobId(9); A's job must still finish.
    let slow = b"pub fn main() { let mut i = 0; while i < 3000000 { i = i + 1 }; print(\"DONE\") }";
    let a_job = tokio::spawn(common::send_run_from(a.clone(), &server, JobId(9), TaskKind::VoxScript, slow));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let cancel = common::send_cancel_from(b, &server, JobId(9)).await;
    assert!(matches!(cancel, JobResponse::Failed(ref m) if m.contains("no such running job")), "{cancel:?}");
    let done = a_job.await.unwrap();
    assert!(matches!(done, JobResponse::Output(ref o) if String::from_utf8_lossy(o).contains("DONE")), "{done:?}");
}

#[tokio::test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: builds and spawns the vox binary"]
async fn ml_task_kinds_are_refused_with_a_reason_when_no_engine_is_installed() {
    let server = common::start_server_with(exec).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let resp = common::send_run_on(&server, JobId(6), TaskKind::TextInfer, b"{}").await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if m.contains("no engine")), "{resp:?}");
}
```

(`common` gains `client_sk_a/b`, `send_run_from(ep, …)`, `send_cancel_from(ep, …)`.)

- [ ] **Step 3: Run to verify they fail** — `cargo test -q -p vox-mesh-transport --test interp_executor -- --ignored 2>&1 | tail -10` → compile error.

- [ ] **Step 4: Implement**

```rust
//! Runs `VoxScript` jobs by spawning the interpreter as a bounded child in its own process
//! group (spec §3.4). The interpreter is the sandbox for Vox code; the process boundary is
//! defence-in-depth. ML task kinds are declarative and refused until an engine registry exists.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::{Context, Result};
use iroh::EndpointId;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{oneshot, Semaphore};
use vox_mesh_types::TaskKind;

use crate::endpoint::{JobExecutor, ReceivedJob};
use crate::protocol::{JobId, JobLimits, JobRequest, JobResponse, QueueStats};
use crate::trust::{MeshTrust, TrustLevel};

const DENIAL_MARKER: &str = "vox: capability denied:";
const DRAIN_GRACE: Duration = Duration::from_secs(5);

pub struct InterpExecutor {
    trust: Arc<MeshTrust>,
    vox_bin: PathBuf,
    limits: JobLimits,
    slots: Semaphore,
    running: Mutex<HashMap<(EndpointId, JobId), oneshot::Sender<()>>>,
}

impl InterpExecutor {
    pub fn new(trust: Arc<MeshTrust>, vox_bin: PathBuf, limits: JobLimits) -> Self {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(2);
        Self { trust, vox_bin, limits, slots: Semaphore::new(n), running: Mutex::new(HashMap::new()) }
    }

    /// The only place a trust level becomes capabilities. Typed, so a job dir containing a
    /// grammar separator is refused rather than encoded. `secrets` is never granted by level.
    pub fn caps_for(level: TrustLevel, job_dir: &Path) -> Result<vox_compiler_caps::CapabilitySet, String> {
        // NOTE: `CapabilitySet` lives in vox-compiler (L2); vox-mesh-transport (L2) may not
        // depend on it without an edge. Task 9a below duplicates the ~40-line parse/serialise
        // subset as `crate::caps_spec` under the defactor policy (`// vox:defactored-from
        // vox-compiler 2026-09-05`); replace `vox_compiler_caps::` with `crate::caps_spec::`.
        let extra: &[&str] = match level {
            TrustLevel::Sandboxed => &["time:real"],
            TrustLevel::Native => &["time:real", "net:allow", "process:allow", "env:ro"],
        };
        vox_compiler_caps::CapabilitySet::from_roots(vec![], vec![job_dir.to_path_buf()], extra).map_err(|e| e.to_string())
    }

    async fn read_capped<R: tokio::io::AsyncRead + Unpin>(mut r: R, max: usize) -> (Vec<u8>, bool) {
        let mut buf = Vec::new();
        let mut truncated = false;
        let mut chunk = [0u8; 8192];
        loop {
            match r.read(&mut chunk).await {
                Ok(0) => break,
                Ok(n) => {
                    if buf.len() < max { let take = n.min(max - buf.len()); buf.extend_from_slice(&chunk[..take]); }
                    if buf.len() + n > max { truncated = true; }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        (buf, truncated)
    }

    async fn run_script(&self, job: &ReceivedJob, job_id: JobId, level: TrustLevel) -> Result<JobResponse> {
        let Ok(_slot) = self.slots.try_acquire() else {
            return Ok(JobResponse::Failed("node is at its concurrent-job ceiling; retry".into()));
        };
        let dir = tempfile::Builder::new().prefix("vox-mesh-job-").tempdir()?;
        let home = tempfile::Builder::new().prefix("vox-mesh-home-").tempdir()?;   // read-only to the script
        let main = dir.path().join("main.vox");
        tokio::fs::write(&main, &job.payload).await?;
        let caps = Self::caps_for(level, dir.path()).map_err(|e| anyhow::anyhow!(e))?;

        let mut cmd = Command::new(&self.vox_bin);
        cmd.arg("run").arg("--mode").arg("interp")
            .arg("--caps").arg(caps.to_spec())
            .arg("--max-steps").arg(self.limits.max_steps.to_string())
            .arg("--max-memory").arg(self.limits.max_memory_bytes.to_string())
            .arg(&main)
            .env_clear()
            .current_dir(dir.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if let Some(p) = std::env::var_os("PATH").filter(|p| !p.is_empty()) { cmd.env("PATH", p); }
        #[cfg(unix)]
        {
            cmd.env("HOME", home.path()).env("TMPDIR", dir.path());
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            cmd.env("USERPROFILE", home.path()).env("TEMP", dir.path()).env("TMP", dir.path());
            if let Some(sr) = std::env::var_os("SystemRoot") { cmd.env("SystemRoot", sr); }
        }
        let mut child = cmd.spawn().context("spawn vox")?;
        let pid = child.id();

        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        let key = (job.peer, job_id);
        self.running.lock().unwrap_or_else(PoisonError::into_inner).insert(key, cancel_tx);

        let max_out = self.limits.max_output_bytes;
        let out_task = tokio::spawn(Self::read_capped(child.stdout.take().expect("piped"), max_out));
        let err_task = tokio::spawn(Self::read_capped(child.stderr.take().expect("piped"), 64 * 1024));

        enum Done { Exited(std::process::ExitStatus), Timeout, Cancelled }
        let done = tokio::select! {
            s = child.wait() => Done::Exited(s?),
            _ = tokio::time::sleep(self.limits.wall_clock) => Done::Timeout,
            _ = cancel_rx => Done::Cancelled,
        };
        self.running.lock().unwrap_or_else(PoisonError::into_inner).remove(&key);

        let kill_group = |pid: Option<u32>| {
            #[cfg(unix)]
            if let Some(pid) = pid { unsafe { libc::kill(-(pid as i32), libc::SIGKILL); } }
            let _ = pid;
        };
        let status = match done {
            Done::Exited(s) => s,
            Done::Timeout => {
                kill_group(pid); let _ = child.kill().await;
                let (partial, _) = tokio::time::timeout(DRAIN_GRACE, out_task).await.ok().and_then(Result::ok).unwrap_or_default();
                return Ok(JobResponse::Failed(format!("wall clock of {:?} exceeded; job killed; {} bytes of output before the kill", self.limits.wall_clock, partial.len())));
            }
            Done::Cancelled => {
                kill_group(pid); let _ = child.kill().await;
                return Ok(JobResponse::Failed("cancelled by peer".into()));
            }
        };
        // A `process:allow` script can daemonise a grandchild holding the pipe: bound the drain.
        let (mut stdout, truncated) = tokio::time::timeout(DRAIN_GRACE, out_task).await.ok().and_then(Result::ok).unwrap_or_default();
        let (stderr, _) = tokio::time::timeout(DRAIN_GRACE, err_task).await.ok().and_then(Result::ok).unwrap_or_default();
        let stderr = String::from_utf8_lossy(&stderr).into_owned();
        if truncated { stdout.extend_from_slice(format!("\n[vox: output truncated at {max_out} bytes]\n").as_bytes()); }

        Ok(match status.code() {
            Some(0) => JobResponse::Output(stdout),
            // 77 is only a denial when the interpreter said so; a script can `process.exit(77)`.
            Some(77) if stderr.contains(DENIAL_MARKER) => JobResponse::Failed(format!("capability denied: {}", stderr.trim())),
            Some(78) => JobResponse::Failed(format!("execution budget exceeded: {}", stderr.trim())),
            Some(79) => JobResponse::Failed(format!("memory limit exceeded: {}", stderr.trim())),
            // Backtraces leak host paths; never forward stderr for a Rust panic.
            Some(101) => JobResponse::Failed("interpreter panicked; this is a vox bug, not a script error".into()),
            None => JobResponse::Failed("killed by signal (stack overflow or OOM killer)".into()),
            Some(c) => JobResponse::Failed(format!("exit {c}: {}", stderr.trim())),
        })
    }
}

impl JobExecutor for InterpExecutor {
    fn execute<'a>(&'a self, job: ReceivedJob) -> Pin<Box<dyn std::future::Future<Output = Result<JobResponse>> + Send + 'a>> {
        Box::pin(async move {
            match &job.request {
                JobRequest::Probe => Ok(JobResponse::Probed {
                    host_triple: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
                    vox: env!("CARGO_PKG_VERSION").to_string(),
                    task_kinds: vec![TaskKind::VoxScript],
                    engines: Vec::new(),
                }),
                JobRequest::QueueStats => Ok(JobResponse::QueueStats(QueueStats {
                    pending_count: self.running.lock().unwrap_or_else(PoisonError::into_inner).len() as u64,
                    ..Default::default()
                })),
                JobRequest::Cancel { job_id } => {
                    // Keyed by the CALLER's identity: a wrong guess cannot even evict the row,
                    // and "not yours" and "not there" are one message (no oracle).
                    let tx = self.running.lock().unwrap_or_else(PoisonError::into_inner).remove(&(job.peer, *job_id));
                    Ok(match tx {
                        Some(tx) => { let _ = tx.send(()); JobResponse::Output(b"cancelled".to_vec()) }
                        None => JobResponse::Failed("no such running job".into()),
                    })
                }
                JobRequest::Run { job_id, kind: TaskKind::VoxScript, .. } => {
                    let Some(level) = self.trust.level(&job.peer) else { return Ok(JobResponse::Failed("not trusted".into())); };
                    self.run_script(&job, *job_id, level).await
                }
                JobRequest::Run { kind, .. } => Ok(JobResponse::Failed(format!(
                    "no engine installed for {kind}; this node runs VoxScript only until an ML engine is registered"
                ))),
            }
        })
    }
}
```

- [ ] **Step 4a: `caps_spec` — the defactored subset.** `vox-mesh-transport` may not depend on `vox-compiler`. Create `crates/vox-mesh-transport/src/caps_spec.rs` with a `// vox:defactored-from vox-compiler 2026-09-05` header containing only `CapabilitySet::from_roots` + `to_spec` + the separator check (~40 lines; no path checking — the child does that). The round-trip is pinned by `caps_mapping_never_grants_more_than_the_trust_level`, and a test in `vox-compiler` parses the string `caps_spec` produces so the two cannot drift silently.

- [ ] **Step 5: Register the slow tests** — append `or test(a_voxscript_job_runs_and_returns_its_output) or test(a_sandboxed_peer_cannot_read_the_host_filesystem) or test(a_forged_exit_77_is_not_reported_as_a_denial) or test(a_runaway_allocation_is_killed_and_reported) or test(output_is_capped_and_marked) or test(one_peer_cannot_cancel_another_peers_job) or test(ml_task_kinds_are_refused_with_a_reason_when_no_engine_is_installed)` to the `concat!` in `pre_push.rs:1551-1568`.

- [ ] **Step 6: Run; mutate twice.** `cargo test -q -p vox-mesh-transport --test interp_executor -- --include-ignored 2>&1 | tail -15`. Mutation (a): `Sandboxed` arm returns the `Native` extras → `caps_mapping…` MUST FAIL. Mutation (b): in the `Cancel` arm, key on `job_id` only (`.remove` over any peer) → `one_peer_cannot_cancel…` MUST FAIL. Restore both; grep.

- [ ] **Step 7: Commit** — `cargo fmt -p vox-mesh-transport`; `git commit -m "feat(mesh): InterpExecutor — bounded, process-grouped, peer-scoped VoxScript execution"`.

---

## Task 10: Wire the executor; delete `ProbeOnlyExecutor`, the bundle lane, and `secret_gate`

**Files:** `crates/vox-ml-cli/src/commands/mesh_cli.rs:197`; `vox-mesh-transport/src/endpoint.rs:93-129`; **`tests/mailbox.rs:12,62`** (uses `ProbeOnlyExecutor`); `tests/security.rs` (`a_trusted_peer_gets_a_sandbox_by_default`); `vox-orchestrator/src/a2a/remote_worker.rs` (`:196-250` source lane, `:254-430` bundle lane, `:660-690` call site, **`:965-1290` test module** asserting on `BundleKind`/`classify_bundle`/`run_dispatched_bundle`); delete `a2a/secret_gate.rs` (+ `secret_bag.rs` if `jwe.rs` has no other consumer — `rg -n secret_bag crates/vox-orchestrator/src`); `envelope.rs:91-101,150-151`; `task_submit.rs:861-862,1117-1118`; `tests/populi_single_owner.rs:503-504,635-636`; `vox-secrets/src/spec/{ids.rs:305, registry/missing.rs:1307}`; `vox-populi/src/transport/handlers/dispatch.rs:262`, `vox-plugin-populi-mesh/src/transport/handlers/dispatch.rs:238`; `contracts/secrets/*` + `contracts/config/env-vars.v1.yaml:1294` via regen; `vox-config/src/{config_registry.rs:591-622, operator_registry.rs:716-727}` (doc note).

Findings (mesh #13, #14; deletions items 4, 8; executability): `ExecTier` rename would invert meaning — delete the module; HTTP lane must use a per-dispatch tempdir; `secrets-contracts` before `secrets-parity`; `VOX_MESH_EXEC_POLICY` config key survives with a different meaning.

- [ ] **Step 1: Retarget the security test.** `a_trusted_peer_gets_a_sandbox_by_default` asserts `job.limits.isolation == Isolation::Interpreter` on the spy's recorded job.

- [ ] **Step 2: Swap the executor.** `mesh_cli.rs:197`:

```rust
                let vox_bin = std::env::current_exe().ok()
                    .and_then(|p| p.parent().map(|d| d.join(if cfg!(windows) { "vox.exe" } else { "vox" })))
                    .filter(|p| p.exists())
                    .unwrap_or_else(|| std::path::PathBuf::from("vox"));
                // voxup places only `vox` on PATH (install.rs:145-165); vox-ml-cli lives in the
                // toolchain dir beside the extracted `vox`, so current_exe().parent() is the
                // right first guess in every layout; PATH is the fallback.
                let exec = std::sync::Arc::new(vox_mesh_transport::InterpExecutor::new(
                    trust.clone(), vox_bin, vox_mesh_transport::protocol::JobLimits::default(),
                ));
```

Log `vox --version` at construction and warn on mismatch with `env!("CARGO_PKG_VERSION")` — a stale `vox` that lacks `--caps` fails on an unknown flag.

- [ ] **Step 3: Delete `ProbeOnlyExecutor`** (`endpoint.rs:93-129`; it is not re-exported from `lib.rs`). Fix `tests/mailbox.rs:12,62` to use `common::SpyExecutor`. Build: `cargo build -q -p vox-mesh-transport -p vox-ml-cli --features populi`.

- [ ] **Step 4: Delete the bundle lane and `secret_gate`.** In `remote_worker.rs` delete `run_dispatched_bundle`, `BundleKind`, `classify_bundle`, the bundle call site, and the `#[cfg(test)]` tests at `:965-1290` that reference them. Delete `secret_gate.rs` (and `secret_bag.rs` if orphaned); at the source-lane call site replace `gate_secrets(...)` with `let secret_env: Vec<(String, String)> = Vec::new(); // nothing is forwarded to a dispatched child; capabilities come from the receiver's trust row`. In `run_dispatched_source`, replace the shared-`temp_dir()` file with a per-dispatch `tempfile::tempdir()` and pass `--caps` scoped to it:

```rust
    let dir = tempfile::Builder::new().prefix("vox-dispatch-").tempdir()?;
    let tmp_file = dir.path().join("main.vox");
    …
    cmd.arg("run").arg("--mode").arg("interp")
        .arg("--caps").arg(format!("fs:rw={},time:real", dir.path().display()))
        .arg(&tmp_file);
```

Delete the `policy` parameter and every `no-exec`/`source-only`/`permissive` branch. Remove `exec_bundle_b64`/`exec_bundle_blake3_hex` from `envelope.rs` and the four `None` initialisers. Remove `SecretId::VoxMeshExecPolicy` and its registry entry; in the two HTTP handlers replace the secret read with the literal `"source-only"` plus `// vox-deprecated-since="0.6.0" retire-by="0.7.0" reason="mesh-phase6" canonical="vox_mesh_transport::InterpExecutor"`.

- [ ] **Step 5: Regenerate secrets contracts, then check parity** — `cargo run -q -p vox-cli -- ci secrets-contracts && cargo run -q -p vox-cli -- ci secrets-parity && cargo run -q -p vox-cli -- ci secret-env-guard`. Add a comment at `vox-config/src/config_registry.rs:591`: `// VOX_MESH_EXEC_POLICY here is task PLACEMENT (local_only/prefer_remote/remote_only). The SecretId of the same name (execution policy: no-exec/source-only/permissive) was deleted 2026-09-05 with the native bundle lane; removing it also removed the name from the managed-secret regex, so secret-env-guard no longer polices direct env::var reads of this key.`

- [ ] **Step 6: Build, test, commit** — `cargo test -q -p vox-mesh-transport -p vox-orchestrator 2>&1 | tail -5 && cargo clippy -q -p vox-orchestrator -p vox-ml-cli -p vox-secrets -p vox-populi -p vox-plugin-populi-mesh --all-targets -- -D warnings`. `git commit -m "feat(mesh): serve with InterpExecutor; delete ProbeOnlyExecutor, the native bundle lane (F2), and secret_gate"`.

---

## Task 11: `PopuliHttpOp::Dispatch` over the mesh — with a payload that can execute

**Files:** `crates/vox-mesh-transport/src/directory.rs:28-34,48-63,140,159-169` (`PeerEntry.addrs`); `crates/vox-workflow-runtime/src/workflow/populi.rs` (`:118-124` shim synthesis, `Dispatch`/`Wait` arms, `mesh_envelope` at `:203`, `probed_peers` at `:290-299`); `workflow/types.rs` (`PopuliActivity`).

Findings (mesh #10–#12, #17; executability #14): `workflow_durable_shim` exists nowhere; `PeerEntry` has no `addrs`; `mesh_envelope` takes `&PopuliActivity`; `wait_ok` has zero consumers but `result_output` is de-facto contract; first-fit needs its seam declared.

- [ ] **Step 1: `PeerEntry.addrs`.** `directory.rs:28-34` add `pub addrs: Vec<std::net::SocketAddr>`; `fan_out` (`:140`) returns a 4-tuple carrying the parsed `addrs`; `directory()` (`:48-63`) populates it; `queue_stats` ignores it. Test: `a_probed_peer_carries_the_addresses_it_was_dialled_on` asserting `addrs` equals the loopback address the test server bound.

- [ ] **Step 2: Decide the payload, then write the failing test.** Read `PopuliActivity` in `types.rs`. If it carries the activity's Vox source (a field such as `source`/`body`), `Dispatch` sends that with a `pub fn main()` wrapper appended; if it does not, `Dispatch` returns `Err("activity `X` has no dispatchable source; inline source is required for mesh dispatch")` and the `workflow_durable_shim` synthesis at `:118-124` is **deleted as dead**. Either way, append to `populi.rs` tests an end-to-end test against a loopback `InterpExecutor` (helpers from `vox-mesh-transport/tests/common` cannot be imported across crates — copy `start_server_with` minimally, or gate the test behind `#[ignore]` slow and register it):

```rust
    #[tokio::test]
    #[ignore = "owner:mesh sunset:2026-12-31 slow: spawns the vox binary via InterpExecutor"]
    async fn dispatch_runs_real_source_on_a_loopback_peer() {
        // arrange: loopback server with InterpExecutor, trust this process's endpoint id
        // act: execute_populi_step(&PopuliActivity { populi_op: Dispatch, <source: "pub fn main() { print(\"WF_RAN\") }">, .. })
        // assert: envelope["control"] == "dispatch_ok", envelope["result_output"] contains "WF_RAN",
        //         envelope["peer"] is the server id, envelope["candidates"] == 1, no "control_url" key
    }

    #[test]
    fn wait_is_inline_and_keeps_the_result_keys() {
        let env = wait_envelope(&sample_activity());
        assert_eq!(env["control"], "completed_inline");
        assert_eq!(env["success"], true);
        assert!(env.get("result_output").is_some() && env.get("exit_code").is_some());
    }
```

- [ ] **Step 3: Implement.** `run_on_peer(ep, peer, src)` as in revision 1 but with `JobRequest::Run { job_id: JobId(next_local_id()), kind: VoxScript, payload_bytes }` and a `read_frame` max of `16 * 1024 * 1024` (consistent with the 10 MiB output cap). `Dispatch` picks the first `PeerEntry` with `VoxScript` in `task_kinds` — `// ponytail: first-fit, no queue-depth weighting. Phase 4 Task 4.1 replaces this with a PlacementRecord; the seam is this function's return value.` — and emits `"peer"` and `"candidates": n` into the envelope through `mesh_envelope(activity, "dispatch_ok", json!({…}))` (keep `mesh_envelope`'s signature; do not add a `&str` variant). `Wait` → `mesh_envelope(activity, "completed_inline", json!({"success": true, "result_output": Value::Null, "exit_code": 0, "detail": "mesh jobs are synchronous; the Dispatch step already carried the result"}))`. Delete the HTTP `Dispatch`/`Wait` code and the `VOX_MESH_CONTROL_ADDR` text.

- [ ] **Step 4: Test and commit** — `cargo test -q -p vox-workflow-runtime --features mens -- --include-ignored 2>&1 | tail -5 && cargo clippy -q -p vox-workflow-runtime --all-targets --features mens -- -D warnings`. `git commit -m "feat(workflow): Dispatch runs real source on a mesh peer; Wait is inline and keeps its keys"`.

---

## Task 12: Delete the wasi script lane and the rejected isolation tiers — with the contract chain

**Files (compile):** delete `vox-cli/src/commands/wasm.rs`, `commands/runtime/run/backend/wasi.rs`, `src/isolation.rs`; modify `Cargo.toml:79-87,182,293-294`, `lib.rs:238-252`, **`cli_dispatch/mod.rs:428-441`**, `commands/mod.rs:127-128`, `cli_args.rs:165-214`, `commands/runtime/run/script.rs:28-118`, `backend/mod.rs:4-5,18-52,78-79` (keep `parse_cargo_error`; drop its `target_wasi` param), `backend/tests.rs` (drop the four wasi-specific tests, keep the rest), `doctor/checks_standard/toolchain.rs:205-235`, `crates/voxup/src/install.rs` (**fn at `326-338`, call at `168`**), `vox-codegen/src/codegen_rust/pipeline.rs:54,223,268-285,432` (`WasiBinary` target + `vox-script-wasi` path dep — dead after this), **`vox-populi/src/transport/handlers/dispatch.rs:358-366`** (spawns `vox wasm run`) and **`vox-plugin-populi-mesh/…/dispatch.rs:334-341`** (spawns `--isolation wasm`) → both return `Err("precompiled bundles are no longer executed; use the mesh (ADR-048)")`.
**Files (contracts, in this order):** `contracts/operations/catalog.v1.yaml:14742,14762` → `contracts/cli/command-registry.yaml:3626` → `contracts/capability/capability-registry.yaml:5437` + `model-manifest.generated.json:6300` → `docs/src/reference/cli-command-surface.generated.md` → `contracts/reports/gui-surface-registry.v1.json:82`, `gui-surface-coverage.v1.json:569,1195` → `crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt:565`; `contracts/ci/crate-edges.allow.v1.json:348-351` via `--tighten`; `contracts/channels/stable.toml:20` (`wasm_sysroot`, no reader — delete the line).
**Files (docs):** `docs/src/reference/cli.md:152,855,934`; `docs/src/architecture/external-frontend-interop-plan-2026.md:162` (link to `backend/wasi.rs` — `check-links` fails); `GEMINI.md:21`, `.cursor/rules/voxscript-first-automation.mdc:28`, `docs/src/explanation/expl-architecture.md:352`, `docs/src/reference/mobile-edge-ai.md:32`; `crates/vox-cli/tests/run_mode_dispatch.rs:68` (stale ignored test — delete).

- [ ] **Step 1: Failing test** — as revision 1 (`isolation_and_wasm_surfaces_are_gone`; clap 4.5 wording `unrecognized subcommand` / `unexpected argument '--isolation' found` verified).

- [ ] **Step 2: Delete and fix every compile site listed above.** `git rm` the three files; follow the compiler. `toolchain.rs`: `Check::pass("WASI target (optional)", "not required: scripts run under the interpreter; `--mode script` targets the host")`.

- [ ] **Step 3: Regenerate the contract chain in order.** Use each generator's `--write`/regen flag (`rg -n "catalog.v1|command-registry|capability-registry|gui-surface" crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` lists them as `run_ssot_drift` sub-steps); then `UPDATE_CLI_CATALOG_BASELINE=1 cargo test -q -p vox-cli command_catalog`; then `cargo run -q -p vox-cli -- ci crate-edges --tighten` and confirm **only** `["vox-cli","vox-wasm-engine"]` was removed (`vox-plugin-runtime-wasm → vox-wasm-engine` at `:1710` stays). Then `cargo run -q -p vox-cli -- ci ssot-drift` must be green.

- [ ] **Step 4: Build and verify** — `cargo build -q -p vox-cli -p voxup -p vox-codegen -p vox-populi -p vox-plugin-populi-mesh && cargo test -q -p vox-cli --test run_mode_dispatch && cargo clippy -q -p vox-cli -p voxup -p vox-codegen --all-targets -- -D warnings`; `cargo tree -p vox-cli -e features -i wasmtime 2>&1 | head -2` → `did not match any packages`; `cargo run -q -p vox-cli -- ci check-links`.

- [ ] **Step 5: Commit** — `git commit -m "chore(cli): delete the wasi script lane, vox wasm, and rejected isolation tiers — with their contracts"`.

---

## Task 13: Delete the MicroVM stub; rename `vox_ir` → `hir_export` (scoped)

**Files:** delete `vox-skill-runtime/src/microvm.rs`, `tests/microvm_tier.rs` (**move** its `Tier` ordering and `plan_for_min_tier` error-path assertions into `src/runtime.rs` tests); modify `lib.rs:21,27`, `runtime.rs:105-115`, `detect.rs:165-190`; `contracts/toestub/weak-test-baseline.v1.json:6544,6551` via `regen_weak_test_baseline`. Rename `vox-codegen/src/vox_ir/` → `hir_export/`; **scoped** symbol edits only at `vox-codegen/src/lib.rs:1,22`, `vox-cli/src/commands/check.rs:136-139`, `cli_args.rs:77`, `vox-compiler/tests/ir_emission_test.rs:43,74`, `vox-codegen/Cargo.toml:9` (description); update **both** schema mirrors together — `crates/vox-compiler/src/vox-ir.v1.schema.json:5` and `docs/src/reference/vox-ir.schema.json:4,5`; rewrite `docs/src/reference/vox-ir-specification.md` (currently asserts it *is* the canonical IR); fix the link at `docs/src/architecture/codegen-ssot-and-split-brain-audit-2026.md:136`; path text at `pipeline-parity-ssot-2026-06-14.md:66`, `mesh-phase1-language-spine-plan-2026.md:78`, `where-things-live.md:122`. Keep the `--emit-ir` flag name and the `"2.0.0"` version literal.

- [ ] **Steps:** failing tests (`tier_ordering_and_min_tier_error_path` moved verbatim; `hir_export_is_a_json_envelope_and_says_so`) → `git rm` / `git mv` → the enumerated edits (no blind `sed` over `crates/`) → `cargo test -q -p vox-skill-runtime -p vox-codegen -p vox-compiler --test ir_emission_test && cargo run -q -p vox-cli -- ci check-links && cargo run -q -p vox-cli -- ci ssot-drift` → commit `chore: delete the MicroVM stub; rename vox_ir to hir_export (it is not an IR)`.

---

## Task 14: `sandbox.rs` and `native.rs` stop presenting `VOX_SANDBOX=1` as isolation

**Files:** `crates/vox-cli/src/commands/runtime/run/sandbox.rs:190-214`, **`backend/native.rs:121`** (the second `cmd.env("VOX_SANDBOX", "1")`), `script.rs:20` (`#[derive(Default)]` on `ScriptOpts` — it has none today; all fields derive cleanly once `isolation` is gone).

- [ ] **Steps:** failing test `macos_does_not_pretend_an_env_var_is_a_sandbox` (also assert `native.rs`'s command builder sets no `VOX_SANDBOX`) → replace the "Other" branch with the `tracing::warn!` naming `isolation.md` and delete both `env` calls → `cargo test -q -p vox-cli sandbox` → commit `fix(sandbox): stop presenting VOX_SANDBOX=1 as isolation`. Note the contradicting plan at `docs/src/architecture/vox-language-rules-phase4-runtime-monitors-2026.md:224` (proposes *setting* `VOX_SANDBOX=true`) with a one-line status banner pointing at ADR-048.

---

## Task 15: Documentation, ADR, retirement rows, bookkeeping

**Files:** create `docs/src/reference/isolation.md`, `docs/src/adr/048-interpreter-is-the-execution-and-sandbox-tier.md`; modify `docs/src/adr/{index,README}.md`, `AGENTS.md` (§VoxScript-First tier table **and** §Retired Surfaces rows), `contracts/retirement/retired-surfaces.v1.yaml`, `contracts/documentation/retired-symbols.v1.yaml`, `docs/src/architecture/where-things-live.md`, the mesh plan, `research-index.md`, plus the residual doc references from Task 12's list not yet touched.

- [ ] **Step 1: `isolation.md`** — revision 1's page, corrected: grammar table per spec §3.2 (one dir per token; `env:ro|rw`; `random:`; `deterministic`); the limits table gains the depth bound and the sentence "`--max-steps` bounds evaluated HIR nodes, not CPU time; a local run has no wall-clock bound"; exit codes `0 / 1 fault / 77 / 78 / 79 / 101 interpreter bug`; "`vox run` without `--caps` grants everything; `--caps` is opt-in locally and mandatory on the mesh"; the legacy directive is unscoped and not a boundary; `process:allow` means "runs any binary on this host as the daemon user"; what `--max-memory` does not count; the TOCTOU residual.
- [ ] **Step 2: ADR-048** — revision 1's text plus: decision 5 "every side-effecting entry point, including `import`, is gated; `db`/`repo` are pure"; consequence "the `vox-compiler → vox-crypto` edge and object iteration order are recorded maintainer decisions".
- [ ] **Step 3: Retirement rows.** `retired-surfaces.v1.yaml` + AGENTS.md §Retired Surfaces: `--isolation wasm|container|gvisor|microvm` → `--caps …`; `vox wasm run` → mesh `InterpExecutor` / plugins `vox-plugin-runtime-wasm`; `script-wasi` feature → none; `ProbeOnlyExecutor` → `InterpExecutor`; `MicroVmRuntime`/`Tier::MicroVm` → none; `VoxMeshExecPolicy` (SecretId) → trust rows; `exec_bundle_b64` → none. `retired-symbols.v1.yaml`: doc-regex rows for the same. Run `cargo run -q -p vox-cli -- ci retired-symbol-check` and the retirement parity check.
- [ ] **Step 4: AGENTS.md tier table** (as revision 1, with the `--caps fs:ro=.,net:none` row) and the remaining references: `GEMINI.md:21`, `.cursor/rules/voxscript-first-automation.mdc:28`, `docs/src/explanation/expl-architecture.md:352`, `docs/src/reference/mobile-edge-ai.md:32`, `docs/src/reference/cli.md:855,934`.
- [ ] **Step 5: Mesh plan + indexes** — Status row "Tasks 3.1–3.4 done and merged"; Known-gaps "No sandbox exists" → "Sandbox: the interpreter, per ADR-048"; Task 3.4 `[~]` → `[x]` with "Dispatch runs real source over the mesh (interpreter-first plan Task 11)"; Task 3.1 note "inbox drain: not funded by ADR-048; blocked on agent-id→EndpointId mapping"; Task 6.1 "bundle lane deleted by ADR-048 Task 10". ADR-048 into `adr/index.md`, `README.md`, `research-index.md`.
- [ ] **Step 6: Lint, fast gate, commit** — `cargo run -q -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/isolation.md docs/src/adr/048-interpreter-is-the-execution-and-sandbox-tier.md && cargo run -q -p vox-cli -- ci pre-push`. `git commit -m "docs: isolation reference, ADR-048, retirement rows, and plan bookkeeping"`.

---

## Task 16: Full gate and the honest report

- [ ] **Step 1:** `ls crates/vox-gui/ui/dist || (cd crates/vox-gui/ui && pnpm install && pnpm build)`.
- [ ] **Step 2:** `cargo run -q -p vox-cli -- ci pre-push --complete 2>&1 | tail -30`. If it halts at `doc-inventory verify`, leave the regenerated file uncommitted, re-run so clippy and toestub execute, then `git checkout -- docs/agents/doc-inventory.json`.
- [ ] **Step 3:** `cargo run -q -p vox-cli -- ci pre-push --full` (runs the slow set incl. the differential gate and the executor tests). Expected PASS, or a `KNOWN_TIER_ASYMMETRIES` list that exactly matches the gate's failures.
- [ ] **Step 4:** Cross-machine smoke **after rebuilding both ends at PROTO 2** (BLAPTOP04 is at PROTO 1 until then): `vox mesh id` on both, `vox mesh join <ticket>`, dispatch `pub fn main() { print("cross-machine") }` via a small driver; record the round-trip. If unreachable or not rebuilt, say so; do not fake the number.
- [ ] **Step 5: Report** — what shipped per task, every mutation and its result, every `KNOWN_TIER_ASYMMETRIES` entry with its reason, the two maintainer decisions still open (`vox-crypto` edge; object order), and the pre-push output verbatim. Nothing is pushed.

---

## Self-review against the spec (revision 2)

| Spec section | Task |
|---|---|
| §3.1 script-shaped routing, no auto-switch | 7 |
| §3.2 items 1–10 (grammar, non-optional caps, fatal, every entry point incl. import, fs scoping, allocator in lib, step-not-time, depth, output marker, determinism knobs) | 3, 4, 5, 6, 9 |
| §3.2 threat model (process:allow = shell; forged 77) | 9, 15 |
| §3.3 parity fixes, KNOWN list, eight goldens, EXPECT-EXIT, nextest registration, two recorded decisions | 1, 1b, 2 |
| §3.4 executor, env, caps via constructor, JobLimits, concurrency, `(peer, JobId)` cancel, PROTO/REFUSED_PROTO/postcard, exit mapping, bundle+secret_gate deletion, Dispatch payload, inbox drain withdrawn | 8, 9, 10, 11 |
| §3.5 deletions **and retirements**; `VOX_MESH_EXEC_POLICY` collision; scoped rename | 10, 12, 13, 14, 15 |
| §3.6 verify-before-flip, contract chain, secrets regen, dispatch handlers, docs, check-links | 0, 7, 12, 15 |
| §4 mutation-verified guards (denial, import, symlink, memory, caps mapping, cancel ownership) | 4, 5, 6, 9 |
| §5 PROTO incompatibility; allocator scope | 16, 6 |

**Still narrowed, stated:** `eval/builtins.rs` is not made table-driven (the fs arm now has a table; the rest does not). The inbox drain and lease-over-mesh are out of scope by decision. `crypto` and object-order parity wait on the maintainer.
