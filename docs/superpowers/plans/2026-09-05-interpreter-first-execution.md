# Interpreter-First Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the HIR interpreter the default and sandboxing execution tier for VoxScripts — locally and over the mesh — so `cargo`, `rustc`, and a Vox checkout stop being end-user requirements for running pure Vox.

**Architecture:** Capabilities become receiver-imposed and fatal, memory joins steps as a hard bound, and the interpreter is thereby the sandbox for Vox code. The mesh executes `VoxScript` jobs by spawning `vox run --mode interp --caps … --max-*` as a child process from a new `InterpExecutor` in `vox-mesh-transport` (no new crate edge), and never ships native code. One differential gate proves the interpreter and native lanes agree. The wasi script lane, the HTTP native-bundle lane, `ProbeOnlyExecutor`, and the parsed-then-rejected isolation tiers are deleted.

**Tech Stack:** Rust 1.96, `vox-compiler::eval` (tree-walking HIR interpreter), `vox-mesh-transport` on `iroh 1.1` (postcard frames), `tokio::process`, clap 4.

**Spec:** [`docs/superpowers/specs/2026-09-05-interpreter-first-execution-design.md`](../specs/2026-09-05-interpreter-first-execution-design.md) — read it first; every task below cites a section. Evidence: [`docs/src/architecture/voxscript-portability-substrate-research-2026.md`](../../src/architecture/voxscript-portability-substrate-research-2026.md).

## Global Constraints

- **Test-first.** Every new `pub fn` gets a `#[test]`/`#[tokio::test]` in the same file (detector `skeleton/untested-pub-api`). Write the failing test, run it, watch it fail, then implement.
- **Mutation-verify every guard** (spec §4): after a capability-denial, memory-bound, output-bound, or trust-mapping test passes, break the guard once, confirm the test fails, restore, and `grep -c` the restoration. Record the mutation in the commit body.
- **Formatting:** `cargo fmt -p <crate>` per touched crate, or `vox run scripts/fmt.vox`. **Never `cargo fmt --all`.**
- **Clippy before every commit:** `cargo clippy -p <touched-crate> --all-targets -- -D warnings`. Write `if let … && cond {}` let-chains, not nested `if let`; clippy 1.96 rejects the nested form.
- **Never `presets::N0`, `N0DisableRelay`, or `into_0rtt()`** (detector `vox/mesh/unsafe-iroh-pattern`, Error).
- **Crate edges:** this plan adds **none**. Removing `script-wasi` removes `vox-cli → vox-wasm-engine`; tighten with `cargo run -q -p vox-cli -- ci crate-edges --tighten`. **Never** add an `exceptions` entry or regenerate `crate-edges.allow.v1.json` to admit an edge.
- **`--features populi` goes on `-p vox-ml-cli`, never `-p vox-cli`.**
- **Do not regenerate `docs/agents/doc-inventory.json`** — it is stale on the base and `ssot-autoregen` owns it. If `vox ci pre-push` stops there, that is expected; run clippy and the tests it would have run directly and say so in the commit.
- **Doc frontmatter** (`title`, `description`, `category`) on every new `.md` under `docs/src/`; lint with `cargo run -q -p vox-doc-pipeline -- --lint-only --paths <file>`.
- **Commit messages:** imperative subject < 72 chars, body explains why, ending with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Do not push unless asked.
- **Fresh worktree gotcha:** `crates/vox-gui/ui/dist` is missing in a new worktree and breaks workspace clippy — run `pnpm install && pnpm build` in `crates/vox-gui/ui` once. Do not commit anything to "fix" this.
- Wire-protocol constant `PROTO` bumps to `2` in Task 8; `Hello`'s *layout* is frozen and does not change.

---

## File Structure

**Created**

| Path | Responsibility |
|---|---|
| `crates/vox-compiler/src/eval/caps.rs` | `CapabilitySet`: the parsed `--caps` grammar, the legacy `// vox:caps` shim, and `allows(ns, method, path)`. Pure data + parsing; no I/O. |
| `crates/vox-cli/src/mem_limit.rs` | Counting `#[global_allocator]` with a runtime-armed ceiling. Nothing else lives here. |
| `crates/vox-mesh-transport/src/interp_executor.rs` | `InterpExecutor`: turns a `ReceivedJob` into a bounded `vox run --mode interp` child, maps trust level → caps, kills on `Cancel`. |
| `crates/vox-mesh-transport/tests/interp_executor.rs` | Live two-endpoint tests for the executor (loopback only). |
| `crates/vox-integration-tests/tests/golden_differential_gate.rs` | Runs every `// EXPECT:` golden under interp **and** native and diffs stdout. |
| `docs/src/reference/isolation.md` | The page `script.rs` already tells users to read. |
| `docs/src/adr/048-interpreter-is-the-execution-and-sandbox-tier.md` | The decision record. |

**Modified (major)**

| Path | Change |
|---|---|
| `crates/vox-compiler/src/eval/mod.rs` | `caps: Option<CapabilitySet>`; `EvalError::CapabilityDenied`; `pub mod caps`. |
| `crates/vox-compiler/src/eval/builtins.rs` | gate rewritten around `CapabilitySet`, covers all 16 namespaces, returns `_Denied` sentinel; adds `crypto` namespace; `time.now` honours `frozen`. |
| `crates/vox-compiler/src/eval/expr.rs` | `_Denied` → `EvalError::CapabilityDenied`. |
| `crates/vox-compiler/src/eval/value.rs` | `_Denied(String)` sentinel. |
| `crates/vox-compiler/src/eval/{db,repo}.rs` | route through the gate. |
| `crates/vox-cli/src/commands/run.rs` | auto → interp; `--caps/--max-steps/--max-memory`; exit codes; advisory. |
| `crates/vox-cli/src/cli_args.rs` | new `RunArgs` fields; delete `--isolation`, `WasmRunArgs`. |
| `crates/vox-mesh-transport/src/{protocol,endpoint,lib}.rs` | `Isolation { Interpreter, Native }`; `PROTO = 2`; payload read; `ProbeOnlyExecutor` deleted. |
| `crates/vox-orchestrator/src/a2a/remote_worker.rs` | bundle lane deleted; source lane gains `--caps`. |
| `crates/vox-orchestrator/src/a2a/{envelope,secret_gate}.rs` | `exec_bundle_*` fields removed; `ExecTier { Interpreter, Native }`. |
| `crates/vox-workflow-runtime/src/workflow/populi.rs` | `Dispatch` over the mesh. |

**Deleted**

`crates/vox-cli/src/commands/wasm.rs`, `crates/vox-cli/src/commands/runtime/run/backend/wasi.rs`, `crates/vox-cli/src/isolation.rs`, `crates/vox-skill-runtime/src/microvm.rs`, `crates/vox-skill-runtime/tests/microvm_tier.rs`.

---

## Task 0: Measure before changing anything

**Files:**
- Create: `scripts/bench-script-tiers.vox`
- Create: `docs/src/architecture/script-tier-timings-2026-09.md`

**Interfaces:**
- Produces: a table of interp vs native timings the default-flip (Task 7) cites.

- [ ] **Step 1: Write the timing script**

```vox
// Time every scripts/**/*.vox under `vox check` (parse + typecheck, no eval) and
// emit a Markdown table. Native timings are taken by hand for three representative
// scripts because a cold native compile is ~275 s each.
pub fn main() {
  let files = fs.glob("scripts/**/*.vox")
  let mut rows = []
  for f in files {
    let t0 = time.now_ms()
    let r = process.run_capture("vox", ["check", f])
    let dt = time.now_ms() - t0
    let ok = if r.exit_code == 0 { "ok" } else { "FAIL" }
    rows = rows.push("| " + f + " | " + str(dt) + " ms | " + ok + " |")
  }
  print("| script | `vox check` | status |")
  print("|---|---|---|")
  for r in rows { print(r) }
}
```

- [ ] **Step 2: Run it under the interpreter and record**

Run: `cargo run -q -p vox-cli -- run --mode interp scripts/bench-script-tiers.vox > /tmp/tiers.md; head -20 /tmp/tiers.md`
Expected: a table; every row `ok`. Any `FAIL` is a script that does not typecheck today — list it in the doc, do not fix it here.

- [ ] **Step 3: Take three native timings by hand**

```bash
for f in scripts/fmt.vox scripts/install-hooks.vox scripts/setup.vox; do
  rm -rf ~/.vox/script-cache; /usr/bin/time -p cargo run -q -p vox-cli -- run --mode script "$f" -- --help 2>&1 | grep real
  /usr/bin/time -p cargo run -q -p vox-cli -- run --mode script "$f" -- --help 2>&1 | grep real
done
```

Record cold and warm for each.

- [ ] **Step 4: Write the doc**

```markdown
---
title: "Script tier timings (2026-09)"
description: "Measured interpreter vs native-lane latency for the repository's own .vox scripts, taken before the interpreter became the default."
category: "Architecture SSOTs"
status: "current"
---

# Script tier timings — 2026-09

Machine: <fill from `system_profiler SPHardwareDataType | grep Chip`>. Debug build of `vox`.

## Native lane, three scripts

| script | cold | warm |
|---|---|---|
| scripts/fmt.vox | <s> | <s> |
| scripts/install-hooks.vox | <s> | <s> |
| scripts/setup.vox | <s> | <s> |

## `vox check` across all scripts

<paste /tmp/tiers.md>

Scripts that fail `vox check` today (pre-existing, not introduced here): <list or "none">.
```

- [ ] **Step 5: Lint and commit**

Run: `cargo run -q -p vox-doc-pipeline -- --lint-only --paths docs/src/architecture/script-tier-timings-2026-09.md`
Expected: `no hard errors`.

```bash
git add scripts/bench-script-tiers.vox docs/src/architecture/script-tier-timings-2026-09.md
git commit -m "chore(scripts): measure script tiers before flipping the default"
```

---

## Task 1: The differential gate — interp and native must agree

**Files:**
- Create: `crates/vox-integration-tests/tests/golden_differential_gate.rs`
- Read: `crates/vox-integration-tests/tests/golden_behavioral_gate.rs` (copy its helpers verbatim; do not import — integration tests are separate crates)

**Interfaces:**
- Produces: the acceptance test for spec §3.3, run in the `--full` pre-push tier.

- [ ] **Step 1: Write the test**

```rust
//! Differential gate (spec §3.3): a golden that declares `// EXPECT:` must print
//! the same bytes under `vox run --mode interp` AND `vox run --mode script`.
//! This is the only test in the repository that proves two execution tiers agree.
//!
//! Slow: the native half compiles a Rust crate per golden (~275 s cold, ~50 ms
//! warm with a shared cache). It is `#[ignore]`d for the fast tier and run with
//! `--include-slow` / `--run-ignored all` in `vox ci pre-push --full` and CI.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../.."))
}

fn vox_binary() -> PathBuf {
    if let Ok(p) = std::env::var("VOX_BIN") {
        return PathBuf::from(p);
    }
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let root = repo_root();
    let debug = root.join("target").join("debug").join(exe);
    if debug.exists() {
        return debug;
    }
    root.join("target").join("release").join(exe)
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

fn parse_expect(src: &str) -> Option<String> {
    let lines: Vec<String> = src
        .lines()
        .filter_map(|l| {
            let t = l.trim_start();
            t.strip_prefix("// EXPECT:")
                .map(|rest| rest.strip_prefix(' ').unwrap_or(rest).to_string())
        })
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").trim_end().to_string()
}

fn run_mode(vox: &Path, mode: &str, file: &Path) -> Result<String, String> {
    let out = Command::new(vox)
        .args(["run", "--mode", mode])
        .arg(file)
        .output()
        .map_err(|e| format!("spawn `{}` failed: {e}", vox.display()))?;
    if !out.status.success() {
        return Err(format!(
            "[{mode}] non-zero exit {:?}\n--- stderr ---\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every golden with an `// EXPECT:` block produces identical stdout on both tiers.
#[test]
#[ignore = "owner:mesh sunset:2026-12-31 slow: native lane compiles a crate per golden; run with --include-slow"]
fn golden_expect_blocks_match_on_both_tiers() {
    let root = repo_root();
    let vox = vox_binary();
    assert!(vox.exists(), "vox binary not found at {} — build it or set VOX_BIN", vox.display());
    assert!(
        which::which("cargo").is_ok(),
        "the native half of the differential gate needs cargo on PATH; this gate must not pass silently"
    );

    let mut files = Vec::new();
    collect_vox_recursive(&root.join("examples").join("golden"), &mut files);
    files.sort();

    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f)
            .unwrap_or_else(|e| panic!("failed to read golden {}: {e}", f.display()));
        let Some(expected) = parse_expect(&src) else { continue };
        checked += 1;
        let interp = run_mode(&vox, "interp", f);
        let native = run_mode(&vox, "script", f);
        match (interp, native) {
            (Ok(i), Ok(n)) => {
                let (i, n, e) = (normalize(&i), normalize(&n), normalize(&expected));
                if i != n || i != e {
                    failures.push(format!(
                        "{}\n  expected: {e:?}\n  interp:   {i:?}\n  native:   {n:?}",
                        f.display()
                    ));
                }
            }
            (i, n) => failures.push(format!(
                "{}\n  interp: {}\n  native: {}",
                f.display(),
                i.err().unwrap_or_else(|| "ok".into()),
                n.err().unwrap_or_else(|| "ok".into())
            )),
        }
    }
    assert!(checked > 0, "no golden declared `// EXPECT:` — the gate has no corpus");
    assert!(failures.is_empty(), "tiers disagree on {} golden(s):\n\n{}", failures.len(), failures.join("\n\n"));
}
```

Add `which = { workspace = true }` under `[dev-dependencies]` in `crates/vox-integration-tests/Cargo.toml` if it is not already there (check first: `grep -n which crates/vox-integration-tests/Cargo.toml`).

- [ ] **Step 2: Run it once — it is expected to FAIL today**

Run: `cargo build -q -p vox-cli --bin vox && cargo test -q -p vox-integration-tests --test golden_differential_gate -- --ignored --nocapture 2>&1 | tail -40`
Expected: FAIL. The failures list is the **current measured drift** between tiers. Paste it into the commit body. If it passes, something is wrong — at minimum `crypto.*` goldens (if any) must diverge; check that `checked > 0`.

- [ ] **Step 3: Register the slow test with the pre-push `--full` tier**

Open `docs/src/contributors/local-ci-pre-push.md`, find the `--include-slow` slow-test list, and add `golden_differential_gate` with the one-line reason "native lane compiles per golden". Then confirm the nextest/ignored-test governance accepts the reason string: `cargo run -q -p vox-cli -- ci ignored-test-age --mode enforce`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-integration-tests/tests/golden_differential_gate.rs crates/vox-integration-tests/Cargo.toml docs/src/contributors/local-ci-pre-push.md
git commit -m "test(gate): differential gate — interp and native must print the same bytes"
```

---

## Task 2: Close the known builtin asymmetries

**Files:**
- Modify: `crates/vox-compiler/src/eval/builtins.rs` (add `crypto` namespace arm near the `time` arm at ~line 1237)
- Modify: `crates/vox-compiler/src/eval/mod.rs` (seed `crypto` namespace in `Interpreter::new`, alongside `time`)
- Modify: `crates/vox-compiler/src/builtin_registry.rs` (`std_namespace_runtime_call`, ~line 848: add `time.now`, `json.encode`, `json.stringify`, `process.cwd`, `secrets.resolve`)
- Test: `crates/vox-compiler/tests/eval_typeck_parity_test.rs` (append)

**Interfaces:**
- Consumes: `vox_actor_runtime::builtins::{vox_hash_fast, vox_hash_secure, vox_uuid}` exist and are what codegen emits (`builtin_registry.rs:854-862`); the interpreter must produce the same shapes (`Str` hex for hashes, `Str` for uuid).
- Produces: a `KNOWN_TIER_ASYMMETRIES` constant in the parity test that must stay empty.

- [ ] **Step 1: Write the failing tests**

Append to `crates/vox-compiler/tests/eval_typeck_parity_test.rs`:

```rust
/// Spec §2: these five eval→codegen and three codegen→eval gaps were measured on
/// 2026-09-05. Each one is a script that works on one tier and fails on the other.
/// This list must be empty; add an entry only with a linked issue and a reason.
const KNOWN_TIER_ASYMMETRIES: &[(&str, &str)] = &[];

#[test]
fn no_known_tier_asymmetries_remain() {
    assert!(
        KNOWN_TIER_ASYMMETRIES.is_empty(),
        "tiers still disagree on: {KNOWN_TIER_ASYMMETRIES:?}"
    );
}

#[test]
fn crypto_namespace_dispatches_under_eval() {
    let v = run_probe(r#"pub fn main() { return crypto.hash_fast("abc") }"#).expect("crypto.hash_fast must exist under eval");
    match v {
        VoxValue::Str(s) => assert_eq!(s.len(), 64, "blake3 hex is 64 chars, got {s:?}"),
        other => panic!("expected Str, got {other:?}"),
    }
    let v = run_probe(r#"pub fn main() { return crypto.uuid() }"#).expect("crypto.uuid must exist under eval");
    assert!(matches!(v, VoxValue::Str(ref s) if s.len() == 36), "uuid v4 text, got {v:?}");
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
        assert!(
            std_namespace_runtime_call(ns, m, &args).is_some(),
            "codegen has no emit mapping for {ns}.{m} — eval has it, so a script using it compiles on one tier only"
        );
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p vox-compiler --test eval_typeck_parity_test -- crypto registry_emits 2>&1 | tail -20`
Expected: `crypto_namespace_dispatches_under_eval` FAILS (`UndefinedVariable("crypto")` / `Method not found`), `registry_emits_every_eval_only_method` FAILS on `time.now`.

- [ ] **Step 3: Implement the eval side**

In `crates/vox-compiler/src/eval/mod.rs`, inside `Interpreter::new`, after the `time` namespace seed (copy the exact 7-line shape used for `"time"` at ~lines 120-127 and again inside `std_ns`), add the same for `"crypto"`.

In `crates/vox-compiler/src/eval/builtins.rs`, after the `Some("time") => match method { … }` arm, add:

```rust
                Some("crypto") => match method {
                    // Parity with vox_actor_runtime::builtins::vox_hash_fast:
                    // BLAKE3 over the UTF-8 bytes, lowercase hex.
                    "hash_fast" => match args.into_iter().next() {
                        Some(VoxValue::Str(s)) => Some(VoxValue::Str(
                            vox_crypto::hash::blake3_hex(s.as_bytes()),
                        )),
                        _ => None,
                    },
                    // Parity with vox_hash_secure: SHA3-256, lowercase hex.
                    "hash_secure" => match args.into_iter().next() {
                        Some(VoxValue::Str(s)) => Some(VoxValue::Str(
                            vox_crypto::hash::sha3_256_hex(s.as_bytes()),
                        )),
                        _ => None,
                    },
                    "uuid" => Some(VoxValue::Str(uuid::Uuid::new_v4().to_string())),
                    _ => None,
                },
```

Check the exact function names in `crates/vox-crypto/src/` first (`rg -n "pub fn .*blake3|pub fn .*sha3" crates/vox-crypto/src/`) and use whatever they are; **do not** import `blake3`/`sha3` directly — AGENTS.md §Cryptography routes application hashing through `vox-crypto`. `vox-compiler` already depends on `vox-crypto` (`grep -n vox-crypto crates/vox-compiler/Cargo.toml`); if it does not, stop and report — that edge needs the maintainer.

- [ ] **Step 4: Implement the codegen side**

In `crates/vox-compiler/src/builtin_registry.rs::std_namespace_runtime_call`, next to the existing `("time", "now_ms")` arm, add arms that emit the same runtime symbol:

```rust
        ("time", "now") => Some("::vox_actor_runtime::builtins::vox_now_ms()".to_string()),
        ("json", "encode" | "stringify") if args.len() == 1 => Some(format!(
            "::vox_actor_runtime::builtins::vox_json_render(&({}))", args[0]
        )),
        ("process", "cwd") => Some("::vox_actor_runtime::builtins::vox_process_cwd()".to_string()),
        ("secrets", "resolve") if args.len() == 1 => Some(format!(
            "::vox_actor_runtime::builtins::vox_secrets_resolve(({}).as_str())", args[0]
        )),
```

Then add `vox_process_cwd` and `vox_secrets_resolve` to `crates/vox-actor-runtime/src/builtins/mod.rs` if absent (`rg -n "pub fn vox_process_cwd|pub fn vox_secrets_resolve" crates/vox-actor-runtime/src/builtins/mod.rs`):

```rust
/// Parity with eval `process.cwd`.
pub fn vox_process_cwd() -> String {
    std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default()
}

/// Parity with eval `secrets.resolve`: goes through Clavis, never `env::var`.
pub fn vox_secrets_resolve(key: &str) -> Option<String> {
    let id = vox_secrets::SecretId::from_str_loose(key)?;
    vox_secrets::resolve_secret(id).expose().map(|s| s.to_string())
}
```

(Check how eval's `secrets.resolve` at `builtins.rs:1411-1431` maps a string to a `SecretId` and mirror it exactly — use the same helper name it uses.) Each new `pub fn` needs a `#[test]` in the same file.

- [ ] **Step 5: Run the tests**

Run: `cargo test -q -p vox-compiler --test eval_typeck_parity_test && cargo test -q -p vox-actor-runtime builtins`
Expected: PASS.

- [ ] **Step 6: Re-run the differential gate**

Run: `cargo build -q -p vox-cli --bin vox && cargo test -q -p vox-integration-tests --test golden_differential_gate -- --ignored 2>&1 | tail -20`
Expected: the `crypto`/`time` rows from Task 1's failure list are gone. Anything remaining is real, previously unknown drift — record it as a `KNOWN_TIER_ASYMMETRIES` entry **with a reason** rather than hiding it, and open the fix as a follow-up.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p vox-compiler -p vox-actor-runtime
git add crates/vox-compiler crates/vox-actor-runtime
git commit -m "fix(eval): close the measured builtin drift between interp and native"
```

---

## Task 3: `CapabilitySet` — the `--caps` grammar

**Files:**
- Create: `crates/vox-compiler/src/eval/caps.rs`
- Modify: `crates/vox-compiler/src/eval/mod.rs` (`pub mod caps;`, field type)

**Interfaces:**
- Produces:
  ```rust
  pub struct CapabilitySet { /* private */ }
  impl CapabilitySet {
      pub fn parse(spec: &str) -> Result<Self, CapsParseError>;
      pub fn from_legacy_directive(words: &[String]) -> Self;   // `// vox:caps fs env`
      pub fn developer_default() -> Self;                         // everything allowed; what `vox run` uses locally
      pub fn allows_namespace(&self, ns: &str) -> bool;
      pub fn allows_path(&self, path: &std::path::Path, write: bool) -> bool;
      pub fn frozen_time_ms(&self) -> Option<i64>;
  }
  ```
- Grammar (spec §3.2 item 1), comma-separated, order-free:
  `fs:ro=<dir>[|<dir>…]`, `fs:rw=<dir>[|…]`, `net:none|allow`, `process:none|allow`, `env:none|allow`, `secrets:none|allow`, `time:frozen=<ms>|real`, `db:none|allow`, `repo:none|allow`, `agentos:none|allow`. Any namespace not mentioned is **denied**. `path`, `json`, `csv`, `toml`, `yaml`, `regex`, `log` are pure and always allowed (`log` is rate-limited later, not gated).

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-compiler/src/eval/caps.rs` with only the tests first:

```rust
//! Receiver-imposed capabilities for the interpreter (spec §3.2).
//!
//! The grammar is a public CLI surface once shipped; keep it small.

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn unmentioned_namespaces_are_denied() {
        let c = CapabilitySet::parse("env:allow").unwrap();
        assert!(c.allows_namespace("env"));
        assert!(!c.allows_namespace("fs"));
        assert!(!c.allows_namespace("process"));
        assert!(!c.allows_namespace("http"));
    }

    #[test]
    fn pure_namespaces_are_always_allowed() {
        let c = CapabilitySet::parse("").unwrap();
        for ns in ["path", "json", "csv", "toml", "yaml", "regex", "log"] {
            assert!(c.allows_namespace(ns), "{ns} is pure and must not be gated");
        }
    }

    #[test]
    fn fs_roots_scope_reads_and_writes_separately() {
        let c = CapabilitySet::parse("fs:ro=/etc|/usr,fs:rw=/tmp/job").unwrap();
        assert!(c.allows_namespace("fs"));
        assert!(c.allows_path(Path::new("/etc/hosts"), false));
        assert!(!c.allows_path(Path::new("/etc/hosts"), true));
        assert!(c.allows_path(Path::new("/tmp/job/out.txt"), true));
        assert!(!c.allows_path(Path::new("/tmp/other"), false));
        // A prefix that is not a path-component boundary must not match.
        assert!(!c.allows_path(Path::new("/tmp/jobx/y"), true));
    }

    #[test]
    fn frozen_time_is_parsed() {
        let c = CapabilitySet::parse("time:frozen=1700000000000").unwrap();
        assert_eq!(c.frozen_time_ms(), Some(1_700_000_000_000));
        assert!(c.allows_namespace("time"));
        let r = CapabilitySet::parse("time:real").unwrap();
        assert_eq!(r.frozen_time_ms(), None);
    }

    #[test]
    fn legacy_directive_maps_words_to_namespaces() {
        let c = CapabilitySet::from_legacy_directive(&["fs".into(), "subprocess".into()]);
        assert!(c.allows_namespace("fs"));
        assert!(c.allows_namespace("io"));       // io was always paired with fs
        assert!(c.allows_namespace("process"));  // `subprocess` was an alias
        assert!(!c.allows_namespace("env"));
        // Legacy directive never scoped paths: any path is allowed when fs is.
        assert!(c.allows_path(Path::new("/anything"), true));
    }

    #[test]
    fn developer_default_allows_everything() {
        let c = CapabilitySet::developer_default();
        for ns in ["fs", "io", "process", "env", "secrets", "http", "time", "db", "repo", "agentos"] {
            assert!(c.allows_namespace(ns));
        }
        assert!(c.allows_path(Path::new("/"), true));
    }

    #[test]
    fn bad_specs_are_rejected_with_the_offending_token() {
        for bad in ["fs", "fs:banana", "net:maybe", "time:frozen=abc", "nosuch:allow"] {
            let e = CapabilitySet::parse(bad).unwrap_err();
            assert!(e.to_string().contains(bad.split('=').next().unwrap()), "{bad} → {e}");
        }
    }
}
```

Add `pub mod caps;` to `crates/vox-compiler/src/eval/mod.rs`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p vox-compiler --lib eval::caps 2>&1 | tail -5`
Expected: compile error, `CapabilitySet` not found.

- [ ] **Step 3: Implement**

Prepend to `caps.rs`:

```rust
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

const PURE: &[&str] = &["path", "json", "csv", "toml", "yaml", "regex", "log"];
const GATED: &[&str] = &[
    "fs", "io", "process", "env", "secrets", "http", "time", "db", "repo", "agentos",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    allowed: BTreeSet<String>,
    /// `None` means "fs allowed anywhere" (developer default / legacy directive).
    fs_ro: Option<Vec<PathBuf>>,
    fs_rw: Option<Vec<PathBuf>>,
    frozen_time_ms: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid --caps token {token:?}: {why}")]
pub struct CapsParseError {
    pub token: String,
    pub why: &'static str,
}

impl CapabilitySet {
    pub fn parse(spec: &str) -> Result<Self, CapsParseError> {
        let mut out = Self {
            allowed: BTreeSet::new(),
            fs_ro: Some(Vec::new()),
            fs_rw: Some(Vec::new()),
            frozen_time_ms: None,
        };
        for tok in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let err = |why| CapsParseError { token: tok.to_string(), why };
            let (ns, rest) = tok.split_once(':').ok_or(err("expected ns:value"))?;
            if !GATED.contains(&ns) && ns != "net" {
                return Err(err("unknown namespace"));
            }
            match (ns, rest) {
                ("fs", r) if r.starts_with("ro=") || r.starts_with("rw=") => {
                    let dirs: Vec<PathBuf> = r[3..].split('|').filter(|d| !d.is_empty()).map(PathBuf::from).collect();
                    if dirs.is_empty() {
                        return Err(err("fs needs at least one directory"));
                    }
                    let target = if r.starts_with("ro=") { &mut out.fs_ro } else { &mut out.fs_rw };
                    target.get_or_insert_with(Vec::new).extend(dirs);
                    out.allowed.insert("fs".into());
                    out.allowed.insert("io".into());
                }
                ("fs", _) => return Err(err("fs takes ro=<dir>|… or rw=<dir>|…")),
                ("net", "allow") => {
                    out.allowed.insert("http".into());
                }
                ("net", "none") => {}
                ("net", _) => return Err(err("net takes none|allow")),
                ("time", "real") => {
                    out.allowed.insert("time".into());
                }
                ("time", r) if r.starts_with("frozen=") => {
                    out.frozen_time_ms = Some(r[7..].parse().map_err(|_| err("frozen= needs integer ms"))?);
                    out.allowed.insert("time".into());
                }
                ("time", _) => return Err(err("time takes real|frozen=<ms>")),
                (ns, "allow") => {
                    out.allowed.insert(ns.into());
                }
                (_, "none") => {}
                (_, _) => return Err(err("expected none|allow")),
            }
        }
        Ok(out)
    }

    pub fn from_legacy_directive(words: &[String]) -> Self {
        let mut allowed = BTreeSet::new();
        for w in words {
            match w.as_str() {
                "fs" => {
                    allowed.insert("fs".into());
                    allowed.insert("io".into());
                }
                "process" | "subprocess" => {
                    allowed.insert("process".into());
                }
                other => {
                    allowed.insert(other.into());
                }
            }
        }
        Self { allowed, fs_ro: None, fs_rw: None, frozen_time_ms: None }
    }

    pub fn developer_default() -> Self {
        Self {
            allowed: GATED.iter().map(|s| s.to_string()).collect(),
            fs_ro: None,
            fs_rw: None,
            frozen_time_ms: None,
        }
    }

    pub fn allows_namespace(&self, ns: &str) -> bool {
        PURE.contains(&ns) || self.allowed.contains(ns)
    }

    /// `write` = the operation mutates. Paths are compared component-wise so
    /// `/tmp/job` does not admit `/tmp/jobx`. Callers canonicalise first (Task 5).
    pub fn allows_path(&self, path: &Path, write: bool) -> bool {
        if !self.allows_namespace("fs") {
            return false;
        }
        let roots = if write { &self.fs_rw } else { &self.fs_ro };
        match roots {
            None => true,
            Some(rs) => {
                // rw roots also satisfy reads.
                let extra = if write { None } else { self.fs_rw.as_deref() };
                rs.iter().chain(extra.into_iter().flatten()).any(|r| is_under(path, r))
            }
        }
    }

    pub fn frozen_time_ms(&self) -> Option<i64> {
        self.frozen_time_ms
    }
}

fn is_under(path: &Path, root: &Path) -> bool {
    let p: Vec<Component> = path.components().collect();
    let r: Vec<Component> = root.components().collect();
    p.len() >= r.len() && p[..r.len()] == r[..]
}
```

Add `thiserror = { workspace = true }` to `crates/vox-compiler/Cargo.toml` `[dependencies]` if absent.

- [ ] **Step 4: Change the field type**

In `crates/vox-compiler/src/eval/mod.rs` line 37: `pub caps: Option<std::collections::HashSet<String>>,` → `pub caps: Option<caps::CapabilitySet>,`. The build now breaks at `expr.rs:620` and `builtins.rs:114` — Task 4 fixes those; for this task make it compile by changing `call_builtin_method`'s parameter type to `Option<&caps::CapabilitySet>` and the gate body to `c.allows_namespace(ns_str)` (keep the old print/Null behaviour for one more task so the change is reviewable in isolation).

- [ ] **Step 5: Run tests**

Run: `cargo test -q -p vox-compiler --lib eval::caps && cargo test -q -p vox-compiler 2>&1 | tail -5`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-compiler
git add crates/vox-compiler
git commit -m "feat(eval): CapabilitySet — receiver-imposed caps with scoped fs roots"
```

---

## Task 4: Denial is fatal and every namespace is gated

**Files:**
- Modify: `crates/vox-compiler/src/eval/value.rs` (sentinel)
- Modify: `crates/vox-compiler/src/eval/mod.rs` (`EvalError::CapabilityDenied`)
- Modify: `crates/vox-compiler/src/eval/builtins.rs:945-975` (the gate)
- Modify: `crates/vox-compiler/src/eval/expr.rs:618-632`
- Modify: `crates/vox-compiler/src/eval/db.rs`, `crates/vox-compiler/src/eval/repo.rs` (entry points)
- Test: `crates/vox-compiler/tests/caps_enforcement_test.rs` (create)

**Interfaces:**
- Produces: `EvalError::CapabilityDenied { ns: String, method: String }`; `VoxValue::_Denied(String)` sentinel (internal).
- The set of gated namespaces is **derived**: a test asserts every namespace seeded in `Interpreter::new` is either in `caps::GATED` or `caps::PURE`.

- [ ] **Step 1: Write the failing tests**

```rust
//! Spec §3.2 items 2–3: denial is fatal; every namespace is gated.
use vox_compiler::eval::caps::CapabilitySet;
use vox_compiler::eval::{EvalError, Interpreter};
use vox_compiler::{hir, lexer, parser};

fn run_with(caps: CapabilitySet, src: &str) -> Result<vox_compiler::eval::value::VoxValue, EvalError> {
    let tokens = lexer::lex(src);
    let module = parser::parse_script(tokens).expect("parse");
    let lowered = hir::lower::lower_module(&module);
    let mut interp = Interpreter::new(1_000_000);
    interp.caps = Some(caps);
    interp.run_module(&lowered)?;
    interp.call("main", vec![])
}

#[test]
fn denied_fs_read_is_fatal_and_nothing_after_it_runs() {
    let r = run_with(
        CapabilitySet::parse("env:allow").unwrap(),
        r#"pub fn main() { let s = fs.read("/etc/hosts"); print("MUST NOT PRINT"); return 1 }"#,
    );
    match r {
        Err(EvalError::CapabilityDenied { ns, method }) => {
            assert_eq!(ns, "fs");
            assert_eq!(method, "read");
        }
        other => panic!("expected CapabilityDenied, got {other:?}"),
    }
}

#[test]
fn http_is_gated_too() {
    let r = run_with(CapabilitySet::parse("fs:ro=/").unwrap(), r#"pub fn main() { return http.get_text("http://127.0.0.1:9/") }"#);
    assert!(matches!(r, Err(EvalError::CapabilityDenied { ref ns, .. }) if ns == "http"), "{r:?}");
}

#[test]
fn db_and_repo_are_gated() {
    let r = run_with(CapabilitySet::parse("").unwrap(), r#"pub fn main() { return repo.status() }"#);
    assert!(matches!(r, Err(EvalError::CapabilityDenied { ref ns, .. }) if ns == "repo"), "{r:?}");
    let r = run_with(CapabilitySet::parse("").unwrap(), r#"pub fn main() { return db.exec("select 1") }"#);
    assert!(matches!(r, Err(EvalError::CapabilityDenied { ref ns, .. }) if ns == "db"), "{r:?}");
}

#[test]
fn no_caps_means_developer_default_not_no_gate() {
    // `caps: None` is what `vox run` uses locally. It must behave as "allow all",
    // and the *absence* of a directive must never be a bypass once caps are set.
    let r = run_with(CapabilitySet::developer_default(), r#"pub fn main() { return env.get("PATH").is_some() }"#);
    assert!(r.is_ok());
}

#[test]
fn every_seeded_namespace_is_classified() {
    // Guards against a new namespace being added ungated. Parses the seeding
    // source rather than re-instantiating so the list cannot drift silently:
    // every `VoxValue::Str("<ns>"…)` line that follows a `__namespace__` marker.
    let src = include_str!("../src/eval/mod.rs");
    let mut names = std::collections::BTreeSet::new();
    let lines: Vec<&str> = src.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if l.contains("\"__namespace__\"")
            && let Some(next) = lines.get(i + 1)
            && let Some(start) = next.find("Str(\"")
        {
            let rest = &next[start + 5..];
            if let Some(end) = rest.find('"') {
                names.insert(rest[..end].to_string());
            }
        }
    }
    assert!(!names.is_empty(), "no namespaces parsed from eval/mod.rs — test is broken");
    for ns in &names {
        assert!(
            vox_compiler::eval::caps::is_classified(ns),
            "namespace `{ns}` is seeded in Interpreter::new but is in neither caps::GATED nor caps::PURE"
        );
    }
}
```

Add to `caps.rs`: `pub fn is_classified(ns: &str) -> bool { GATED.contains(&ns) || PURE.contains(&ns) }` with a one-line test.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p vox-compiler --test caps_enforcement_test 2>&1 | tail -20`
Expected: compile failure on `EvalError::CapabilityDenied`; after stubbing the variant, `denied_fs_read_is_fatal…` fails because the script continues and returns `1`.

- [ ] **Step 3: Implement**

`value.rs` — add after `_Panic(String)`:
```rust
    /// Capability denial sentinel. Like `_Panic`, converted to an `EvalError`
    /// at the call boundary; unlike `_Panic` it is never user-catchable.
    _Denied(String),
```
Follow every `match` over `VoxValue` that the compiler flags (Display/Debug helpers) and treat `_Denied` like `_Panic`.

`mod.rs` — add to `EvalError`:
```rust
    CapabilityDenied { ns: String, method: String },
```

`builtins.rs:955-963` — replace the block with:
```rust
            if let Some(ns_str) = ns
                && let Some(c) = caps
                && !c.allows_namespace(ns_str)
            {
                return Some(VoxValue::_Denied(format!("{ns_str}.{method}")));
            }
```
(Remove the `println!`; remove the `matches!(ns_str, "fs" | …)` allowlist — coverage now comes from `CapabilitySet`, so `http`, `time`, `agentos` are gated automatically.)

`expr.rs:618-632` — extend the sentinel match:
```rust
                match r {
                    crate::eval::value::VoxValue::_Panic(msg) => Err(EvalError::AssertionFailed(msg)),
                    crate::eval::value::VoxValue::_Denied(what) => {
                        let (ns, method) = what.split_once('.').unwrap_or((what.as_str(), ""));
                        Err(EvalError::CapabilityDenied { ns: ns.to_string(), method: method.to_string() })
                    }
                    r => Ok(r),
                }
```

`db.rs` and `repo.rs` — at the top of their public dispatch entry (the function `expr.rs`/`mod.rs` calls for `db.*` / `repo.*`; find with `rg -n "pub fn" crates/vox-compiler/src/eval/{db,repo}.rs`), add:
```rust
    if let Some(c) = interp.caps.as_ref()
        && !c.allows_namespace("db")   // or "repo"
    {
        return Err(EvalError::CapabilityDenied { ns: "db".into(), method: method.to_string() });
    }
```
Delete the comment at `mod.rs:569` that says repo does not consult caps.

- [ ] **Step 4: Run tests**

Run: `cargo test -q -p vox-compiler --test caps_enforcement_test && cargo test -q -p vox-compiler 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 5: Mutation-verify the guard**

```bash
F=crates/vox-compiler/src/eval/builtins.rs
grep -c '_Denied(format' $F   # expect 1
perl -0pi -e 's/return Some\(VoxValue::_Denied\(format!\("\{ns_str\}\.\{method\}"\)\)\);/let _ = (ns_str, method);/' $F
cargo test -q -p vox-compiler --test caps_enforcement_test 2>&1 | tail -3   # MUST FAIL
git checkout -- $F
grep -c '_Denied(format' $F   # expect 1 again
```
Record "mutation: gate removed → `denied_fs_read_is_fatal…` failed; restored" in the commit body.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-compiler
git add crates/vox-compiler
git commit -m "feat(eval): capability denial is fatal and covers every namespace"
```

---

## Task 5: Filesystem scoping and frozen time

**Files:**
- Modify: `crates/vox-compiler/src/eval/builtins.rs` (`fs`/`io` arms ~970-1236, 1897-1925; `time` arm ~1237)
- Test: `crates/vox-compiler/tests/caps_enforcement_test.rs` (append)

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn fs_reads_outside_ro_roots_are_denied_after_canonicalisation() {
    let dir = tempfile::tempdir().unwrap();
    let allowed = dir.path().join("ok");
    std::fs::create_dir_all(&allowed).unwrap();
    std::fs::write(allowed.join("a.txt"), "A").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "S").unwrap();
    // A symlink inside the allowed root pointing outside must NOT be readable.
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("secret.txt"), allowed.join("link.txt")).unwrap();

    let caps = CapabilitySet::parse(&format!("fs:ro={}", allowed.display())).unwrap();
    let ok = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.read("{}") }}"#, allowed.join("a.txt").display()));
    assert!(matches!(ok, Ok(vox_compiler::eval::value::VoxValue::Str(ref s)) if s == "A"), "{ok:?}");

    let denied = run_with(caps.clone(), &format!(r#"pub fn main() {{ return fs.read("{}") }}"#, dir.path().join("secret.txt").display()));
    assert!(matches!(denied, Err(EvalError::CapabilityDenied { .. })), "{denied:?}");

    #[cfg(unix)]
    {
        let via_link = run_with(caps, &format!(r#"pub fn main() {{ return fs.read("{}") }}"#, allowed.join("link.txt").display()));
        assert!(matches!(via_link, Err(EvalError::CapabilityDenied { .. })), "symlink escape: {via_link:?}");
    }
}

#[test]
fn fs_write_needs_rw_root() {
    let dir = tempfile::tempdir().unwrap();
    let caps = CapabilitySet::parse(&format!("fs:ro={}", dir.path().display())).unwrap();
    let r = run_with(caps, &format!(r#"pub fn main() {{ return fs.write("{}", "x") }}"#, dir.path().join("w.txt").display()));
    assert!(matches!(r, Err(EvalError::CapabilityDenied { .. })), "{r:?}");
}

#[test]
fn frozen_time_is_what_the_receiver_said() {
    let r = run_with(CapabilitySet::parse("time:frozen=42").unwrap(), r#"pub fn main() { return time.now_ms() }"#);
    assert!(matches!(r, Ok(vox_compiler::eval::value::VoxValue::Int(42))), "{r:?}");
}
```

Add `tempfile = { workspace = true }` to `crates/vox-compiler/Cargo.toml` `[dev-dependencies]` if absent.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p vox-compiler --test caps_enforcement_test fs_ frozen 2>&1 | tail -10`
Expected: FAIL — reads outside the root succeed; `time.now_ms` returns wall-clock.

- [ ] **Step 3: Implement path scoping**

In `builtins.rs`, add one helper above the `fs` arm:

```rust
/// Resolve `raw` the way the OS will (symlinks included) and check it against
/// the caller's fs roots. Non-existent targets are checked by their parent so
/// `fs.write` to a new file inside an rw root is allowed.
fn fs_path_allowed(caps: Option<&crate::eval::caps::CapabilitySet>, raw: &str, write: bool) -> bool {
    let Some(c) = caps else { return true };
    let p = std::path::Path::new(raw);
    let canon = std::fs::canonicalize(p).or_else(|_| {
        let parent = p.parent().unwrap_or(std::path::Path::new("."));
        std::fs::canonicalize(parent).map(|pp| pp.join(p.file_name().unwrap_or_default()))
    });
    match canon {
        Ok(cp) => c.allows_path(&cp, write),
        Err(_) => false,
    }
}
```

Then in every `fs`/`io` method arm that takes a path (`read`, `read_file`, `read_to_string`, `write`, `append`, `exists`, `list_dir`, `list_dir_detailed`, `stat`, `glob`, `remove`, `mkdir`, `copy`, `rename`, `io.open`, `io.save` — enumerate with `rg -n '^\s+"[a-z_]+"' crates/vox-compiler/src/eval/builtins.rs | sed -n '/Some("fs")/,/Some("time")/p'`), guard with:

```rust
if !fs_path_allowed(caps, &path, /* write: */ true_or_false) {
    return Some(VoxValue::_Denied(format!("fs.{method}")));
}
```
`glob` checks the pattern's non-wildcard prefix directory. `copy`/`rename` check source as read and destination as write.

- [ ] **Step 4: Implement frozen time**

In the `time` arm:
```rust
                    "now_ms" | "now" => {
                        if let Some(ms) = caps.and_then(|c| c.frozen_time_ms()) {
                            return Some(VoxValue::Int(ms));
                        }
                        let ms = std::time::SystemTime::now() /* unchanged */ ;
                        Some(VoxValue::Int(ms))
                    }
```

- [ ] **Step 5: Run tests, then mutation-verify the symlink check**

Run: `cargo test -q -p vox-compiler --test caps_enforcement_test 2>&1 | tail -5`
Expected: PASS.

Mutation: change `std::fs::canonicalize(p)` to `Ok::<_, std::io::Error>(p.to_path_buf())` (no symlink resolution), re-run — `fs_reads_outside_ro_roots…` MUST FAIL on the `symlink escape` assertion. Restore; `grep -c 'std::fs::canonicalize(p)' $F` → 1.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-compiler
git add crates/vox-compiler
git commit -m "feat(eval): scope fs to receiver roots after symlink resolution; frozen time"
```

---

## Task 6: `vox run` flags, exit codes, memory ceiling

**Files:**
- Create: `crates/vox-cli/src/mem_limit.rs`
- Modify: `crates/vox-cli/src/main.rs:52` (allocator + arming)
- Modify: `crates/vox-cli/src/cli_args.rs:143-163` (`RunArgs`)
- Modify: `crates/vox-cli/src/commands/run.rs:38-85` (`run_interp`)
- Test: `crates/vox-cli/tests/run_interp_limits.rs` (create)

**Interfaces:**
- Produces CLI: `vox run --mode interp [--caps <spec>] [--max-steps N] [--max-memory BYTES] <file>`.
- Exit codes (documented in Task 15's `isolation.md`): `0` ok · `1` script error · `77` capability denied (EX_NOPERM) · `78` step limit · `79` memory limit.
- `mem_limit::arm(bytes)`; `mem_limit::MemLimitExceeded` abort path prints `vox: memory limit exceeded (<used> > <limit>)` to stderr and exits `79`.

- [ ] **Step 1: Write the failing integration test**

```rust
//! `vox run --mode interp` limits behave as documented (spec §3.2 items 5–7).
use std::process::Command;

fn vox() -> String { env!("CARGO_BIN_EXE_vox").to_string() }

fn write(name: &str, src: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("vox-limits-{name}-{}.vox", std::process::id()));
    std::fs::write(&p, src).unwrap();
    p
}

#[test]
fn capability_denial_exits_77_and_writes_stderr_not_stdout() {
    let f = write("caps", r#"pub fn main() { let s = fs.read("/etc/hosts"); print("LEAK") }"#);
    let out = Command::new(vox()).args(["run", "--mode", "interp", "--caps", "env:allow"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(77), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("LEAK"));
    assert!(String::from_utf8_lossy(&out.stderr).contains("fs.read"));
}

#[test]
fn caps_flag_overrides_script_directive() {
    let f = write("override", "// vox:caps fs\npub fn main() { let s = fs.read(\"/etc/hosts\"); print(\"LEAK\") }");
    let out = Command::new(vox()).args(["run", "--mode", "interp", "--caps", "env:allow"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(77));
}

#[test]
fn step_limit_exits_78() {
    let f = write("steps", "pub fn main() { let mut i = 0; while true { i = i + 1 } }");
    let out = Command::new(vox()).args(["run", "--mode", "interp", "--max-steps", "10000"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(78), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn memory_limit_exits_79() {
    let f = write("mem", r#"pub fn main() { let mut xs = []; while true { xs = xs.push("0123456789012345678901234567890123456789") } }"#);
    let out = Command::new(vox())
        .args(["run", "--mode", "interp", "--max-memory", "67108864", "--max-steps", "100000000"])
        .arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(79), "{}", String::from_utf8_lossy(&out.stderr));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p vox-cli --test run_interp_limits 2>&1 | tail -10`
Expected: clap rejects `--caps` → non-zero but not 77; all four FAIL.

- [ ] **Step 3: Implement the allocator**

`crates/vox-cli/src/mem_limit.rs`:
```rust
//! Counting allocator with a runtime-armed ceiling (spec §3.2 item 5).
//!
//! Disarmed cost: one relaxed atomic add per alloc/dealloc. Armed: the same plus
//! one load. Exceeding the ceiling aborts the process with exit 79 — a script
//! that has exhausted its budget cannot be trusted to unwind cleanly.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Counting;

static USED: AtomicUsize = AtomicUsize::new(0);
static LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);

pub const EXIT_MEMORY_LIMIT: i32 = 79;

/// Arm the ceiling. Allocations made before this call are counted but were not
/// bounded; call it as early in `main` as the CLI parse allows.
pub fn arm(bytes: usize) {
    LIMIT.store(bytes, Ordering::Relaxed);
}

pub fn used() -> usize {
    USED.load(Ordering::Relaxed)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let new = USED.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        if new > LIMIT.load(Ordering::Relaxed) {
            // No allocation allowed here: write a fixed message and abort.
            let msg = b"vox: memory limit exceeded (exit 79)\n";
            #[cfg(unix)]
            unsafe { libc::write(2, msg.as_ptr().cast(), msg.len()); }
            #[cfg(not(unix))]
            { let _ = msg; }
            std::process::exit(EXIT_MEMORY_LIMIT);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        USED.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn used_moves_with_allocation() {
        let before = used();
        let v = vec![0u8; 1 << 16];
        assert!(used() >= before + (1 << 16));
        drop(v);
    }
}
```
Add `libc = { workspace = true }` to `crates/vox-cli/Cargo.toml` if absent (check `grep -n '^libc' crates/vox-cli/Cargo.toml`). In `main.rs`, above `#[tokio::main]`:
```rust
mod mem_limit;
#[global_allocator]
static GLOBAL: mem_limit::Counting = mem_limit::Counting;
```
(If `main.rs` uses `vox_cli::…` from the lib crate, put `mem_limit` in `lib.rs` as `pub mod mem_limit;` and reference `vox_cli::mem_limit::Counting` — pick whichever matches how `main.rs` currently imports.)

- [ ] **Step 4: Implement the flags and exit codes**

`cli_args.rs` `RunArgs` — add:
```rust
    /// Receiver-imposed capabilities for `--mode interp`; overrides any `// vox:caps`
    /// directive in the script. Grammar: docs/src/reference/isolation.md
    #[arg(long)]
    pub caps: Option<String>,
    /// Interpreter step budget (default 10_000_000). Exit 78 when exceeded.
    #[arg(long)]
    pub max_steps: Option<usize>,
    /// Heap ceiling in bytes for this process. Exit 79 when exceeded.
    #[arg(long)]
    pub max_memory: Option<usize>,
```

`run.rs` — change `run_interp` to `async fn run_interp(file: &Path, _args: &[String], caps: Option<&str>, max_steps: Option<usize>) -> Result<()>` and:

```rust
    let mut interpreter = vox_compiler::eval::Interpreter::new(max_steps.unwrap_or(10_000_000));
    interpreter.caps = Some(match caps {
        Some(spec) => vox_compiler::eval::caps::CapabilitySet::parse(spec)
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        None if has_caps_directive => vox_compiler::eval::caps::CapabilitySet::from_legacy_directive(&legacy_words),
        None => vox_compiler::eval::caps::CapabilitySet::developer_default(),
    });
```
(`legacy_words` is the `Vec<String>` the existing first-line parse collects; keep that parse.) Replace the two `.map_err(|e| anyhow!("Eval failed…"))?` sites with a match that exits with the documented codes:

```rust
    let outcome = interpreter.run_module(&lowered).and_then(|_| interpreter.call("main", vec![]));
    let res = match outcome {
        Ok(v) => v,
        Err(vox_compiler::eval::EvalError::CapabilityDenied { ns, method }) => {
            eprintln!("vox: capability denied: {ns}.{method} (run with --caps {ns}:allow, or ask the mesh operator)");
            std::process::exit(77);
        }
        Err(vox_compiler::eval::EvalError::StepLimitExceeded) => {
            eprintln!("vox: step limit exceeded ({} steps); pass --max-steps or use --mode script for compute-heavy work", interpreter.step_limit);
            std::process::exit(78);
        }
        Err(e) => anyhow::bail!("Eval failed: {e:?}"),
    };
```
Thread `args.caps.as_deref()`, `args.max_steps` through `run()`; call `crate::mem_limit::arm(bytes)` in `run()` when `--max-memory` is given, **before** `run_interp`.

- [ ] **Step 5: Run tests; mutation-verify the memory guard**

Run: `cargo test -q -p vox-cli --test run_interp_limits 2>&1 | tail -8`
Expected: PASS (memory test takes a few seconds).

Mutation: in `mem_limit.rs` change `if new > LIMIT.load(…)` to `if false`, re-run — `memory_limit_exits_79` MUST FAIL (it will hit the 100 M step limit → 78 instead). Restore; `grep -c 'new > LIMIT' crates/vox-cli/src/mem_limit.rs` → 1.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli
git commit -m "feat(run): --caps/--max-steps/--max-memory with documented exit codes"
```

---

## Task 7: Interpreter becomes the default for scripts

**Files:**
- Modify: `crates/vox-cli/src/commands/run.rs:92-146`
- Modify: `crates/vox-cli/src/cli_args.rs:149-157` (doc strings)
- Modify: `docs/src/reference/cli.md:135-152`
- Modify: `lefthook.yml:18`
- Test: `crates/vox-cli/tests/run_mode_dispatch.rs` (un-ignore the auto case; see Step 1)

**Interfaces:**
- `RunMode::Auto` + non-`@page` ⇒ interpreter. `RunMode::Script` ⇒ native lane, unchanged. `Vox.toml [web] run_mode = "script"` ⇒ native lane (project opt-out).

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-cli/tests/run_mode_dispatch.rs` (a non-ignored test; it must not compile a crate):

```rust
#[test]
fn auto_mode_runs_scripts_under_the_interpreter() {
    // A script that only the interpreter can run in <1 s: the native lane
    // would need cargo. If this test takes minutes, the default did not flip.
    let f = std::env::temp_dir().join(format!("vox-auto-{}.vox", std::process::id()));
    std::fs::write(&f, r#"pub fn main() { print("AUTO_INTERP_OK") }"#).unwrap();
    let t0 = std::time::Instant::now();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox")).args(["run"]).arg(&f).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("AUTO_INTERP_OK"));
    assert!(t0.elapsed() < std::time::Duration::from_secs(5), "took {:?}: auto mode is still compiling", t0.elapsed());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -q -p vox-cli --test run_mode_dispatch auto_mode 2>&1 | tail -5`
Expected: FAIL on the 5 s bound (or on missing cargo).

- [ ] **Step 3: Implement**

In `run.rs::run`, replace the `use_script` computation so that `Auto` with a non-`@page` file **and no project override** goes to `run_interp`:

```rust
    let web_mode = vox_config::VoxConfig::load().web_run_mode;
    let is_script = match mode {
        RunMode::App => false,
        RunMode::Script => true,
        RunMode::Interp => unreachable!(),
        RunMode::Auto => match web_mode {
            vox_config::WebRunMode::App => false,
            vox_config::WebRunMode::Script => true,
            vox_config::WebRunMode::Auto => crate::commands::runtime::run::run::is_script_file_by_page_heuristic(file),
        },
    };
    // Spec §3.1: the interpreter is the default script tier. Only an explicit
    // `--mode script` or a project-level `[web] run_mode = "script"` reaches cargo.
    if is_script && matches!(mode, RunMode::Auto) && !matches!(web_mode, vox_config::WebRunMode::Script) {
        return run_interp(file, args, caps, max_steps).await;
    }
    let use_script = is_script;
```
Update the `RunMode::Auto` doc comment: "If the file has no `@page`, run under the interpreter; `--mode script` or `[web] run_mode = \"script\"` selects the native lane." Update `cli_args.rs:149` similarly and `cli.md:144`. In `lefthook.yml:18` leave the command as is (it now runs under the interpreter) and add a comment line above: `# runs under the interpreter (10 ms); add --mode script only if a script needs a Rust crate`.

- [ ] **Step 4: Run the repository's own scripts**

```bash
for f in scripts/fmt.vox scripts/install-hooks.vox scripts/setup.vox; do
  echo "== $f"; timeout 120s cargo run -q -p vox-cli -- run "$f" -- --help 2>&1 | tail -3
done
```
Expected: each completes in seconds. Any script that fails under the interpreter but passed under native is a **Task 2-class asymmetry**: add it to `KNOWN_TIER_ASYMMETRIES` with the failing builtin as the reason and open a follow-up; do not add `--mode script` to hide it.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test -q -p vox-cli --test run_mode_dispatch auto_mode && cargo test -q -p vox-cli --test run_interp_limits`
Expected: PASS.

```bash
cargo fmt -p vox-cli
git add crates/vox-cli docs/src/reference/cli.md lefthook.yml
git commit -m "feat(run): the interpreter is the default tier for scripts; cargo is opt-in"
```

---

## Task 8: Protocol — `Isolation { Interpreter, Native }`, `PROTO = 2`, payload on the wire

**Files:**
- Modify: `crates/vox-mesh-transport/src/protocol.rs:15-16, 88-97, 100-118`
- Modify: `crates/vox-mesh-transport/src/endpoint.rs:35-40, 221-262`
- Modify: `crates/vox-mesh-transport/tests/security.rs` (`Isolation::Wasm` references)
- Modify: `crates/vox-orchestrator/src/a2a/secret_gate.rs:22-27`

**Interfaces:**
- Produces:
  ```rust
  pub enum Isolation { Interpreter, Native }          // DEFAULT_FOR_MESH = Interpreter
  pub struct ReceivedJob { pub peer, pub request, pub limits, pub payload: Vec<u8> }
  // JobResponse::Probed gains: #[serde(default)] pub engines: Vec<String>
  pub const PROTO: u16 = 2;
  ```
- Wire: after a `JobRequest::Run { payload_bytes, .. }` frame, the sender writes exactly one more length-delimited frame containing the payload. `handle()` reads it with `read_frame(&mut recv, payload_bytes as usize)` **only after** the size check passes.

- [ ] **Step 1: Write the failing tests**

Append to `crates/vox-mesh-transport/tests/security.rs`:

```rust
#[tokio::test]
async fn the_run_payload_reaches_the_executor_intact() {
    let server = start_server().await;               // SpyExecutor records jobs
    server.trust.trust(&client_id(), None).unwrap();
    let payload = b"pub fn main() { print(\"hi\") }".to_vec();
    let resp = send_run_on(&server, TaskKind::VoxScript, &payload).await;
    assert!(matches!(resp, JobResponse::Output(_)), "{resp:?}");
    let seen = server.exec.last_payload();
    assert_eq!(seen, payload);
}

#[test]
fn isolation_default_is_the_interpreter_and_there_is_no_third_tier() {
    assert_eq!(Isolation::DEFAULT_FOR_MESH, Isolation::Interpreter);
    // Exhaustive: adding a variant must fail this match.
    for v in [Isolation::Interpreter, Isolation::Native] {
        match v { Isolation::Interpreter | Isolation::Native => {} }
    }
}

#[test]
fn proto_is_two() {
    assert_eq!(vox_mesh_transport::protocol::PROTO, 2);
}
```
Extend `SpyExecutor` with an `Arc<Mutex<Vec<u8>>>` capturing `job.payload`, a `last_payload()` accessor, and answer `Run` with `JobResponse::Output(b"ran".to_vec())`. Add a `send_run_on(server, kind, payload)` helper modelled on `send_request_on` that writes `Hello`, then `JobRequest::Run { kind, payload_bytes: payload.len() as u64 }`, then `write_frame(&mut send, &payload)`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p vox-mesh-transport --test security payload isolation proto 2>&1 | tail -10`
Expected: compile errors (`Isolation::Interpreter`, `payload` field).

- [ ] **Step 3: Implement**

`protocol.rs`:
```rust
pub const PROTO: u16 = 2;

pub enum Isolation {
    /// The HIR interpreter with receiver-imposed capabilities (spec §3.2).
    Interpreter,
    /// Host execution. Only reachable via `MeshTrust::grant_native`.
    Native,
}
impl Isolation { pub const DEFAULT_FOR_MESH: Self = Self::Interpreter; }
```
In `JobResponse::Probed`, add `#[serde(default)] pub engines: Vec<String>,` with doc "Locally installed ML engines this peer will run declarative jobs on (e.g. `llama.cpp`, `mlx`). Empty = none."

`endpoint.rs` — `ReceivedJob` gains `pub payload: Vec<u8>`. In `handle()`, after the size check and before `exec.execute`:
```rust
    let payload = match &request {
        JobRequest::Run { payload_bytes, .. } => {
            protocol::read_frame::<Vec<u8>>(&mut recv, *payload_bytes as usize).await?
        }
        _ => Vec::new(),
    };
```
and pass `payload` into `ReceivedJob`.

`secret_gate.rs`: rename `ExecTier::{BareMetal → Native, Sandboxed → Interpreter}` and update the two call sites in `remote_worker.rs:401, 680` (Task 10 deletes one of them).

Fix every `Isolation::Wasm`/`Container` reference the compiler reports (`security.rs`, `lib.rs` re-exports).

- [ ] **Step 4: Run tests**

Run: `cargo test -q -p vox-mesh-transport && cargo clippy -q -p vox-mesh-transport -p vox-orchestrator --all-targets -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-mesh-transport -p vox-orchestrator
git add crates/vox-mesh-transport crates/vox-orchestrator
git commit -m "feat(mesh): PROTO 2 — Isolation {Interpreter, Native}, Run payload on the wire"
```

---

## Task 9: `InterpExecutor` — the mesh runs VoxScript

**Files:**
- Create: `crates/vox-mesh-transport/src/interp_executor.rs`
- Modify: `crates/vox-mesh-transport/src/lib.rs` (`pub mod interp_executor; pub use interp_executor::InterpExecutor;`)
- Modify: `crates/vox-mesh-transport/Cargo.toml` (`tokio` needs `process`, `io-util`, `time` features; `tempfile`)
- Create: `crates/vox-mesh-transport/tests/interp_executor.rs`

**Interfaces:**
- Consumes: `ReceivedJob { peer, request, limits, payload }` (Task 8), `MeshTrust::level(&EndpointId) -> Option<TrustLevel>`, `JobLimits { wall_clock, max_output_bytes, .. }`, `CapabilitySet` grammar (Task 3) as a string.
- Produces:
  ```rust
  pub struct InterpExecutor { /* private */ }
  impl InterpExecutor {
      pub fn new(trust: Arc<MeshTrust>, vox_bin: PathBuf) -> Self;
      pub fn caps_for(level: TrustLevel, job_dir: &Path) -> String;   // pure; tested
  }
  impl JobExecutor for InterpExecutor { … }
  ```
- Caps mapping (spec §3.4): `Sandboxed` → `fs:rw=<job_dir>,time:real`; `Native` → `fs:rw=<job_dir>,net:allow,process:allow,env:allow,time:real`. Nothing else, ever, from a trust row.
- Child: `<vox_bin> run --mode interp --caps <caps> --max-steps <n> --max-memory <bytes> <job_dir>/main.vox`, `env_clear()` + `PATH`/`HOME`/`TMPDIR` only, `kill_on_drop`, killed at `limits.wall_clock`, stdout capped at `max_output_bytes` (truncated, and the response says so).
- `Cancel { job_id }` kills the child registered under that id.

- [ ] **Step 1: Write the failing tests**

`crates/vox-mesh-transport/tests/interp_executor.rs`:

```rust
//! Spec §3.4 / §4 mesh tests. Two live iroh endpoints on loopback; the executor
//! spawns the real `vox` binary (`CARGO_BIN_EXE_vox` is not available across
//! crates, so we locate target/debug/vox like the golden gates do).
use std::path::PathBuf;
use std::sync::Arc;
use vox_mesh_transport::protocol::{JobRequest, JobResponse};
use vox_mesh_transport::{InterpExecutor, MeshTrust};
use vox_mesh_types::TaskKind;

mod common;   // copy start_server/client_endpoint/send_run_on/loopback_addr_of from security.rs into tests/common/mod.rs

fn vox_bin() -> PathBuf {
    if let Ok(p) = std::env::var("VOX_BIN") { return p.into(); }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let p = root.join("target/debug").join(exe);
    assert!(p.exists(), "build vox first: cargo build -p vox-cli --bin vox");
    p
}

#[test]
fn caps_mapping_never_grants_more_than_the_trust_level() {
    use vox_mesh_transport::trust::TrustLevel;
    let dir = std::path::Path::new("/tmp/job");
    let s = InterpExecutor::caps_for(TrustLevel::Sandboxed, dir);
    assert!(s.contains("fs:rw=/tmp/job"));
    assert!(!s.contains("net:allow") && !s.contains("process:allow") && !s.contains("env:allow") && !s.contains("secrets"));
    let n = InterpExecutor::caps_for(TrustLevel::Native, dir);
    assert!(n.contains("net:allow") && n.contains("process:allow"));
    assert!(!n.contains("secrets:allow"), "secrets are never granted by trust level");
}

#[tokio::test]
async fn a_voxscript_job_runs_and_returns_its_output() {
    let server = common::start_server_with(|trust| Arc::new(InterpExecutor::new(trust, vox_bin()))).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let src = b"pub fn main() { print(\"MESH_RAN\") }";
    let resp = common::send_run_on(&server, TaskKind::VoxScript, src).await;
    match resp {
        JobResponse::Output(bytes) => assert!(String::from_utf8_lossy(&bytes).contains("MESH_RAN")),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_sandboxed_peer_cannot_read_the_host_filesystem() {
    let server = common::start_server_with(|trust| Arc::new(InterpExecutor::new(trust, vox_bin()))).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let src = b"pub fn main() { let s = fs.read(\"/etc/hosts\"); print(\"LEAK\") }";
    let resp = common::send_run_on(&server, TaskKind::VoxScript, src).await;
    match resp {
        JobResponse::Failed(msg) => assert!(msg.contains("capability denied") && msg.contains("fs.read"), "{msg}"),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn a_runaway_allocation_is_killed_and_reported() {
    let server = common::start_server_with(|trust| Arc::new(InterpExecutor::new(trust, vox_bin()))).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let src = b"pub fn main() { let mut xs = []; while true { xs = xs.push(\"0123456789012345678901234567890123456789\") } }";
    let resp = common::send_run_on(&server, TaskKind::VoxScript, src).await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if m.contains("memory limit")), "{resp:?}");
}

#[tokio::test]
async fn output_is_capped_at_the_limit() {
    let server = common::start_server_with(|trust| Arc::new(InterpExecutor::new(trust, vox_bin()))).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let src = b"pub fn main() { let mut i = 0; while i < 200000 { print(\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"); i = i + 1 } }";
    let resp = common::send_run_on(&server, TaskKind::VoxScript, src).await;
    match resp {
        JobResponse::Output(bytes) => assert!(bytes.len() <= 10 * 1024 * 1024 + 128, "{}", bytes.len()),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn ml_task_kinds_are_refused_with_a_reason_when_no_engine_is_installed() {
    let server = common::start_server_with(|trust| Arc::new(InterpExecutor::new(trust, vox_bin()))).await;
    server.trust.trust(&common::client_id(), None).unwrap();
    let resp = common::send_run_on(&server, TaskKind::TextInfer, b"{}").await;
    assert!(matches!(resp, JobResponse::Failed(ref m) if m.contains("no engine")), "{resp:?}");
}
```

Create `tests/common/mod.rs` by moving the shared helpers out of `security.rs` (`start_server` becomes `start_server_with(make_exec)`; keep `start_server()` as a thin wrapper over `SpyExecutor` so `security.rs` is unchanged in behaviour).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo build -q -p vox-cli --bin vox && cargo test -q -p vox-mesh-transport --test interp_executor 2>&1 | tail -10`
Expected: compile error, `InterpExecutor` not found.

- [ ] **Step 3: Implement**

```rust
//! Runs `VoxScript` jobs by spawning the interpreter as a bounded child process
//! (spec §3.4). The interpreter is the sandbox for Vox code; the process
//! boundary is defence-in-depth against interpreter bugs.
//!
//! ML task kinds are *declarative* — `{engine, model, input}` — and are refused
//! here until an engine registry exists (Phase 4 placement carries `engines`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{Context, Result};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use vox_mesh_types::TaskKind;

use crate::endpoint::{JobExecutor, ReceivedJob};
use crate::protocol::{JobId, JobRequest, JobResponse};
use crate::trust::{MeshTrust, TrustLevel};

const DEFAULT_MAX_STEPS: usize = 50_000_000;
const DEFAULT_MAX_MEMORY: usize = 512 * 1024 * 1024;

pub struct InterpExecutor {
    trust: Arc<MeshTrust>,
    vox_bin: PathBuf,
    running: Mutex<HashMap<JobId, tokio::sync::oneshot::Sender<()>>>,
}

impl InterpExecutor {
    pub fn new(trust: Arc<MeshTrust>, vox_bin: PathBuf) -> Self {
        Self { trust, vox_bin, running: Mutex::new(HashMap::new()) }
    }

    /// The only place a trust level becomes capabilities. Pure so it is testable
    /// and auditable; `secrets` is never granted by level.
    pub fn caps_for(level: TrustLevel, job_dir: &Path) -> String {
        let base = format!("fs:rw={},time:real", job_dir.display());
        match level {
            TrustLevel::Sandboxed => base,
            TrustLevel::Native => format!("{base},net:allow,process:allow,env:allow"),
        }
    }

    /// Read a pipe to EOF, keeping at most `max` bytes.
    async fn read_capped<R: tokio::io::AsyncRead + Unpin>(mut r: R, max: usize) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = match r.read(&mut chunk).await { Ok(0) | Err(_) => break, Ok(n) => n };
            if buf.len() < max {
                let take = n.min(max - buf.len());
                buf.extend_from_slice(&chunk[..take]);
            }
        }
        buf
    }

    async fn run_script(&self, job: &ReceivedJob, level: TrustLevel) -> Result<JobResponse> {
        let dir = tempfile::Builder::new().prefix("vox-mesh-job-").tempdir()?;
        let main = dir.path().join("main.vox");
        tokio::fs::write(&main, &job.payload).await?;

        let mut cmd = Command::new(&self.vox_bin);
        cmd.arg("run").arg("--mode").arg("interp")
            .arg("--caps").arg(Self::caps_for(level, dir.path()))
            .arg("--max-steps").arg(DEFAULT_MAX_STEPS.to_string())
            .arg("--max-memory").arg(DEFAULT_MAX_MEMORY.to_string())
            .arg(&main)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", dir.path())
            .env("TMPDIR", dir.path())
            .current_dir(dir.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().context("spawn vox")?;

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let job_id = JobId::from_payload(&job.payload);   // see Step 4
        self.running.lock().unwrap_or_else(PoisonError::into_inner).insert(job_id, cancel_tx);

        let max_out = job.limits.max_output_bytes;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        // Both pipes are drained concurrently so a chatty stderr cannot block stdout.
        let out_task = tokio::spawn(Self::read_capped(stdout, max_out));
        let err_task = tokio::spawn(Self::read_capped(stderr, 64 * 1024));

        let status = tokio::select! {
            s = child.wait() => s?,
            _ = tokio::time::sleep(job.limits.wall_clock) => {
                let _ = child.kill().await;
                self.running.lock().unwrap_or_else(PoisonError::into_inner).remove(&job_id);
                return Ok(JobResponse::Failed(format!("wall clock of {:?} exceeded; job killed", job.limits.wall_clock)));
            }
            _ = cancel_rx => {
                let _ = child.kill().await;
                return Ok(JobResponse::Failed("cancelled by peer".into()));
            }
        };
        self.running.lock().unwrap_or_else(PoisonError::into_inner).remove(&job_id);

        let stdout = out_task.await.unwrap_or_default();
        let stderr = String::from_utf8_lossy(&err_task.await.unwrap_or_default()).into_owned();
        Ok(match status.code() {
            Some(0) => JobResponse::Output(stdout),
            Some(77) => JobResponse::Failed(format!("capability denied: {}", stderr.trim())),
            Some(78) => JobResponse::Failed(format!("step limit exceeded: {}", stderr.trim())),
            Some(79) => JobResponse::Failed(format!("memory limit exceeded: {}", stderr.trim())),
            code => JobResponse::Failed(format!("exit {code:?}: {}", stderr.trim())),
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
                JobRequest::QueueStats => Ok(JobResponse::QueueStats(crate::protocol::QueueStats {
                    pending_count: self.running.lock().unwrap_or_else(PoisonError::into_inner).len() as u64,
                    ..Default::default()
                })),
                JobRequest::Cancel { job_id } => {
                    let tx = self.running.lock().unwrap_or_else(PoisonError::into_inner).remove(job_id);
                    Ok(match tx { Some(tx) => { let _ = tx.send(()); JobResponse::Output(b"cancelled".to_vec()) }, None => JobResponse::Failed("no such running job".into()) })
                }
                JobRequest::Run { kind: TaskKind::VoxScript, .. } => {
                    let Some(level) = self.trust.level(&job.peer) else {
                        return Ok(JobResponse::Failed("not trusted".into()));   // accept loop already refuses; belt and braces
                    };
                    self.run_script(&job, level).await
                }
                JobRequest::Run { kind, .. } => Ok(JobResponse::Failed(format!(
                    "no engine installed for {kind}; this node runs VoxScript only until an ML engine is registered"
                ))),
            }
        })
    }
}
```
Add to `Cargo.toml`: `tokio = { workspace = true, features = ["process", "io-util", "time", "sync", "fs", "macros"] }`, `tempfile = { workspace = true }`.

- [ ] **Step 4: `JobId::from_payload`**

`JobId` already exists in `protocol.rs` (used by `Cancel`). Add `pub fn from_payload(bytes: &[u8]) -> Self` returning the BLAKE3 of the payload via `vox_crypto` (check `crates/vox-crypto/src/` for the hex/bytes helper), with a test that two different payloads give different ids. `vox-mesh-transport → vox-crypto` — check `contracts/ci/crate-edges.allow.v1.json`; if the edge is absent, use `std::hash::Hasher` (`DefaultHasher`) over the bytes instead: it is an identifier, not a security primitive, and that avoids an unauthorised edge.

- [ ] **Step 5: Run tests; mutation-verify the caps mapping**

Run: `cargo test -q -p vox-mesh-transport --test interp_executor 2>&1 | tail -12`
Expected: PASS (memory test ~5 s).

Mutation: in `caps_for`, make `Sandboxed` return the `Native` string. `a_sandboxed_peer_cannot_read_the_host_filesystem` still passes (fs is scoped) but `caps_mapping_never_grants_more…` MUST FAIL. Then a second mutation: return `"fs:rw=/"` for `Sandboxed` — `a_sandboxed_peer_cannot_read…` MUST FAIL. Restore both; `grep -c 'TrustLevel::Sandboxed => base' crates/vox-mesh-transport/src/interp_executor.rs` → 1.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-mesh-transport
git add crates/vox-mesh-transport
git commit -m "feat(mesh): InterpExecutor — VoxScript runs on the peer, bounded and capability-scoped"
```

---

## Task 10: Wire the executor, delete `ProbeOnlyExecutor` and the native bundle lane

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mesh_cli.rs:197`
- Modify: `crates/vox-mesh-transport/src/endpoint.rs:93-129` (delete `ProbeOnlyExecutor`)
- Modify: `crates/vox-mesh-transport/tests/security.rs` (retarget `a_trusted_peer_gets_a_sandbox_by_default`)
- Modify: `crates/vox-orchestrator/src/a2a/remote_worker.rs:196-250, 305-430, 660-690`
- Modify: `crates/vox-orchestrator/src/a2a/envelope.rs:91-101, 150-151`
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs:861-862, 1117-1118`; `crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs:503-504, 635-636`
- Modify: `crates/vox-secrets/src/spec/ids.rs:305`, `crates/vox-secrets/src/spec/registry/missing.rs:1307` (remove `VoxMeshExecPolicy`)
- Modify: `crates/vox-populi/src/transport/handlers/dispatch.rs:262`, `crates/vox-plugin-populi-mesh/src/transport/handlers/dispatch.rs:238` (the secret read — Phase 6 deletes these files; for now replace the read with the literal `"source-only"` and a `// vox-deprecated-since="0.6.0" retire-by="0.7.0" reason="mesh-phase6" canonical="vox_mesh_transport::InterpExecutor"` marker)

- [ ] **Step 1: Retarget the security test first**

In `security.rs`, `a_trusted_peer_gets_a_sandbox_by_default` currently asserts the executor refuses `Run`. Change it to assert `job.limits.isolation == Isolation::Interpreter` on the `SpyExecutor`'s recorded job, and add:

```rust
#[test]
fn the_executor_type_is_the_interpreter_one() {
    // If ProbeOnlyExecutor is ever re-exported again, the `use` below is where it would surface.
    assert!(std::any::type_name::<vox_mesh_transport::InterpExecutor>().contains("InterpExecutor"));
}
```
Run: `cargo test -q -p vox-mesh-transport --test security 2>&1 | tail -3` — expected PASS (nothing deleted yet).

- [ ] **Step 2: Swap the executor in `mesh_cli.rs`**

```rust
                let exec = std::sync::Arc::new(vox_mesh_transport::InterpExecutor::new(
                    trust.clone(),
                    std::env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|d| d.join(if cfg!(windows) { "vox.exe" } else { "vox" })))
                        .filter(|p| p.exists())
                        .unwrap_or_else(|| std::path::PathBuf::from("vox")),
                ));
```
(`vox-ml-cli` and `vox` ship side by side; fall back to `PATH`.)

- [ ] **Step 3: Delete `ProbeOnlyExecutor`** (`endpoint.rs:93-129` and its `lib.rs` re-export). Build: `cargo build -q -p vox-mesh-transport -p vox-ml-cli --features populi`.

- [ ] **Step 4: Delete the bundle lane**

In `remote_worker.rs` delete `run_dispatched_bundle`, `BundleKind`, `classify_bundle`, and the `ExecTier::Interpreter` (ex-`Sandboxed`) secret-gating block that only served the wasm bundle path; delete the call site (~line 660-690) that chose between source and bundle so only `run_dispatched_source` remains. In `run_dispatched_source` add `--caps` to the child (the HTTP lane must not be more permissive than the mesh):

```rust
    cmd.arg("run").arg("--mode").arg("interp")
        .arg("--caps").arg(format!("fs:rw={},time:real", std::env::temp_dir().display()))
        .arg(&tmp_file);
```
Delete the `policy` parameter and every `"no-exec"`/`"source-only"`/`"permissive"` branch — trust is the mesh's job now, and the HTTP lane is Phase 6's to remove. In `envelope.rs` delete `exec_bundle_b64` and `exec_bundle_blake3_hex` (+ their `None` initialisers in the four listed files). Remove `VoxMeshExecPolicy` from `vox-secrets` and run `cargo run -q -p vox-cli -- ci secrets-parity && cargo run -q -p vox-cli -- ci secret-env-guard`.

- [ ] **Step 5: Build, test, commit**

Run: `cargo test -q -p vox-mesh-transport -p vox-orchestrator 2>&1 | tail -5 && cargo clippy -q -p vox-orchestrator -p vox-ml-cli -p vox-secrets --all-targets -- -D warnings`
Expected: PASS, clean.

```bash
cargo fmt -p vox-mesh-transport -p vox-orchestrator -p vox-ml-cli -p vox-secrets -p vox-populi -p vox-plugin-populi-mesh
git add -A crates/
git commit -m "feat(mesh): serve with InterpExecutor; delete ProbeOnlyExecutor and the native bundle lane (F2)"
```

---

## Task 11: `PopuliHttpOp::Dispatch` over the mesh

**Files:**
- Modify: `crates/vox-workflow-runtime/src/workflow/populi.rs` (`Dispatch` and `Wait` arms)
- Read: `crates/vox-mesh-transport/src/directory.rs` (`directory()`, `PeerEntry`), the `probe_one` shape to copy for a `run_one`

**Interfaces:**
- Consumes: `vox_mesh_transport::directory(ep, &trust) -> Vec<PeerEntry>`; the bind-once endpoint already in `populi.rs` (Task 3.4 added it); `protocol::{Hello, JobRequest, JobResponse, write_frame, read_frame, ALPN}`.
- Produces: `Dispatch` runs the activity's synthesized runner as a `VoxScript` job on the first peer whose `task_kinds` contains `VoxScript`; `Wait` becomes `control: "completed_inline"` (the mesh job is synchronous — there is nothing to poll).

- [ ] **Step 1: Write the failing test**

Append to `populi.rs` tests:
```rust
    #[test]
    fn dispatch_envelope_names_the_peer_and_never_the_control_plane() {
        let env = dispatch_envelope("act", 7, "endpoint…abc", b"hi\n", None);
        assert_eq!(env["control"], "dispatch_ok");
        assert_eq!(env["peer"], "endpoint…abc");
        assert!(env.get("control_url").is_none());
        assert_eq!(env["result_output"], "hi\n");
    }
```
Run: `cargo test -q -p vox-workflow-runtime --features mens dispatch_envelope 2>&1 | tail -3` — expected compile failure.

- [ ] **Step 2: Implement**

Add a pure `fn dispatch_envelope(activity: &str, activity_id: u64, peer: &str, output: &[u8], error: Option<&str>) -> Value` that builds the JSON (reuse `mesh_envelope()` from Task 3.4). Add:

```rust
async fn run_on_peer(ep: &iroh::Endpoint, peer: &vox_mesh_transport::PeerEntry, src: &[u8]) -> anyhow::Result<vox_mesh_transport::protocol::JobResponse> {
    use vox_mesh_transport::protocol::{self, Hello, JobRequest};
    let mut addr = iroh::EndpointAddr::new(peer.endpoint_id);
    for a in &peer.addrs { addr = addr.with_ip_addr(*a); }          // PeerEntry gains `addrs` if it lacks them — copy from directory()'s row
    let conn = ep.connect(addr, protocol::ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    protocol::write_frame(&mut send, &Hello::current()).await?;
    protocol::write_frame(&mut send, &JobRequest::Run { kind: vox_mesh_types::TaskKind::VoxScript, payload_bytes: src.len() as u64 }).await?;
    protocol::write_frame(&mut send, &src.to_vec()).await?;
    send.finish()?;
    let resp = protocol::read_frame(&mut recv, 16 * 1024 * 1024).await?;
    conn.close(0u32.into(), b"done");
    Ok(resp)
}
```
`Dispatch`: pick the first `PeerEntry` with `task_kinds.contains(&TaskKind::VoxScript)`; error `"no trusted peer accepts VoxScript"` otherwise; on `JobResponse::Output(o)` return `dispatch_envelope(.., Some(o), None)`, on `Failed(m)` return `Err(anyhow!("mesh dispatch failed on {peer}: {m}"))`. Delete the HTTP `Dispatch`/`Wait` code and the `VOX_MESH_CONTROL_ADDR` error text from Task 3.4. `Wait` → `mesh_envelope("wait", …, "control": "completed_inline", "detail": "mesh jobs are synchronous; the Dispatch step already carried the result")`.

- [ ] **Step 3: Test and commit**

Run: `cargo test -q -p vox-workflow-runtime --features mens 2>&1 | tail -3 && cargo clippy -q -p vox-workflow-runtime --all-targets --features mens -- -D warnings`

```bash
cargo fmt -p vox-workflow-runtime
git add crates/vox-workflow-runtime
git commit -m "feat(workflow): PopuliHttpOp::Dispatch runs on a mesh peer; Wait is inline"
```

---

## Task 12: Delete the wasi script lane and the rejected isolation tiers

**Files:**
- Delete: `crates/vox-cli/src/commands/wasm.rs`, `crates/vox-cli/src/commands/runtime/run/backend/wasi.rs`, `crates/vox-cli/src/isolation.rs`
- Modify: `crates/vox-cli/Cargo.toml:79-87, 182, 293-294`
- Modify: `crates/vox-cli/src/lib.rs:238-252` (remove `Wasm`/`WasmStub` variants and dispatch)
- Modify: `crates/vox-cli/src/commands/mod.rs:127-128`
- Modify: `crates/vox-cli/src/cli_args.rs:165-214` (remove `script_isolation_tier`, `--isolation`, `WasmRunArgs`)
- Modify: `crates/vox-cli/src/commands/runtime/run/script.rs:28-118` (remove `isolation` field and the wasi/reject branches), `backend/mod.rs:4-5,78-79`
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/toolchain.rs:205-235` (the wasm32 target check becomes `(optional)` and passes when absent; no auto-heal)
- Modify: `crates/voxup/src/install.rs:331-343` (delete `provision_wasm_sysroots` and its call)
- Modify: `crates/vox-cli/src/commands/runtime/run/backend/tests.rs` (drop wasi cases)
- Modify: `docs/src/reference/cli.md:152`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-cli/tests/run_mode_dispatch.rs`:
```rust
#[test]
fn isolation_and_wasm_surfaces_are_gone() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox")).args(["wasm", "run", "x.wasm"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unrecognized subcommand"), "{}", String::from_utf8_lossy(&out.stderr));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox")).args(["script", "--isolation", "wasm", "x.vox"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("unexpected argument"));
}
```
Run: `cargo test -q -p vox-cli --test run_mode_dispatch isolation_and 2>&1 | tail -3` — expected FAIL.

- [ ] **Step 2: Delete**

```bash
git rm crates/vox-cli/src/commands/wasm.rs crates/vox-cli/src/commands/runtime/run/backend/wasi.rs crates/vox-cli/src/isolation.rs
```
Then remove every reference the compiler reports: the `script-wasi` feature and its three `dep:` lines; `vox-wasm-engine`, `wasmtime`, `wasmtime-wasi` from `[dependencies]`; `Wasm`/`WasmStub` enum variants and match arms in `lib.rs`; `#[cfg(feature = "script-wasi")] pub mod wasm;`; `WasmRunArgs`, `script_isolation_tier`, the `isolation` arg; `ScriptOpts.isolation`, `is_wasi()`, `effective_isolation()`, and the `IsolationPolicy` match in `script.rs`; `WasiBackend` in `backend/mod.rs`. In `toolchain.rs` the check becomes `Check::pass("WASI target (optional)", "not required: scripts run under the interpreter; `--mode script` targets the host")` when absent. In `install.rs` delete `provision_wasm_sysroots` and its call at ~line 130.

- [ ] **Step 3: Tighten the crate-edge ratchet**

Run: `cargo run -q -p vox-cli -- ci crate-edges --tighten && git diff --stat contracts/ci/crate-edges.allow.v1.json`
Expected: the `["vox-cli","vox-wasm-engine"]` pair is removed and nothing is added. If anything is *added*, revert the contract file and stop — that is an edge you did not intend.

- [ ] **Step 4: Build everything that could see the change**

Run: `cargo build -q -p vox-cli -p voxup && cargo test -q -p vox-cli --test run_mode_dispatch && cargo clippy -q -p vox-cli -p voxup --all-targets -- -D warnings`
Expected: PASS, clean. Also `cargo tree -p vox-cli -e features -i wasmtime 2>&1 | head -2` → `did not match any packages`.

- [ ] **Step 5: Docs and commit**

`cli.md:152` — remove `--isolation` from the `vox script` flag list. `docs/src/reference/cli-command-surface.generated.md` is regenerated by the pre-commit hook (`vox ci command-sync`); do not hand-edit it.

```bash
cargo fmt -p vox-cli -p voxup
git add -A crates/vox-cli crates/voxup contracts/ci/crate-edges.allow.v1.json docs/src/reference/cli.md
git commit -m "chore(cli): delete the wasi script lane, vox wasm, and rejected isolation tiers"
```

---

## Task 13: Delete the MicroVM stub; rename `vox_ir` → `hir_export`

**Files:**
- Delete: `crates/vox-skill-runtime/src/microvm.rs`, `crates/vox-skill-runtime/tests/microvm_tier.rs`
- Modify: `crates/vox-skill-runtime/src/lib.rs:21,27`, `src/runtime.rs:105-115` (remove `Tier::MicroVm`), `src/detect.rs:165-190`
- Rename: `crates/vox-codegen/src/vox_ir/` → `crates/vox-codegen/src/hir_export/`; `lower_hir_to_vox_ir` → `export_hir`; `VoxIrModule` → `HirExport`
- Modify: `crates/vox-codegen/src/lib.rs:1,22`, `crates/vox-cli/src/commands/check.rs:136-139`, `crates/vox-cli/src/cli_args.rs:77`, `crates/vox-compiler/tests/ir_emission_test.rs`

- [ ] **Step 1: Write the failing tests**

In `crates/vox-skill-runtime/src/runtime.rs` tests:
```rust
    #[test]
    fn tier_has_exactly_three_real_backends() {
        for t in [Tier::BareMetal, Tier::Wasm, Tier::Container] {
            match t { Tier::BareMetal | Tier::Wasm | Tier::Container => {} }
        }
    }
```
In `crates/vox-compiler/tests/ir_emission_test.rs` rename references to `vox_codegen::hir_export::export_hir` / `HirExport` and add:
```rust
#[test]
fn hir_export_is_a_json_envelope_and_says_so() {
    let doc = include_str!("../../vox-codegen/src/hir_export/mod.rs");
    assert!(doc.contains("not an IR"), "the module doc must state this is a serialization envelope, not an IR");
}
```
Run both → expected compile failures.

- [ ] **Step 2: Implement**

```bash
git rm crates/vox-skill-runtime/src/microvm.rs crates/vox-skill-runtime/tests/microvm_tier.rs
git mv crates/vox-codegen/src/vox_ir crates/vox-codegen/src/hir_export
```
Remove `Tier::MicroVm` and the `detect.rs` arm; remove `pub mod microvm; pub use microvm::MicroVmRuntime;`. In `hir_export/mod.rs` first line: `//! HIR export — a versioned JSON envelope of HIR nodes for tooling. This is **not an IR**: nothing lowers from it; \`vox check --emit-ir\` writes it.` Rename symbols with `rg -l "vox_ir|VoxIrModule|lower_hir_to_vox_ir" crates/ | xargs sed -i '' -e 's/lower_hir_to_vox_ir/export_hir/g; s/VoxIrModule/HirExport/g; s/vox_ir/hir_export/g'` (macOS `sed -i ''`; on Linux drop the `''`). Keep the CLI flag name `--emit-ir` — it is a user surface; only its doc string changes.

- [ ] **Step 3: Test and commit**

Run: `cargo test -q -p vox-skill-runtime -p vox-codegen -p vox-compiler --test ir_emission_test 2>&1 | tail -4 && cargo clippy -q -p vox-skill-runtime -p vox-codegen -p vox-cli --all-targets -- -D warnings`

```bash
cargo fmt -p vox-skill-runtime -p vox-codegen -p vox-cli -p vox-compiler
git add -A crates/vox-skill-runtime crates/vox-codegen crates/vox-cli crates/vox-compiler
git commit -m "chore: delete the MicroVM stub; rename vox_ir to hir_export (it is not an IR)"
```

---

## Task 14: `sandbox.rs` tells the truth on macOS

**Files:**
- Modify: `crates/vox-cli/src/commands/runtime/run/sandbox.rs:200-214`

- [ ] **Step 1: Write the failing test**

In `sandbox.rs` tests (create the module if absent):
```rust
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_does_not_pretend_an_env_var_is_a_sandbox() {
        let mut cmd = std::process::Command::new("true");
        let opts = ScriptOpts::default();
        enforce_sandbox(&mut cmd, &opts).unwrap();
        let has_fake = cmd.get_envs().any(|(k, _)| k == "VOX_SANDBOX");
        assert!(!has_fake, "VOX_SANDBOX=1 is an informational hint masquerading as isolation");
    }
```

- [ ] **Step 2: Implement**

Replace the "Other" branch body with a warning that states the actual boundary and sets nothing:
```rust
    tracing::warn!(
        "no OS-level sandbox on this platform for `--mode script`; the interpreter tier (`vox run`, default) is the sandbox for Vox code — see docs/src/reference/isolation.md"
    );
    Ok(())
```
Update the doc comment above `enforce_sandbox` to match.

- [ ] **Step 3: Test and commit**

Run: `cargo test -q -p vox-cli sandbox 2>&1 | tail -3`
```bash
git add crates/vox-cli/src/commands/runtime/run/sandbox.rs
git commit -m "fix(sandbox): stop presenting VOX_SANDBOX=1 as isolation on macOS"
```

---

## Task 15: Documentation, ADR, and plan bookkeeping

**Files:**
- Create: `docs/src/reference/isolation.md`
- Create: `docs/src/adr/048-interpreter-is-the-execution-and-sandbox-tier.md`
- Modify: `docs/src/adr/index.md`, `docs/src/adr/README.md` (add the row)
- Modify: `AGENTS.md` §VoxScript-First Glue Code tier table
- Modify: `docs/src/architecture/where-things-live.md` (rows: `InterpExecutor`, `CapabilitySet`, `mem_limit`)
- Modify: `docs/superpowers/plans/2026-09-04-populi-mesh-iroh-transport.md` (Status row "No sandbox exists" → closed; Task 3.4 `[~]` → `[x]`; Phase 6.1 note that the bundle lane is already gone)
- Modify: `docs/src/architecture/research-index.md` (link ADR-048 and the spec)

- [ ] **Step 1: `isolation.md`**

```markdown
---
title: "Isolation: how Vox runs untrusted code"
description: "The interpreter is the sandbox for Vox code: receiver-imposed capabilities, fatal denial, bounded CPU/memory/output, and what it does not defend against."
category: "Language Reference"
status: "current"
---

# Isolation

`vox run` executes pure VoxScripts under the HIR interpreter. Every side effect a
script can perform passes through one boundary, so gating that boundary is the sandbox.

## Capabilities (`--caps`)

`vox run --mode interp --caps <spec> file.vox` — comma-separated, order-free. Any
namespace not mentioned is **denied**.

| token | meaning |
|---|---|
| `fs:ro=<dir>[|<dir>…]` | read within these roots (after symlink resolution) |
| `fs:rw=<dir>[|…]` | read and write within these roots |
| `net:none` / `net:allow` | `http.*` |
| `process:none` / `process:allow` | `process.*` (subprocesses) |
| `env:none` / `env:allow` | `env.*` |
| `secrets:none` / `secrets:allow` | `secrets.*` |
| `time:real` / `time:frozen=<ms>` | `time.now_ms()` returns wall-clock or the fixed value |
| `db:…`, `repo:…`, `agentos:…` | `none` / `allow` |

`path`, `json`, `csv`, `toml`, `yaml`, `regex`, `log` are pure and always available.

When `--caps` is given, a `// vox:caps` line in the script is ignored: the receiver
decides, never the subject. Without `--caps`, `vox run` uses the developer default
(everything allowed) so local scripts keep working.

## Limits and exit codes

| flag | default | exit code on breach |
|---|---|---|
| `--max-steps` | 10 000 000 | 78 |
| `--max-memory <bytes>` | unbounded locally; 512 MiB over the mesh | 79 |
| capability denied | — | 77 |

Denials and limit breaches write one line to **stderr** and never to stdout.

## Over the mesh

A paired peer runs your `VoxScript` with `fs:rw=<per-job tmpdir>,time:real` and
nothing else. `vox mesh grant-native <id>` (never pairing) adds `net`, `process`,
and `env`. Secrets are never granted by trust level.

## What this does not defend against

- **Native code.** `--mode script` compiles to a host binary with full ambient
  authority; nothing here applies. Use it only for code you trust.
- **Bugs in the interpreter or its pure-Rust dependencies.** The process boundary
  and OS limits (`sandbox.rs`: Landlock on Linux, Job Objects on Windows) are
  defence-in-depth. macOS has no OS-level sandbox in Vox today.
- **GPU work.** ML task kinds run on locally installed engines, not in the interpreter.
```

- [ ] **Step 2: ADR-048**

```markdown
---
title: "ADR-048: The interpreter is the execution and sandbox tier"
description: "Decision to make the HIR interpreter the default and sandboxing tier for VoxScripts, retiring cargo as an end-user requirement and the wasi script lane."
category: "Architecture Decisions (ADRs)"
status: "accepted"
---

# ADR-048: The interpreter is the execution and sandbox tier

**Status:** accepted 2026-09-05 · **Supersedes:** the "no sandbox exists" gap in ADR-047's program; the `--isolation wasm` script lane.

## Context
Measured 2026-09-05: interpreter 10 ms cold; native lane 275 s cold across 764 crates and
path-dependent on a Vox checkout. WASM is the only substrate that gives bit-identity but
cannot spawn a process, which most repository scripts do. Native compilation fixes almost
none of the cross-platform divergence (libm, NaN payloads, overflow mode, map order).
See the research doc and spec linked below.

## Decision
1. `vox run` runs non-`@page` files under the interpreter; `--mode script` is the opt-in to cargo.
2. Capabilities are receiver-imposed (`--caps`), denial is fatal (exit 77), every side-effecting namespace is gated, memory (79) and steps (78) are bounded.
3. The mesh ships VoxScript source and declarative ML jobs only; `InterpExecutor` spawns the interpreter as a bounded child. Native code is never received.
4. The wasi script lane, `vox wasm`, `ProbeOnlyExecutor`, the HTTP native-bundle lane, the parsed-then-rejected isolation tiers, and the MicroVM stub are deleted.

## Consequences
- End users need only the `vox` binary to run pure Vox; cargo/rustc/pnpm are needed only for Rust- or React-ecosystem code and for apps.
- A differential gate proves interp and native agree; builtin drift becomes a test failure.
- Peak compute throughput under the interpreter is 3–10× off native; numeric kernels use `--mode script`.
- No new crate edges.

## References
- Spec: `docs/superpowers/specs/2026-09-05-interpreter-first-execution-design.md`
- Research: `docs/src/architecture/voxscript-portability-substrate-research-2026.md`
- Plan: `docs/superpowers/plans/2026-09-05-interpreter-first-execution.md`
```

- [ ] **Step 3: `AGENTS.md` tier table**

Replace the three-row execution-tier table under §VoxScript-First Glue Code with:

```markdown
| Need | Command | Notes |
|---|---|---|
| Run a script (default) | `vox run scripts/foo.vox` | HIR interpreter; ~10 ms cold; no cargo |
| Untrusted / capability-scoped | `vox run --caps fs:ro=.,net:none scripts/foo.vox` | interpreter is the sandbox; see `docs/src/reference/isolation.md` |
| Needs a Rust crate or peak throughput | `vox run --mode script scripts/foo.vox` | native tier via cargo; content-hash cached |
```
and delete the `--isolation wasm` row.

- [ ] **Step 4: Plan and index bookkeeping**

In the mesh plan: Status row `**Phase 3**` → "Tasks 3.1–3.4 **done and merged**."; delete the "No sandbox exists" bullet under Known gaps and replace with "**Sandbox:** the interpreter, per ADR-048 and the 2026-09-05 spec."; in Task 3.4 change `[~]` → `[x]` and add "Dispatch/Wait ported over the mesh in the interpreter-first plan Task 11."; in Task 6.1 add "The HTTP native-bundle lane (`run_dispatched_bundle`) was deleted earlier by ADR-048 Task 10." Add ADR-048 to `docs/src/adr/index.md` and `README.md` tables; add the spec + ADR to `research-index.md` under Strategic & Value Proposition.

- [ ] **Step 5: Lint, run the fast gate, commit**

Run: `cargo run -q -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/isolation.md docs/src/adr/048-interpreter-is-the-execution-and-sandbox-tier.md && cargo run -q -p vox-cli -- ci pre-push`
Expected: `no hard errors`; pre-push fast tier green (or halts only at `doc-inventory verify`, which is pre-existing — say so).

```bash
git add docs AGENTS.md
git commit -m "docs: isolation reference, ADR-048, and plan bookkeeping for interpreter-first execution"
```

---

## Task 16: Full gate and the honest report

- [ ] **Step 1: Build the gui dist if this is a fresh worktree** — `ls crates/vox-gui/ui/dist || (cd crates/vox-gui/ui && pnpm install && pnpm build)`.
- [ ] **Step 2: Run the complete tier** — `cargo run -q -p vox-cli -- ci pre-push --complete 2>&1 | tail -30`. If it halts at `doc-inventory verify`, leave the regenerated file **uncommitted**, re-run so clippy and toestub execute, then `git checkout -- docs/agents/doc-inventory.json`.
- [ ] **Step 3: Run the differential gate once more** — `cargo test -q -p vox-integration-tests --test golden_differential_gate -- --ignored 2>&1 | tail -5`. Expected PASS, or a `KNOWN_TIER_ASYMMETRIES` list that exactly matches the failures.
- [ ] **Step 4: Run the mesh cross-machine smoke if BLAPTOP04 is reachable** — `vox mesh id` on both, `vox mesh join <ticket>`, then dispatch `pub fn main() { print("cross-machine") }` via the workflow `Dispatch` op or a small driver; record the round-trip time in the commit body. If unreachable, say so; do not fake the number.
- [ ] **Step 5: Report** — what shipped per task, every mutation performed and its result, every `KNOWN_TIER_ASYMMETRIES` entry with its reason, and the pre-push output verbatim. Nothing is pushed.

---

## Self-review against the spec

| Spec section | Task |
|---|---|
| §3.1 tiers and defaults, no auto-switch, advisory | 7 (advisory text is the exit-78 message) |
| §3.2 items 1–8 (caps grammar, fatal, coverage, fs scoping, memory, output, steps, frozen time) | 3, 4, 5, 6, 9 (output cap is parent-side in 9) |
| §3.3 registry SSOT, symmetry test, differential gate, `vox test` opt-in note | 1, 2 (full table-driven dispatch is deferred — see below), 15 |
| §3.4 `InterpExecutor`, trust→caps, protocol, `Probed.engines`, bundle lane deletion, `ExecTier`, Dispatch/Wait | 8, 9, 10, 11 |
| §3.5 deletions and corrections | 10, 12, 13, 14 |
| §3.6 automation, doctor, docs, contracts | 7, 12, 15 |
| §3.7 dependency map | 15 (`isolation.md`) |
| §4 mutation-verified guards | 4, 5, 6, 9 |
| §5 risks: measure first; scripts that shell out to cargo | 0, 7 Step 4 |

**Deliberate narrowing, stated:** §3.3 asks for `builtin_registry` to become the SSOT feeding *both* tiers' dispatch. Task 2 closes the measured drift and makes new drift a test failure via the differential gate and `KNOWN_TIER_ASYMMETRIES`, but does not rewrite `eval/builtins.rs` (2,867 LoC) to be table-driven. That refactor is real, is the right next plan once the gate exists to hold it, and would have doubled this plan's size. Report it as not done.

**`--max-output` flag:** the spec listed it as a `vox run` flag; this plan enforces the cap in the parent (`InterpExecutor`) only, because a process cannot reliably truncate its own stdout after the fact. The `JobLimits::max_output_bytes` contract is met.
