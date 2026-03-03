---
id = "vox.compiler"
name = "Vox Compiler"
version = "0.1.0"
author = "vox-team"
description = "Compiles Vox source files and runs cargo check/build for the workspace."
category = "compiler"
tools = ["vox_validate_file", "vox_run_tests", "vox_check_workspace"]
tags = ["compile", "build", "cargo", "check"]
permissions = ["read_files", "shell_exec"]
---

# Vox Compiler Skill

Provides compilation and build verification tools for the Vox workspace.

## Tools

- `vox_validate_file` — parse-check a single Rust or Vox source file
- `vox_run_tests` — run `cargo test` for a specific crate
- `vox_check_workspace` — run `cargo check` across the entire workspace

## Usage

Use `vox_check_workspace` before submitting any PR to catch compilation errors early.
Use `vox_validate_file` for quick syntax checks during editing.
Run `vox_run_tests` to verify a specific crate after changes.
