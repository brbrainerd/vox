---
title: "vox-plugin-cloud (archived)"
description: "CloudSync plugin scaffold retired from workspace and plugin catalog until SP7 implementation."
category: "architecture"
status: "archived"
---

# Archived — do not build or catalog

This crate was **removed from the plugin catalog** and **excluded from the Cargo workspace** on 2026-06-10 per the full-value implementation plan (Phase 1 contraction).

All methods returned `"not yet implemented"`. Restore when CloudSync (HF Hub / S3 artifact sync) is implemented for real.

Canonical trait surface remains in `vox-plugin-api::extensions::cloud_sync`.
