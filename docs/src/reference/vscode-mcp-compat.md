---
title: "VS Code extension and vox-mcp compatibility"
description: "Maps canonical MCP registry, VS Code parity scripts, activation model, wire aliases, MCP-related settings, release checklist, and manual compatibility/theme verification between the extension and vox-mcp."
category: "Language Reference"

schema_type: "TechArticle"
---

# VS Code extension ↔ `vox-mcp` compatibility

## Single sources of truth

| Artifact | Role |
|----------|------|
| [`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml) | Canonical MCP tool **names**, descriptions, and **`product_lane`** (builds `vox-mcp-registry`; each listed tool exposes `_meta.vox_product_lane` in its tool descriptor) |
| [`apps/editor/vox-vscode/scripts/check-mcp-tool-parity.mjs`](../../../apps/editor/vox-vscode/scripts/check-mcp-tool-parity.mjs) | **`npm run compile`** (and CI) runs this after registry generation: every `call('…')` / `callTool({ name: … })` in extension sources resolves to the canonical registry; aliases from [`tool_aliases.rs`](../../../crates/vox-orchestrator-mcp/src/tool_aliases.rs) |
| `apps/editor/vox-vscode/scripts/check-activation-parity.mjs` | **`npm run compile`** (and CI): every `contributes.commands` id has matching `onCommand:…` in `activationEvents` |
| [`apps/editor/vox-vscode/scripts/generate-mcp-tool-registry.mjs`](../../../apps/editor/vox-vscode/scripts/generate-mcp-tool-registry.mjs) | First step of **`npm run compile`**: emits `mcpToolRegistry.generated.ts` (canonical tool names + `MCP_EXTENSION_EXPECTED_TOOLS`) |
| Runtime `list_tools` | **Actual** advertised tools (includes skill-merged tools); `CapabilityRegistry` stores a fingerprint |
| [`apps/editor/vox-vscode/src/types/index.ts`](../../../apps/editor/vox-vscode/src/types/index.ts) (`WebviewMessage` and related protocol types) | zod schema for **host → webview** posts (`SidebarProvider.postMessage` validates before `postMessage`) |
| [`apps/editor/vox-vscode/scripts/smoke-host-messages.mjs`](../../../apps/editor/vox-vscode/scripts/smoke-host-messages.mjs) | Runs after `tsc` to ensure the host schema still accepts representative payloads |

## Activation (lazy load)

The extension is **not** `onStartupFinished`. It activates when:

- the workspace contains **`*.vox`**, or
- the user opens the **Vox Workspace** sidebar (`onView:vox-sidebar.chat`) or **Snapshots** (`onView:vox-snapshots`), or
- the user runs **any** contributed **`vox.*`** command (see `activationEvents` in [`apps/editor/vox-vscode/package.json`](../../../apps/editor/vox-vscode/package.json): build/run/LSP, inline edit family including **`vox.inlineEdit.accept`** / **`vox.inlineEdit.escapeReject`**, snapshots/VCS, plan, agent, model picker, Oratio, command catalog, etc.).

**`vox.inlineEdit.reject`** / **`vox.inlineEdit.regenerate`** are primarily CodeLens-driven; they also have **`onCommand`** activation so a bound key or replay does not depend on a prior command.

## Wire aliases (match `vox-mcp` `TOOL_WIRE_ALIASES`)

- `vox_budget_history` → `vox_cost_history`
- `vox_model_list` → `vox_list_models`
- `vox_map_vscode_session` → `vox_map_agent_session`
- (etc. — keep parity script in sync with [`crates/vox-orchestrator-mcp/src/tool_aliases.rs`](../../../crates/vox-orchestrator-mcp/src/tool_aliases.rs))

## Client disclosure (telemetry / debug surfaces)

User-visible copy and debug-style logging for the extension should stay aligned with **[`architecture/telemetry-client-disclosure-ssot.md`](../archive/research-2026-q1/telemetry-client-disclosure-ssot.md)** (orchestrator/MCP budget views, optional MCP payload logging).

## Extension settings

| Setting | Purpose |
|---------|---------|
| `vox.mcp.serverPath` | CLI binary for stdio (`vox mcp`) |
| `vox.mcp.debugPayloads` | Log tool args/results (truncated) -> the **Vox** output channel |
| `vox.mcp.warnOnMissingTools` | Log when `list_tools` lacks names in generated `MCP_EXTENSION_EXPECTED_TOOLS` (includes **`vox_oratio_transcribe`** and **`vox_speech_to_code`** for Oratio palette / voice capture) |

When testing optional orchestrator sidecar pilots, launch VS Code with matching env for the MCP process {

- `VOX_ORCHESTRATOR_DAEMON_SOCKET=<tcp-host:port>`
- optional `VOX_MCP_ORCHESTRATOR_RPC_READS=1` and/or `VOX_MCP_ORCHESTRATOR_RPC_WRITES=1`
- optional strict mismatch signal `VOX_MCP_ORCHESTRATOR_DAEMON_REPOSITORY_ID_STRICT=1`

MCP currently probes TCP peers only (stdio transport is valid for the daemon process itself but skipped for MCP peer probing).

## Release checklist

1. Bump `vox-vscode` `package.json` version with the MCP/server bundle you test against.
2. `cd apps/editor/vox-vscode && npm run compile && npm run lint` (`compile` runs MCP + activation parity checks after registry generation)
3. Manual smoke { connect MCP, open **Vox Workspace** (or **Vox: Open Chat** from the palette in a folder without `*.vox`), confirm the status strip shows `execution_mode` and tool count; test **Explorer** right-click on an audio file plus **Vox: Oratio —** transcribe / speech-to-code when `vox_oratio_transcribe` / `vox_speech_to_code` are advertised.

## Compatibility matrix (manual)

| Extension version | Notes |
|-------------------|--------|
| 0.2.x | Expects `ToolResult` JSON envelope unwrapping, `vox_compiler::ast_inspect`, runtime capability strip |

Document the **pinned** `vox` / `vox-mcp` crate version per release in your rollout notes when cutting editor builds.

## Visual / webview regression

Automated Playwright against the embedded webview is **not** in-repo yet. Before release, manually verify **Vox Workspace** in **Default Dark**, **Light+**, and **High Contrast** themes: dashboard strip, Agent Flow (task graph + lifecycle buttons), and Pipeline tab. File an issue if you want `@vscode/test-web` coverage added to CI.
