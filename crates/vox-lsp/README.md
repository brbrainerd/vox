# Vox Language Server

This crate provides the Language Server Protocol (LSP) implementation for Vox.

## Features
- **Diagnostics**: Reports syntax errors (parser) and type errors (checker).
- **Go to Definition**: (Planned)
- **Completion**: (Planned)
- **Hover**: (Planned)

## Installation

1. Build the LSP server:
   ```bash
   cargo build --release -p vox-lsp
   ```
2. The binary will be at `target/release/vox-lsp`.

## Editor Configuration

### VS Code
Use the `vscode-vox` extension (TBD) or configure manually:
```json
"vox.lsp.serverPath": "/path/to/target/release/vox-lsp"
```
