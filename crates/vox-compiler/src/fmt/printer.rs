//! Formatter state and declaration / top-level printing (OP-0205).

use crate::ast::decl::*;
use crate::ast::expr::Param;
use crate::ast::stmt::Stmt;
use crate::ast::types::TypeExpr;

pub(crate) struct Printer {
    pub(crate) out: String,
    pub(crate) indent_level: usize,
}

impl Printer {
    pub(crate) fn new() -> Self {
        Self {
            out: String::new(),
            indent_level: 0,
        }
    }

    pub(crate) fn finish(self) -> String {
        self.out
    }

    pub(crate) fn indent(&mut self) {
        self.indent_level += 4;
    }

    pub(crate) fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(4);
    }

    pub(crate) fn write_indent(&mut self) {
        self.out.push_str(&" ".repeat(self.indent_level));
    }

    pub(crate) fn print_module(&mut self, module: &Module) {
        let mut last_was_import = false;
        for (i, decl) in module.declarations.iter().enumerate() {
            let is_import = matches!(decl, Decl::Import(_));

            if i > 0 {
                if is_import && last_was_import {
                    self.out.push('\n'); // Single newline between imports
                } else {
                    self.out.push_str("\n\n"); // Two newlines between top-level decl sections
                }
            }

            self.print_decl(decl);
            last_was_import = is_import;
        }
    }

    pub(crate) fn print_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Import(i) => self.print_import(i),
            Decl::Function(f) => self.print_fn(f, ""),
            Decl::TypeDef(t) => self.print_typedef(t),
            Decl::Const(c) => {
                self.write_indent();
                if c.is_pub {
                    self.out.push_str("pub ");
                }
                self.out.push_str("const ");
                self.out.push_str(&c.name);
                if let Some(ref ty) = c.type_ann {
                    self.out.push_str(": ");
                    self.print_type(ty);
                }
                self.out.push_str(" = ");
                self.print_expr(&c.value);
            }
            Decl::HttpRoute(h) => {
                self.write_indent();
                self.out.push_str("http ");
                self.out.push_str(match h.method {
                    HttpMethod::Get => "get",
                    HttpMethod::Post => "post",
                    HttpMethod::Put => "put",
                    HttpMethod::Delete => "delete",
                });
                self.out.push(' ');
                self.out.push_str(&format!("\"{}\"", h.path));
                self.print_fn_body(&h.params, &h.return_type, &h.body);
            }
            Decl::Table(t) => {
                self.write_indent();
                if t.is_pub {
                    self.out.push_str("pub ");
                }
                self.out.push_str("@table type ");
                self.out.push_str(&t.name);
                self.out.push_str(
                    " {
",
                );
                self.indent();
                for field in &t.fields {
                    self.write_indent();
                    self.out.push_str(&field.name);
                    self.out.push_str(": ");
                    self.print_type(&field.type_ann);
                    self.out.push('\n');
                }
                self.dedent();
                self.write_indent();
                self.out.push('}');
            }

            Decl::Skill(s) => self.print_fn(&s.func, "@skill "),
            Decl::AgentDef(a) => self.print_fn(&a.func, "@agent_def "),
            Decl::Scheduled(s) => {
                self.write_indent();
                self.out
                    .push_str(&format!("@scheduled(\"{}\") ", s.interval));
                self.out.push_str("fn ");
                self.out.push_str(&s.func.name);
                self.print_fn_body(&s.func.params, &s.func.return_type, &s.func.body);
            }
            Decl::Test(t) => self.print_fn(&t.func, "@test "),
            Decl::Example(e) => self.print_fn(&e.func, "@example "),
            Decl::McpTool(m) => {
                self.write_indent();
                self.out
                    .push_str(&format!("@mcp.tool(\"{}\") ", m.description));
                self.out.push_str("fn ");
                self.out.push_str(&m.func.name);
                self.print_fn_body(&m.func.params, &m.func.return_type, &m.func.body);
            }
            Decl::McpResource(m) => {
                self.write_indent();
                let ue = m.uri.replace('\\', "\\\\").replace('"', "\\\"");
                let de = m.description.replace('\\', "\\\\").replace('"', "\\\"");
                self.out
                    .push_str(&format!("@mcp.resource(\"{ue}\", \"{de}\") "));
                self.out.push_str("fn ");
                self.out.push_str(&m.func.name);
                self.print_fn_body(&m.func.params, &m.func.return_type, &m.func.body);
            }
            Decl::Routes(r) => {
                self.write_indent();
                self.out.push_str(
                    "routes {
",
                );
                self.indent();
                for entry in &r.entries {
                    self.write_indent();
                    self.out.push_str(&format!("\"{}\"", entry.path));
                    self.out.push_str(" to ");
                    self.out.push_str(&entry.component_name);
                    self.out.push('\n');
                }
                self.dedent();
                self.write_indent();
                self.out.push('}');
            }
            Decl::Config(c) => {
                self.write_indent();
                self.out.push_str("@config ");
                self.out.push_str(&c.name);
                self.out.push_str(":\n");
                self.indent();
                for field in &c.fields {
                    self.write_indent();
                    self.out.push_str(&field.name);
                    self.out.push_str(": ");
                    self.print_type(&field.type_ann);
                    self.out.push('\n');
                }
                self.dedent();
            }
            Decl::Environment(e) => self.print_environment(e),
            Decl::ReactiveComponent(r) => self.print_reactive_component(r),
            Decl::V0Component(v) => self.print_v0_component(v),
            Decl::Fragment(f) => self.print_fragment_decl(f),
            Decl::ReactiveModule(rm) => self.print_reactive_module_decl(rm),
            _ => {}
        }
    }

    /// Pretty-print a `fragment Name(args) { body }` declaration (ADR-033).
    fn print_fragment_decl(&mut self, f: &FragmentDecl) {
        self.write_indent();
        self.out.push_str("fragment ");
        self.out.push_str(&f.name);
        self.out.push('(');
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.out.push_str(&p.name);
            if let Some(t) = &p.type_ann {
                self.out.push_str(": ");
                self.print_type(t);
            }
        }
        self.out.push_str(") {\n");
        self.indent();
        self.write_indent();
        self.print_expr(&f.body);
        self.out.push('\n');
        self.dedent();
        self.write_indent();
        self.out.push('}');
    }

    /// Pretty-print a `.vox.ui` reactive module (ADR-032). The members are the same
    /// shape as the body of a `component { }` block, so reuse the same per-member
    /// emission as `print_reactive_component`.
    fn print_reactive_module_decl(&mut self, rm: &ReactiveModuleDecl) {
        // ReactiveModuleDecl has no surrounding keyword in source — the file's
        // `.vox.ui` extension makes the members module-scope. The pretty-printer
        // round-trips by emitting the members directly, one per line.
        for m in &rm.members {
            self.write_indent();
            match m {
                ReactiveMemberDecl::State(s) => {
                    self.out.push_str("state ");
                    self.out.push_str(&s.name);
                    if let Some(t) = &s.ty {
                        self.out.push_str(": ");
                        self.print_type(t);
                    }
                    self.out.push_str(" = ");
                    self.print_expr(&s.init);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::Derived(d) => {
                    self.out.push_str("derived ");
                    self.out.push_str(&d.name);
                    if let Some(t) = &d.ty {
                        self.out.push_str(": ");
                        self.print_type(t);
                    }
                    self.out.push_str(" = ");
                    self.print_expr(&d.expr);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::Effect(e) => {
                    self.out.push_str("effect: ");
                    self.print_expr(&e.body);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::OnMount(om) => {
                    self.out.push_str("on mount: ");
                    self.print_expr(&om.body);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::OnCleanup(oc) => {
                    self.out.push_str("on cleanup: ");
                    self.print_expr(&oc.body);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::Stmt(s) => self.print_stmt(s),
            }
        }
    }

    fn print_v0_component(&mut self, v: &V0ComponentDecl) {
        self.write_indent();
        self.out.push_str("@v0 ");
        if let Some(ref path) = v.image_path {
            self.out.push_str("from ");
            self.out.push_str(&format!("\"{}\" ", path));
        } else {
            self.out.push_str(&format!("\"{}\" ", v.v0_id));
        }
        self.out.push_str(&v.name);
        self.out.push_str(" {\n");
        if !v.props.is_empty() {
            self.indent();
            for p in &v.props {
                self.write_indent();
                self.out.push_str(&p.name);
                if p.is_optional {
                    self.out.push('?');
                }
                self.out.push_str(": ");
                self.print_type(&p.ty);
                self.out.push('\n');
            }
            self.dedent();
        }
        self.write_indent();
        self.out.push('}');
    }

    fn print_reactive_component(&mut self, r: &ReactiveComponentDecl) {
        self.write_indent();
        self.out.push_str("component ");
        self.out.push_str(&r.name);
        self.out.push('(');
        for (i, p) in r.params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.out.push_str(&p.name);
            if let Some(ty) = p.type_ann.as_ref() {
                self.out.push_str(": ");
                self.print_type(ty);
            }
        }
        self.out.push_str(") {\n");
        self.indent();

        for m in &r.members {
            self.write_indent();
            match m {
                ReactiveMemberDecl::State(s) => {
                    self.out.push_str("state ");
                    self.out.push_str(&s.name);
                    if let Some(ty) = &s.ty {
                        self.out.push_str(": ");
                        self.print_type(ty);
                    }
                    self.out.push_str(" = ");
                    self.print_expr(&s.init);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::Derived(d) => {
                    self.out.push_str("derived ");
                    self.out.push_str(&d.name);
                    if let Some(ty) = &d.ty {
                        self.out.push_str(": ");
                        self.print_type(ty);
                    }
                    self.out.push_str(" = ");
                    self.print_expr(&d.expr);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::Effect(e) => {
                    self.out.push_str("effect: ");
                    self.print_expr(&e.body);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::OnMount(m) => {
                    self.out.push_str("mount: ");
                    self.print_expr(&m.body);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::OnCleanup(c) => {
                    self.out.push_str("cleanup: ");
                    self.print_expr(&c.body);
                    self.out.push('\n');
                }
                ReactiveMemberDecl::Stmt(s) => {
                    self.print_stmt(s);
                }
            }
        }

        if let Some(view) = &r.view {
            self.write_indent();
            self.out.push_str("view: ");
            self.print_expr(view);
            self.out.push('\n');
        }

        self.dedent();
        self.write_indent();
        self.out.push('}');

        if !r.styles.is_empty() {
            self.out.push_str("\n\n");
            self.write_indent();
            self.out.push_str("style {\n");
            self.indent();
            for style in &r.styles {
                self.write_indent();
                self.out.push_str(&style.selector);
                self.out.push_str(" {\n");
                self.indent();
                for (k, v) in &style.properties {
                    self.write_indent();
                    self.out.push_str(k);
                    self.out.push_str(": ");
                    self.out.push_str(v);
                    self.out.push_str(";\n");
                }
                self.dedent();
                self.write_indent();
                self.out.push_str("}\n");
            }
            self.dedent();
            self.write_indent();
            self.out.push('}');
        }
    }

    fn print_import(&mut self, i: &ImportDecl) {
        self.write_indent();
        self.out.push_str("import ");
        for (idx, path) in i.paths.iter().enumerate() {
            if idx > 0 {
                self.out.push_str(", ");
            }
            match &path.kind {
                ImportPathKind::SymbolPath { segments } => {
                    self.out.push_str(&segments.join("."));
                }
                ImportPathKind::ReactComponent {
                    local_name,
                    module_specifier,
                } => {
                    self.out.push_str("react ");
                    self.out.push_str(local_name);
                    self.out.push_str(" from ");
                    self.out.push('"');
                    self.out.push_str(module_specifier);
                    self.out.push('"');
                }
                ImportPathKind::RustCrate(spec) => {
                    self.out.push_str("rust:");
                    self.out.push_str(&spec.crate_name);
                    let mut meta = Vec::new();
                    if let Some(v) = &spec.version {
                        meta.push(format!("version: \"{v}\""));
                    }
                    if let Some(v) = &spec.path {
                        meta.push(format!("path: \"{v}\""));
                    }
                    if let Some(v) = &spec.git {
                        meta.push(format!("git: \"{v}\""));
                    }
                    if let Some(v) = &spec.rev {
                        meta.push(format!("rev: \"{v}\""));
                    }
                    if !meta.is_empty() {
                        self.out.push('(');
                        self.out.push_str(&meta.join(", "));
                        self.out.push(')');
                    }
                }
            }
            if let Some(alias) = &path.alias {
                self.out.push_str(" as ");
                self.out.push_str(alias);
            }
        }
    }

    fn print_typedef(&mut self, t: &TypeDefDecl) {
        self.write_indent();
        if t.is_pub {
            self.out.push_str("pub ");
        }
        self.out.push_str("type ");
        self.out.push_str(&t.name);

        if let Some(ref alias) = t.type_alias {
            self.out.push_str(" = ");
            self.print_type(alias);
        } else if !t.variants.is_empty() {
            self.out.push_str(" = \n");
            self.indent();
            for var in &t.variants {
                self.write_indent();
                self.out.push_str("| ");
                self.out.push_str(&var.name);
                if !var.fields.is_empty() {
                    self.out.push_str(" { ");
                    for (i, field) in var.fields.iter().enumerate() {
                        if i > 0 {
                            self.out.push_str(", ");
                        }
                        self.out.push_str(&field.name);
                        self.out.push_str(": ");
                        self.print_type(&field.type_ann);
                    }
                    self.out.push_str(" }");
                }
                self.out.push('\n');
            }
            self.dedent();
        } else {
            self.out.push_str(" {\n");
            self.indent();
            for field in &t.fields {
                self.write_indent();
                self.out.push_str(&field.name);
                self.out.push_str(": ");
                self.print_type(&field.type_ann);
                self.out.push('\n');
            }
            self.dedent();
            self.write_indent();
            self.out.push('}');
        }
    }

    fn print_fn(&mut self, f: &FnDecl, prefix: &str) {
        self.write_indent();
        self.out.push_str(prefix);
        if f.is_pub {
            self.out.push_str("pub ");
        }
        if f.is_async {
            self.out.push_str("async ");
        }
        self.out.push_str("fn ");
        self.out.push_str(&f.name);
        self.print_fn_body(&f.params, &f.return_type, &f.body);
    }

    pub(crate) fn print_fn_body(
        &mut self,
        params: &[Param],
        ret: &Option<TypeExpr>,
        body: &[Stmt],
    ) {
        self.out.push('(');
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.out.push_str(&p.name);
            if let Some(ty) = p.type_ann.as_ref() {
                self.out.push_str(": ");
                self.print_type(ty);
            }
        }
        self.out.push(')');

        if let Some(r) = ret.as_ref() {
            self.out.push_str(" to ");
            self.print_type(r);
        }

        if body.is_empty() {
            self.out.push_str(
                " {
",
            );
            self.indent();
            self.write_indent();
            self.out.push_str("pass\n");
            self.dedent();
            self.write_indent();
            self.out.push('}');
        } else {
            self.out.push_str(
                " {
",
            );
            self.indent();
            for s in body {
                self.print_stmt(s);
            }
            self.dedent();
            self.write_indent();
            self.out.push('}');
        }
    }

    fn print_environment(&mut self, env: &EnvironmentDecl) {
        self.write_indent();
        self.out.push_str("@environment ");
        self.out.push_str(&env.name);
        self.out.push_str(":\n");
        self.indent();

        if let Some(ref base) = env.base_image {
            self.write_indent();
            self.out.push_str("base: ");
            self.out.push_str(&format!("\"{}\"", base));
            self.out.push('\n');
        }

        if let Some(ref wd) = env.workdir {
            self.write_indent();
            self.out.push_str("workdir: ");
            self.out.push_str(&format!("\"{}\"", wd));
            self.out.push('\n');
        }

        if !env.packages.is_empty() {
            self.write_indent();
            self.out.push_str("packages: [");
            for (i, pkg) in env.packages.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                self.out.push_str(&format!("\"{}\"", pkg));
            }
            self.out.push_str("]\n");
        }

        if !env.env_vars.is_empty() {
            self.write_indent();
            self.out.push_str("env:\n");
            self.indent();
            for (k, v) in &env.env_vars {
                self.write_indent();
                self.out.push_str(k);
                self.out.push_str(": ");
                self.out.push_str(&format!("\"{}\"", v));
                self.out.push('\n');
            }
            self.dedent();
        }

        if !env.exposed_ports.is_empty() {
            self.write_indent();
            self.out.push_str("expose: [");
            for (i, p) in env.exposed_ports.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                self.out.push_str(&p.to_string());
            }
            self.out.push_str("]\n");
        }

        if !env.volumes.is_empty() {
            self.write_indent();
            self.out.push_str("volumes: [");
            for (i, v) in env.volumes.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                self.out.push_str(&format!("\"{}\"", v));
            }
            self.out.push_str("]\n");
        }

        if !env.copy_instructions.is_empty() {
            self.write_indent();
            self.out.push_str("copy:\n");
            self.indent();
            for (src, dest) in &env.copy_instructions {
                self.write_indent();
                self.out.push_str(&format!("\"{}\" to \"{}\"\n", src, dest));
            }
            self.dedent();
        }

        if !env.run_commands.is_empty() {
            self.write_indent();
            self.out.push_str("run: [");
            for (i, cmd) in env.run_commands.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                self.out.push_str(&format!("\"{}\"", cmd));
            }
            self.out.push_str("]\n");
        }

        if !env.cmd.is_empty() {
            self.write_indent();
            self.out.push_str("cmd: [");
            for (i, c) in env.cmd.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                self.out.push_str(&format!("\"{}\"", c));
            }
            self.out.push_str("]\n");
        }

        self.dedent();
    }
}
