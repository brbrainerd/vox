//! Core Decl surface types: imports, HTTP method, [Decl], [Module] (OP-0207).

use crate::span::Span;

use super::*;

/// HTTP method for route declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HttpMethod {
    /// HTTP `GET`.
    Get,
    /// HTTP `POST`.
    Post,
    /// HTTP `PUT`.
    Put,
    /// HTTP `DELETE`.
    Delete,
}

/// An import path segment: `react.use_state`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportPath {
    /// Import source kind and metadata.
    pub kind: ImportPathKind,
    /// Optional local alias (`import x as y`).
    pub alias: Option<String>,
    /// Source span of this path.
    pub span: Span,
}

/// Import source variants parsed from one `import` entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ImportPathKind {
    /// Dot-separated symbol path (e.g. `react.use_state`).
    SymbolPath { segments: Vec<String> },
    /// Consume an external React/TS component or hook from a module — Phase 5
    /// interop. Supports default, named, and namespace import shapes:
    ///   `import react MyButton from "../ui/MyButton.tsx"`        (default)
    ///   `import react { Dialog, Trigger as T } from "@radix-ui/react-dialog"` (named)
    ///   `import react * as Dialog from "@radix-ui/react-dialog"` (namespace)
    ReactComponent {
        /// ES module specifier string literal (relative or bare package path).
        module_specifier: String,
        /// What is bound from the module.
        binding: ReactBinding,
    },
    /// Rust crate import (`import rust:serde_json`).
    RustCrate(RustCrateImport),
    /// Intra-project Vox file import (`import "./helpers/walk_docs.vox"`).
    /// Path is resolved relative to the importing file's directory. Only `pub`
    /// declarations from the target file are made available; bare names merge
    /// into the importing file's scope (with an optional `as alias` namespace).
    /// See `docs/src/architecture/intra-project-imports-rfc-2026-05-23.md`.
    LocalFile {
        /// Source path string exactly as written (e.g. `./helpers/walk_docs.vox`).
        path: String,
    },
}

/// What an `import react … from "<spec>"` binds from the module.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReactBinding {
    /// `import react X from "<spec>"` — default export bound to `local_name`.
    Default {
        /// Local binding name (typically PascalCase).
        local_name: String,
    },
    /// `import react { A, B as C } from "<spec>"` — named exports.
    Named(Vec<ReactNamedImport>),
    /// `import react * as Ns from "<spec>"` — namespace import; members reached
    /// as `Ns.Member` (usable as a dotted JSX tag, e.g. `<Ns.Root/>`).
    Namespace {
        /// Local namespace binding name.
        local_name: String,
    },
}

/// One name in a React named import (`{ imported as local }`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReactNamedImport {
    /// Name exported by the module.
    pub imported: String,
    /// Local binding name (== `imported` unless an `as` alias is given).
    pub local: String,
}

/// Rust crate import metadata.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RustCrateImport {
    /// Dependency key / crate name.
    pub crate_name: String,
    /// Optional semantic version requirement.
    pub version: Option<String>,
    /// Optional local path source.
    pub path: Option<String>,
    /// Optional git source URL.
    pub git: Option<String>,
    /// Optional git revision / branch hint.
    pub rev: Option<String>,
}

/// Import declaration: `import react.use_state, network.HTTP`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportDecl {
    /// Imported paths listed in a single `import` declaration.
    pub paths: Vec<ImportPath>,
    /// Span covering the full `import` syntax.
    pub span: Span,
}

/// One item in a `vox_compiler::Module`: any construct that can appear at column 0 (after indentation) in a file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Decl {
    /// Top-level or nested function.
    Function(FnDecl),
    /// Algebraic type, struct, or type alias.
    TypeDef(TypeDefDecl),
    /// ES-module style import list.
    Import(ImportDecl),
    /// Immutable constant (`const` / `@const`).
    Const(ConstDecl),
    /// HTTP handler bound to a method and path.
    HttpRoute(HttpRouteDecl),
    /// MCP tool exposed to clients.
    McpTool(McpToolDecl),
    /// MCP resource URI handler.
    McpResource(McpResourceDecl),
    /// Unit test entrypoint.
    Test(TestDecl),
    /// Authored reference example (corpus / docs surface).
    Example(ExampleDecl),
    /// Property-based test declaration.
    Forall(ForallDecl),
    /// Codex table schema.
    Table(TableDecl),
    /// Document collection schema.
    Collection(CollectionDecl),
    /// B-tree style index on columns.
    Index(IndexDecl),
    /// Vector / embedding index.
    VectorIndex(VectorIndexDecl),
    /// Full-text search index.
    SearchIndex(SearchIndexDecl),
    /// v0.dev generated component stub.
    V0Component(V0ComponentDecl),
    /// Client-side route table.
    Routes(RoutesDecl),
    /// Unified endpoint function (`@endpoint`).
    Endpoint(EndpointDecl),

    /// Packaged LLM / tool skill.
    Skill(SkillDecl),
    /// Agent definition (capabilities + handlers).
    AgentDef(AgentDefDecl),
    /// Native agent runtime declaration.
    Agent(AgentDecl),
    /// Inter-agent message shape.
    Message(MessageDecl),
    /// Cron / interval scheduled job.
    Scheduled(ScheduledDecl),
    /// Typed configuration block.
    Config(ConfigDecl),
    /// Route loading / suspense UI.
    Loading(LoadingDecl),
    /// Design-token theme (light/dark).
    Theme(ThemeDecl),
    /// Container / deployment environment spec.
    Environment(EnvironmentDecl),
    /// Static page for SSG.
    Page(PageDecl),
    /// Reactive component declaration (Path C).
    ReactiveComponent(ReactiveComponentDecl),
    /// `.vox.ui` reactive module — a top-level container for reactive members
    /// (`state` / `derived` / `effect` / `on mount` / `on cleanup`) shared across
    /// components. Per ADR-032, only legal in files classified as
    /// `vox_compiler::module::FileKind::ReactiveModule`. Lowers to a generated React
    /// context + provider + `use<Name>()` hook.
    ReactiveModule(ReactiveModuleDecl),
    /// Typed parametric fragment (ADR-033). `fragment Name(arg: T) { <markup> }` —
    /// passable as a prop, rendered with `<RenderFragment of={Name} args={(…)} />`.
    /// Codegen gated on Phase 6 typed-primitive stabilization.
    Fragment(FragmentDecl),
    /// Typed URL path declaration (`url Name { … }`).
    Url(UrlDecl),
    /// First-class state machine with exhaustiveness enforcement.
    StateMachine(StateMachineDecl),
    /// Durable workflow declaration (TASK-2.6 Path A).
    Workflow(WorkflowDecl),
    /// Durable activity declaration (TASK-2.6 Path A).
    Activity(ActivityDecl),
    /// Actor-model handler declaration (TASK-2.6 Path A).
    Actor(ActorDecl),
    /// Form declaration — generates a React form component with validation.
    Form(FormDecl),
    /// Mobile back-button handler (`@back_button`).
    BackButton(mobile::BackButtonDecl),
    /// Mobile deep-link / universal-link handler (`@deep_link`).
    DeepLink(mobile::DeepLinkDecl),
    /// Mobile push-notification wiring (`@push`).
    Push(mobile::PushDecl),
    /// Project-level design-token block (`@tokens { … }`).
    Tokens(TokensDecl),
}
impl Decl {
    /// Primary source span for this declaration (used for diagnostics).
    pub fn span(&self) -> Span {
        match self {
            Decl::Function(f) => f.span,
            Decl::TypeDef(t) => t.span,
            Decl::Import(i) => i.span,
            Decl::HttpRoute(h) => h.span,
            Decl::McpTool(m) => m.func.span,
            Decl::Test(t) => t.func.span,
            Decl::Example(e) => e.func.span,
            Decl::Forall(f) => f.func.span,
            Decl::Table(t) => t.span,
            Decl::Collection(c) => c.span,
            Decl::Index(i) => i.span,
            Decl::VectorIndex(v) => v.span,
            Decl::SearchIndex(s) => s.span,
            Decl::V0Component(v) => v.span,
            Decl::Routes(r) => r.span,
            Decl::Endpoint(e) => e.func.span,

            Decl::Skill(s) => s.func.span,
            Decl::AgentDef(ad) => ad.func.span,
            Decl::Agent(a) => a.span,
            Decl::Message(m) => m.span,
            Decl::Scheduled(s) => s.func.span,
            Decl::Const(c) => c.span,
            Decl::Config(c) => c.span,
            Decl::Loading(l) => l.func.span,
            Decl::Theme(t) => t.span,
            Decl::McpResource(m) => m.func.span,
            Decl::Environment(e) => e.span,
            Decl::Page(p) => p.span,
            Decl::ReactiveComponent(r) => r.span,
            Decl::ReactiveModule(r) => r.span,
            Decl::Fragment(f) => f.span,
            Decl::Url(u) => u.span,
            Decl::StateMachine(s) => s.span,
            Decl::Workflow(w) => w.span,
            Decl::Activity(a) => a.span,
            Decl::Actor(a) => a.span,
            Decl::Form(f) => f.span,
            Decl::BackButton(b) => b.span,
            Decl::DeepLink(d) => d.span,
            Decl::Push(p) => p.span,
            Decl::Tokens(t) => t.span,
        }
    }
}
/// Parsed contents of one compilation unit (usually one `.vox` file).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Module {
    /// Declarations in the order they appeared in source.
    pub declarations: Vec<Decl>,
    /// Span covering the whole file (or recovered module) for file-level diagnostics.
    pub span: Span,
}

impl Module {
    /// Returns `true` if the module defines a top-level `fn main` (CLI script entrypoint).
    #[must_use]
    pub fn has_entrypoint(&self) -> bool {
        self.declarations
            .iter()
            .any(|d| matches!(d, Decl::Function(f) if f.name == "main"))
    }
}
