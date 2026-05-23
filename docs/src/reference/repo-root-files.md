---
title: "repo-root-files"
category: "Language Reference"
status: "current"
training_eligible: false
---
# Repository Root Files

This document provides the canonical list of configuration files permitted at the repository root and their purposes. As part of our architectural governance, unmanaged or generated files are forbidden from existing at the root level.

## Canonical Configuration Files

- `vox-agent.json`: Manual configuration for AI agents (instructions, allowed tools, LSP configuration). This is a checked-in, tracked file that synchronizes agent behavior across the workspace.
- `vox-schema.json`: Architectural single source of truth (SSOT) that dictates allowable paths, crate responsibilities, and structural thresholds like god-object limits.
- `vox.tokens.json`: UI design token map parsed during build for CSS deterministic output.

## Governance

All of these files MUST be tracked in version control (git). They should NEVER be added to `.gitignore`, `.voxignore`, `.aiignore`, or `.cursorignore`. 
The `vox ci data-storage-guard` enforces parity to ensure that no ignored file is mistakenly tracked by git, eliminating "ignored-but-tracked" contradictions.

