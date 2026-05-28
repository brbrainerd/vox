---
title: "AI-first fixtures: typed telemetry + typecheck guards"
date: 2026-05-11
category: release-note
---

## Summary

- **`TelemetryEvent::AiFixture`** records model intent, prompt dispatch, search dispatch, subagent routing, and hole observations through the existing research-metrics sink.
- **Rust codegen** exercises real memory lookup, docs search execution, web cascade, and `DispatchRouter::route_with_telemetry` + `MessageBus` hooks for `@subagent`.
- **Compiler typecheck** now enforces the catalog diagnostic IDs for task categories, `@prompt` stages, subagent chain depth, search corpus/policy, and distributed policy warnings.
- **TypeScript emit** surfaces `vox/codegen/missing-ts-ai-lowering` as a structured diagnostic; set **`VOX_TS_STRICT_AI=1`** to fail codegen when AI fixtures are present.
- **`@subagent(policy = distributed)`** generates `cfg(feature = "populi-transport")` bodies and adds a matching **`[features]`** section to generated `Cargo.toml` when that policy appears (full mesh relay remains incremental).

See `contracts/agentos/ai-first-fixtures.v1.yaml` and `docs/src/architecture/research-index.md` for SSOT links.
