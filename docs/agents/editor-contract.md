# Vox VS Code Extension — Editor Contract

The VS Code extension is the **presentation layer** of the Vox ecosystem. It owns the editor
UX and delegates all state, coordination, and computation to the Vox Orchestrator via MCP.

## Sole Responsibilities

| Area | What the Extension Does |
|---|---|
| Language decorations | IntelliSense / syntax highlighting via LSP client (`vox-lsp`) |
| Inline AI | Ghost text (tab-complete) and `Ctrl+K` inline edit trigger |
| Sidebar / chat | Webview panel streamed with JSON from MCP |
| Status bar | Polling Orchestrator status via `vox_orchestrator_status` |
| VCS UX | Snapshot tree view, undo/redo buttons (data from `vox_oplog`) |
| Gamification HUD | Renders XP/mood badge (data from `vox_gamify_state`) |
| Keybindings | `vox.chat`, `vox.plan`, `vox.rebalance`, etc. |
| Toasts / progress | UI-only; triggered by MCP responses |
| File hooks | On save: emits training record request to Orchestrator |

## Hard Rules

- **No business logic in the extension**. All logic lives in Rust (Orchestrator or CLI).
- **No direct Rust imports**. All communication goes through `VoxMcpClient.call(toolName, params)`.
- **No state stored in the extension** beyond the current webview render buffer.
- **VS Code settings are UX overrides only**. Structural config (model, budget, data paths) is
  read via `vox_get_config` MCP tool, not `workspace.getConfiguration` directly.

## MCP Connection Lifecycle

```
VS Code activate()
  → spawn  `vox-mcp` binary (stdio)
  → VoxMcpClient.connect()
  → background: StatusBarManager polls every 10s
  → background: AgentController polls every 5s
  → background: GamifyManager polls every 30s
```

## Extension Source Layout

```
vox-vscode/src/
  extension.ts              ← activation, wires everything below
  core/
    VoxMcpClient.ts         ← typed MCP call wrapper (ONLY comms layer)
    ConfigManager.ts        ← VS Code setting accessors (UX prefs only)
    LspClientManager.ts     ← starts/stops the vox-lsp server
    StatusBarManager.ts     ← polls vox_orchestrator_status
  inline/
    GhostTextProvider.ts    ← tab-complete ghost text
    InlineEditController.ts ← Ctrl+K inline diff
  vcs/
    SnapshotProvider.ts     ← VCS tree view + undo/redo
  agents/
    AgentController.ts      ← polls vox_orchestrator_status for agent list
  gamify/
    GamifyManager.ts        ← polls vox_gamify_state
  commands/
    model.ts                ← model-switch quick-pick
  SidebarProvider.ts        ← webview provider + postMessage bridge
```

## Settings Owned by the Extension (UX-only)

| Setting | Purpose |
|---|---|
| `vox.mcpBinaryPath` | Path to the `vox-mcp` binary |
| `vox.vcsShowSnapshotBar` | Toggle VCS snapshot sidebar |
| `vox.statusBarVisible` | Show/hide status bar item |
| `vox.inlineGhostText` | Enable/disable tab ghost text |
