# vox-codegen-ts

TypeScript/TSX code generator for the Vox compiler. Emits React components, fetch wrappers, ADT types, and route definitions.

## Purpose

Transforms the typed HIR into TypeScript source files. The emitter is modularized by concern — each module handles a specific category of output.

## Key Files

| File | Purpose |
|------|---------|
| `emitter.rs` | `generate()` — main entry point, orchestrates all modules |
| `jsx.rs` | React JSX component rendering |
| `component.rs` | `@component` declarations and hook wiring |
| `activity.rs` | Activity and workflow client wrappers |
| `routes.rs` | React Router route definitions |
| `adt.rs` | TypeScript discriminated union types from Vox ADTs |

## Output Mapping

| Vox | Generated TypeScript |
|-----|---------------------|
| `@component fn` | React functional component |
| `@server fn` | Typed `fetch()` wrapper |
| `type A = \| B \| C` | Discriminated union type |
| `routes:` | React Router `<Route>` elements |
| `@deprecated` | `/** @deprecated */` JSDoc |
| `style:` | CSS-in-JS object |

## Usage

```rust
use vox_codegen_ts::generate;

let ts_output = generate(&ast_module);
// ts_output: String — complete TypeScript/TSX file
```
