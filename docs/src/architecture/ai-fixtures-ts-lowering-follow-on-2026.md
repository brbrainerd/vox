---
title: "AI Fixtures TS Lowering Follow-on (2026)"
description: "Follow-on plan for implementing TypeScript target lowering for AI fixture variants."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Plans for TypeScript lowering of AI fixtures."
---

# AI Fixtures TS Lowering Follow-on (2026)

This follow-on tracks TypeScript codegen parity for AI fixture variants currently surfaced as:

- `vox/codegen/missing-ts-ai-lowering`

Scope (future):

1. Map `HirAiFixture::{Prompt,Subagent,Search,Hole,IntentRouted}` to TS runtime shims.
2. Define TS-safe ACI envelope helpers and fixture telemetry sinks.
3. Add parity tests against Rust fixture codegen snapshots.
