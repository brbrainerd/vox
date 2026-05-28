---
title: "Terminal execution policy research findings 2026"
description: "Evidence-backed synthesis for PowerShell-first agent shells, IDE allow/deny limitations, and a single-source terminal policy model aligned with Vox operations SSOT."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-02"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Terminal execution policy research findings 2026

## Purpose

This document persists research on how AI-assisted IDEs and CLIs gate **terminal command execution**, why **prefix allowlists** and **simple deny rules** break down on compound commands and shell wrappers, and how Vox can converge on **PowerShell 7 (`pwsh`)** as the preferred agent shell on Windows while planning a **single machine-verifiable policy SSOT** that projects into each tool’s native format.

It is **research**, not a shipped contract. Implementation should follow a future blueprint (contract + `vox ci` sync/verify) similar to [operations catalog SSOT](operations-catalog-ssot.md) and [completion policy SSOT](completion-policy-ssot.md).

## Provenance vocabulary

| Label | Meaning |
| --- | --- |
| **documented** | Stated in vendor or first-party project documentation. |
| **community-reported** | Forum threads, GitHub issues, or third-party guides; behavior may change between releases. |
| **security-advisory** | Published CVE/GHSA or equivalent; treat as hard evidence for parser/allowlist risk. |

## Executive summary

1. **Different hosts implement policy differently** — Cursor uses global `permissions.json` prefix rules; Gemini CLI uses a tiered TOML policy engine; Codex uses Starlark `prefix_rule` with documented shell-wrapper handling. No universal “one regex fits all.”
2. **Approval fatigue and false prompts** come from **string-level** or **prefix-only** matching when the model emits pipes, env prefixes, or `shell -c '…'` wrappers — matchers often disagree on what the “real” command is (**documented** + **community-reported**).
3. **Security** requires **conservative fallback** when parsing is ambiguous; real bypass classes exist where static analysis disagrees with runtime shell folding (**security-advisory**).
4. **PowerShell** helps agents produce **structured inspection output** (`ConvertTo-Json`, strict error semantics) but is **not** a substitute for sandboxing or a deny-first policy tier (**documented**).
5. **Vox already owns** the right integration seam: [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml), [`crates/vox-cli/src/commands/ci/operations_catalog.rs`](../../../crates/vox-cli/src/commands/ci/operations_catalog.rs) (`operations-sync` / `operations-verify`), and planner metadata (`side_effect_class`, `scope_kind`, …). A future **`terminal/exec-policy.v1`** contract should **compile** to Cursor, Gemini, Codex, and Antigravity artifacts under CI, not be edited by hand in four places.

## External evidence by platform

### Cursor — `permissions.json` and terminal allowlists (**documented**)

- Global file: `~/.cursor/permissions.json` (JSONC supported).
- `terminalAllowlist`: array of **prefix** strings; **case-sensitive**; patterns like `npm:install*` use `:` to separate base command from argument glob.
- **Override semantics**: when a key is present, it **replaces** the in-app list for that key (not merged).
- **No per-repo file** in this reference path; team admin controls can supersede user settings.
- Explicit caveat: allowlists are **not** a security boundary — see Cursor’s own security guidance linked from the same page.

Reference: [Cursor permissions.json reference](https://docs.cursor.com/)

### Cursor CLI — separate permissions model (**documented**)

The same doc notes **CLI permissions are separate** from the editor `permissions.json` surface. Any repo-wide automation must account for **two** configuration worlds if both are used.

Reference: [Cursor permissions.json reference](https://docs.cursor.com/) (CLI permissions note)

### Cursor — community-reported matcher pain (**community-reported**)

Users report that allow/deny behavior is hard to reason about (e.g. `grep` allowed but specific flag/regex invocations still prompting; prefix semantics vs whole-line expectations). Cursor staff have acknowledged **prefix matching** and recommended **deny overrides** for dangerous subcommands until richer matching exists.

Reference: [Cursor forum — How does command allowlist/denylist really work?](https://forum.cursor.com/t/how-does-command-allowlist-denylist-really-work/102782/4)

### Gemini CLI — policy engine (**documented**)

- TOML rules under user, workspace, and admin locations; **priority + tier** resolution.
- Decisions: `allow`, `deny`, `ask_user` (non-interactive can downgrade `ask_user` → `deny`).
- Rich conditions: `commandPrefix`, `commandRegex` (with documented JSON-argument encoding caveats), `argsPattern`, MCP server rules, optional `allowRedirection`, approval **modes** (`default`, `autoEdit`, `plan`, `yolo`).

Reference: [Gemini CLI policy engine](https://geminicli.com/docs/reference/policy-engine)

### Codex — rules and execution policies (**documented**)

- Starlark-style `prefix_rule()` with **ordered token** patterns, `match` / `not_match` examples, and `codex execpolicy check` for offline evaluation.
- **Shell wrappers**: documentation describes when a `bash -lc` / `zsh -lc` script is **split** into multiple commands for policy (linear chains of “safe” operators) vs when the whole invocation stays opaque (redirections, substitutions, env assignments in script) — **conservative** behavior when uncertain.
- **Strictest wins**: `forbidden` > `prompt` > `allow`.

References:

- [Codex rules](https://developers.openai.com/codex/rules)
- [Codex execution policies](https://www.mintlify.com/openai/codex/advanced/exec-policies)

### Codex — wrapper and env-prefix mismatch reports (**community-reported**)

GitHub issue discussion { `prefix_rule` may fail to match when the executed argv is a **shell wrapper** or when commands use leading **`VAR=value`** assignments, causing repeated approvals and brittle saved rules.

Reference { [openai/codex#13175](https://github.com/openai/codex/issues/13175)

### OpenClaw — allowlist bypass class (**security-advisory**)

Published advisory: allowlist analysis could be bypassed when **line continuation** + **command substitution** folding differs between static analysis and actual shell execution — patched by rejecting dangerous continuation patterns and hardening wrapper handling.

Reference: [GHSA-9868-vxmx-w862](https://github.com/openclaw/openclaw/security/advisories/GHSA-9868-vxmx-w862)

### Google Antigravity — browser allow/deny (**documented**)

Official Antigravity documentation for **browser** URL allowlist/denylist (denylist via service; local allowlist file). This is **not** the same subsystem as terminal execution policy, but it illustrates the product’s layered “prompt + list” security UX.

Reference: [Antigravity allowlist / denylist (browser)](https://antigravity.google/docs/allowlist-denylist)

### Antigravity — terminal execution policy (third-party hardening guide) (**community-reported**)

Community security write-ups describe terminal modes such as **Auto**, **Off (allow list only)**, and **Turbo (deny list only)** and recommend **allow-list-only** for high-sensitivity work. Treat as **operational guidance**, not Google’s normative spec, unless corroborated by official docs you pin to a version.

Reference: [antigravity.codes — Antigravity security guide](https://antigravity.codes/blog/antigravity-security-guide)

## PowerShell as the preferred Windows agent shell (**documented**)

Relevant first-party PowerShell documentation:

- **`ConvertTo-Json`**: serializes .NET objects to JSON; supports `-Depth`, `-Compress`, `-AsArray` (helpful for stable machine-readable listings). Default `-Depth` is shallow — agents should set depth explicitly when emitting nested objects.
- **`-ErrorAction Stop`**: turns **non-terminating** errors into terminating failures for the current command (preference variables behave differently in nested scopes — document for script modules).
- **`Set-StrictMode`**: additional **parse-time / usage** strictness (uninitialized variables, invalid property access, bad indexing by version). Complements but does not replace explicit error handling.

References:

- [ConvertTo-Json](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/convertto-json)
- [about_CommonParameters (-ErrorAction)](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_commonparameters)
- [Set-StrictMode](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/set-strictmode)

**Implication for agents:** prefer `Get-ChildItem | ConvertTo-Json` (with explicit `-Depth`) over ad hoc text scraping when the goal is **structured** state for the model — but **policy** should still assume malicious or mistaken compound scripts are possible.

## Recommended direction for Vox (research — not shipped)

### 1. Single canonical policy contract

Introduce a versioned contract under `contracts/` (name TBD, e.g. `contracts/terminal/exec-policy.v1.yaml`) that defines:

- **Shell profile**: default `pwsh` on Windows; document POSIX dev exceptions only where CI/docs already require them ([runner contract](../ci/runner-contract.md)).
- **Risk classes** aligned with existing planner hints in the operations catalog (`side_effect_class`, `scope_kind`, `reversible`, …).
- **Deny wins** patterns (regex or structured) applied **before** allow.
- **Normalization rules**: strip leading env assignments when safe; unwrap known `-c` / `-File` forms when the inner script passes a **strict** parser; otherwise classify as **high risk** / `ask_user`.
- **Projection targets**: fragments for Cursor `terminalAllowlist`, Gemini `*.toml`, Codex `.rules`, and human “paste blocks” for Antigravity — all **generated**, never hand-edited as primaries.

### 2. CI enforcement

Add `vox ci terminal-policy-sync` / `terminal-policy-verify` mirroring [`operations_catalog.rs`](../../../crates/vox-cli/src/commands/ci/operations_catalog.rs):

- verify committed fragments match contract
- ship golden tests for compound commands (pipe, `&&`, nested `pwsh -c`, env prefixes)

### 3. Runtime alignment

Route **Vox-native execution** through the same semantic layer {

- [`crates/vox-runtime/src/builtins.rs`](../../../crates/vox-runtime/src/builtins.rs) — `vox_process_run*` (scripts)
- [`crates/vox-cli/src/commands/runtime/shell/mod.rs`](../../../crates/vox-cli/src/commands/runtime/shell/mod.rs) — `vox shell` passthrough
- Orchestrator / MENS / MCP any future “run command” tools

Today these paths are **not** unified; this doc records the **intent** for a later implementation phase.

### 4. Contributor-facing discipline (already partial SSOT)

- [`GEMINI.md`](../../../GEMINI.md) — Antigravity overlay; PowerShell-first command shape.
- [`docs/src/contributors/agent-instruction-architecture.md`](../contributors/agent-instruction-architecture.md) — layering model and copy-paste blocks.

Keep these **short**; put evidence tables and long citations **here**.

## Non-goals (this research pass)

- Final JSON Schema for `exec-policy.v1` (deferred to implementation blueprint).
- Changing Cursor/Gemini/Codex on-disk config on developer machines automatically.
- Replacing [Clavis secret policy](../../../AGENTS.md) or [completion policy](completion-policy-ssot.md).

## Related Vox docs

- [Agent instruction architecture](../contributors/agent-instruction-architecture.md)
- [Operations catalog SSOT](operations-catalog-ssot.md)
- [AI IDE feature research findings 2026](ai-ide-feature-research-findings-2026.md)
- [Cross-platform shell discipline](../../../AGENTS.md) (`AGENTS.md`)

## Maintenance

When adding IDE hosts or changing policy engines:

1. Update the **evidence** sections with `documented` vs `community-reported` labels.
2. Bump `last_updated` in frontmatter.
3. Run `vox ci check-docs-ssot` after link edits.


