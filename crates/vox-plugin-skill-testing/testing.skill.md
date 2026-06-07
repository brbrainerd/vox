---
name = "skill-testing"
description = "Runs tests, displays coverage summaries, and validates test output for Vox crates."

[metadata]
"vox-id" = "vox.testing"
"vox-version" = "0.1.0"
"vox-author" = "vox-team"
"vox-category" = "testing"
"vox-tools" = ["vox_run_tests", "vox_test_all"]
"vox-tags" = ["test", "coverage", "ci", "validation"]
"vox-permissions" = ["read_files", "shell_exec"]
---

# Vox Testing Skill

Provides test execution and coverage tooling for the Vox workspace.

## Tools

- `vox_run_tests` — run `cargo test` for a specific crate pattern
- `vox_test_all` — run the full test suite across all crates

## Usage

Run `vox_test_all` before merging any branch.
Use `vox_run_tests` with a crate name to quickly validate a single component.
Always run tests after modifying schema migrations or public APIs.
