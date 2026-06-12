//! TypeScript / React codegen for Vox web modules (components, routes, activities, etc.).
//!
//! Submodules map HIR/AST constructs to TS/JSX; this crate re-exports [`emitter::generate`].
#![allow(clippy::collapsible_if)]

/// Parent crate modules when built standalone (`vox-codegen-ts`) vs embedded (`vox-codegen` via `#[path]`).
#[cfg(feature = "standalone")]
mod parent {
    pub use vox_codegen::{projection_bundle, web_ir, web_migration_env};
}
#[cfg(not(feature = "standalone"))]
mod parent {
    pub use crate::{projection_bundle, web_ir, web_migration_env};
}

pub(crate) use parent::{projection_bundle, web_ir, web_migration_env};

/// Algebraic data types → TypeScript unions and helpers.
pub mod adt;
/// Single source of truth for Vox method/function/namespace → TypeScript lowering.
pub mod builtin_registry;
/// `@component` and related React component codegen.
pub mod component;
/// Main HIR → TypeScript emitter ([`generate`]).
pub mod emitter;
/// SSOT for external React/RN component libraries (CSS imports, providers, peers).
pub mod external_libs;
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
/// Mobile primitive emit (`@back_button`, `@deep_link`, `@push`) → `mobile.ts` via `@vox/runtime` adapter (Tasks D2-D4).
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
/// Svelte-mineable features plan; not yet wired into `routes`).
pub mod route_pattern;
/// React Native + Expo lowering for `BuildTarget::Mobile`. Consumes the same `HirModule`
/// as the web emit; produces RN-flavored TSX + Expo project scaffolding.
/// Screen-root horizontal inset opt-out (`bleed`) shared by web + RN emitters.
pub mod screen_inset;

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
/// `vox-app.tsx` web app bootstrap (dependency-free router / flat mount).
pub mod web_entry;
/// Zod schema emission.
pub mod zod_emit;

pub use emitter::{CodegenOptions, generate, generate_with_options};
pub use schema::{generate_voxdb_schema, generate_voxdb_schema_from_hir};

/// The set of component names that are SCREEN ROOTS: the top-level view of any
/// component referenced by a `routes { }` entry (recursively, including nested
/// children and the not-found component). When a module declares no routes, the
/// single flat-app component is the screen root.
///
/// Screen roots receive default horizontal edge padding (so content doesn't
/// kiss the device edges) unless their root view opts out with `bleed`. Nested
/// components (e.g. a `NavBar` rendered inside a screen) are NOT screen roots,
/// so they never get — and never double-up — the screen inset. Both the web
/// and RN emitters key off this same set so the guarantee is identical.
pub fn screen_root_component_names(
    hir: &vox_compiler::hir::HirModule,
) -> std::collections::HashSet<String> {
    use vox_compiler::ast::decl::ui::RouteEntry;
    fn walk(entry: &RouteEntry, out: &mut std::collections::HashSet<String>) {
        out.insert(entry.component_name.clone());
        for child in &entry.children {
            walk(child, out);
        }
    }
    let mut names = std::collections::HashSet::new();
    let mut had_route = false;
    for decl in &hir.client_routes {
        for entry in &decl.entries {
            had_route = true;
            walk(entry, &mut names);
        }
        if let Some(nf) = &decl.not_found_component {
            names.insert(nf.clone());
        }
    }
    // Flat (route-less) app: the first declared component is the screen.
    if !had_route {
        if let Some(first) = hir.components.first() {
            names.insert(first.name.clone());
        }
    }
    names
}
