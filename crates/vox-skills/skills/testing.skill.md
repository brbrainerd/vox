---
id = "vox.testing"
name = "Vox Testing"
version = "0.1.0"
author = "vox-team"
description = "Runs tests, displays coverage summaries, and validates test output for Vox crates."
category = "testing"
tools = ["vox_run_tests", "vox_test_all"]
tags = ["test", "coverage", "ci", "validation"]
permissions = ["read_files", "shell_exec"]
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
