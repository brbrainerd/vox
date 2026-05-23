---
title: "VoxScript as Universal Glue Code — Research & Architecture 2026"
description: "Strategy, security model, execution tiers, and migration policy for replacing .ps1/.sh/.py glue scripts with .vox files driven by `vox run`."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-17"
training_eligible: false
training_rationale: "Establishes the foundational policy for VoxScript as the project's sole glue language, replacing shell scripts and Python."
archived_date: 2026-04-18
---

# VoxScript as Universal Glue Code — Research & Architecture 2026

## Executive Summary

The Vox project currently maintains three parallel glue-code surfaces:

| Surface | Examples | Problem |
|---|---|---|
| PowerShell `.ps1` | `mens-full-pipeline.ps1`, `vox-dev.ps1`, `install.ps1` | Windows-only idioms; CRLF cruft; brittle IDE approval matching |
| Bash `.sh` | `vox-dev.sh`, `check_*.sh`, `install.sh` | POSIX-only; no Windows path semantics |
| Python `.py` | `annotate.py`, `fix_links.py`, `extract.py`, `scratch_update*.py` | Interpreter dependency; PEP churn; `uv`/venv friction |

The policy proposed here eliminates all three categories as canonical glue surfaces and replaces them with **`.vox` files** executed via `vox run`. This creates a single, cross-platform, type-checked, Rust-backed automation layer that uses the project's own compiler as a self-hosted quality gate.

---

## 1. What Already Exists (Do Not Reinvent)

The execution infrastructure is already built. Agents must understand these tiers before proposing new automation:

### 1.1 Interpreter Tier (`vox run --interp`)

- **Path:** `crates/vox-cli/src/commands/run.rs` → `RunMode::Interp`
- **Mechanism:** HIR tree-walker (`crates/vox-compiler/src/eval/mod.rs`)
- **Step limit:** 10,000,000 steps (prevents infinite loops)
- **Cold-start latency:** < 50ms (no Rust compile step)
- **Capabilities:** Pure Vox computation, no I/O beyond `print`
- **Best for:** Fast automation, config computation, data transforms

### 1.2 Script/Native Tier (`vox run` / `vox run --mode script`)

- **Path:** `crates/vox-cli/src/commands/runtime/run/script.rs`
- **Mechanism:** Compiles `.vox` → native Rust binary (codegen_rust); content-hash cached in `~/.vox/script-cache/<xxh3>/`
- **Shared target:** `~/.vox/script-target/` — `vox-runtime` deps compiled **once**, subsequent builds fast
- **Sandbox:** Landlock (Linux) / Job Objects (Windows)
- **Best for:** CI pipelines, file manipulation, subprocess orchestration

### 1.3 WASI Tier (`vox run --isolation wasm`)

- **Mechanism:** Compile `.vox` → WASI module, execute via Wasmtime
- **Sandbox:** Capability-based; explicit `--wasi-dir` preopens required
- **Best for:** Untrusted user automation, remote agent execution

### 1.4 Trust Classes

```
trusted_dev   → permissive (native, no enforced FS restriction)
semi_trusted  → wasm (Wasmtime WASI)
untrusted     → wasm (Wasmtime WASI, no default FS access)
```

archived_date: 2026-04-18
---

## 2. Execution Tiers Mapped to Script Migration Classes

| Current Script Role | Replacement Tier | Rationale |
|---|---|---|
| Dev launcher, path resolution | `vox run --interp` | Fast, no compile, pure logic |
| CI corpus prep, file walks | `vox run` (script/native) | File I/O, subprocess, cached |
| Training pipeline orchestration | `vox run` (script/native) | Subprocess (cargo, vox), long-running |
| Schema migration transforms | `vox run --interp` | Data transform, no subprocess |
| Install / bootstrap | `vox run` (script/native) | Needs filesystem write, subprocess |
| Untrusted user automation | `vox run --isolation wasm` | Capability-limited |

---

## 3. Security Model

### 3.1 "Eating our own dogfood" and the Trust Boundary

When Vox scripts **operate on the Vox codebase itself** — e.g., modifying Rust sources, rewriting Cargo.toml, patching schemas — the execution trust level is critical. The following invariants apply:

**Invariant S-1: Source-modifying scripts run as `trusted_dev`, never `untrusted`.**
These scripts need write access to the workspace. Attempting to sandbox them via WASI would require preopening the entire workspace, which negates the WASI security model.

**Invariant S-2: Codebase-modifying `.vox` scripts MUST be committed to VCS before being executed by agents.**
An agent MUST NOT generate and immediately execute a `.vox` script that modifies the repository without human review. The script must be committed (or at minimum staged) and validated by `vox check` before `vox run` is called on it.

**Invariant S-3: No `.vox` script may use `shell_exec` or `subprocess` to bypass the compiler sandbox.**
All subprocess spawning must go through `vox-runtime`'s process primitives, which are observable via telemetry.

**Invariant S-4: Scripts that call `vox ci` or `cargo` are auditable via vox-orchestrator telemetry.**
Every `vox run` invocation emits a `vox.script.*` tracing event. CI must enforce that these events appear in the telemetry journal.

### 3.2 Supply Chain Attack Surface

The risk profile when `.vox` replaces `.ps1`:

| Vector | Shell Scripts | VoxScript |
|---|---|---|
| String injection into `Invoke-Expression` | **High risk** | Not possible — no eval of strings as code |
| Untrusted input passed to subprocess | Medium risk | Medium risk — same; `process::spawn` still risks this |
| PATH hijacking | High risk | Reduced — Vox resolves executables via `vox_config::paths` |
| Exfiltration via network calls | Unrestricted | Requires explicit Vox stdlib import; visible in HIR |
| Dependency confusion (npm/pip) | High risk | None — no package manager surface |
| Macro expansion / code generation | High risk (PowerShell DSLs) | Blocked by E091 (`SyntacticConfigurabilityNotAllowed`) |

**Net assessment:** VoxScript provides materially better auditability because the AST is inspectable by the compiler before execution. The `check_file` pipeline in `vox-compiler` can emit structured diagnostics on any `.vox` script before it is run.

### 3.3 Capability Permissions Model (Future Wave)

The current execution model is binary: `trusted_dev` (permissive) or `wasm` (capability-limited). A future hardening wave should introduce **declarative capability annotations** at the script header:

```vox
// vox:caps fs.write="cwd/**" subprocess.allow=["cargo", "vox"] network.deny
fn main() {
    // ...
}
```

This would allow the runtime to enforce a narrower permission set even for `trusted_dev` scripts, moving toward a least-privilege model without requiring WASI.

archived_date: 2026-04-18
---

## 4. Cross-Platform Build and Deployment Model

### 4.1 How `vox run` achieves cross-platform parity today

The script compilation pipeline in `script.rs` already handles:
- **Windows**: compiles to `.exe` via MSVC toolchain (inherits CUDA/nvcc path from `.vscode/settings.json`)
- **Linux/macOS**: compiles to native ELF/Mach-O
- **WASI**: compiles to `.wasm` via `wasm32-wasip1` target

The shared `~/.vox/script-target/` directory means `vox-runtime` is compiled once per platform, not per script. This is equivalent to Python's `site-packages` warm cache.

### 4.2 Cold-start latency analysis

| Tier | First run | Warm run (cache hit) |
|---|---|---|
| `--interp` | ~30–80ms (parse + HIR) | Same (no cache) |
| `--mode script` (native) | ~15–60s (full Rust compile) | <200ms (binary exec) |
| `--mode script` (wasm) | ~20–90s (WASI compile) | <500ms (Wasmtime startup) |

**Implication:** For short CI steps, `--interp` is always preferred. For long-running pipelines or those needing subprocess (cargo, git, vox), the native tier's warm-cache path is fast enough.

### 4.3 Dependency surface (the no-Python, no-shell pledge)

Running a `.vox` script requires exactly:
1. The `vox` binary (or `cargo run -p vox-cli` for development)
2. For native script tier: Rust/Cargo toolchain (already required to build Vox)
3. For WASI tier: Wasmtime (bundled with the Vox distribution, no separate install)

No Python. No `uv`. No PowerShell 7. No `bash`. No `node`.

This means a fresh Vox checkout on any OS can bootstrap its entire automation layer with `cargo build -p vox-cli --release && ./target/release/vox run scripts/bootstrap.vox`.

### 4.4 Bootstrap chicken-and-egg problem

The most important implication: **the scripts that build Vox cannot themselves depend on Vox being built**. This creates a genuine bootstrap constraint.

**Resolution:**
- **`scripts/bootstrap.vox`** is the one allowed file that MAY be run via `cargo run -p vox-cli -- run scripts/bootstrap.vox`
- The `vox-dev.ps1` / `vox-dev.sh` launchers are retained **only** as thin forwarders to `cargo run -p vox-cli`
- All other automation migrates to `.vox`

The launcher scripts are exempt from the "no shell" policy because they exist specifically to ensure `vox` is available — they must remain minimal and contain no logic.

---

## 5. Maintenance Implications

### 5.1 What we gain

1. **Single language across the stack.** Agents, contributors, and CI all read the same language surface.
2. **Compile-time verification.** `vox check scripts/foo.vox` catches type errors before CI runs the script.
3. **Doctest integration.** Scripts in `docs/` can be validated by `vox-doc-pipeline` (applies existing doctest policy).
4. **Telemetry.** Every `vox run` emits `vox.script.*` traces; anomalies are detectable.
5. **MENS training corpus.** Real, useful `.vox` scripts become natural training examples — they are high-signal because they do real work.
6. **Unified tooling.** `vox fmt`, `vox check`, `vox lsp` all work on scripts without special cases.

### 5.2 What we lose / risks

1. **Platform-native idioms.** PowerShell `Get-ChildItem -Recurse -Filter *.jsonl | Measure-Object -Line` is more concise than the equivalent Vox loop. VoxScript's stdlib must grow to cover common patterns.
2. **Subprocess ergonomics.** Calling `cargo`, `git`, `pnpm` from VoxScript is more verbose than shell. This is acceptable but must be documented.
3. **Incremental migration friction.** Scripts that mix PowerShell constructs with vox invocations cannot be mechanically translated — they require human judgment.
4. **Interpreter capability gaps.** `--interp` currently cannot do I/O beyond `print`. Scripts requiring file access must use the native tier.

### 5.3 Migration priority matrix

| Script | Risk | Priority | Target tier |
|---|---|---|---|
| `scripts/mens-full-pipeline.ps1` | High (complex, long) | High | Native script |
| `scripts/install.ps1` / `install.sh` | Medium (bootstrap) | Medium | Native script (post-bootstrap) |
| `scripts/ci/*.py` | Low (simple transforms) | High | Interp |
| `annotate.py`, `fix_links.py` | Low | High | Interp |
| `scratch_update*.py` | Low | High | Interp (or delete) |
| `scripts/windows/vox-dev.ps1` | Critical (bootstrap) | **Retain** | Thin launcher only |
| `scripts/vox-dev.sh` | Critical (bootstrap) | **Retain** | Thin launcher only |

archived_date: 2026-04-18
---

## 6. What Has Not Been Considered (Open Questions)

### 6.1 VoxScript stdlib for glue tasks

**Status (Wave 1):** COMPLETED. The `fs`, `path`, `json`, and `Object` namespaces have been implemented in the interpreter. Scripts can now read/write files, manipulate paths, and serialize/deserialize JSON.

**Gap:** `process.spawn` and `clavis` are still missing from the interpreter tier.

### 6.2 Interp tier and I/O

The interpreter (`eval/mod.rs`) is currently pure-computation. Adding I/O means:
- Either: Adding host-callable builtins that perform I/O (simplest, but requires `unsafe` or Tokio in the interpreter thread)
- Or: Promoting I/O scripts to the native/script tier only

**Recommendation:** Add a `vox:io` capability annotation. Scripts without it run in the pure interpreter. Scripts with it require the native tier. This preserves the interpreter's determinism for computation-only tasks.

### 6.3 Long-running scripts and signal handling

The `mens-full-pipeline.ps1` spawns background jobs with `Start-Process -PassThru` and tail-loops log files. VoxScript's `actor` model can express this natively, but the **workflow** primitive (durable, journaled) is better suited for long pipelines.

**Question:** Should CI automation scripts use `workflow` declarations (durable, restartable) or plain `fn main()`? 

**Recommendation:** Use `workflow` for multi-step pipelines where crash-recovery matters (e.g., MENS training). Use `fn main()` for short scripts.

### 6.4 Secrets in scripts

Scripts that currently call `$env:OPENAI_API_KEY` or read `.env` files must instead use the Clavis API:

```vox
// vox:skip
// vox:caps clavis.read=["OPENAI_API_KEY"]
fn main() {
    let key = clavis.resolve("OPENAI_API_KEY")
    // ...
}
```

This requires a `clavis` builtin namespace in the Vox stdlib — currently absent.

### 6.5 IDE "blue shell" and command approval friction

The original motivation for this policy is eliminating "blue cruft" — the IDE-level friction of command approval for PowerShell. The key insight: `vox run scripts/foo.vox` is a **single, predictable command shape** that IDE allowlists can approve once. Compare to `pwsh -File scripts/foo.ps1` which triggers per-script approval in some IDEs.

**Implication for AGENTS.md:** Agents should prefer `vox run <script.vox>` over any shell invocation for project automation, because:
1. Single approved command shape
2. Content-verified (compiler checks the script before running)
3. Auditable (telemetry)

### 6.6 CI runner compatibility

CI jobs run on Linux bash runners (`docs/src/ci/runner-contract.md`). The migration is safe because:
- `vox` binary is built before CI automation runs
- `vox run` works on Linux without PowerShell
- `.vox` scripts have LF line endings (enforced by `.gitattributes`)

No `.ps1` scripts should exist in CI runner job definitions. Any that do should be migrated.

### 6.7 Performance: interpreted vs compiled for CI

For a CI step that previously ran `python scripts/prep_corpus.py` (cold Python startup ~300ms), the equivalent `vox run --interp scripts/prep_corpus.vox` will be faster (~50ms). The native tier adds a cold-compile cost that amortizes over repeated runs.

---

## 7. Implementation Plan (Wave-Gated)

### Wave 0: Policy + Stdlib Gaps (COMPLETED)

1. Update `AGENTS.md` with the "VoxScript-first glue" heuristic
2. Update `GEMINI.md`, `CLAUDE.md`, `.cursor/rules/` with the same
3. Document this research in `docs/src/architecture/vox-as-glue-research-2026.md` (this file)
4. Update `docs/src/architecture/research-index.md`
5. Add `fs`, `process`, `env`, `path` builtin namespaces to the interpreter tier (DONE: fs, path, json, Object)
6. Add `clavis` builtin namespace (calls `vox_clavis::resolve_secret`) (TODO)

### Wave 1: Migrate Simple Scripts (COMPLETED)

1. Convert `scripts/generate-bench-scaffold.vox` (SUCCESS: Replaced Python benchmark generator)
2. Fix parser lookahead for multi-line object/list literals (SUCCESS: Verified in `pratt_match.rs`)
3. Implement `recover_to_top_level` with brace-matching (SUCCESS: Eliminates cascading top-level errors)
4. Convert `annotate.py`, `fix_links.py`, `extract.py` → `.vox` (IN PROGRESS)

### Wave 2: Migrate Complex Pipelines (Medium Risk)

1. Convert `scripts/mens-full-pipeline.ps1` → `scripts/mens-full-pipeline.vox`
   - Use `workflow` declaration for the 4-step pipeline
   - Use `process.spawn("cargo", [...])` for cargo/vox invocations
2. Convert `scripts/windows/mens_train_watch.ps1` → `scripts/mens_train_watch.vox`
3. Convert `scripts/windows/run_4080_experiment_cycles.ps1` → `scripts/experiments/run_4080_cycles.vox`

### Wave 3: Bootstrap and Install (High Risk — needs careful sequencing)

1. Convert `scripts/install.ps1` / `scripts/install.sh` → `scripts/install.vox`
   - Thin `.ps1` / `.sh` launcher retained as single-line: `cargo run -p vox-cli -- run scripts/install.vox`
2. Retire `scripts/windows/vox-dev.ps1` content; keep as a 3-line forwarder
3. Update CI job definitions to call `vox run` instead of `pwsh -File`

### Wave 4: CI Runner Hardening

1. Add `vox ci script-hygiene` check: fails if any `.py` or `.ps1` outside `scripts/windows/` (launcher exception) has been modified
2. Enforce `// vox:caps` annotations on all scripts in `scripts/` that use I/O
3. Add MENS corpus lane: collect all committed `.vox` scripts as training examples

archived_date: 2026-04-18
---

## 8. Naming and File Layout Conventions

```
scripts/
  bootstrap.vox          # Thin: sets up dev environment post-cargo-build
  install.vox            # Installs vox to PATH
  mens/
    full-pipeline.vox    # Replaces mens-full-pipeline.ps1
    train-watch.vox      # Replaces mens_train_watch.ps1
  ci/
    patch-catalog.vox    # Replaces patch_catalog.py
    corpus-prep.vox      # Replaces prep_mens_mix_inputs.sh
    add-canonical.vox    # Replaces add_canonical_names.py
  windows/
    vox-dev.ps1          # RETAINED: thin cargo-run launcher (exempt)
  vox-dev.sh             # RETAINED: thin cargo-run launcher (exempt)
```

**Naming convention:** `kebab-case.vox` (consistent with Vox source conventions). No underscores in filenames.

---

archived_date: 2026-04-18
---

## 10. Lessons Learned from Wave 1

1. **Parser Recovery is Critical:** Early recursive-descent parsers fail catastrophically on nested block errors. Brace-matching recovery in `recover_to_top_level` is required for multi-function scripts.
2. **Whitespace Sensitivity in Literals:** Object and list literals must explicitly skip newlines in their internal loops, or they will collide with the interpreter's newline-as-statement-boundary rule.
3. **Immutable List Ergonomics:** Since Vox lists are currently immutable in the interpreter, the `list = list.push(item)` pattern is mandatory. This must be documented for Python/PS1 users.
4. **Namespace Shadowing:** Scripts must be careful not to name local variables after stdlib namespaces (e.g., `let path = ...` shadows the `path` builtin). Explicit namespace use is recommended.


