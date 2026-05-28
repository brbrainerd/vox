---
title: "AI CLI Generation Standard"
description: "Architecture and design principles for LLM-driven CLI command generation via internal AST representations."
category: "Architecture SSOTs"
status: "current"
sort_order: 10
last_updated: "2026-04-10"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# AI CLI Generation Standard

As the Vox CLI becomes deeply integrated with the MENS model and agentic workflows, we must ensure that all command generations are syntactically valid and structurally sound. Relying on raw text token generation for CLI commands often leads to flag hallucinations, syntax errors, and unpredictable string formatting.

This standard establishes the **Intermediate Representation (AST/JSON)** pattern as the single source of truth for MENS-to-CLI invocation.

## 1. The Intermediate Representation (IR) Pattern

Instead of generating a raw terminal string (e.g., `vox populi train --gpu`), the MENS model must emit a structured intent mapping that aligns with an Abstract Syntax Tree (AST).

### 1.1 Structural Constraints

The MENS output is constrained to a predefined JSON schema that maps 1:1 with `clap` structs:

1.  **Command/Subcommand Nodes:** Represents the hierarchical selection (e.g., `command: "populi"`, `subcommand: "train"`).
2.  **Argument Nodes:** Positional arguments as an array of structured objects.
3.  **Flag/Option Nodes:** Key-value pairs matching explicit `clap` `long` arguments.

```json
// Example: Valid MENS AST Output
{
  "command": "populi",
  "subcommand": "train",
  "flags": {
    "gpu": true,
    "batch-size": 32
  },
  "arguments": []
}
```

### 1.2 Schema Synchronization via Contracts (SSOT)

To prevent drift between the CLI interface and the schema MENS uses for generation, Vox employs a strict **Contract-Driven Schema Architecture**.
Instead of heavy schema crates (like `schemars`) leaking UI parsing logic into our backend domains, the Single Source of Truth for *all* constraints exists within `contracts/operations/catalog.v1.yaml`.

During the build pipeline (`vox ci operations-sync`), this YAML catalog validates and exports `model-manifest.generated.json`. This exact JSON is injected into the MENS context window during planning steps, ensuring the LLM is always aware of the valid keys and types available, without any dependency bloat in our Rust crates.

### 1.3 CLI to MCP Schema Parity

Some operations expose the exact same capabilities via CLI commands and MCP tool calls. These pairs use independent backing structs (so `vox-cli` avoids `schemars` dependencies) but must maintain exact parameter parity via the contract YAML.

| CLI command         | MCP tool equivalent    | Params struct (vox-mcp)              |
|---------------------|------------------------|--------------------------------------|
| `vox check <file>`    | `vox_validate_file`      | `crate::params::ValidateFileParams`    |
| `vox build <crate>`   | `vox_build_crate`        | `crate::params::OptionalCrateNameParams`|
| `vox run tests`       | `vox_run_tests`          | `crate::params::RunTestsParams`        |

## 2. Validation and Translation Layer

Before arbitrary generated commands are shelled out or executed against internal APIs, they must pass through the **CLI AST Validator**.

### 2.1 The Validator Workflow

1. **Parse:** Deserialize LLM JSON to the internal AST.
2. **Schema Verification:** Validate against the known capability registry of Vox arguments (enforcing non-null types and enum constraints) by flattening the JSON structure back into an array of strictly-typed string tokens.
3. **Delegation:** Translate the valid AST directly into `VoxArgs` invocation without spawning a sub-shell. Specifically, Vox converts the AST map into a synthetic iteration of strings `["vox", "populi", "train", "--gpu", "--batch-size=32"]` and invokes `VoxArgs::try_parse_from(...)`. This prevents injection attacks and strips text manipulation hazards.

### 2.2 AST-Guided Self-Repair

If `try_parse_from` rejects the tokenized payload (e.g., the LLM hallucinates `--force` on a command that doesn't support it, or passes a string to an integer flag), the validator intercepts the `clap::Error`.
Instead of panic, it returns a structured diagnostic:
- **Error Kind:** e.g., `UnknownArgument`
- **Context:** The specific node that failed.
- **Usage Hint:** The `clap` generated help output for that subcommand.

This creates a multi-turn prompt context allowing MENS to quickly self-repair its AST state instead of guessing blindly.

## 3. Human UX vs Agent Intent

The CLI is designed with progressive disclosure for humans (`--help` headings, soft aliases). However, for the MENS agent:
- Generating commands does not rely on short flags (`-v`, `-f`).
- Enforces verbose flag names strictly to ensure unambiguous API intent.
- Follows the [Language Surface Authority](language-surface-ssot.md) and [Terminal Execution Policy](terminal-exec-policy-research-findings-2026.md) regarding boundaries between host shell pipelines and direct structured commands.

## 4. Expanding the CLI Surface

When maintaining or extending the `vox-cli`:
- **Do not introduce implicit text behaviors:** Ensure side effects and modifiers are represented directly in the command struct.
- **Maintain Contract Parity:** Every new command merged into the `clap` parser MUST first be defined in the schema inside `contracts/operations/catalog.v1.yaml`. Our integration tests (`vox-integration-tests`) continuously cross-validate the active `clap` AST against this YAML contract to prevent undocumented feature drift. 
- **Fail Fast:** If manual string manipulation is found inside a CLI action handler (e.g., parsing a raw string flag instead of using `clap`'s typed value parsers), it violates this standard and will break MENS context generation.


