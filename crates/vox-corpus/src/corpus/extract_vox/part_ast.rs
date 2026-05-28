//! AST-backed declaration slicing for Mens corpus rows (`ast-extract` feature).

use vox_compiler::ast::decl::Decl;
use vox_compiler::ast::decl::Module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

/// Returns `(construct_kind, display_name, source_slice)` per top-level declaration.
pub fn extract_decl_blocks_ast(source: &str) -> Option<Vec<(String, String, String)>> {
    let tokens = lex(source);
    let module: Module = parse(tokens).ok()?;
    let mut out = Vec::new();
    for decl in &module.declarations {
        let span = decl.span();
        let end = span.end.min(source.len());
        let start = span.start.min(end);
        let block = source.get(start..end)?.to_string();
        if block.trim().is_empty() {
            continue;
        }
        let (kind, name) = decl_kind_and_name(decl);
        out.push((kind, name, block));
    }
    if out.is_empty() { None } else { Some(out) }
}

fn http_method_label(m: vox_compiler::ast::decl::HttpMethod) -> &'static str {
    use vox_compiler::ast::decl::HttpMethod;
    match m {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
    }
}

fn decl_kind_and_name(decl: &Decl) -> (String, String) {
    match decl {
        Decl::Function(f) => ("fn".into(), f.name.clone()),
        Decl::TypeDef(t) => ("type".into(), t.name.clone()),
        Decl::Import(_) => ("import".into(), "import".into()),
        Decl::Const(c) => ("const".into(), c.name.clone()),
        Decl::HttpRoute(h) => (
            "http_route".into(),
            format!("{} {}", http_method_label(h.method), h.path),
        ),
        Decl::McpTool(m) => ("mcp_tool".into(), m.func.name.clone()),
        Decl::McpResource(m) => ("mcp_resource".into(), m.func.name.clone()),
        Decl::Test(t) => ("test".into(), t.func.name.clone()),
        Decl::Forall(f) => ("forall".into(), f.func.name.clone()),
        Decl::Table(t) => ("table".into(), t.name.clone()),
        Decl::Collection(c) => ("collection".into(), c.name.clone()),
        Decl::Index(i) => ("index".into(), i.index_name.clone()),
        Decl::VectorIndex(v) => ("vector_index".into(), v.index_name.clone()),
        Decl::SearchIndex(s) => ("search_index".into(), s.index_name.clone()),
        Decl::V0Component(v) => ("v0_component".into(), v.name.clone()),
        Decl::Routes(_) => ("routes".into(), "routes".into()),
        Decl::Skill(s) => ("skill".into(), s.func.name.clone()),
        Decl::AgentDef(a) => ("agent_def".into(), a.func.name.clone()),
        Decl::Agent(a) => ("agent".into(), a.name.clone()),
        Decl::Message(m) => ("message".into(), m.name.clone()),
        Decl::Scheduled(s) => ("scheduled".into(), s.func.name.clone()),
        Decl::Config(c) => ("config".into(), c.name.clone()),
        Decl::Loading(l) => ("loading".into(), l.func.name.clone()),
        Decl::Theme(t) => ("theme".into(), t.name.clone()),
        Decl::Environment(e) => ("environment".into(), e.name.clone()),
        Decl::Page(p) => ("page".into(), p.func.name.clone()),
        Decl::ReactiveComponent(r) => ("reactive_component".into(), r.name.clone()),
        Decl::ReactiveModule(r) => ("reactive_module".into(), r.name.clone()),
        Decl::Fragment(f) => ("fragment".into(), f.name.clone()),
        Decl::Endpoint(e) => ("endpoint".into(), e.func.name.clone()),
        Decl::Url(u) => ("url".into(), u.name.clone()),
        Decl::StateMachine(s) => ("state_machine".into(), s.name.clone()),
        Decl::Workflow(w) => ("workflow".into(), w.name.clone()),
        Decl::Activity(a) => ("activity".into(), a.name.clone()),
        Decl::Actor(a) => ("actor".into(), a.name.clone()),
        Decl::Form(f) => ("form".into(), f.name.clone()),
        Decl::BackButton(_) => ("back_button".into(), "back_button".into()),
        Decl::DeepLink(d) => ("deep_link".into(), d.scheme.clone()),
        Decl::Push(_) => ("push".into(), "push".into()),
        Decl::Tokens(_) => ("tokens".into(), "tokens".into()),
    }
}
