---
title: "Standard library surfaces"
description: "How script-mode std.* surfaces relate to host shells and cross-platform ergonomics."
category: "Language Reference"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# Std Surfaces

Vox script-mode builtins under `std.fs`, `std.path`, `std.process`, and related namespaces are defined in [Automation primitives](../archive/research-2026-q1/vox-automation-primitives.md). They lower to Rust `std` APIs and stay host-neutral at the language level.

## Lessons from PowerShell-shaped ergonomics mapped to std

PowerShell-shaped habits—explicit path normalization, resolving tools on `PATH`, and treating paths as typed data—map cleanly onto `std.path.*`, `std.fs.*`, and `std.process.which`. The automation primitives page ties those habits to the concrete Vox surface; this section exists as a stable anchor for cross-links from architecture docs.
