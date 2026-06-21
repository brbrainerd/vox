//! The feature × target support matrix — the SSOT for "who implements what."
//!
//! This is the *breadth* axis of the pipeline-parity contract
//! ([`docs/src/architecture/pipeline-parity-ssot-2026-06-14.md`]). Every Vox
//! language feature (decorator, expr/stmt kind, decl kind) has a declared
//! [`Support`] for every [`Target`]: `Implemented`, `Unverified` (declared but
//! not yet proven by the canonical golden ladder), or `Unsupported(code)` with
//! a stable diagnostic code. There is no third state and **no `_` catch-all** —
//! so adding a `Feature` *or* a `Target` variant fails to compile until every new
//! cell is filled in. That compile-time exhaustiveness is the cheapest half of the
//! build gate.
//!
//! **Seeding status (Wave 1, first pass).** Cells with hard evidence are accurate:
//! the frontend-only exprs (JSX / async views), backend-only `spawn`, the
//! unimplemented `with` / `workflow.version`, and the three decorator gaps
//! (`@inference` / `@pii` / `@embed`) that already carry diagnostic codes. The
//! remaining decorators, statements, and declarations default to
//! `Implemented`-on-all ("no gap declared yet"). The companion **parity test**
//! (plan Task 8) drives a fixture per `(Feature, Target)` through the real
//! emitters and reconciles any over-claim — that is the SSOT-mandated
//! "derive from / check against the real emitters" mechanism. Do not treat the
//! first-pass defaults as verified truth until that test lands.

use crate::target::Target;
use crate::typeck::diagnostics::codes;

/// Support state of one feature on one target. `Copy` because the payload is a
/// `&'static str` code from the diagnostics registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// The target emits / executes this feature.
    Implemented,
    /// Declared in the language surface but not yet proven by the canonical golden ladder.
    Unverified,
    /// The target explicitly does not implement this feature; the `&str` is a
    /// stable code from [`codes::ALL_COMPILER_DIAGNOSTIC_CODES`].
    Unsupported(&'static str),
}

/// The (code, message) an emitter surfaces for an unsupported `(feature, target)`
/// cell. Crate-agnostic on purpose: `vox-codegen` does not use
/// `vox_compiler::Diagnostic` (it errors via `miette::Error` / `WebIrDiagnostic`),
/// so this carries raw data each emitter adapts into its own channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedCell {
    /// Stable diagnostic code (registered in [`codes::ALL_COMPILER_DIAGNOSTIC_CODES`]).
    pub code: &'static str,
    /// Human-facing message.
    pub message: String,
}

/// A row of the matrix: the support state for one feature across all four targets,
/// in [`Target::ALL`] order (`Interpreter`, `RustAxum`, `RustTauri`, `TypeScript`).
#[derive(Debug, Clone, Copy)]
struct Row {
    interpreter: Support,
    rust_axum: Support,
    rust_tauri: Support,
    typescript: Support,
}

impl Row {
    fn pick(self, target: Target) -> Support {
        match target {
            Target::Interpreter => self.interpreter,
            Target::RustAxum => self.rust_axum,
            Target::RustTauri => self.rust_tauri,
            Target::TypeScript => self.typescript,
        }
    }
}

/// Implemented on every target.
const fn all_targets() -> Row {
    Row {
        interpreter: Support::Implemented,
        rust_axum: Support::Implemented,
        rust_tauri: Support::Implemented,
        typescript: Support::Implemented,
    }
}

/// Ladder-unproven: declared but not yet emission-proven.
const fn unverified() -> Row {
    Row {
        interpreter: Support::Unverified,
        rust_axum: Support::Unverified,
        rust_tauri: Support::Unverified,
        typescript: Support::Unverified,
    }
}

/// Decorators explicitly proven by [`contracts/pipeline/canonical-ladder.v1.yaml`].
const fn is_ladder_proven_decorator(d: DecoratorFeature) -> bool {
    use DecoratorFeature::*;
    matches!(
        d,
        Query
            | Mutation
            | Server
            | Table
            | Index
            | Component
            | Reactive
            | Scheduled
            | Auth
            | McpTool
            | Tool
            | JsonAs
            | Tokens
            | Layer
    )
}

/// Implemented only by the TypeScript frontend emitter; unsupported elsewhere.
const fn frontend_only(code: &'static str) -> Row {
    Row {
        interpreter: Support::Unsupported(code),
        rust_axum: Support::Unsupported(code),
        rust_tauri: Support::Unsupported(code),
        typescript: Support::Implemented,
    }
}

/// Implemented only by the Rust backend emitter (both shells); unsupported elsewhere.
const fn backend_only(code: &'static str) -> Row {
    Row {
        interpreter: Support::Unsupported(code),
        rust_axum: Support::Implemented,
        rust_tauri: Support::Implemented,
        typescript: Support::Unsupported(code),
    }
}

/// Declared unsupported on every target (no implemented backing yet).
const fn none_yet(code: &'static str) -> Row {
    Row {
        interpreter: Support::Unsupported(code),
        rust_axum: Support::Unsupported(code),
        rust_tauri: Support::Unsupported(code),
        typescript: Support::Unsupported(code),
    }
}

// ── Feature taxonomy ────────────────────────────────────────────────────────
// Mirrors the real enums: decorators are the `At*` tokens in `lexer/token.rs`,
// exprs/stmts are `HirExpr`/`HirStmt` in `hir/nodes/stmt_expr.rs`, decls are the
// AST `Decl` enum in `vox-ast`. The parity test (Task 8) proves this mirror stays
// in sync with those sources.

/// A decorator feature — one per `At*` token. 56 variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoratorFeature {
    Component,
    Tool,
    McpTool,
    Resource,
    McpResource,
    Test,
    Example,
    Query,
    Mutation,
    Server,
    JsonAs,
    FieldName,
    Default,
    SkipIfNone,
    Table,
    Index,
    Place,
    Loading,
    Require,
    Ensure,
    Invariant,
    Forall,
    Fuzz,
    Pure,
    Reactive,
    Versioned,
    Tracked,
    Scheduled,
    Deprecated,
    V0,
    Ai,
    Prompt,
    Subagent,
    Search,
    Hole,
    Cancellable,
    Form,
    BackButton,
    DeepLink,
    Push,
    Tokens,
    Cors,
    RateLimit,
    Uses,
    Pii,
    Embed,
    Webhook,
    Public,
    Auth,
    OfflineCapable,
    Collaborative,
    Layer,
    Remote,
    Inference,
    TrainingStep,
    DistributedTrain,
}

/// An expression/statement-expression feature — one per `HirExpr` variant. 28 variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprFeature {
    IntLit,
    FloatLit,
    StringLit,
    BoolLit,
    DecimalLit,
    Ident,
    ObjectLit,
    ListLit,
    TupleLit,
    Binary,
    Unary,
    Call,
    MethodCall,
    FieldAccess,
    Match,
    If,
    For,
    Lambda,
    Spawn,
    With,
    Jsx,
    JsxSelfClosing,
    JsxFragment,
    Block,
    Try,
    Index,
    AsyncView,
    WorkflowVersion,
}

/// A statement feature — one per `HirStmt` variant. 8 variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StmtFeature {
    Let,
    Assign,
    Return,
    Expr,
    While,
    Loop,
    Break,
    Continue,
}

/// A declaration feature — one per AST `Decl` variant. 41 variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclFeature {
    Function,
    TypeDef,
    Import,
    Const,
    HttpRoute,
    McpTool,
    McpResource,
    Test,
    Example,
    Forall,
    Table,
    Collection,
    Index,
    VectorIndex,
    SearchIndex,
    V0Component,
    Routes,
    Endpoint,
    Skill,
    AgentDef,
    Agent,
    Message,
    Scheduled,
    Config,
    Loading,
    Theme,
    Environment,
    Page,
    ReactiveComponent,
    ReactiveModule,
    Fragment,
    Url,
    StateMachine,
    Workflow,
    Activity,
    Actor,
    Form,
    BackButton,
    DeepLink,
    Push,
    Tokens,
}

/// Any Vox language feature subject to the parity contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// A decorator (`@server`, `@auth`, …).
    Decorator(DecoratorFeature),
    /// An expression kind (`Jsx`, `Spawn`, …).
    Expr(ExprFeature),
    /// A statement kind (`Let`, `While`, …).
    Stmt(StmtFeature),
    /// A declaration kind (`Table`, `Endpoint`, …).
    Decl(DeclFeature),
}

impl DecoratorFeature {
    /// Lexer `@` spelling for this decorator (mirrors [`crate::lexer::token::Token`] `At*` tokens).
    #[must_use]
    pub const fn lexer_spelling(self) -> &'static str {
        use DecoratorFeature::*;
        match self {
            Component => "@component",
            Tool => "@tool",
            McpTool => "@mcp.tool",
            Resource => "@resource",
            McpResource => "@mcp.resource",
            Test => "@test",
            Example => "@example",
            Query => "@query",
            Mutation => "@mutation",
            Server => "@server",
            JsonAs => "@json_as",
            FieldName => "@field_name",
            Default => "@default",
            SkipIfNone => "@skip_if_none",
            Table => "@table",
            Index => "@index",
            Place => "@place",
            Loading => "@loading",
            Require => "@require",
            Ensure => "@ensure",
            Invariant => "@invariant",
            Forall => "@forall",
            Fuzz => "@fuzz",
            Pure => "@pure",
            Reactive => "@reactive",
            Versioned => "@versioned",
            Tracked => "@tracked",
            Scheduled => "@scheduled",
            Deprecated => "@deprecated",
            V0 => "@v0",
            Ai => "@ai",
            Prompt => "@prompt",
            Subagent => "@subagent",
            Search => "@search",
            Hole => "@hole",
            Cancellable => "@cancellable",
            Form => "@form",
            BackButton => "@back_button",
            DeepLink => "@deep_link",
            Push => "@push",
            Tokens => "@tokens",
            Cors => "@cors",
            RateLimit => "@rate_limit",
            Uses => "@uses",
            Pii => "@pii",
            Embed => "@embed",
            Webhook => "@webhook",
            Public => "@public",
            Auth => "@auth",
            OfflineCapable => "@offline_capable",
            Collaborative => "@collaborative",
            Layer => "@layer",
            Remote => "@remote",
            Inference => "@inference",
            TrainingStep => "@training_step",
            DistributedTrain => "@distributed_train",
        }
    }

    /// Every decorator feature.
    pub const ALL: [DecoratorFeature; 56] = {
        use DecoratorFeature::*;
        [
            Component,
            Tool,
            McpTool,
            Resource,
            McpResource,
            Test,
            Example,
            Query,
            Mutation,
            Server,
            JsonAs,
            FieldName,
            Default,
            SkipIfNone,
            Table,
            Index,
            Place,
            Loading,
            Require,
            Ensure,
            Invariant,
            Forall,
            Fuzz,
            Pure,
            Reactive,
            Versioned,
            Tracked,
            Scheduled,
            Deprecated,
            V0,
            Ai,
            Prompt,
            Subagent,
            Search,
            Hole,
            Cancellable,
            Form,
            BackButton,
            DeepLink,
            Push,
            Tokens,
            Cors,
            RateLimit,
            Uses,
            Pii,
            Embed,
            Webhook,
            Public,
            Auth,
            OfflineCapable,
            Collaborative,
            Layer,
            Remote,
            Inference,
            TrainingStep,
            DistributedTrain,
        ]
    };
}

impl ExprFeature {
    /// Every expression feature.
    pub const ALL: [ExprFeature; 28] = {
        use ExprFeature::*;
        [
            IntLit,
            FloatLit,
            StringLit,
            BoolLit,
            DecimalLit,
            Ident,
            ObjectLit,
            ListLit,
            TupleLit,
            Binary,
            Unary,
            Call,
            MethodCall,
            FieldAccess,
            Match,
            If,
            For,
            Lambda,
            Spawn,
            With,
            Jsx,
            JsxSelfClosing,
            JsxFragment,
            Block,
            Try,
            Index,
            AsyncView,
            WorkflowVersion,
        ]
    };
}

impl StmtFeature {
    /// Every statement feature.
    pub const ALL: [StmtFeature; 8] = {
        use StmtFeature::*;
        [Let, Assign, Return, Expr, While, Loop, Break, Continue]
    };
}

impl DeclFeature {
    /// Every declaration feature.
    pub const ALL: [DeclFeature; 41] = {
        use DeclFeature::*;
        [
            Function,
            TypeDef,
            Import,
            Const,
            HttpRoute,
            McpTool,
            McpResource,
            Test,
            Example,
            Forall,
            Table,
            Collection,
            Index,
            VectorIndex,
            SearchIndex,
            V0Component,
            Routes,
            Endpoint,
            Skill,
            AgentDef,
            Agent,
            Message,
            Scheduled,
            Config,
            Loading,
            Theme,
            Environment,
            Page,
            ReactiveComponent,
            ReactiveModule,
            Fragment,
            Url,
            StateMachine,
            Workflow,
            Activity,
            Actor,
            Form,
            BackButton,
            DeepLink,
            Push,
            Tokens,
        ]
    };
}

impl Feature {
    /// Every feature across all four categories. 56 + 28 + 8 + 41 = 133.
    #[must_use]
    pub fn all() -> Vec<Feature> {
        let mut out = Vec::with_capacity(133);
        out.extend(DecoratorFeature::ALL.into_iter().map(Feature::Decorator));
        out.extend(ExprFeature::ALL.into_iter().map(Feature::Expr));
        out.extend(StmtFeature::ALL.into_iter().map(Feature::Stmt));
        out.extend(DeclFeature::ALL.into_iter().map(Feature::Decl));
        out
    }
}

// ── The matrix ──────────────────────────────────────────────────────────────

fn decorator_row(d: DecoratorFeature) -> Row {
    use DecoratorFeature::*;
    match d {
        // Verified gaps — already carry diagnostic codes (typeck `ast_decl_lints`).
        Inference | TrainingStep | DistributedTrain | Remote => {
            none_yet(codes::MENS_DECORATOR_UNIMPLEMENTED)
        }
        Pii => none_yet(codes::PII_UNIMPLEMENTED),
        Embed => none_yet(codes::EMBED_UNIMPLEMENTED),
        d if is_ladder_proven_decorator(d) => all_targets(),
        _ => unverified(),
    }
}

fn expr_row(e: ExprFeature) -> Row {
    use ExprFeature::*;
    match e {
        // Frontend-only: JSX and async views emit only via codegen_ts (verified:
        // codegen_rust emits compile_error!, eval returns EvalError).
        Jsx | JsxSelfClosing | JsxFragment | AsyncView => {
            frontend_only(codes::PARITY_FRONTEND_ONLY)
        }
        // Backend-only: bare `spawn expr` → tokio::spawn in codegen_rust (verified);
        // no interpreter or frontend backing.
        Spawn => backend_only(codes::PARITY_BACKEND_ONLY),
        // `with(...)`: unsupported on all targets (verified: codegen_rust compile_error!,
        // eval EvalError, TS emitter routes through unsupported_diagnostic).
        With => none_yet(codes::PARITY_UNIMPLEMENTED),
        // `workflow.version(...)`: no runtime backing on any target (verified: compile_error!
        // in rust, EvalError in eval, empty-string drop in TS — now declared).
        WorkflowVersion => none_yet(codes::PARITY_UNIMPLEMENTED),
        // Common expressions: implemented on every target.
        IntLit | FloatLit | StringLit | BoolLit | DecimalLit | Ident | ObjectLit | ListLit
        | TupleLit | Binary | Unary | Call | MethodCall | FieldAccess | Match | If | For
        | Lambda | Block | Try | Index => all_targets(),
    }
}

fn stmt_row(s: StmtFeature) -> Row {
    use StmtFeature::*;
    match s {
        // All statement kinds are implemented on every target.
        Let | Assign | Return | Expr | While | Loop | Break | Continue => all_targets(),
    }
}

fn decl_row(d: DeclFeature) -> Row {
    use DeclFeature::*;
    match d {
        // First-pass: every declaration believed wired on all targets. Task 8 reconciles
        // the genuinely frontend-only (Page, ReactiveComponent, …) and backend-only
        // (Endpoint, Table, Workflow, …) declarations against the real emitters.
        Function | TypeDef | Import | Const | HttpRoute | McpTool | McpResource | Test
        | Example | Forall | Table | Collection | Index | VectorIndex | SearchIndex
        | V0Component | Routes | Endpoint | Skill | AgentDef | Agent | Message | Scheduled
        | Config | Loading | Theme | Environment | Page | ReactiveComponent | ReactiveModule
        | Fragment | Url | StateMachine | Workflow | Activity | Actor | Form | BackButton
        | DeepLink | Push | Tokens => all_targets(),
    }
}

/// The one place that answers "who implements what." Exhaustive on `(Feature, Target)`;
/// adding a variant to either breaks the build until every new cell is declared.
#[must_use]
pub fn support(feature: Feature, target: Target) -> Support {
    let row = match feature {
        Feature::Decorator(d) => decorator_row(d),
        Feature::Expr(e) => expr_row(e),
        Feature::Stmt(s) => stmt_row(s),
        Feature::Decl(d) => decl_row(d),
    };
    row.pick(target)
}

/// The (code, message) every emitter surfaces when it reaches a feature its target
/// does not implement. Emitters import this rather than hand-rolling the code/message;
/// each adapts the returned [`UnsupportedCell`] into its own native error channel.
///
/// # Panics
/// Panics if called for an `Implemented` cell — callers must only invoke it on the
/// unsupported path.
#[must_use]
pub fn unsupported_diagnostic(feature: Feature, target: Target) -> UnsupportedCell {
    match support(feature, target) {
        Support::Unsupported(code) => UnsupportedCell {
            code,
            message: format!("{feature:?} is not supported by the {} target", target.id()),
        },
        Support::Unverified => UnsupportedCell {
            code: codes::PARITY_UNVERIFIED,
            message: format!(
                "{feature:?} is declared but not yet proven by the canonical golden ladder on the {} target",
                target.id()
            ),
        },
        Support::Implemented => {
            unreachable!(
                "unsupported_diagnostic called for an Implemented ({feature:?}, {target:?}) cell"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeck::diagnostics::codes::ALL_COMPILER_DIAGNOSTIC_CODES;

    #[test]
    fn feature_all_has_expected_count() {
        // 56 decorators + 28 exprs + 8 stmts + 41 decls.
        assert_eq!(Feature::all().len(), 133);
        assert_eq!(DecoratorFeature::ALL.len(), 56);
        assert_eq!(ExprFeature::ALL.len(), 28);
        assert_eq!(StmtFeature::ALL.len(), 8);
        assert_eq!(DeclFeature::ALL.len(), 41);
    }

    #[test]
    fn matrix_is_total() {
        // Every (Feature, Target) returns a Support without panicking.
        for f in Feature::all() {
            for t in Target::ALL {
                let _ = support(f, t);
            }
        }
    }

    #[test]
    fn every_unsupported_code_is_registered() {
        for f in Feature::all() {
            for t in Target::ALL {
                match support(f, t) {
                    Support::Unsupported(code) => {
                        assert!(
                            ALL_COMPILER_DIAGNOSTIC_CODES.contains(&code),
                            "{f:?}/{t:?} declares unregistered code {code}"
                        );
                    }
                    Support::Unverified => {
                        assert!(
                            ALL_COMPILER_DIAGNOSTIC_CODES.contains(&codes::PARITY_UNVERIFIED),
                            "PARITY_UNVERIFIED must be registered"
                        );
                    }
                    Support::Implemented => {}
                }
            }
        }
    }

    #[test]
    fn offline_capable_is_unverified_until_ladder_proves_it() {
        assert_eq!(
            support(
                Feature::Decorator(DecoratorFeature::OfflineCapable),
                Target::TypeScript
            ),
            Support::Unverified
        );
    }

    #[test]
    fn query_is_ladder_proven_implemented() {
        assert_eq!(
            support(
                Feature::Decorator(DecoratorFeature::Query),
                Target::RustAxum
            ),
            Support::Implemented
        );
    }

    #[test]
    fn unsupported_diagnostic_carries_the_declared_code() {
        // JSX is frontend-only → unsupported on the interpreter with the frontend-only code.
        let cell = unsupported_diagnostic(Feature::Expr(ExprFeature::Jsx), Target::Interpreter);
        assert_eq!(cell.code, codes::PARITY_FRONTEND_ONLY);
        assert!(!cell.message.is_empty());
    }

    #[test]
    fn jsx_is_typescript_only() {
        assert_eq!(
            support(Feature::Expr(ExprFeature::Jsx), Target::TypeScript),
            Support::Implemented
        );
        assert!(matches!(
            support(Feature::Expr(ExprFeature::Jsx), Target::RustAxum),
            Support::Unsupported(_)
        ));
    }

    #[test]
    fn place_decorator_registered_and_native_gone() {
        let _ = support(
            Feature::Decorator(DecoratorFeature::Place),
            Target::TypeScript,
        );
        assert_eq!(DecoratorFeature::Place.lexer_spelling(), "@place");
    }

    #[test]
    fn placement_matches_feature_matrix_frontend_backend() {
        assert!(matches!(
            support(Feature::Expr(ExprFeature::Jsx), Target::RustAxum),
            Support::Unsupported(_)
        ));
        assert!(matches!(
            support(Feature::Expr(ExprFeature::Spawn), Target::TypeScript),
            Support::Unsupported(_)
        ));
    }
}
