---
id = "vox.git"
name = "Vox Git"
version = "0.1.0"
author = "vox-team"
description = "Git workflow assistance: status, diff, commit messaging, branch management, and file ownership."
category = "git"
tools = ["vox_my_files", "vox_claim_file", "vox_transfer_file", "vox_check_file_owner"]
tags = ["git", "version-control", "branch", "diff", "commit"]
permissions = ["read_files", "write_files", "shell_exec"]
---

# Vox Git Skill

Provides git workflow assistance within the Vox multi-agent system, including file ownership tracking.

## Tools

- `vox_my_files` — list all files owned by the current agent
- `vox_claim_file` — claim ownership of a file for exclusive editing
- `vox_transfer_file` — transfer ownership to another agent
- `vox_check_file_owner` — check which agent owns a file

## Workflow

1. Before editing a file, call `vox_claim_file` to prevent conflicts.
2. After completing edits, either keep ownership or call `vox_transfer_file` to hand off.
3. Use `vox_my_files` to audit what you currently own.
4. Always check `vox_check_file_owner` before modifying a file another agent may be using.
