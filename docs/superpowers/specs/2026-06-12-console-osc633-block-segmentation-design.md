---
title: "Vox Console — OSC 633/133 block segmentation"
description: "Parse shell-integration markers from the PTY stream into command/output blocks; per-block exit-status indicator + block-aware send-to-agent."
category: "Architecture SSOTs"
status: "current"
---

# Vox Console — OSC 633/133 block segmentation (2026-06-12)

## Summary

Follow-up to the merged Vox Console (PR #268), which shipped raw xterm
scrollback. This adds **shell-integration block segmentation**: the spawned
shell emits OSC 633 markers, the terminal parses them into command/output
**blocks**, paints a per-block exit-status indicator, and wires
"send to agent" to the actual last block (command + output + exit code)
instead of the typed-line heuristic.

Scope is the **Core** slice agreed during brainstorming: block model + status
gutter dot + block-aware send/copy. No per-block hover toolbars, no block-list
side panel, no re-run/fold.

## Why OSC 633 (not plain 133)

Plain OSC 133 (FinalTerm/iTerm2) marks block boundaries but does **not** carry
the command text. The VS Code superset OSC 633 adds `E;<command-line>`, which
"send block to agent" needs. We parse the 633 set (and treat the 133 subset
A/B/C/D as aliases when present). Markers used:

- `OSC 633 ; A ST` — prompt start
- `OSC 633 ; B ST` — prompt end / command-line start
- `OSC 633 ; E ; <command> ST` — the command line (decoded)
- `OSC 633 ; C ST` — pre-execution (output starts)
- `OSC 633 ; D ; <exit> ST` — command finished, with exit code

(`ST` = string terminator, `\x07` or `\x1b\\`.)

## Architecture & data flow

```
shell (pwsh|bash) --emits OSC 633--> PTY --bytes--> Tauri vox://pty-output
   --> TerminalTab: term.write(data) + xterm registerOscHandler(633)
       --> osc633 reducer (pure) builds Block[]
           --> per-block decoration (status dot) on the command's start line
           --> onBlock(latestCompletedBlock) up to Console
               --> send-to-agent / copy use the block (cmd+output+exit)
```

## Components (each independently testable)

### 1. `crates/vox-gui/src/commands/pty.rs` — shell-integration injection
- `shell_integration_snippet(shell: &str) -> Option<String>` (new, pure):
  returns the OSC 633 init for `pwsh` / `bash`; `None` for anything else.
  - **pwsh**: wrap the existing `prompt` function so it emits `A` (prompt
    start) and `B` (prompt end); use a `PSConsoleHostReadLine`/command hook to
    emit `E;<command>` and `C` before execution and `D;$LASTEXITCODE` after.
    Preserve the user's original prompt output between A and B.
  - **bash**: set `PROMPT_COMMAND` to emit `D;$?` then `A`; `PS1` wraps `B`;
    a `DEBUG` trap (guarded to fire once per command) emits `E;<BASH_COMMAND>`
    then `C`. Preserve the user's `PS1`.
- `pty_spawn`: after the child is spawned, if
  `shell_integration_snippet(&default_shell())` is `Some(s)`, write `s` to the
  PTY writer **before** returning. Unknown shells → no injection (today's raw
  scrollback behavior, unchanged).
- The snippet is written so its own echo is suppressed where practical (e.g.
  leading control to clear the injected line); if a stray line shows, it is
  cosmetic only and does not affect parsing.

### 2. `crates/vox-gui/ui/src/components/surfaces/Console/osc633.ts` — pure reducer
- Types: `Block { id: number; command: string; exitCode: number | null;
  startLine: number; endLine: number; running: boolean }`.
- `createBlockReducer()` returning `{ onMarker(kind, payload, cursorLine), blocks(), latestCompleted() }`.
  - `A` → begin a new pending block (record `startLine`).
  - `E;<cmd>` → set `command` (URL/percent-decoded per the 633 encoding).
  - `C` → mark `running = true` (output region begins at `cursorLine`).
  - `D;<exit>` → set `exitCode`, `running = false`, `endLine = cursorLine`,
    finalize; becomes `latestCompleted()`.
  - `B` → no state change needed for Core (prompt-end), accepted and ignored.
  - Defensive: out-of-order / missing markers never throw; a `D` with no open
    block is dropped; a second `A` without `D` finalizes the prior as
    `exitCode: null`.
- Pure and DOM-free → unit-tested directly.

### 3. `TerminalTab.tsx` — wire the handler + status decoration
- New optional prop `onBlock?: (b: Block) => void`.
- In the mount effect: `term.parser.registerOscHandler(633, (data) => { reducer.onMarker(...); return true; })`. Parse `data` as `"<kind>" | "<kind>;<payload>"`.
- On each finalized block, call `term.registerMarker(...)` + `term.registerDecoration({ marker })` to paint a small status dot in the gutter at the block's command line — **green** (exit 0), **red** (non-zero), **neutral** (unknown). Dispose decorations with the terminal.
- Call `onBlock(reducer.latestCompleted())` when a block finalizes.

### 4. `Console.tsx` — block-aware send + copy
- Replace `lastLine` usage: keep `latestBlock` state set from `TerminalTab`'s
  `onBlock`. The send-to-agent composer prefills with the block rendered as
  `"$ <command>\n<…output…>\n(exit <code>)"`; falls back to `lastLine` when no
  block exists (no shell integration).
- Add a small "copy last block" affordance next to "send to agent" that copies
  the same rendered block to the clipboard.

## Error handling / degradation
- No markers (exotic shell, injection failed, integration off): no blocks, no
  dots; send-to-agent falls back to the existing typed-line behavior. Fully
  backwards-compatible.
- Malformed/partial OSC payloads: reducer ignores them; terminal output is
  unaffected (xterm still renders the raw bytes).
- Injection write failure: logged at debug, terminal still usable.

## Testing
- `osc633.ts` (vitest): full block (A→E→C→D), missing exit (A→E→C then new A),
  interleaved/again, `D` with no open block, command-text decoding.
- `pty.rs` (unit): `shell_integration_snippet("pwsh").is_some()`,
  `("bash").is_some()`, `("fish"|"cmd").is_none()`; snippet contains the 633
  marker bytes for each supported shell.
- `TerminalTab.tsx` (vitest, xterm mocked): OSC handler registered; `onBlock`
  fires after a `D` marker is delivered to the handler.
- `Console.tsx` (vitest): with a block present, the composer prefill contains
  the command + exit; without a block, falls back to the typed line.
- Gates: `tsc`, full vitest, `cargo test -p vox-gui` (pty), `cargo fmt`.

## Where things live (add rows in same PR)
| Concept | Location |
|---|---|
| Shell-integration snippet (OSC 633 injection) | `crates/vox-gui/src/commands/pty.rs` (`shell_integration_snippet`) |
| OSC 633 block reducer (pure) | `crates/vox-gui/ui/src/components/surfaces/Console/osc633.ts` |
| Block status decoration + onBlock wiring | `crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.tsx` |
| Block-aware send/copy | `crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx` |

## Out of scope (this slice)
Per-block hover toolbars; re-run; output folding/collapse; block-list side
panel; per-block duration timing; clickable jump-to-block; OSC 633 cwd (`P;Cwd`)
and nonce/auth extensions.
