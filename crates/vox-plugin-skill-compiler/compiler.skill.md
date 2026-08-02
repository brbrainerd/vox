---
name: skill-compiler
description: "Compiles Vox source files and runs cargo check/build for the workspace."
metadata:
  vox-id: vox.compiler
  vox-version: "0.1.0"
  vox-author: vox-team
  vox-category: compiler
  vox-tools:
    - vox_validate_file
    - vox_run_tests
    - vox_check_workspace
  vox-tags:
    - compile
    - build
    - cargo
    - check
  vox-permissions:
    - read_files
    - shell_exec
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
