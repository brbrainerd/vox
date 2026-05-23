---
title: "Editor integrations (LSP & grammar)"
description: "How to wire Vox into Neovim, Helix, Zed, JetBrains, Sublime, and VS Code-class editors using the repo LSP server and Tree-sitter grammar."
category: "How-To Guides"
last_updated: "2026-05-11"
training_eligible: true
schema_type: "HowTo"
---

# Editor integrations (LSP & grammar)

Vox ships three editor-facing artifacts in-tree:

| Artifact | Location | Role |
| --- | --- | --- |
| **Language server** | [`crates/vox-lsp`](../../../crates/vox-lsp) | stdio JSON-RPC server (`vox lsp`): diagnostics, completions, hover, semantic tokens, symbols, code lenses, structured quick-fixes. |
| **VS Code extension** | [`apps/editor/vox-vscode`](../../../apps/editor/vox-vscode) | Reference integration for VS Code / Cursor / forks that load VSIX-shaped extensions. |
| **Tree-sitter grammar** | [`tree-sitter-vox`](../../../tree-sitter-vox) | Syntax highlighting / incremental parsing for editors that consume Tree-sitter queries. |

Capability inventory (research): [`vox-lsp-capabilities-ssot-2026.md`](../architecture/vox-lsp-capabilities-ssot-2026.md).

## VS Code / Cursor / forks

1. Build or install the **`vox`** CLI (see [`reference/cli.md`](../reference/cli.md)).
2. Load / package the extension under **`apps/editor/vox-vscode`** (workspace-specific instructions live in that folder’s `README` / contribution docs).
3. Point **`vox.lsp.command`** (or equivalent contributed setting) at `vox lsp` or `cargo run -p vox-lsp` during bring-up.

## Neovim (`nvim-lspconfig`)

Minimal pattern — spawn the same binary CI uses:

```lua
-- Example: adjust to your `vox` install path.
vim.lsp.start({
  name = 'vox',
  cmd = { 'vox', 'lsp' },
  root_dir = vim.fs.dirname(vim.fs.find({ 'Vox.toml', 'Cargo.toml', '.git' }, { upward = true })[1]),
})
```

Use Tree-sitter queries from **`tree-sitter-vox/queries/`** with `nvim-treesitter` if you want highlighting parity with VS Code.

## Helix

Add a `language-server` block in **`languages.toml`**:

```toml
[[language]]
name = "vox"
scope = "source.vox"
file-types = ["vox"]
roots = ["Vox.toml", "Cargo.toml"]
language-servers = [ "vox-lsp" ]

[language-server.vox-lsp]
command = "vox"
args = ["lsp"]
```

Helix does not ship a Vox grammar — pair with **`tree-sitter-vox`** build steps from that crate’s README when you need injections.

## Zed

Zed extensions are Wasm-hosted; there is **no first-party Zed extension** in this repo yet. Practical options:

- Use **external file-type / grammar** hooks if your Zed build supports custom Tree-sitter grammars, pointing at **`tree-sitter-vox`**.
- Track LSP via a future extension that shells out to `vox lsp` (same command line as other editors).

## JetBrains (IntelliJ / RustRover / plugin SDK)

No JetBrains plugin sources ship here. Recommended interim:

- Enable **LSP client** support (Gateway / third-party LSP plugins) and register `vox lsp`.
- Optional: import **`tree-sitter-vox`** into a small plugin for lexical highlighting until a full PSI exists.

## Sublime Text

Use **LSP package** + a client stanza:

```json
{
  "clients": {
    "vox": {
      "enabled": true,
      "command": ["vox", "lsp"],
      "selector": "source.vox"
    }
  }
}
```

Define the `.vox` syntax via **`.sublime-syntax`** generated from Tree-sitter or hand-maintained scopes — Tree-sitter bridge packages can consume **`tree-sitter-vox`**.

## Troubleshooting

- **Diagnostics differ from `vox check`** — LSP uses `validate_document` / `validate_document_with_hir`; confirm parity notes in [`vox-lsp-capabilities-ssot-2026.md`](../architecture/vox-lsp-capabilities-ssot-2026.md).
- **No document formatting in LSP** — formatting remains **`vox fmt`** / CLI (`document/formatting` is intentionally **not** advertised today).

## See also

- [`reference/cli.md`](../reference/cli.md) — `vox lsp` flags.
- [`vox-playground-architecture-research-2026.md`](../architecture/vox-playground-architecture-research-2026.md) — planned browser playground vs `vox shell repl`.
