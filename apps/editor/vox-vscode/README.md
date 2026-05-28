---
title: "Vox VS Code Extension"
description: "Syntax highlighting, LSP integration, and build commands for Vox — the AI-native, full-stack programming language."
category: "reference"
status: "current"
training_eligible: true
training_rationale: "Provides documentation for the VS Code extension, including commands and configuration."
---
# Vox Language Extension for VS Code

> Syntax highlighting, LSP integration, and build commands for [Vox](https://github.com/vox-foundation/vox) — the AI-native, full-stack programming language.

**Frontend output (2026):** `vox build` emits **`routes.manifest.ts`** and **`vox-client.ts`**; generated **`App.tsx` / `VoxTanStackRouter.tsx` / `serverFns.ts`** are retired. Use **`vox build --scaffold`** (or `VOX_WEB_EMIT_SCAFFOLD=1`) to seed a user-owned `app/App.tsx` + Vite shell that imports the manifest. Extension docs, snippets, and path assumptions should align with manifest-first adapters (see [`react-interop-hybrid-adapter-cookbook.md`](../../../docs/src/architecture/react-interop-hybrid-adapter-cookbook.md)).

## Features

- **Syntax Highlighting** — Full TextMate grammar for `.vox` files covering keywords, decorators (`@table`, `@endpoint`, `@agent_def`, `@skill`, `@mcp.tool`, etc.), bare declarations (`component Name() {}`, `state_machine Name {}`), types, JSX tags, strings, and comments.
- **Language Server Protocol** — Real-time diagnostics, hover information, and completions via `vox-lsp`.
- **Build Commands** — "Vox: Build Current File" and "Vox: Run Current Project" from the command palette.
- **Visual Editor** — "Vox: Open Visual Editor" opens a webview preview (`dist/index.html` when present, otherwise a localhost dev iframe).
- **MCP + Oratio** — With `vox mcp` connected (`vox.mcp.serverPath`), run any contributed **Vox:** command (or open **Vox Workspace**) to activate even in folders without `.vox` files; use **Vox: Oratio —** or **Explorer** right-click on audio (case-insensitive extension match). In-workspace files use a relative MCP path; external picks copy into `.vox/tmp/`. Voice capture writes mono WAV under `.vox/tmp/` then calls the same MCP tools. See [`docs/src/reference/speech-capture-architecture.md`](../../../docs/src/reference/speech-capture-architecture.md) in this workspace.
- **Status Bar** — Quick-access build button in the VS Code status bar.

## Installation

### From VSIX (Recommended)

```bash
cd apps/editor/vox-vscode
npm install
npm run compile
npx @vscode/vsce package
code --install-extension vox-lang-0.2.0.vsix
```

### From Source

```bash
cd apps/editor/vox-vscode
npm install
npm run compile
# Then press F5 in VS Code to launch the Extension Development Host
```

`npm run compile` already runs **MCP registry generation**, **`check:mcp-parity`**, and **`check:activation-parity`** before TypeScript/esbuild (same gates as CI). For parity with the full job:

```bash
npm run lint
npm run compile
```

## Requirements

- **VS Code** 1.85.0 or later
- **Vox LSP** — Build with `cargo build -p vox-lsp --release` (the extension will attempt to run it via `cargo run -p vox-lsp` as fallback)

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `vox.lsp.enabled` | `true` | Enable the Vox Language Server |
| `vox.lsp.serverPath` | `""` | Custom path to the `vox-lsp` binary |
| `vox.mcp.serverPath` | `vox` | CLI used for `vox mcp` (stdio MCP) |
| `vox.vcs.showSnapshotBar` | `true` | Show **Snapshots** under the Vox sidebar; **off** still exposes undo/redo/snapshot palette commands (QuickPick for list) |
| `vox.build.outputDir` | `"dist"` | Default output directory for builds |

## Privacy and telemetry

- **Vox product telemetry SSOT** (trust boundaries, naming, and debug flags): [`docs/src/architecture/telemetry-trust-ssot.md`](../../../docs/src/architecture/telemetry-trust-ssot.md), [`docs/src/architecture/telemetry-client-disclosure-ssot.md`](../../../docs/src/architecture/telemetry-client-disclosure-ssot.md).
- **MCP debug payloads** (`vox.mcp.debugPayloads`): see [`docs/src/reference/vscode-mcp-compat.md`](../../../docs/src/reference/vscode-mcp-compat.md) — high-sensitivity diagnostic, not anonymous usage data.
- The webview may expose a **local** “telemetry” or insights tab for **on-machine** stats; it is not a separate remote analytics product unless documented otherwise in the SSOT above.

## Commands

| Command | Description |
|---------|-------------|
| `Vox: Build Current File` | Runs `vox build` on the active `.vox` file |
| `Vox: Run Current Project` | Runs `vox run` on `src/main.vox` |
| `Vox: Restart Language Server` | Restarts the Vox LSP client |
| `Vox: Open Visual Editor` | Webview: workspace `dist/index.html` or localhost preview |
| `Vox: Oratio — Transcribe audio file` | MCP `vox_oratio_transcribe` on a picked file (copied to `.vox/tmp/`) |
| `Vox: Oratio — Speech to code (audio file)` | MCP `vox_speech_to_code` on a picked audio file |
| `Vox: Oratio — Voice capture → transcribe` | Webview mic → WAV → `vox_oratio_transcribe` |
| `Vox: Oratio — Voice capture → speech to code` | Webview mic → WAV → `vox_speech_to_code` |

## Supported Syntax

The extension highlights all Vox language constructs:

- **Keywords**: `fn`, `let`, `mut`, `return`, `if`, `else`, `for`, `while`, `match`, `type`, `import`
- **Decorators**: `@table`, `@endpoint`, `@test`, `@action`, `@skill`, `@agent_def`, `@mcp.tool`, `@v0`, `@pure`, `@deprecated`, `@require`, `@index`, `@storage`
- **Bare declarations**: `component Name() {}`, `state_machine Name {}`, `routes {}`
- **Types**: `int`, `str`, `bool`, `Unit`, `Element`, `List`, `Map`, `Set`, `Result`, `Option`, `Id`
- **Comments**: `#` line comments
- **JSX**: Inline JSX tags within `component` blocks


