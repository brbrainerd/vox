# Vox Language Extension for VS Code

> Syntax highlighting, LSP integration, and build commands for [Vox](https://github.com/brbrainerd/vox) — the AI-native, full-stack programming language.

## Features

- **Syntax Highlighting** — Full TextMate grammar for `.vox` files covering keywords, decorators (`@table`, `@server`, `@component`, `@agent_def`, `@skill`, `@mcp.tool`, etc.), types, JSX tags, strings, and comments.
- **Language Server Protocol** — Real-time diagnostics, hover information, and completions via `vox-lsp`.
- **Build Commands** — "Vox: Build Current File" and "Vox: Run Current Project" from the command palette.
- **Status Bar** — Quick-access build button in the VS Code status bar.

## Installation

### From VSIX (Recommended)

```bash
cd vox-vscode
npm install
npm run compile
npx @vscode/vsce package
code --install-extension vox-lang-0.1.0.vsix
```

### From Source

```bash
cd vox-vscode
npm install
npm run compile
# Then press F5 in VS Code to launch the Extension Development Host
```

## Requirements

- **VS Code** 1.85.0 or later
- **Vox LSP** — Build with `cargo build -p vox-lsp --release` (the extension will attempt to run it via `cargo run -p vox-lsp` as fallback)

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `vox.lsp.enabled` | `true` | Enable the Vox Language Server |
| `vox.lsp.serverPath` | `""` | Custom path to the `vox-lsp` binary |
| `vox.build.outputDir` | `"dist"` | Default output directory for builds |

## Commands

| Command | Description |
|---------|-------------|
| `Vox: Build Current File` | Runs `vox build` on the active `.vox` file |
| `Vox: Run Current Project` | Runs `vox run` on `src/main.vox` |
| `Vox: Restart Language Server` | Restarts the Vox LSP client |

## Supported Syntax

The extension highlights all Vox language constructs:

- **Keywords**: `fn`, `let`, `mut`, `ret`, `if`, `else`, `for`, `while`, `match`, `type`, `actor`, `workflow`, `activity`, `routes`, `style`, `import`
- **Decorators**: `@table`, `@server`, `@component`, `@test`, `@query`, `@mutation`, `@action`, `@skill`, `@agent_def`, `@mcp.tool`, `@v0`, `@pure`, `@deprecated`, `@require`, `@index`, `@storage`
- **Types**: `int`, `str`, `bool`, `Unit`, `Element`, `List`, `Map`, `Set`, `Result`, `Option`, `Id`
- **Comments**: `#` line comments
- **JSX**: Inline JSX tags within `@component` functions
