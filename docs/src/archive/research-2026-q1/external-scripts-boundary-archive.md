---
title: "Explicitly out of scope for Rust migration"
description: "Official documentation for Explicitly out of scope for Rust migration for the Vox language. Detailed technical reference, architecture gu"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Explicitly out of scope for Rust migration

- **Third-party GitHub Actions** (checkout, cache, toolchain installers) — remain YAML-native.
- **GPU / CUDA host setup** on self-hosted runners — may use shell bootstrap outside `vox ci`.
- **Hugging Face / cloud publish** flows in ML workflows — optional `uv`/`curl` steps where no stable Rust API exists yet.

Record new long-lived shell **guard** logic in [`docs/agents/script-registry.json`](../../agents/script-registry.json) and prefer a `vox ci` subcommand if the check must be reproducible on developer laptops.


