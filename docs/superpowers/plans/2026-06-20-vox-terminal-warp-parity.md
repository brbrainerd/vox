---
title: "Vox Terminal (Warp-parity standalone) — Implementation Plan"
description: "End-to-end plan to extract a UI-agnostic terminal/agent core and ship a headless-capable ratatui TUI (vox-term) plus a refactored GUI Console, with a VoxMENS training flywheel. Authored for Claude Code → Sonnet 4.6 handoff."
category: "Plans"
status: "draft"
training_eligible: false
training_rationale: "Internal implementation plan; superseded once executed."
---

# Vox Terminal (Warp-parity) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a standalone, headless-capable Vox terminal+agent application (`vox-term`, ratatui) and an embeddable shared engine (`vox-terminal-core`) that brings ~95% of Warp.dev's experience onto the Vox harness, with the existing GUI Console refactored to render the *same* engine.

**Architecture:** Extract a UI-agnostic `vox-terminal-core` crate that owns the block model, input router, PTY/shell host, OSC-633 block parser, Vox-interpreter adapter, a thin adapter over `vox-orchestrator` (agent loop / HITL / hopper / budget / model-select), and a typed transcript-event stream. Front-ends (`vox-term` ratatui TUI; the existing React/Tauri Console) are thin renderers over it. A curation/redaction layer turns transcripts into a VoxMENS-ready corpus.

**Tech Stack:** Rust, `ratatui` + `crossterm` (TUI), `portable-pty` (PTY/ConPTY — already in use), `vte`/clean-room OSC-633 parser, `nucleo` (command palette), `reedline` (line editing), `tokio`, `vox-orchestrator`, `vox-journal` (transcript substrate), `vox-telemetry-otlp` redaction, `vox-similarity` (dedup), `vox-compiler` `eval --interp` (Vox-native execution). Default font: **IBM Plex Mono Nerd Font**.

---

## 0. How to read this plan

This spans multiple subsystems, so it is structured as a **master plan with sequenced Tracks**. Each Track produces working, testable software on its own.

- **Track 1 (`vox-terminal-core` foundation)** is the critical path and is written out in **full bite-sized TDD steps** — every other Track depends on its public types.
- **Tracks 2–7** are decomposed to **file map + interface + task list** granularity. They depend on types Track 1 creates, so their step-level TDD is intentionally deferred to the **Phase-4 audit/rewrite** (per the project's research→plan→audit→handoff process). Do **not** start a Track 2–7 task before Track 1's interfaces are merged and frozen.
- §4 (Claude Code execution strategy) tells the implementing harness *how* to parallelize and where the harness's own limits bite.

---

## 1. Codebase audit — retire / move / keep

Grounded in the current tree (read, not guessed):

| Existing asset | Path | Disposition | Rationale |
|---|---|---|---|
| PTY host + OSC-633 shell-integration snippets (pwsh/bash) | `crates/vox-gui/src/commands/pty.rs` | **MOVE → `vox-terminal-core::pty` (de-Tauri-fy)** | Pure logic (`default_shell`, `shell_integration_snippet`, `PtyManager`) is reusable; only the `#[tauri::command]` wrappers + `Emitter` are GUI-specific. Replace event emission with an `mpsc` byte stream. Leave a thin Tauri shim in vox-gui that calls core. |
| OSC-633 block parser | `crates/vox-gui/ui/src/components/surfaces/Console/osc633.ts` | **REWRITE in Rust → `vox-terminal-core::osc633`** | Block assembly currently only exists in TS, so the TUI can't get blocks. Port the state machine to Rust (it's our code — no license concern). Keep `osc633.ts` until Track 3 retires it. |
| Live EventBus dashboard | `crates/vox-cli/src/commands/live.rs` | **KEEP as reference; later re-skin over core** | Already subscribes to `orch.event_bus()` and renders an ANSI dashboard — the canonical pattern for the TUI's event rendering. Not the agent loop. |
| Vox-expression REPL | `crates/vox-cli/src/commands/repl.rs` | **MOVE logic → `vox-terminal-core::vox_interp`** | The interpreter wiring (`run_frontend_str_with_options` + `Interpreter`) is exactly the Vox-native execution path. `vox repl` becomes a thin alias. |
| Agent-definition CRUD | `crates/vox-cli/src/commands/agent.rs` | **KEEP unchanged** | Not a loop — just registry CRUD. Out of scope. |
| React Console surfaces | `crates/vox-gui/ui/.../surfaces/Console/*` | **REFACTOR → thin renderer over core (Track 3)** | `Console.tsx`, `TerminalTab.tsx`, `AgentStrip.tsx`, `DiscoveryRail.tsx` keep their look; their block/agent logic delegates to new Tauri commands backed by core. |
| Orchestrator engine | `crates/vox-orchestrator/*` | **KEEP; ADAPT (never reimplement)** | `feedback`, `hopper`, `budget`, `attention`, `events`, `build_repo_scoped_orchestrator`, `event_bus()` are consumed through a thin `vox-terminal-core::agent` adapter. |
| Transcript substrate | `crates/vox-journal` | **KEEP; consume** | Append-only crash-safe JSONL is the flywheel's source of truth. |
| Redaction/projection | `crates/vox-telemetry-otlp` (`redact_event`, `project_event`) | **KEEP; reuse in flywheel** | Secret-stripping for the training corpus. |
| Near-dup similarity | `crates/vox-similarity` | **KEEP; reuse in flywheel** | Corpus dedup. |
| History/clip store, context_windows domain | `crates/vox-db` | **KEEP; consume** | Persistence for blocks/history. |

**Net new crates:** `vox-terminal-core` (lib, L3 — depends on orchestrator) and `vox-term` (bin, L4).

---

## 2. Where (and whether) to draw from Warp.dev source

Warp open-sourced in **April 2026** ([github.com/warpdotdev/warp](https://github.com/warpdotdev/warp)) under a **split license**: `warpui_core` + `warpui` are **MIT**; **everything else — terminal, Agent Mode, blocks, persistence, LSP, settings, CLI — is AGPLv3.**

**Rule: clean-room reuse of *patterns*, not *code*.** The parts we'd want (block engine, Agent Mode, shell integration, persistence) are AGPLv3; AGPL's network-copyleft would force all linking Vox code to AGPL and to publish source to network users — incompatible with Vox's licensing/plugin/commercial posture. **No AGPL Warp source enters the tree.** We read those crates as an *architecture reference* and reimplement.

| Warp area | Draw type | What we take | Where it lands |
|---|---|---|---|
| Block model (`terminal`/blocks) | **Reference only** | The *idea*: command+output+exit+timing as selectable data | `vox-terminal-core::block` (clean-room) |
| Agent Mode context threading | **Reference only** | How blocks feed agent context; turn boundaries | `vox-terminal-core::agent` (over our orchestrator) |
| Shell integration | **None needed** | We already have our own OSC-633 (`pty.rs`) | already ours |
| `warpui` GPU framework (MIT) | **Vendorable IF needed** | Only relevant if we later add a GPU renderer — **out of scope** (we chose ratatui) | n/a for this plan |
| Alacritty grid/state model | **Use upstream directly** (`alacritty_terminal`, Apache-2.0/MIT) | The grid model Warp itself forked — take it license-clean from the source, not from Warp | `vox-terminal-core::pty` child-output interpretation |

**Decision gate G-LEGAL (Phase-4):** confirm with the owner that we are *not* AGPL-licensing `vox-term`. If the owner *chooses* AGPL for the standalone app only, a literal code-vendor of Warp's blocks/Agent-Mode becomes legal — but it would have to be an isolated AGPL binary that talks to Vox over a process boundary, never a linked crate. Default assumption: **clean-room, no vendor.**

---

## 3. Track breakdown & dependency graph

```
T1  vox-terminal-core foundation        (critical path, sequential)
      │
      ├──────────────┬──────────────┬───────────────┐
      ▼              ▼              ▼               ▼
T2 vox-term TUI   T3 GUI Console  T4 Input router  T5 Flywheel
   (ratatui)         refactor        completion        (curation→MENS)
      │              │              │
      └──────────────┴──────────────┘
                     ▼
T6  Shell audit + Nushell/zsh/fish integration   (research gate G-SHELL → impl)
T7  Packaging / distribution (vox term binary + font)
```

- **T1 must fully land and freeze its public API before T2–T5 begin.**
- **T2, T3, T4, T5** are mutually independent once T1 is frozen → parallelizable (see §4).
- **T6** depends on the PTY host (T1) and the shell-integration seam; its design half is a **research gate**.
- **T7** depends on T2.

---

## 4. Claude Code execution strategy (parallelism, workflows, subagents, limits)

This plan will be executed by **Claude 4.6 Sonnet** via the Claude Code harness. Guidance and the harness's own limits:

**Sequencing vs. parallelism**
- **T1 is strictly sequential, single-threaded.** Its tasks churn shared public types; parallel agents would conflict constantly. Use **subagent-driven-development one task at a time**, reviewing between tasks.
- **After T1's API freezes**, T2/T3/T4/T5 touch disjoint files → safe to **fan out**. Each parallel worker should run in **its own git worktree** (`isolation: "worktree"`) to avoid working-tree conflicts. A `Workflow` `parallel()`/`pipeline()` over the four tracks is appropriate *only if the owner opts into multi-agent orchestration*; otherwise dispatch sequential subagents.
- Within a track, the per-task TDD loop (write failing test → run → implement → run → commit) stays sequential.

**Harness limitations to design around (document these in the handoff)**
- **Subagents are read-only in this repo's sandbox** (recorded gotcha): dispatched agents get shell/write **DENIED** in worktree sandboxes. *Implication:* parallel **writing** subagents may not work in the owner's Claude Code environment — prefer the pattern "subagent produces the diff/plan; main session writes + verifies + commits," or run parallelism via worktrees the main session owns. The handoff doc (Phase 5) must state this explicitly so Sonnet doesn't assume parallel writers.
- **Context window:** Track 1's file count is large; instruct the worker to read only the files named per task, not the whole crate.
- **Rate limits:** any web/deep-research (G-SHELL, G-LEGAL) must use **2–3 targeted fetches**, never the mass-verify workflow (recorded gotcha: 110-agent verify trips server rate-limit and mislabels 0–0 as "refuted").
- **`cargo` on Windows:** never pipe `cargo` to `head`/`grep` (recorded gotcha — orphans thousands of processes); redirect to a file. Run clippy per-crate (`cargo clippy -p <crate> -- -D warnings`).
- **Vox formatting ban:** use `vox run scripts/fmt.vox` / `cargo fmt -p <crate>`, never `cargo fmt --all`.

**Verification gates (every track):** `cargo test -p <crate>`, `cargo clippy -p <crate> -- -D warnings`, and `cargo run -p vox-arch-check` must pass before a track is "done." New crates need a `layers.toml` row and a `where-things-live.md` row **in the same PR**.

---

## 5. Track 1 — `vox-terminal-core` foundation (FULL TDD)

**Crate:** `crates/vox-terminal-core/` (new). Add to root `Cargo.toml` workspace members, `layers.toml` (L3), and `where-things-live.md`.

**File map:**
- `src/lib.rs` — module wiring + public re-exports
- `src/block.rs` — `BlockId`, `BlockKind`, `BlockStatus`, `OutputChunk`, `Block`
- `src/input.rs` — `Submission`, `InputMode`, `InputIntent`, `classify()`
- `src/pty.rs` — de-Tauri'd PTY host (moved from vox-gui) + `default_shell`/`shell_integration_snippet`
- `src/osc633.rs` — Rust OSC-633 parser (ported from `osc633.ts`)
- `src/session.rs` — `Session`, `SessionEvent`, block list state machine
- `src/vox_interp.rs` — Vox-native execution adapter (from `repl.rs`)
- `src/agent.rs` — orchestrator adapter (`AgentAdapter`)
- `src/transcript.rs` — `TranscriptEvent` + journal sink
- `tests/` — integration tests per module

### Task 1.1: Scaffold the crate

**Files:**
- Create: `crates/vox-terminal-core/Cargo.toml`
- Create: `crates/vox-terminal-core/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members`)
- Modify: `crates/vox-arch-check/layers.toml` (add `vox-terminal-core` at L3)

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-terminal-core/tests/smoke.rs
#[test]
fn crate_links() {
    assert_eq!(vox_terminal_core::CRATE_NAME, "vox-terminal-core");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-terminal-core --test smoke`
Expected: FAIL — crate/target does not exist yet.

- [ ] **Step 3: Minimal implementation**

```toml
# crates/vox-terminal-core/Cargo.toml
[package]
name = "vox-terminal-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true, features = ["derive"] }
tokio = { workspace = true, features = ["sync", "rt", "macros"] }
portable-pty = { workspace = true }
anyhow = { workspace = true }
```

```rust
// crates/vox-terminal-core/src/lib.rs
//! UI-agnostic terminal + agent engine. Front-ends (ratatui TUI, GUI Console)
//! render its blocks/events and submit input back. Adapts vox-orchestrator;
//! never reimplements the agent loop.
pub const CRATE_NAME: &str = "vox-terminal-core";

pub mod block;
pub mod input;
```

Add `"crates/vox-terminal-core"` to `[workspace] members` in root `Cargo.toml` and a row in `layers.toml`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-terminal-core Cargo.toml crates/vox-arch-check/layers.toml
git commit -m "feat(terminal-core): scaffold vox-terminal-core crate"
```

### Task 1.2: Block model

**Files:**
- Create: `crates/vox-terminal-core/src/block.rs`
- Test: `crates/vox-terminal-core/tests/block.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-terminal-core/tests/block.rs
use vox_terminal_core::block::{Block, BlockKind, BlockStatus, OutputChunk, Stream};

#[test]
fn block_accumulates_output_and_finishes() {
    let mut b = Block::new(BlockKind::Shell, "echo hi");
    assert_eq!(b.status, BlockStatus::Running);
    b.push(OutputChunk::text(Stream::Stdout, "hi\n"));
    b.finish(0);
    assert_eq!(b.status, BlockStatus::Ok);
    assert_eq!(b.exit_code, Some(0));
    assert_eq!(b.plain_output(), "hi\n");
}

#[test]
fn nonzero_exit_marks_failed() {
    let mut b = Block::new(BlockKind::Shell, "false");
    b.finish(1);
    assert_eq!(b.status, BlockStatus::Failed);
    assert_eq!(b.exit_code, Some(1));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-terminal-core --test block`
Expected: FAIL — `block` module / types missing.

- [ ] **Step 3: Minimal implementation**

```rust
// crates/vox-terminal-core/src/block.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind { VoxNative, Shell, AgentTurn, SlashCommand, System }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockStatus { Running, Ok, Failed, Cancelled }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stream { Stdout, Stderr, Agent }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputChunk { pub stream: Stream, pub text: String }
impl OutputChunk {
    pub fn text(stream: Stream, s: impl Into<String>) -> Self { Self { stream, text: s.into() } }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub kind: BlockKind,
    pub input: String,
    pub output: Vec<OutputChunk>,
    pub status: BlockStatus,
    pub exit_code: Option<i32>,
}
impl Block {
    pub fn new(kind: BlockKind, input: impl Into<String>) -> Self {
        Self { kind, input: input.into(), output: vec![], status: BlockStatus::Running, exit_code: None }
    }
    pub fn push(&mut self, c: OutputChunk) { self.output.push(c); }
    pub fn finish(&mut self, exit: i32) {
        self.exit_code = Some(exit);
        self.status = if exit == 0 { BlockStatus::Ok } else { BlockStatus::Failed };
    }
    pub fn plain_output(&self) -> String { self.output.iter().map(|c| c.text.as_str()).collect() }
}
```

Add `pub mod block;` to `lib.rs` (already added in 1.1).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test block`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-terminal-core/src/block.rs crates/vox-terminal-core/tests/block.rs
git commit -m "feat(terminal-core): typed Block model"
```

### Task 1.3: Input router

**Files:**
- Create: `crates/vox-terminal-core/src/input.rs`
- Test: `crates/vox-terminal-core/tests/input.rs`

Routing contract (decided in brainstorming): **Vox-native is the default**; `/sh …` (or a bare `!…`) drops to the PTY shell; `/ai …` is an agent prompt; any other `/<verb> …` is a harness slash-command.

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-terminal-core/tests/input.rs
use vox_terminal_core::input::{classify, InputIntent};

#[test]
fn default_is_vox_native() {
    assert_eq!(classify("let x = 1"), InputIntent::VoxNative("let x = 1".into()));
}
#[test]
fn slash_sh_is_shell() {
    assert_eq!(classify("/sh git status"), InputIntent::Shell("git status".into()));
}
#[test]
fn bang_is_shell() {
    assert_eq!(classify("!ls -la"), InputIntent::Shell("ls -la".into()));
}
#[test]
fn slash_ai_is_agent() {
    assert_eq!(classify("/ai fix the failing test"), InputIntent::Agent("fix the failing test".into()));
}
#[test]
fn other_slash_is_command() {
    assert_eq!(classify("/model list"),
        InputIntent::Command { name: "model".into(), args: "list".into() });
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-terminal-core --test input`
Expected: FAIL — `input` module missing.

- [ ] **Step 3: Minimal implementation**

```rust
// crates/vox-terminal-core/src/input.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputIntent {
    VoxNative(String),
    Shell(String),
    Agent(String),
    Command { name: String, args: String },
}

pub fn classify(raw: &str) -> InputIntent {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix('!') {
        return InputIntent::Shell(rest.trim().to_string());
    }
    if let Some(rest) = t.strip_prefix('/') {
        let (name, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        return match name {
            "sh" | "shell" => InputIntent::Shell(args.trim().to_string()),
            "ai" | "agent" => InputIntent::Agent(args.trim().to_string()),
            _ => InputIntent::Command { name: name.to_string(), args: args.trim().to_string() },
        };
    }
    InputIntent::VoxNative(t.to_string())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test input`
Expected: PASS (all five).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-terminal-core/src/input.rs crates/vox-terminal-core/tests/input.rs
git commit -m "feat(terminal-core): input router (Vox-native default, slash-to-shell)"
```

### Task 1.4: OSC-633 parser (Rust port of `osc633.ts`)

**Files:**
- Read first: `crates/vox-gui/ui/src/components/surfaces/Console/osc633.ts` (the reference state machine — port its semantics)
- Create: `crates/vox-terminal-core/src/osc633.rs`
- Test: `crates/vox-terminal-core/tests/osc633.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-terminal-core/tests/osc633.rs
use vox_terminal_core::osc633::{Osc633Parser, Osc633Event};

#[test]
fn parses_command_and_exit_markers() {
    let mut p = Osc633Parser::new();
    // A (prompt start), E;<cmd>, C (pre-exec), output, D;<exit>
    let mut evs = vec![];
    evs.extend(p.feed(b"\x1b]633;A\x07"));
    evs.extend(p.feed(b"\x1b]633;E;ls -la\x07"));
    evs.extend(p.feed(b"\x1b]633;C\x07"));
    evs.extend(p.feed(b"total 0\n"));
    evs.extend(p.feed(b"\x1b]633;D;0\x07"));
    assert!(evs.contains(&Osc633Event::PromptStart));
    assert!(evs.contains(&Osc633Event::CommandLine("ls -la".into())));
    assert!(evs.contains(&Osc633Event::PreExec));
    assert!(evs.iter().any(|e| matches!(e, Osc633Event::Output(s) if s == "total 0\n")));
    assert!(evs.contains(&Osc633Event::Exit(0)));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-terminal-core --test osc633`
Expected: FAIL — parser missing.

- [ ] **Step 3: Minimal implementation**

Implement a byte-fed scanner that recognizes `ESC ] 633 ; <payload> BEL` (and `ESC \` ST terminator), decodes the `E;` escapes (`\x5c`→`\`, `\x3b`→`;`, `\x0a`→`\n`) matching the snippets in `pty.rs`, and emits `Output(String)` for bytes between markers. Buffer partial sequences across `feed` calls.

```rust
// crates/vox-terminal-core/src/osc633.rs  (shape — port osc633.ts semantics)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Osc633Event {
    PromptStart, PromptEnd, CommandLine(String), PreExec, Exit(i32), Output(String),
}

#[derive(Default)]
pub struct Osc633Parser { buf: Vec<u8> }

impl Osc633Parser {
    pub fn new() -> Self { Self::default() }
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Osc633Event> {
        self.buf.extend_from_slice(bytes);
        // scan self.buf for ESC]633;…BEL / ST, emit events, retain trailing
        // partial; emit non-marker runs as Output. (Full impl mirrors osc633.ts.)
        todo!("port from osc633.ts; keep tests above green")
    }
}
fn decode_command(enc: &str) -> String {
    enc.replace("\\x0a", "\n").replace("\\x3b", ";").replace("\\x5c", "\\")
}
```

> Implementer note: replace the `todo!` with the real scanner; the test in Step 1 plus a partial-sequence test (split a marker across two `feed` calls) define correctness. Add that split test before finishing.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test osc633`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-terminal-core/src/osc633.rs crates/vox-terminal-core/tests/osc633.rs
git commit -m "feat(terminal-core): Rust OSC-633 block parser (port of osc633.ts)"
```

### Task 1.5: PTY host (move + de-Tauri-fy from vox-gui)

**Files:**
- Read first: `crates/vox-gui/src/commands/pty.rs`
- Create: `crates/vox-terminal-core/src/pty.rs`
- Test: `crates/vox-terminal-core/tests/pty.rs`

Move `default_shell`, `shell_integration_snippet`, `PWSH_OSC633`, `BASH_OSC633` **verbatim** (they are pure). Replace the Tauri command + `Emitter` with a `PtyHost::spawn(shell, cols, rows) -> (PtyHandle, mpsc::Receiver<Vec<u8>>)` API; `PtyHandle::{write, resize, kill}`.

- [ ] **Step 1: Write the failing test** (keep the existing pure-fn tests; they must pass post-move)

```rust
// crates/vox-terminal-core/tests/pty.rs
use vox_terminal_core::pty::{default_shell, shell_integration_snippet};

#[test]
fn default_shell_nonempty() { assert!(!default_shell().is_empty()); }
#[test]
fn snippet_supports_pwsh_and_bash() {
    assert!(shell_integration_snippet("pwsh").is_some());
    assert!(shell_integration_snippet("bash").is_some());
    assert!(shell_integration_snippet("fish").is_none()); // until Track 6
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-terminal-core --test pty`
Expected: FAIL — `pty` module missing.

- [ ] **Step 3: Minimal implementation**

Port the pure functions verbatim from `vox-gui/src/commands/pty.rs`; add the `PtyHost`/`PtyHandle` channel API (reader thread sends `Vec<u8>` over `tokio::sync::mpsc`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test pty`
Expected: PASS.

- [ ] **Step 5: Re-point vox-gui to core (no behavior change)**

In `vox-gui/src/commands/pty.rs`, delete the moved pure fns and `pub use vox_terminal_core::pty::{default_shell, shell_integration_snippet};`. Keep the `#[tauri::command]` wrappers; have `pty_spawn` drive `PtyHost` and forward the `mpsc` stream to the existing `vox://pty-output` emitter. Run `cargo test -p vox-gui` (PTY tests still green).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-terminal-core/src/pty.rs crates/vox-terminal-core/tests/pty.rs crates/vox-gui/src/commands/pty.rs
git commit -m "refactor(pty): move PTY host into vox-terminal-core; vox-gui delegates"
```

### Task 1.6: Vox-native execution adapter

**Files:**
- Read first: `crates/vox-cli/src/commands/repl.rs`
- Create: `crates/vox-terminal-core/src/vox_interp.rs`
- Test: `crates/vox-terminal-core/tests/vox_interp.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-terminal-core/tests/vox_interp.rs
use vox_terminal_core::vox_interp::eval_line;

#[test]
fn evaluates_vox_expression() {
    let out = eval_line("fn main() -> Int { 40 + 2 }").unwrap();
    assert!(out.contains("42"));
}
#[test]
fn surfaces_compile_errors() {
    assert!(eval_line("let = ").is_err());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-terminal-core --test vox_interp`
Expected: FAIL — module missing.

- [ ] **Step 3: Minimal implementation**

Port `repl.rs`'s pipeline (`run_frontend_str_with_options` + `Interpreter::call("main", …)`) into `eval_line(src) -> Result<String>`; add `vox-compiler` as a dependency. Return rendered value / error string instead of printing.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test vox_interp`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-terminal-core/src/vox_interp.rs crates/vox-terminal-core/tests/vox_interp.rs crates/vox-terminal-core/Cargo.toml
git commit -m "feat(terminal-core): Vox-native execution adapter (from repl.rs)"
```

### Task 1.7: Transcript events + journal sink

**Files:**
- Create: `crates/vox-terminal-core/src/transcript.rs`
- Test: `crates/vox-terminal-core/tests/transcript.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-terminal-core/tests/transcript.rs
use vox_terminal_core::transcript::{TranscriptEvent, TranscriptKind};

#[test]
fn event_roundtrips_json() {
    let e = TranscriptEvent { session_id: "s1".into(), seq: 1,
        kind: TranscriptKind::Submitted { intent: "Shell".into(), input: "ls".into() } };
    let j = serde_json::to_string(&e).unwrap();
    let back: TranscriptEvent = serde_json::from_str(&j).unwrap();
    assert_eq!(back, e);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-terminal-core --test transcript`
Expected: FAIL.

- [ ] **Step 3: Minimal implementation**

Define `TranscriptEvent { session_id, seq, kind }` and `TranscriptKind` variants `Submitted{intent,input}`, `Output{stream,text}`, `AgentTurn{text}`, `ExitStatus{code}`, `Accepted{block}`, `Rejected{block}`, `Corrected{from,to}`. Add a `JournalSink` that appends serialized events to a `vox-journal` file. (Add `vox-journal` dep.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test transcript`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-terminal-core/src/transcript.rs crates/vox-terminal-core/tests/transcript.rs crates/vox-terminal-core/Cargo.toml
git commit -m "feat(terminal-core): typed transcript events + journal sink"
```

### Task 1.8: Session state machine + SessionEvent stream

**Files:**
- Create: `crates/vox-terminal-core/src/session.rs`
- Test: `crates/vox-terminal-core/tests/session.rs`

The `Session` owns the block list, applies `Osc633Event`s and agent events to the current block, emits `SessionEvent`s (`BlockOpened`, `OutputAppended`, `BlockClosed`, `AgentMessage`) for front-ends, and writes transcript events.

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-terminal-core/tests/session.rs
use vox_terminal_core::session::Session;
use vox_terminal_core::block::BlockStatus;

#[test]
fn shell_block_lifecycle_from_osc_events() {
    let mut s = Session::new("s1");
    s.on_pty_bytes(b"\x1b]633;A\x07\x1b]633;E;ls\x07\x1b]633;C\x07out\n\x1b]633;D;0\x07");
    let blocks = s.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].input, "ls");
    assert_eq!(blocks[0].plain_output(), "out\n");
    assert_eq!(blocks[0].status, BlockStatus::Ok);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-terminal-core --test session`
Expected: FAIL.

- [ ] **Step 3: Minimal implementation**

Wire `Osc633Parser` → block transitions; expose `blocks()`, a `SessionEvent` receiver, and `submit(intent)` that routes VoxNative→`vox_interp`, Shell→`PtyHost`, Agent/Command→`AgentAdapter` (Task 1.9). Emit transcript events on each transition.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-terminal-core --test session`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-terminal-core/src/session.rs crates/vox-terminal-core/tests/session.rs
git commit -m "feat(terminal-core): Session block state machine + event stream"
```

### Task 1.9: Orchestrator agent adapter

**Files:**
- Read first: `crates/vox-cli/src/commands/live.rs` (event subscription pattern), `crates/vox-orchestrator/src/lib.rs` (exports)
- Create: `crates/vox-terminal-core/src/agent.rs`
- Test: `crates/vox-terminal-core/tests/agent.rs`

The adapter subscribes to `orch.event_bus()` and translates `AgentEvent` → `SessionEvent::AgentMessage` / block updates; `submit_prompt(text)` enqueues a task. **It must not reimplement the loop** — only translate. Feedback/HITL surfacing reuses the orchestrator `feedback` module.

- [ ] **Step 1: Write the failing test** — a fake `AgentEvent` (e.g. `TokenStreamed`) maps to a `SessionEvent::AgentMessage` with the streamed text. (Construct via `build_repo_scoped_orchestrator(OrchestratorConfig::default(), None)` and publish a token event, mirroring `live.rs`.)
- [ ] **Step 2: Run to verify it fails.** Run: `cargo test -p vox-terminal-core --test agent`
- [ ] **Step 3: Minimal implementation** — `AgentAdapter { orch }` with `subscribe()` returning a stream of mapped `SessionEvent`s, and `submit_prompt`. Add `vox-orchestrator` dep.
- [ ] **Step 4: Run to verify it passes.**
- [ ] **Step 5: Commit** — `feat(terminal-core): orchestrator agent adapter (event translation only)`

### Task 1.10: Public API freeze + arch-check

- [ ] **Step 1:** Re-export the stable surface in `lib.rs` (`block`, `input`, `osc633`, `pty`, `vox_interp`, `transcript`, `session`, `agent`).
- [ ] **Step 2:** Run `cargo test -p vox-terminal-core`, `cargo clippy -p vox-terminal-core -- -D warnings`, `cargo run -p vox-arch-check`. All green.
- [ ] **Step 3:** Add the `where-things-live.md` row for `vox-terminal-core`.
- [ ] **Step 4: Commit** — `docs(terminal-core): freeze v0 public API + where-things-live row`

> **GATE:** Track 1 API is now frozen. Tracks 2–5 may begin in parallel.

---

## 6. Track 2 — `vox-term` ratatui TUI (decompose; harden in Phase-4)

**Crate:** `crates/vox-term/` (bin, L4). **File map:** `src/main.rs` (crossterm setup, event loop), `src/app.rs` (state: `Session` + view state), `src/ui/blocks.rs` (block list render), `src/ui/input.rs` (reedline input box, mode indicator), `src/ui/palette.rs` (nucleo command palette), `src/theme.rs` (IBM Plex Mono Nerd Font glyphs/colors).

**Task list (each becomes a TDD task in Phase-4):**
1. Crossterm raw-mode lifecycle + clean teardown (test: terminal restored on drop).
2. Render a static block list from `Session::blocks()` (snapshot test of the ratatui buffer).
3. Input box with `reedline`; submit → `classify` → `Session::submit` (test: a `!ls` submission opens a Shell block).
4. Live `SessionEvent` subscription → repaint (test: streamed output appends to the open block).
5. Command palette via `nucleo` over registered slash-commands (test: fuzzy "mdl"→"model").
6. Mode indicator + theme (Nerd-Font powerline glyphs; document font requirement).
7. `vox term` entry: headless check — runs under a dumb terminal / over SSH (test: starts with `TERM=dumb` without panicking).

**Reference (not copy):** `live.rs` for the event-render pattern.

---

## 7. Track 3 — GUI Console refactor (decompose; harden in Phase-4)

**Goal:** the existing React Console renders the *same* core. **Surface the terminal in the same terminal view** in Axis, now backed by `vox-terminal-core` instead of TS-local logic.

**File map:**
- New Rust: `crates/vox-gui/src/commands/terminal_core.rs` — Tauri commands `term_submit`, `term_subscribe` (emits `vox://term-event` from `SessionEvent`), backed by a core `Session` per tab.
- Modify: `crates/vox-gui/ui/.../Console/Console.tsx`, `TerminalTab.tsx` — call the new commands; consume `vox://term-event`.
- **Retire:** `Console/osc633.ts` + `osc633.test.ts` once block events come from core (the Rust parser is the SSOT).
- Keep: `pty.rs` thin wrapper from Task 1.5.

**Task list (Phase-4 TDD):**
1. Add `terminal_core.rs` commands + register in the Tauri builder (test: command returns block JSON).
2. `TerminalTab.tsx` consumes `vox://term-event` instead of parsing OSC locally (vitest: a mocked term-event renders a block).
3. Delete `osc633.ts`/`osc633.test.ts`; update imports; `cargo run -p vox-arch-check` + `gui-surface-registry` parity green.
4. Verify Console parity (blocks, agent strip, send-to-agent) against pre-refactor screenshots.

---

## 8. Track 4 — Input router completion (decompose; harden in Phase-4)

Completes the harness side of `classify`'s `Command { name, args }` dispatch.

**File map:** `crates/vox-terminal-core/src/commands.rs` — a slash-command registry mapping `name`→handler over the orchestrator/CLI surface (`/model`, `/budget`, `/skills`, `/memory`, `/context`, `/sh`, `/ai`). **Task list:** registry type + 1 handler per task (TDD: `/model list` returns the model pool); wire `Session::submit` `Command` arm to it; add a `/help` listing.

---

## 9. Track 5 — VoxMENS training flywheel (decompose; harden in Phase-4)

**Scope (decided):** emit typed transcripts (Track 1) **+ curate** into a MENS-ready corpus. Trainer run itself is out of scope.

**File map:**
- `crates/vox-terminal-core/src/corpus/redact.rs` — wrap `vox-telemetry-otlp::redact_event`/`project_event` to strip secrets from transcript text.
- `crates/vox-terminal-core/src/corpus/curate.rs` — fold a session's transcript into labeled samples (accept/reject/correction pairs), dedup via `vox-similarity`.
- `crates/vox-terminal-core/src/corpus/writer.rs` — write the curated corpus to the MENS-expected location/schema.

**Task list (Phase-4 TDD):** redaction passes a secret-bearing line and asserts the secret is gone; curation turns an `Accepted`/`Rejected` pair into one labeled sample; dedup drops a near-duplicate; writer emits the agreed JSONL. **Privacy gate G-PRIV:** confirm the corpus opt-in/consent model with the owner (reuse telemetry's `VOX_TELEMETRY` off-switch posture).

---

## 10. Track 6 — Shell audit + Nushell/zsh/fish (research gate → impl)

**Research gate G-SHELL (Phase-3, 2–3 targeted fetches only):** rank candidate underlying shells for "ideal for LLM generation." Seed hypothesis: **Vox-native default (already structured) → Nushell (typed/structured pipelines, Rust-native, possibly in-process via `nu-*` crates) → system shell (pwsh/zsh) over PTY for muscle-memory.** Deliverable: a short decision doc under `docs/src/architecture/`.

**Impl tasks (after gate):** add `zsh`/`fish` OSC-633 snippets to `pty.rs` (currently `None`); optionally a Nushell in-process adapter so `/sh` can return structured values to the agent. TDD: `shell_integration_snippet("zsh").is_some()`; Nushell adapter returns structured output for a pipeline.

---

## 11. Track 7 — Packaging / distribution

`vox-term` binary built and shipped; **IBM Plex Mono Nerd Font** documented as the recommended terminal font (and bundled where the installer can). Add a `vox term` alias from `vox-cli`. Headless/SSH smoke test in CI. Follows the existing single-command install program (see memory: install/release/publish program).

---

## 12. Research / decision gates (consolidated)

| Gate | Phase | Question | Method |
|---|---|---|---|
| G-LEGAL | 4 | Is `vox-term` AGPL or not? (Determines whether *any* Warp code can be vendored.) | Owner decision; default = clean-room, no vendor |
| G-SHELL | 3 | Which underlying shell is "ideal for LLM generation"? | 2–3 targeted fetches; decision doc |
| G-PRIV | 5 | Corpus consent/opt-in model | Owner decision; reuse telemetry off-switch |
| G-WARP | 3 | Deep-read Warp's block/Agent-Mode crates for clean-room patterns | Targeted read of AGPL crates *as reference only* |

---

## 13. Self-review (against the brainstorming design)

- **Form factor (TUI, runs anywhere):** Track 2 + Track 7 (headless/SSH smoke tests). ✓
- **SSOT (extract shared core):** Track 1 is the core; Tracks 2/3 are renderers; §1 forbids reimplementing the loop. ✓
- **Input model (Vox-native default, slash-to-shell, agent mode):** Task 1.3 + Track 4. ✓
- **GUI Console = front-end of core; same terminal view:** Track 3. ✓
- **Flywheel (emit + curate):** Track 1.7 + Track 5. ✓
- **Font (IBM Plex Mono Nerd Font):** Track 2.6 + Track 7. ✓
- **Shell audit:** Track 6 / G-SHELL. ✓
- **Warp ingestion (where/how):** §2 — clean-room patterns, no AGPL code; `alacritty_terminal` taken upstream. ✓
- **Claude Code parallelism/workflows/subagents + limits:** §4. ✓
- **Audit / retire / move:** §1 table, grounded in real files. ✓

**Placeholder scan:** the only deliberate `todo!` is in Task 1.4 Step 3, bounded by the tests in that task plus a required split-sequence test — not a plan gap. **Type consistency:** `Block`/`BlockKind`/`BlockStatus`/`OutputChunk`/`Stream`, `InputIntent`, `Osc633Event`, `SessionEvent`, `TranscriptEvent`/`TranscriptKind`, `PtyHost`/`PtyHandle`, `AgentAdapter` are used consistently across tasks.
