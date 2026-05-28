//! Top-level items in a `.vox` file: functions, data definitions, routes, and platform-specific UI.
//!
//! `Decl` is a flat enum: there is no nested module syntax in the AST. Import declarations are
//! included here so the parser can emit a single `Module` per file. Attributes and decorators
//! that the lexer/parser attach often land on fields inside each variant; mutators on `Decl`
//! exist for codegen and CLI tooling that patch metadata after parse.

/// `@config` blocks and typed configuration declarations.
pub mod config;
/// Relational table, collection, and index declarations.
pub mod db;
/// Effect annotations for the `uses` clause.
pub mod effect;
/// `@embed(model:, dimensions:, source_field:)` decorator AST (GA-24).
pub mod embed_decorator;
/// `@form` declaration — generates a React form component with validation.
pub mod form;
/// Functions, components, server handlers, MCP, hooks, and tests.
pub mod fundecl;
/// HTTP ergonomics decorators: `@cors`, `@rate_limit`, `@pii` (GA-06/GA-23).
pub mod http_decorators;
/// `@layer(tier:)` decorator AST (GA-26).
pub mod layer_decorator;
/// Actors, agents, workflows, activities, and HTTP routes.
pub mod logic;
/// Mobile Capacitor primitive declarations (`@back_button`, `@deep_link`, `@push`).
pub mod mobile;
/// State machine declarations (`state_machine Name { … }`).
pub mod state_machine;
/// ADTs, traits, impls, and type aliases.
pub mod typedef;
/// Client routing, layouts, themes, and SSG page metadata.
pub mod ui;
/// Typed URL path declarations (`url Name { … }`).
pub mod url;
/// `@webhook(provider:, secret:, replay_window_secs:)` decorator AST (GA-16).
pub mod webhook;

pub use config::*;
pub use db::*;
pub use effect::*;
pub use form::{FieldConstraint, FormDecl, FormField};
pub use fundecl::*;
pub use logic::*;
pub use mobile::{BackButtonDecl, DeepLinkDecl, PushDecl};
pub use state_machine::*;
pub use typedef::*;
pub use ui::*;
pub use url::*;

mod callable;
mod reactive;
mod types;

pub use types::{
    Decl, HttpMethod, ImportDecl, ImportPath, ImportPathKind, Module, RustCrateImport,
};
pub use ui::{AstColorToken, AstFontToken, AstScalarToken, TokensDecl};
