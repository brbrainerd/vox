//! TypeScript / React codegen for Vox web modules (components, routes, activities, etc.).
//!
//! Submodules map HIR/AST constructs to TS/JSX; this crate re-exports [`emitter::generate`].
#![allow(clippy::collapsible_if)]

/// Algebraic data types → TypeScript unions and helpers.
pub mod adt;
/// Single source of truth for Vox method/function/namespace → TypeScript lowering.
pub mod builtin_registry;
/// `@component` and related React component codegen.
pub mod component;
/// Main HIR → TypeScript emitter ([`generate`]).
pub mod emitter;
/// `@form` declaration → React form components in `forms.tsx` (Task C3).
pub mod form_emit;
/// `fragment` declaration → typed React function components in `fragments.tsx`
/// (Phase F of the Svelte-mineable features plan; per ADR-033).
pub mod fragment_emit;
/// Shared HIR → TS emission (reactive, routes, activities).
pub mod hir_emit;
/// JSX lowering and attribute handling.
pub mod jsx;
/// `package.json` skeleton for Library / client-target SDK folders.
pub mod library_package_emit;
/// Mobile Capacitor primitive emit (`@back_button`, `@deep_link`, `@push`) → `mobile.ts` (Tasks D2-D4).
pub mod mobile_emit;
/// OpenAPI 3.1 specification emit (driven by Contract IR; per Phase 2 of the
/// external frontend interop plan).
pub mod openapi_emit;
/// Reactive components codegen (Path C).
pub mod reactive;
/// `.vox.ui` reactive module → React context + provider + hook (Phase D of the
/// Svelte-mineable features plan; per ADR-032).
pub mod reactive_module_emit;
/// `routes.manifest.ts` (framework-agnostic `VoxRoute[]`).
pub mod route_manifest;
/// Segment-aware route-pattern parser and overlap detection (Phase C of the
/// Svelte-mineable features plan; not yet wired into [`routes`]).
pub mod route_pattern;

/// One-time SPA / shadcn / Tailwind scaffold (user-owned files).
pub mod scaffold;
/// `@table` / VoxDB `schema.ts` generator ([`generate_voxdb_schema`]).
pub mod schema;
/// `state_machine` TypeScript discriminated union + reducer emit.
pub mod state_machine_emit;
/// TanStack Query helper emission (`vox-tanstack-query.tsx`).
pub mod tanstack_query_emit;
/// Design token CSS + TypeScript emit from vox.tokens.json.
pub mod tokens_emit;
/// `url` block TypeScript discriminated union + builder emit.
pub mod url_emit;
/// `vox-client.ts` typed `fetch` SDK.
pub mod vox_client;
/// Zod schema emission.
pub mod zod_emit;

pub use emitter::{CodegenOptions, generate, generate_with_options};
pub use schema::{generate_voxdb_schema, generate_voxdb_schema_from_hir};
