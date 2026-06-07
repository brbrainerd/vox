---
name = "skill-testing-validate"
description = "Executes the 5-stage delivery gate pipeline to autonomously validate and heal Vox code."

[metadata]
"vox-id" = "vox.testing.validate"
"vox-version" = "0.1.0"
"vox-author" = "vox-team"
"vox-category" = "testing"
"vox-tools" = ["vox_validate_file", "vox_validate_source"]
"vox-tags" = ["test", "validation", "self-healing", "ars"]
"vox-permissions" = ["read_files", "write_files", "shell_exec", "ai_invoke"]
---

# Vox Testing Delivery Gate

This skill implements the 5-stage AI-driven delivery gate for the Vox language. When executing validation for a `.vox` file, use the following rigorous multi-stage pipeline to ensure zero-defect integration.

## The 5-Stage Gate

**Stage 1: Compilation & HIR Analysis**
- Execute `vox check <file>` to verify lexing, parsing, and HIG lowering.
- Halt if syntax errors or type errors are unrecoverable without context.

**Stage 2: Contract Analysis**
- Execute `vox test <file>`. This evaluates `@require` and `@ensure` invariants natively during the test pass.
- Focus specifically on preconditions and postconditions being satisfied.

**Stage 3: Property-Based Testing**
- Analyze the output of `vox test <file> --forall-iterations=5000` to execute fuzz tests (`ForallDecl` and `@fuzz`).
- Determine if edge cases (NaN, empty strings, boundary integers) trigger failures.

**Stage 4: Execution & Coverage**
- Review the run output. If branch coverage is requested, run `vox test <file> --coverage`.
- Ensure all business logic paths have been traversed.

**Stage 5: Autonomous Healing Loop (Max 5 Iterations)**
- If any of the stages 1-4 fail, DO NOT immediately surface the error to the user.
- Capture the diagnostic trace and the failing AST/Code.
- Apply a self-healing patch internally and re-run stages 1-4.
- If the system fails to heal the code after **5 consecutive attempts**, abort the gate and ask the user for assistance with a detailed failure report.

## Tools

This skill exposes two validation tools to drive the gate:

- **`vox_validate_file`** — validate a single `.vox` file (lex/parse/lower/contract checks); use it as the per-file verification step in stages 1–4.
- **`vox_validate_source`** — validate an in-memory source string before it is written to disk; use it inside the Stage 5 healing loop to check a candidate patch without touching the filesystem.

Always run `vox_validate_file` as part of your initial verification loop before confirming functionality.
