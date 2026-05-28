---
title: "Telemetry client disclosure SSOT"
description: "VS Code extension, MCP host, and debug surfaces that affect user perception of data collection and trust."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-02"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Telemetry client disclosure SSOT

## Purpose

Users and enterprises evaluate Vox on **what leaves the machine** and **what is named “telemetry.”** This SSOT maps client-visible surfaces and required disclosure patterns.

## Naming collision: webview `telemetry` tab

The VS Code webview sidebar ([`apps/editor/vox-vscode/webview-ui/src/index.tsx`](../../../apps/editor/vox-vscode/webview-ui/src/index.tsx)) shows **local** dashboard-style content (for example [`UnifiedDashboard.tsx`](../../../apps/editor/vox-vscode/webview-ui/src/components/UnifiedDashboard.tsx)), not a remote analytics pipeline.

**Implementation rule:** user-facing copy MUST distinguish:

- **Local stats / budgets** (current tab)
- **Optional product telemetry** (future, if introduced)

Prefer labels such as **“Usage & budgets”** or **“Local insights”** in product copy when implementing UX changes; keep route ids stable for compatibility unless a migration note ships in [CHANGELOG](../../../CHANGELOG.md).

## MCP debug and payload visibility

[vscode-mcp-compat](../reference/vscode-mcp-compat.md) documents **`vox.mcp.debugPayloads`**, which can log tool arguments and results. This is **diagnostic-class (S3 adjacent)** and MUST:

- default **off**
- be documented next to Ludus [`VOX_LUDUS_MCP_TOOL_ARGS`](../../../crates/vox-ludus/src/mcp_privacy.rs) behavior in [env-vars](../reference/env-vars.md)
- never be described as “anonymous telemetry”

## Extension README

[`apps/editor/vox-vscode/README.md`](../../../apps/editor/vox-vscode/README.md) SHOULD link to:

- this SSOT
- [telemetry-trust-ssot](telemetry-trust-ssot.md)
- [telemetry-unification-research-findings-2026](telemetry-unification-research-findings-2026.md) (research context)

## Host application caveat (normative)

MCP hosts (Cursor, VS Code, others) may have **their own** telemetry and network policies. Vox documentation MUST state that **host telemetry is outside Vox’s control plane**, consistent with industry practice (for example VS Code’s extension telemetry caveat in upstream docs).

## Related

- [Telemetry trust boundary and SSOT map](telemetry-trust-ssot.md)
- [Environment variables (SSOT)](../reference/env-vars.md)


