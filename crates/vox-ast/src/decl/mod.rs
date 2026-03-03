pub mod db;
pub mod fundecl;
pub mod logic;
pub mod typedef;
pub mod ui;
pub mod config;

use crate::span::Span;

pub use db::*;
pub use fundecl::*;
pub use logic::*;
pub use typedef::*;
pub use ui::*;
pub use config::*;

/// HTTP method for route declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

/// An import path segment: `react.use_state`
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPath {
    pub segments: Vec<String>,
    pub span: Span,
}

/// Import declaration: `import react.use_state, network.HTTP`
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub paths: Vec<ImportPath>,
    pub span: Span,
}

/// Python library import: `@py.import torch` or `@py.import torch as tc`
///
/// Causes the Rust codegen to emit a `VoxPyRuntime` lazy-static bridge for
/// the named Python module. The alias is used as the binding name in Vox code.
#[derive(Debug, Clone, PartialEq)]
pub struct PyImportDecl {
    /// The Python module to import (e.g. `"torch"`, `"torch.nn"`).
    pub module: String,
    /// The binding name in Vox scope. Defaults to the module's last segment.
    pub alias: String,
    pub span: Span,
}

/// All top-level declaration types in Vox.
#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Function(FnDecl),
    Component(ComponentDecl),
    TypeDef(TypeDefDecl),
    Import(ImportDecl),
    /// Native Python library import (`@py.import module [as alias]`).
    PyImport(PyImportDecl),
    Actor(ActorDecl),
    Const(ConstDecl),
    Workflow(WorkflowDecl),
    Activity(ActivityDecl),
    HttpRoute(HttpRouteDecl),
    McpTool(McpToolDecl),
    McpResource(McpResourceDecl),
    Test(TestDecl),
    ServerFn(ServerFnDecl),
    Table(TableDecl),
    Collection(CollectionDecl),
    Index(IndexDecl),
    VectorIndex(VectorIndexDecl),
    SearchIndex(SearchIndexDecl),
    V0Component(V0ComponentDecl),
    Routes(RoutesDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    Query(QueryDecl),
    Mutation(MutationDecl),
    Action(ActionDecl),
    Skill(SkillDecl),
    AgentDef(AgentDefDecl),
    Agent(AgentDecl),
    Message(MessageDecl),
    Scheduled(ScheduledDecl),
    Config(ConfigDecl),
    Context(ContextDecl),
    Hook(HookDecl),
    Provider(ProviderDecl),
    Fixture(FixtureDecl),
    Layout(LayoutDecl),
    Loading(LoadingDecl),
    NotFound(NotFoundDecl),
    ErrorBoundary(ErrorBoundaryDecl),
    Keyframes(KeyframeDecl),
    Theme(ThemeDecl),
    Mock(MockDecl),
    Environment(EnvironmentDecl),
    Page(PageDecl),
}

impl Decl {
    pub fn set_description(&mut self, desc: String) {
        match self {
            Decl::Table(t) => t.description = Some(desc),
            Decl::Collection(c) => c.description = Some(desc),
            Decl::McpTool(m) => m.description = desc,
            Decl::McpResource(m) => m.description = desc,
            _ => {}
        }
    }
    pub fn set_json_layout(&mut self, layout: String) {
        match self {
            Decl::TypeDef(t) => t.json_layout = Some(layout),
            Decl::Table(t) => t.json_layout = Some(layout),
            _ => {}
        }
    }

    pub fn set_security(&mut self, auth: Option<String>, roles: Vec<String>, cors: Option<String>) {
        if auth.is_none() && roles.is_empty() && cors.is_none() {
            return;
        }
        match self {
            Decl::Function(f) => {
                if auth.is_some() { f.auth_provider = auth; }
                if !roles.is_empty() { f.roles.extend(roles); }
                if cors.is_some() { f.cors = cors; }
            }
            Decl::Component(c) => {
                if auth.is_some() { c.func.auth_provider = auth; }
                if !roles.is_empty() { c.func.roles.extend(roles); }
                if cors.is_some() { c.func.cors = cors; }
            }
            Decl::ServerFn(s) => {
                if auth.is_some() { s.func.auth_provider = auth; }
                if !roles.is_empty() { s.func.roles.extend(roles); }
                if cors.is_some() { s.func.cors = cors; }
            }
            Decl::Query(q) => {
                if auth.is_some() { q.func.auth_provider = auth; }
                if !roles.is_empty() { q.func.roles.extend(roles); }
                if cors.is_some() { q.func.cors = cors; }
            }
            Decl::Mutation(m) => {
                if auth.is_some() { m.func.auth_provider = auth; }
                if !roles.is_empty() { m.func.roles.extend(roles); }
                if cors.is_some() { m.func.cors = cors; }
            }
            Decl::Action(a) => {
                if auth.is_some() { a.func.auth_provider = auth; }
                if !roles.is_empty() { a.func.roles.extend(roles); }
                if cors.is_some() { a.func.cors = cors; }
            }
            Decl::HttpRoute(h) => {
                if auth.is_some() { h.auth_provider = auth; }
                if !roles.is_empty() { h.roles.extend(roles); }
                if cors.is_some() { h.cors = cors; }
            }
            Decl::Table(t) => {
                if auth.is_some() { t.auth_provider = auth; }
                if !roles.is_empty() { t.roles.extend(roles); }
                if cors.is_some() { t.cors = cors; }
            }
            Decl::Layout(l) => {
                if auth.is_some() { l.func.auth_provider = auth; }
                if !roles.is_empty() { l.func.roles.extend(roles); }
                if cors.is_some() { l.func.cors = cors; }
            }
            Decl::Loading(l) => {
                if auth.is_some() { l.func.auth_provider = auth; }
                if !roles.is_empty() { l.func.roles.extend(roles); }
                if cors.is_some() { l.func.cors = cors; }
            }
            Decl::NotFound(n) => {
                if auth.is_some() { n.func.auth_provider = auth; }
                if !roles.is_empty() { n.func.roles.extend(roles); }
                if cors.is_some() { n.func.cors = cors; }
            }
            Decl::ErrorBoundary(e) => {
                if auth.is_some() { e.func.auth_provider = auth; }
                if !roles.is_empty() { e.func.roles.extend(roles); }
                if cors.is_some() { e.func.cors = cors; }
            }
            Decl::Page(p) => {
                if auth.is_some() { p.func.auth_provider = auth; }
                if !roles.is_empty() { p.func.roles.extend(roles); }
                if cors.is_some() { p.func.cors = cors; }
            }
            _ => {}
        }
    }

    pub fn set_decorators(
        &mut self,
        is_deprecated: bool,
        is_pure: bool,
        is_traced: bool,
        is_llm: bool,
        llm_model: Option<String>,
        is_layout: bool,
        is_metric: bool,
        metric_name: Option<String>,
        is_health: bool,
    ) {
        match self {
            Decl::Function(f) => {
                if is_deprecated { f.is_deprecated = true; }
                if is_pure { f.is_pure = true; }
                if is_traced { f.is_traced = true; }
                if is_llm { f.is_llm = true; f.llm_model = llm_model; }
                if is_layout { f.is_layout = true; }
                if is_health { f.is_health = true; }
            }
            Decl::Component(c) => {
                if is_deprecated { c.func.is_deprecated = true; }
                if is_traced { c.func.is_traced = true; }
                if is_metric { c.func.is_metric = true; c.func.metric_name = metric_name.clone(); }
                if is_health { c.func.is_health = true; }
            }
            Decl::Test(t) => {
                if is_deprecated { t.func.is_deprecated = true; }
                if is_traced { t.func.is_traced = true; }
                if is_metric { t.func.is_metric = true; t.func.metric_name = metric_name.clone(); }
                if is_health { t.func.is_health = true; }
            }
            Decl::ServerFn(s) => {
                if is_deprecated { s.func.is_deprecated = true; }
                if is_traced { s.func.is_traced = true; }
                if is_metric { s.func.is_metric = true; s.func.metric_name = metric_name.clone(); }
                if is_health { s.func.is_health = true; }
            }
            Decl::Query(q) => {
                if is_deprecated { q.func.is_deprecated = true; }
                if is_traced { q.func.is_traced = true; }
                if is_metric { q.func.is_metric = true; q.func.metric_name = metric_name.clone(); }
                if is_health { q.func.is_health = true; }
            }
            Decl::Mutation(m) => {
                if is_deprecated { m.func.is_deprecated = true; }
                if is_traced { m.func.is_traced = true; }
                if is_metric { m.func.is_metric = true; m.func.metric_name = metric_name.clone(); }
                if is_health { m.func.is_health = true; }
            }
            Decl::Action(a) => {
                if is_deprecated { a.func.is_deprecated = true; }
                if is_traced { a.func.is_traced = true; }
                if is_metric { a.func.is_metric = true; a.func.metric_name = metric_name.clone(); }
                if is_health { a.func.is_health = true; }
            }
            Decl::Skill(s) => {
                if is_deprecated { s.func.is_deprecated = true; }
                if is_traced { s.func.is_traced = true; }
                if is_metric { s.func.is_metric = true; s.func.metric_name = metric_name.clone(); }
                if is_health { s.func.is_health = true; }
            }
            Decl::AgentDef(a) => {
                if is_deprecated { a.func.is_deprecated = true; }
                if is_traced { a.func.is_traced = true; }
                if is_metric { a.func.is_metric = true; a.func.metric_name = metric_name.clone(); }
                if is_health { a.func.is_health = true; }
            }
            Decl::Scheduled(s) => {
                if is_deprecated { s.func.is_deprecated = true; }
                if is_traced { s.func.is_traced = true; }
                if is_metric { s.func.is_metric = true; s.func.metric_name = metric_name.clone(); }
                if is_health { s.func.is_health = true; }
            }
            Decl::McpTool(m) => {
                if is_deprecated { m.func.is_deprecated = true; }
                if is_traced { m.func.is_traced = true; }
                if is_metric { m.func.is_metric = true; m.func.metric_name = metric_name.clone(); }
                if is_health { m.func.is_health = true; }
            }
            Decl::McpResource(m) => {
                if is_deprecated { m.func.is_deprecated = true; }
                if is_traced { m.func.is_traced = true; }
                if is_metric { m.func.is_metric = true; m.func.metric_name = metric_name.clone(); }
                if is_health { m.func.is_health = true; }
            }
            Decl::Page(p) => {
                if is_deprecated { p.func.is_deprecated = true; }
                if is_traced { p.func.is_traced = true; }
                if is_metric { p.func.is_metric = true; p.func.metric_name = metric_name.clone(); }
                if is_health { p.func.is_health = true; }
            }
            Decl::Workflow(w) => {
                if is_deprecated { w.is_deprecated = true; }
                if is_traced { w.is_traced = true; }
            }
            Decl::Activity(a) => {
                if is_deprecated { a.is_deprecated = true; }
                if is_traced { a.is_traced = true; }
            }
            Decl::HttpRoute(h) => {
                if is_deprecated { h.is_deprecated = true; }
                if is_traced { h.is_traced = true; }
            }
            Decl::Actor(a) => { if is_deprecated { a.is_deprecated = true; } }
            Decl::Table(t) => { if is_deprecated { t.is_deprecated = true; } }
            Decl::Trait(t) => { if is_deprecated { t.is_deprecated = true; } }
            Decl::TypeDef(t) => { if is_deprecated { t.is_deprecated = true; } }
            Decl::Const(c) => { if is_deprecated { c.is_deprecated = true; } }
            Decl::Config(c) => { if is_deprecated { c.is_deprecated = true; } }
            Decl::Environment(e) => { if is_deprecated { e.is_deprecated = true; } }
            Decl::Agent(a) => { if is_deprecated { a.is_deprecated = true; } }
            Decl::Message(m) => { if is_deprecated { m.is_deprecated = true; } }
            Decl::Layout(l) => {
                if is_deprecated { l.func.is_deprecated = true; }
                if is_traced { l.func.is_traced = true; }
            }
            Decl::Loading(l) => {
                if is_deprecated { l.func.is_deprecated = true; }
                if is_traced { l.func.is_traced = true; }
            }
            Decl::NotFound(n) => {
                if is_deprecated { n.func.is_deprecated = true; }
                if is_traced { n.func.is_traced = true; }
            }
            Decl::ErrorBoundary(e) => {
                if is_deprecated { e.func.is_deprecated = true; }
                if is_traced { e.func.is_traced = true; }
            }
            _ => {}
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Decl::Function(f) => f.span,
            Decl::Component(c) => c.func.span,
            Decl::TypeDef(t) => t.span,
            Decl::Import(i) => i.span,
            Decl::PyImport(p) => p.span,
            Decl::Actor(a) => a.span,
            Decl::Workflow(w) => w.span,
            Decl::Activity(a) => a.span,
            Decl::HttpRoute(h) => h.span,
            Decl::McpTool(m) => m.func.span,
            Decl::Test(t) => t.func.span,
            Decl::ServerFn(s) => s.func.span,
            Decl::Table(t) => t.span,
            Decl::Collection(c) => c.span,
            Decl::Index(i) => i.span,
            Decl::VectorIndex(v) => v.span,
            Decl::SearchIndex(s) => s.span,
            Decl::V0Component(v) => v.span,
            Decl::Routes(r) => r.span,
            Decl::Trait(t) => t.span,
            Decl::Impl(i) => i.span,
            Decl::Query(q) => q.func.span,
            Decl::Mutation(m) => m.func.span,
            Decl::Action(a) => a.func.span,
            Decl::Skill(s) => s.func.span,
            Decl::AgentDef(ad) => ad.func.span,
            Decl::Agent(a) => a.span,
            Decl::Message(m) => m.span,
            Decl::Scheduled(s) => s.func.span,
            Decl::Const(c) => c.span,
            Decl::Config(c) => c.span,
            Decl::Context(c) => c.span,
            Decl::Hook(h) => h.func.span,
            Decl::Provider(p) => p.func.span,
            Decl::Fixture(f) => f.func.span,
            Decl::Layout(l) => l.func.span,
            Decl::Loading(l) => l.func.span,
            Decl::NotFound(n) => n.func.span,
            Decl::ErrorBoundary(e) => e.func.span,
            Decl::Keyframes(k) => k.span,
            Decl::Theme(t) => t.span,
            Decl::Mock(m) => m.func.span,
            Decl::McpResource(m) => m.func.span,
            Decl::Environment(e) => e.span,
            Decl::Page(p) => p.span,
        }
    }
}

/// A complete Vox source module (one file).
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub declarations: Vec<Decl>,
    pub span: Span,
}
