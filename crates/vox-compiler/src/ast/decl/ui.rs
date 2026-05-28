use crate::ast::decl::fundecl::{FnDecl, StyleBlock};
use crate::ast::span::Span;
use crate::ast::types::TypeExpr;

/// v0.dev AI-generated component declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct V0ComponentDecl {
    pub v0_id: String,
    /// When set, this stub was generated from `@v0 from "path.png"` (design asset) instead of a v0 chat id string.
    #[serde(default)]
    pub image_path: Option<String>,
    pub name: String,
    pub props: Vec<V0Prop>,
    pub span: Span,
}

/// Prop declaration for a v0 component.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct V0Prop {
    pub name: String,
    pub ty: TypeExpr,
    pub is_optional: bool,
}

/// Client-side routing declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoutesDecl {
    pub entries: Vec<RouteEntry>,
    /// Optional 404 screen component (PascalCase name).
    #[serde(default)]
    pub not_found_component: Option<String>,
    /// Optional global error screen component.
    #[serde(default)]
    pub error_component: Option<String>,
    pub span: Span,
}

/// Deterministic parse metrics for a [`RoutesDecl`] (tooling / gates; OP-0027).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RoutesParseSummary {
    pub entry_count: usize,
    pub paths: Vec<String>,
}

impl RoutesDecl {
    #[must_use]
    /// Paths are listed in source order (stable for snapshot-style tests).
    pub fn parse_summary(&self) -> RoutesParseSummary {
        RoutesParseSummary {
            entry_count: self.entries.len(),
            paths: self.entries.iter().map(|e| e.path.clone()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RouteEntry {
    pub path: String,
    pub component_name: String,
    pub children: Vec<RouteEntry>,
    pub redirect: Option<String>,
    pub is_wildcard: bool,
    /// `@query` / loader function name for this route (emitted into `routes.manifest.ts`).
    #[serde(default)]
    pub loader_name: Option<String>,
    /// Loading / pending component for this route.
    #[serde(default)]
    pub pending_component_name: Option<String>,
    /// Error component for this route (shown on loader failure).
    #[serde(default)]
    pub error_component_name: Option<String>,
    pub span: Span,
}

/// Loading state component — shown during route suspense.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LoadingDecl {
    pub func: FnDecl,
}

/// Theme declaration with light/dark variants.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThemeDecl {
    pub name: String,
    pub light: Vec<(String, String)>,
    pub dark: Vec<(String, String)>,
    pub span: Span,
}

/// Static site generation (SSG) page hint: URL path + handler body in Vox.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageDecl {
    pub path: String,
    pub func: FnDecl,
    pub span: Span,
}

/// Typed parametric fragment declaration (ADR-033).
///
/// `fragment Name(arg: T1, …) { <markup> }` declares a typed, multiply-renderable
/// chunk of markup that can be passed as a prop and rendered with
/// `<RenderFragment of={Name} args={(…)} />`. Parsed in Phase F slice 1; HIR
/// lowering and codegen are gated on Phase 6 (TASK-6.1) typed-primitive
/// stabilization per the ADR's status note.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FragmentDecl {
    /// Fragment name (PascalCase by convention).
    pub name: String,
    /// Typed parameters; same shape as a function or component parameter list.
    pub params: Vec<crate::ast::expr::Param>,
    /// Single markup body — currently parsed as a generic Expr; the codegen slice
    /// will validate that it's a JSX/markup expression once Phase 6 primitives land.
    pub body: crate::ast::expr::Expr,
    /// Source span.
    pub span: Span,
}

/// Project-level design-token declaration block (`@tokens { … }`).
///
/// Groups tokens by category. Lowered to [`crate::hir::nodes::tokens::HirTokensDecl`]
/// at the HIR layer where the contrast typecheck pass validates color pairs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TokensDecl {
    /// Source span of the entire `@tokens { … }` block.
    pub span: Span,
    /// Color token entries: `color <name> light: "<hex>" dark: "<hex>"`.
    pub colors: Vec<AstColorToken>,
    /// Spacing entries: `spacing <name>: "<css-value>"`.
    pub spacing: Vec<AstScalarToken>,
    /// Border-radius entries: `radius <name>: "<css-value>"`.
    pub radius: Vec<AstScalarToken>,
    /// Box-shadow entries: `shadow <name>: "<css-value>"`.
    pub shadows: Vec<AstScalarToken>,
    /// Font-family entries: `font <name> family: "<stack>"`.
    pub fonts: Vec<AstFontToken>,
}

/// A color token with mandatory light/dark pair.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstColorToken {
    pub name: String,
    pub light: String,
    pub dark: String,
    pub span: Span,
}

/// A scalar token (spacing, radius, shadow).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstScalarToken {
    pub name: String,
    pub value: String,
    pub span: Span,
}

/// A font-family token.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstFontToken {
    pub name: String,
    pub family: String,
    pub span: Span,
}

/// `.vox.ui` reactive module declaration (ADR-032).
///
/// A top-level container for reactive members shared across components in the same file.
/// Lowers to a generated React context provider + `use<Name>()` hook in TSX emit; the
/// module's name is derived from the file basename (e.g. `counter.vox.ui` → `Counter`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReactiveModuleDecl {
    /// Module name, derived from the source file's basename. Used to name the generated
    /// React context, provider component, and hook.
    pub name: String,
    /// Reactive members declared at module scope: `state`, `derived`, `effect`,
    /// `on mount`, `on cleanup`. Same shape as inside a [`ReactiveComponentDecl`].
    pub members: Vec<ReactiveMemberDecl>,
    /// Source span.
    pub span: Span,
}

/// Reactive component declaration (Path C reactive model).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReactiveComponentDecl {
    /// Component name.
    pub name: String,
    /// Props/parameters.
    pub params: Vec<crate::ast::expr::Param>,
    /// Body members: state, derived, effect, on_mount, on_cleanup.
    pub members: Vec<ReactiveMemberDecl>,
    /// Optional view expression (JSX).
    pub view: Option<crate::ast::expr::Expr>,
    /// Scoped CSS blocks following the component body (same as classic `@component fn`).
    #[serde(default)]
    pub styles: Vec<StyleBlock>,
    /// Source span.
    pub span: Span,
}

/// Member of a reactive component body.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReactiveMemberDecl {
    /// Reactive state binding.
    State(StateDecl),
    /// Derived (computed) value.
    Derived(DerivedDecl),
    /// Side-effect hook.
    Effect(EffectDecl),
    /// Runs once after first render.
    OnMount(OnMountDecl),
    /// Runs on component teardown.
    OnCleanup(OnCleanupDecl),
    /// Imperative prelude: `let` bindings, hook calls (`use_effect(...)`), assignments, etc.
    Stmt(crate::ast::stmt::Stmt),
}

/// `state name: Type = init_expr`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StateDecl {
    /// Binding name.
    pub name: String,
    /// Optional type annotation.
    pub ty: Option<TypeExpr>,
    /// Initial value expression.
    pub init: crate::ast::expr::Expr,
    /// Source span.
    pub span: Span,
}

/// `derived name: Type = expr`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DerivedDecl {
    /// Binding name.
    pub name: String,
    /// Optional type annotation.
    pub ty: Option<TypeExpr>,
    /// Computation expression.
    pub expr: crate::ast::expr::Expr,
    /// Source span.
    pub span: Span,
}

/// `effect: { body }` or `effect depends_on (a, b): { body }`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EffectDecl {
    /// Effect body expression.
    pub body: crate::ast::expr::Expr,
    /// Explicit dependency list from `effect depends_on (a, b):` clause.
    /// When `Some`, the lint `lint.effect.unresolvable_deps` is suppressed.
    pub explicit_deps: Option<Vec<String>>,
    /// Source span.
    pub span: Span,
}

/// `mount: { body }` — runs once after first render.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OnMountDecl {
    /// Mount body expression.
    pub body: crate::ast::expr::Expr,
    /// Source span.
    pub span: Span,
}

/// `cleanup: { body }` — runs on component teardown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OnCleanupDecl {
    /// Cleanup body expression.
    pub body: crate::ast::expr::Expr,
    /// Source span.
    pub span: Span,
}
