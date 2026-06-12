---
title: "vox-codegen-ts API (deprecated stub)"
description: "Deprecated API stub: TypeScript/TSX generation is implemented in vox-compiler, not a separate vox-codegen-ts crate."
category: "API Reference — Crates"
status: deprecated
---

# vox-codegen-ts API (deprecated)

The historical `vox-codegen-ts` crate name is retired. **HIR-to-TypeScript/TSX emission** lives in the monolith crate [`vox-compiler`](../../../crates/vox-compiler) under `codegen_ts` (see `crates/vox-codegen-ts/src/`).

## Overview

The codegen pipeline lowers HIR through the Web IR layer before final TypeScript emit.
See [internal-web-ir-implementation-blueprint.md](../architecture/internal-web-ir-implementation-blueprint.md)
and [internal-web-ir-implementation-blueprint.md](../architecture/internal-web-ir-implementation-blueprint.md)
for the full Web IR specification.

For the strategic rationale, see [ADR 012](../adr/012-internal-web-ir-strategy.md) (`adr/012`).

## Output files

| File | Description |
|------|-------------|
| `<Component>.tsx` | React component for each `component` declaration |
| `routes.manifest.ts` | Route manifest for TanStack Start / Vite adapter |
| `vox-client.ts` | Type-safe client hooks for `@endpoint(kind: query/mutation)` |
| `types.ts` | TypeScript union types for `type` declarations |
| `server.ts` | Express/Axum route handlers for `@endpoint(kind: server)` |
