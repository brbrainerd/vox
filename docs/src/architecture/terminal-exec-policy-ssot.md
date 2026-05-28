---
title: "Terminal Exec Policy SSOT (2026)"
description: "Live SSOT for the PowerShell-first terminal execution policy: what the claim actually is, what evidence supports it, and what it does NOT claim."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-30"
training_eligible: true
training_rationale: "Stable policy SSOT referenced from AGENTS.md and contracts/terminal/exec-policy.v1.yaml."
schema_type: "TechArticle"
---
# Terminal Exec Policy SSOT (2026)

This file is the live SSOT for the Vox terminal execution policy. It supersedes the archived 2026-Q1 research notes (`docs/src/archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md`, `terminal-ast-validation-research-2026.md`) by extracting only the load-bearing claims and pinning them to current evidence.

## What this policy IS

PowerShell 7 (`pwsh`) is preferred over Bash on Windows agent hosts because it improves **policy enforcement and output parsing at the IDE/host boundary**, not because LLMs produce better code in PowerShell.

Three load-bearing claims:

1. **Structured output is parseable.** PowerShell cmdlets emit .NET objects that round-trip through `ConvertTo-Json -Depth N` deterministically. Bash one-liners are text streams whose shape varies per tool. For an agent reading its own output back, structured JSON is more reliable than text scraping. _Evidence: Microsoft Learn — `ConvertTo-Json`, `Set-StrictMode`, `-ErrorAction Stop`._
2. **AST inspection is available pre-execution.** PowerShell exposes `System.Management.Automation.Language.Parser` so a host can extract `CommandAst` nodes before running anything. Bash has no equivalent first-party AST API of comparable fidelity. This matters for allowlist enforcement, not for codegen. _Evidence: Microsoft Learn — `Parser.ParseInput`._
3. **Prefix-allowlist matchers are brittle.** All major IDEs (Cursor, Gemini CLI, Codex, Antigravity) use string-prefix or simple-regex matching that fails on pipes, env-prefixes, and `bash -c '…'` wrappers — producing both approval fatigue and real bypass classes. _Evidence: GHSA-9868-vxmx-w862 (line-continuation + command-substitution bypass); openai/codex#13175 (wrapper/env-prefix mismatches); Cursor forum thread on prefix-vs-whole-line semantics._

## What this policy is NOT

- **Not a claim that LLMs are more fluent in PowerShell than Bash.** No public eval supports that, and the Vox repo has not run one. If you read the archived research as making this claim, that was a misreading.
- **Not a substitute for sandboxing.** Allowlists, even AST-validated ones, are a defense-in-depth layer — they do not replace process isolation.
- **Not a mandate to ship PowerShell as a project automation surface.** Project automation is **Vox** (see [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md)). PowerShell is retained only for the two thin launcher scripts (`scripts/windows/vox-dev.ps1`, `scripts/vox-dev.sh`) that solve the bootstrap problem.

## Implications for MENS training

A separate "PowerShell spoke" in the MENS training corpus is **not** justified by this policy. Adding one would:

- Contradict the Vox-as-glue policy by re-elevating a retired automation surface to first-class training data.
- Conflate two distinct concerns (host-side allowlisting vs. model-side codegen quality) under one corpus axis.
- Inflate corpus size with examples the model rarely needs to emit, since `.vox` scripts replace `.ps1` scripts in this repo.

The corpus axis that **is** justified by this policy is "agent emits structured-output cmdlets when inspecting state" — but that is a small fluency tweak inside the existing Vox corpus, not a new spoke.

## Contract surface

The machine-checked policy lives in [`contracts/terminal/exec-policy.v1.yaml`](../../../contracts/terminal/exec-policy.v1.yaml). It defines:

- `allowed_cmdlets` — explicit allowlist of inspection cmdlets
- `allowed_binaries` — `cargo`, `rustc`, `git`, `pwsh`, `powershell`
- `blocked_parameters` — currently blocks `-Recurse` globally
- `network_fetch_*` — fetch verbs and domain allowlist

Future work (deferred, not blocking): `vox ci terminal-policy-sync` / `terminal-policy-verify` to project this contract into Cursor `terminalAllowlist`, Gemini TOML, and Codex `prefix_rule` fragments. See [`crates/vox-cli/src/commands/ci/operations_catalog.rs`](../../../crates/vox-cli/src/commands/ci/operations_catalog.rs) for the pattern.

## Do we need a benchmark?

**No, for the policy as scoped above.** The claim ("PS is easier to allowlist and parse than Bash") is supported by structural evidence (AST API exists; structured output exists; documented IDE matcher failures) and does not need an empirical eval to act on.

**Yes, only if the claim is widened** to "agents produce more correct shell commands in PowerShell than Bash." That is a different claim, not currently made by Vox policy. If someone wants to make it, the eval shape is documented in [agent-shell-fluency-eval-design-2026.md](agent-shell-fluency-eval-design-2026.md) — a 20-task A/B run, not a MENS corpus change.

## Related

- [AGENTS.md §Cross-Platform Shell Discipline](../../../AGENTS.md)
- [`contracts/terminal/exec-policy.v1.yaml`](../../../contracts/terminal/exec-policy.v1.yaml)
- [Agent shell fluency eval design (2026)](agent-shell-fluency-eval-design-2026.md)
- Archived (do not ingest autonomously): `docs/src/archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md`, `terminal-ast-validation-research-2026.md`
