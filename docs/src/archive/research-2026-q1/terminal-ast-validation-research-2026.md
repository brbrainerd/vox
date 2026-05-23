---
title: "Terminal AST validation research 2026"
description: "Resolving IDE allowlist/denylist brittleness with a single-source-of-truth PowerShell AST validation engine and robust IDE enforcement."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-02"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Terminal AST Validation Research 2026

## 1. The Core Problem: Static String vs. Semantic Intent

Current AI IDE implementations of shell allowlists (e.g., Cursor's `permissions.json`, Gemini's TOML rules, Antigravity's implicit tool safeguards) rely on simplistic string-matching or regex. When agents emit complex PowerShell commands—featuring pipes (`|`), sequential execution (`;`, `&&`), command substitutions (`$()`), or aliases—the generic parsers in these IDEs fail. 

This results in two frustrating failure modes:
1. **False Positives (Blocked Safe Actions):** A command like `Get-ChildItem -Path . | Select-Object -First 5` is blocked because the IDE's allowlist wasn't configured to expect pipelining semantics, triggering an approval prompt.
2. **False Negatives (Bypassed Unsafe Actions):** A malicious or hallucinated command can disguise a denylisted binary inside a subshell or a string concatenation (e.g., `& ("Rm" + "-Dir")`), flying under the string-matching radar.

Our current stopgap in `GEMINI.md` restricts models to emit only one non-piped command per turn. This creates massive overhead and friction for the agent trying to accomplish multi-step goals.

## 2. Industry Standard Solution: Abstract Syntax Tree (AST) Validation

To solve this fundamentally, cybersecurity practices for PowerShell execution environments rely on semantic validation rather than string filtering. By utilizing PowerShell's built-in `[System.Management.Automation.Language.Parser]` namespace, an input command isn't treated as a string; it is broken down into an Abstract Syntax Tree.

### How it Works
When a command is passed into the parser:
```powershell
$ast = [System.Management.Automation.Language.Parser]::ParseInput($rawCommand, [ref]$tokens, [ref]$errors)
```
The `$ast` object understands the language hierarchically. We can query it to isolate exactly what actual executable or cmdlet will run, regardless of aliases, piping, or variable obfuscation:
```powershell
# Accurately extracts every invoked command across the entire pipe/compound chain
$commands = $ast.FindAll({ $args[0] -is [System.Management.Automation.Language.CommandAst] }, $true)
```

By reading the `CommandAst`, the system can semantically extract the root commands and instantly cross-validate them against an explicitly approved list, effectively blocking malicious injections and permitting arbitrarily complex, safe piping constructs.

## 3. Critique: The "Last-Mile" Compliance Problem

The obvious theoretical approach is to map the SSOT to IDE configs (like `permissions.json` allowing only `vox`) and use system prompts like `GEMINI.md` to tell the agent: *"Always wrap your commands in `vox shell`"*. 

**Will this actually work? No.** 
The major flaw in relying on prompts and soft ide-configs is **Agent Hallucination and Habit**:
- **Cursor AI** limits agent capabilities if it constantly tries to use `pwsh` native syntax and hits a wall of "Permission Denied", spinning the chat into a loop of failures.
- **Antigravity IDE** has a native `run_command` tool. Even if `GEMINI.md` tells it to use `vox shell <cmd>`, the model may frequently forget, calling `run_command(Command: "Remove-Item -Recurse .")` natively. The agent falls back to its baseline training, completely bypassing our `vox` rules framework.

We cannot rely purely on the AI's "chat" obedience. The enforcement must happen at a system or workspace level, completely transparently, so that even if the AI fails to use `vox`, the environment forcibly reroutes its actions through the Vox AST validation engine.

## 4. Implementation Details: Forcing IDE Compliance (Codebase-Wide)

To guarantee that *both* Cursor and Antigravity (and future IDEs) adhere to the Vox terminal SSOT without stripping away details or breaking their native functionality, we implement environment-level interceptors.

### A. The Single Source of Truth
We establish one strict YAML defining permitted command classes, domains, and prohibited dangerous vectors:
`contracts/terminal/exec-policy.v1.yaml`

### B. The AST Validator Engine (`vox check-terminal`)
A pure Rust routine using our existing interop pathways (or a highly optimized proxy script) that wraps the `System.Management.Automation.Language.Parser`. It parses the AST, extracts every `CommandAst`, and cross-validates against `exec-policy.v1.yaml`.

### C. Workspace-Level Hijacking 
Rather than hoping the AI adheres to a prompt, we hijack the environment the AI operates in.

#### 1. Cursor AI Enforcement (Shell Proxy Hijacking)
Cursor runs an integrated terminal instance for its agent. We exploit this by changing the local workspace `.vscode/settings.json` to override the shell executable.
```json
{
    "terminal.integrated.defaultProfile.windows": "Vox Proxy",
    "terminal.integrated.profiles.windows": {
        "Vox Proxy": {
            "path": "${workspaceFolder}/.vox/bin/vox-pwsh-proxy.cmd"
        }
    }
}
```
`vox-pwsh-proxy.cmd` acts as a transparent shell that receives Cursor's piped strings and routes them through `vox check-terminal`. 
- **Benefit**: The Cursor AI thinks it's interacting with standard `pwsh`. It doesn't have to change its behavior. Vox intercepts, parses the AST, and allows/denies transparently without causing prompt loops.

#### 2. Antigravity Enforcement (PowerShell Profile Injection)
Antigravity executes commands interactively using PowerShell. We enforce compliance by leveraging the local PowerShell `$PROFILE` (or injecting a `-NoProfile -Command "Import-Module VoxInterceptor"` wrapper) into all agent workspace environments.
We use a `PreCommandLookupAction` or `PSReadLine` hook inside the PowerShell session that runs *automatically* when Antigravity submits the `run_command` tool.
- When Antigravity calls a command, the PowerShell host invokes `vox check-terminal <command text>`.
- If the AST parser flags a denied command, the PowerShell session immediately halts execution and returns a structured error explicitly referencing the `vox-schema` policy: *"Vox Policy Blocked: Attempted to run a destructive command outside allowed paths. Review GEMINI.md."*
- **Benefit**: Antigravity is natively restrained by the interpreter it calls, preventing it from applying "its own rules" and ensuring our codebase SSOT fundamentally rules the local execution space.

## 5. Alignment with Existing Codebase Rules

- **`docs/agents/editor-contract.md`**: Enforces "No business logic in the extension/IDE. All logic lives in Rust." By pushing validation into `vox check-terminal`, neither Cursor nor Antigravity extension layers need custom business logic.
- **`docs/src/architecture/terminal-exec-policy-research-findings-2026.md`**: Validates the recommendation to avoid flat configuration targets, transitioning instead to dynamic policy injection via proxying.
- **`GEMINI.md` & `AGENTS.md`**: Strict limitations on piping commands (`|`, `&&`) can confidently be removed once the `vox check-terminal` AST validation correctly parses compound payloads.

## 6. Summary

By transitioning from simplistic prompt-based execution limits to an **environment-hijacking deployment**, we remove the burden from the LLM. Both Cursor and Antigravity can operate as they normally do, generating complex, piped commands. 
The workspace terminal settings/profiles silently route every execution through `vox check-terminal`, executing the PowerShell AST parse against `contracts/terminal/exec-policy.v1.yaml`. This guarantees codebase-wide persistence without divergence.


