# vox-gui Wave 2 — Console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement task-by-task.

**Goal:** Apply the 24-item checklist to the Console community (`surfaces/Console/`).

**Surfaces:** Console shell, TerminalTab, InputEditor, DiscoveryRail, AgentStrip, SendToAgent, AgentTab.

**Prerequisites:** Phase 0 complete; Wave 1 gate passed.

---

## Checklist focus

| Item | Target |
|------|--------|
| IPC | Route discovery + PTY through `transport.ts` |
| Tokens | `visualTokens.ts` for xterm/SVG (done Phase 0A) |
| a11y | `role="textbox"`, labeled regions, toolbar `type="button"` |
| Tests | vitest suite (existing); e2e console route optional |

---

## Task 1: IPC audit

- [ ] **Step 1:** Grep `invoke` in `Console/` — allowlist only test files
- [ ] **Step 2:** Migrate `discoverySuggest` callers if any raw invoke remains
- [ ] **Step 3:** Update `ipcBoundaries.test.ts` allowlist shrink

---

## Task 2: Async states

- [ ] **Step 1:** DiscoveryRail loading skeleton while catalog fetch pending
- [ ] **Step 2:** AgentStrip empty state uses semantic tokens (done)

---

## Exit criteria

- `pnpm test src/components/surfaces/Console/` green
- Zero raw hex in Console/
- Toolbar controls explicit `type="button"` (Console.test.tsx)
