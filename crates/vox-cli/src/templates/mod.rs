//! Embedded templates for scaffolding a complete web application.
//! These are baked into the compiler binary so no external files are needed.
//!
//! ## TanStack npm versions
//! [`TANSTACK_REACT_ROUTER_RANGE`] is shared by the SPA and TanStack Start scaffolds. For the
//! **file-route** Start path (no `routes:`), run **`pnpm run routes:gen`** after changing
//! `src/routes/**` to refresh `routeTree.gen.ts` via `tsr` from **`@tanstack/router-cli`**.

mod spa;
mod tanstack;

pub use spa::*;
pub use tanstack::*;
