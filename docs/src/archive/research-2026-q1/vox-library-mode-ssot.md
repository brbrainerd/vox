---
title: "vox-library-mode-ssot.md"
description: "Documentation for vox-library-mode-ssot.md."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Project architecture context."
archived_date: 2026-04-18
---
# Vox Library Mode SSOT

This document defines the behavior and output of the Vox compiler's Library Mode (`vox build --mode library`).

## Objective
To decouple the Vox frontend from the React ecosystem, allowing developers to use Vox-generated types, schemas, and API clients in any framework (Svelte, Vue, Solid, or vanilla TypeScript).

## Output Artifacts

When running in Library Mode, the compiler emits the following files to the output directory:

| Filename | Purpose |
|---|---|
| `types.ts` | TypeScript interfaces for all Vox ADTs and record types. |
| `schemas.ts` | Zod schemas for runtime validation of Vox types. |
| `vox-client.ts` | Framework-agnostic fetch client using standard `fetch` and `zod`. |
| `routes.manifest.json` | JSON manifest of all client-side routes for custom routing integration. |
| `index.ts` | Standard barrel export for the library. |
| `package.json` | Minimal package definition for easier workspace integration. |

## Codegen Invariants

### Zero React Bleed
The generated `vox-client.ts` MUST NOT import any React-specific symbols (e.g., `useQuery`, `useEffect`). It uses standard browser `fetch` and `zod` for parsing.

### Runtime Validation
Every API call in `vox-client.ts` is automatically wrapped in a `.parse()` call from the corresponding Zod schema in `schemas.ts`. This ensures that data entering the library at runtime matches the compile-time types.

### Transparent Error Handling
The client throws a `VoxApiError` which includes the HTTP status, the endpoint path, and the raw response text, allowing consumers to implement custom error UI.

## Consumption Patterns

### Vanilla TypeScript
```typescript
import { create_user } from "./vox_generated/vox-client";

const user = await create_user({ name: "Alice" });
console.log(user.id, user.name);
```

### Svelte / Vue
The generated SDK can be used directly in Svelte `load` functions or Vue `onMounted` hooks without additional configuration.

## Transition Policy
Running `vox build --mode library` on an existing App-mode directory will automatically remove stale React artifacts (`App.tsx`, `VoxTanStackRouter.tsx`, `serverFns.ts`) to prevent developer confusion.

