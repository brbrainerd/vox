//! Security and decorator patching on Decl (OP-0207).

use super::types::Decl;

impl Decl {
    pub fn set_security(&mut self, auth: Option<String>, roles: Vec<String>, cors: Option<String>) {
        if auth.is_none() && roles.is_empty() && cors.is_none() {
            return;
        }
        match self {
            Decl::Function(f) => {
                if auth.is_some() {
                    f.auth_provider = auth;
                }
                if !roles.is_empty() {
                    f.roles.extend(roles);
                }
                if cors.is_some() {
                    f.cors = cors;
                }
            }
            Decl::Endpoint(e) => {
                if auth.is_some() {
                    e.func.auth_provider = auth;
                }
                if !roles.is_empty() {
                    e.func.roles.extend(roles);
                }
                if cors.is_some() {
                    e.func.cors = cors;
                }
            }

            Decl::HttpRoute(h) => {
                if auth.is_some() {
                    h.auth_provider = auth;
                }
                if !roles.is_empty() {
                    h.roles.extend(roles);
                }
                if cors.is_some() {
                    h.cors = cors;
                }
            }
            Decl::Table(t) => {
                if auth.is_some() {
                    t.auth_provider = auth;
                }
                if !roles.is_empty() {
                    t.roles.extend(roles);
                }
                if cors.is_some() {
                    t.cors = cors;
                }
            }
            Decl::Loading(l) => {
                if auth.is_some() {
                    l.func.auth_provider = auth;
                }
                if !roles.is_empty() {
                    l.func.roles.extend(roles);
                }
                if cors.is_some() {
                    l.func.cors = cors;
                }
            }
            Decl::Page(p) => {
                if auth.is_some() {
                    p.func.auth_provider = auth;
                }
                if !roles.is_empty() {
                    p.func.roles.extend(roles);
                }
                if cors.is_some() {
                    p.func.cors = cors;
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// Applies boolean flags from `@` decorators (deprecated, pure, traced, LLM, metrics, health, …).
    ///
    /// The flat parameter list matches legacy parser entry points; each flag updates only the
    /// declaration kinds where that concept exists (e.g. `is_layout` only touches [`Decl::Function`]).
    pub fn set_decorators(
        &mut self,
        is_deprecated: bool,
        is_pure: bool,
        is_traced: bool,
        is_mobile_native: bool,
    ) {
        match self {
            Decl::Function(f) => {
                if is_deprecated {
                    f.is_deprecated = true;
                }
                if is_pure {
                    f.is_pure = true;
                }
                if is_traced {
                    f.is_traced = true;
                }
                if is_mobile_native {
                    f.is_mobile_native = true;
                }
            }
            Decl::Test(t) => {
                if is_deprecated {
                    t.func.is_deprecated = true;
                }
                if is_traced {
                    t.func.is_traced = true;
                }
            }
            Decl::Endpoint(e) => {
                if is_deprecated {
                    e.func.is_deprecated = true;
                }
                if is_traced {
                    e.func.is_traced = true;
                }
            }

            Decl::Skill(s) => {
                if is_deprecated {
                    s.func.is_deprecated = true;
                }
                if is_traced {
                    s.func.is_traced = true;
                }
            }
            Decl::AgentDef(a) => {
                if is_deprecated {
                    a.func.is_deprecated = true;
                }
                if is_traced {
                    a.func.is_traced = true;
                }
            }
            Decl::Scheduled(s) => {
                if is_deprecated {
                    s.func.is_deprecated = true;
                }
                if is_traced {
                    s.func.is_traced = true;
                }
            }
            Decl::McpTool(m) => {
                if is_deprecated {
                    m.func.is_deprecated = true;
                }
                if is_traced {
                    m.func.is_traced = true;
                }
            }
            Decl::McpResource(m) => {
                if is_deprecated {
                    m.func.is_deprecated = true;
                }
                if is_traced {
                    m.func.is_traced = true;
                }
            }
            Decl::Page(p) => {
                if is_deprecated {
                    p.func.is_deprecated = true;
                }
                if is_traced {
                    p.func.is_traced = true;
                }
            }
            Decl::HttpRoute(h) => {
                if is_deprecated {
                    h.is_deprecated = true;
                }
                if is_traced {
                    h.is_traced = true;
                }
            }
            Decl::Table(t) if is_deprecated => {
                t.is_deprecated = true;
            }
            Decl::TypeDef(t) if is_deprecated => {
                t.is_deprecated = true;
            }
            Decl::Const(c) if is_deprecated => {
                c.is_deprecated = true;
            }
            Decl::Config(c) if is_deprecated => {
                c.is_deprecated = true;
            }
            Decl::Environment(e) if is_deprecated => {
                e.is_deprecated = true;
            }
            Decl::Agent(a) if is_deprecated => {
                a.is_deprecated = true;
            }
            Decl::Message(m) if is_deprecated => {
                m.is_deprecated = true;
            }
            Decl::Loading(l) => {
                if is_deprecated {
                    l.func.is_deprecated = true;
                }
                if is_traced {
                    l.func.is_traced = true;
                }
            }
            _ => {}
        }
    }
}
