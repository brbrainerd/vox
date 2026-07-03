---
title: "Workflow concurrency exceptions"
description: "Registered exceptions for workflows that intentionally omit cancel-in-progress: true (cancelling mid-run would be wrong)."
category: "CI & Quality"
last_updated: "2026-07-02"
training_eligible: true

schema_type: "TechArticle"
---

# Workflow concurrency exceptions

`vox ci workflow-concurrency-guard` requires every workflow triggered by `push`
or `pull_request` to declare a top-level `concurrency:` mapping containing
`cancel-in-progress: true`, so superseded runs die at the source instead of
flooding the fleet. Omitting `concurrency:` entirely, using a bare group
string, or declaring a group without `cancel-in-progress: true` all count as
violations — a non-cancelling group serializes runs but provides no flood
protection. A workflow may be listed here (backticked filename + reason) when
cancel-in-progress would be incorrect. This is also the
conceptual "never cancel" set behind the queue clearer's tag-push exemption
(`vox ci queue`, spec §2).

- `release-binaries.yml` — tag-push only; a release build must never be cancelled by a later tag.
- `release-gui.yml` — tag-push only; same as above.
- `release-installers.yml` — tag-push only; same as above.
- `scorecard.yml` — pushes to `main` only; supply-chain scorecard runs should complete, and main runs are exempt from queue clearing anyway.
