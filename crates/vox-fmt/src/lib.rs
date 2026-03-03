use vox_ast::decl::*;
use vox_ast::stmt::*;
use vox_ast::expr::*;
use vox_ast::pattern::*;
use vox_ast::types::*;
use vox_parser::parser::parse;
use vox_lexer::lex;

pub fn format(source: &str) -> String {
    let tokens = lex(source);
    let module = match parse(tokens) {
        Ok(m) => m,
        Err(_) => return source.to_string(), // Incomplete/invalid source - return as-is
    };

    let mut printer = Printer::new();
    printer.print_module(&module);
    printer.finish().trim_end().to_string() + "\n"
}

struct Printer {
    out: String,
    indent_level: usize,
}

impl Printer {
    fn new() -> Self {
        Self { out: String::new(), indent_level: 0 }
    }

    fn finish(self) -> String {
        self.out
    }

    fn indent(&mut self) {
        self.indent_level += 4;
    }

    fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(4);
    }

    fn write_indent(&mut self) {
        self.out.push_str(&" ".repeat(self.indent_level));
    }

    fn print_module(&mut self, module: &Module) {
        let mut last_was_import = false;
        for (i, decl) in module.declarations.iter().enumerate() {
            let is_import = matches!(decl, Decl::Import(_) | Decl::PyImport(_));

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

    fn print_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Import(i) => self.print_import(i),
            Decl::PyImport(p) => {
                self.write_indent();
                self.out.push_str("@py.import ");
                self.out.push_str(&p.module);
                if p.alias != p.module.split('.').last().unwrap_or(&p.module) {
                    self.out.push_str(" as ");
                    self.out.push_str(&p.alias);
                }
            }
            Decl::Function(f) => self.print_fn(f, ""),
            Decl::Component(c) => self.print_fn(&c.func, "@component "),
            Decl::TypeDef(t) => self.print_typedef(t),
            Decl::Const(c) => {
                self.write_indent();
                if c.is_pub { self.out.push_str("pub "); }
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
                    vox_ast::decl::HttpMethod::Get => "get",
                    vox_ast::decl::HttpMethod::Post => "post",
                    vox_ast::decl::HttpMethod::Put => "put",
                    vox_ast::decl::HttpMethod::Delete => "delete",
                });
                self.out.push(' ');
                self.out.push_str(&format!("\"{}\"", h.path));
                self.print_fn_body(&h.params, &h.return_type, &h.body);
            }
            Decl::Table(t) => {
                self.write_indent();
                if t.is_pub { self.out.push_str("pub "); }
                self.out.push_str("@table type ");
                self.out.push_str(&t.name);
                self.out.push_str(":\n");
                self.indent();
                for field in &t.fields {
                    self.write_indent();
                    self.out.push_str(&field.name);
                    self.out.push_str(": ");
                    self.print_type(&field.type_ann);
                    self.out.push('\n');
                }
                self.dedent();
            }
            Decl::ServerFn(s) => self.print_fn(&s.func, "@server "),
            Decl::Query(q) => self.print_fn(&q.func, "@query "),
            Decl::Mutation(m) => self.print_fn(&m.func, "@mutation "),
            Decl::Action(a) => self.print_fn(&a.func, "@action "),
            Decl::Skill(s) => self.print_fn(&s.func, "@skill "),
            Decl::AgentDef(a) => self.print_fn(&a.func, "@agent_def "),
            Decl::Scheduled(s) => {
                self.write_indent();
                self.out.push_str(&format!("@scheduled(\"{}\") ", s.interval));
                self.out.push_str("fn ");
                self.out.push_str(&s.func.name);
                self.print_fn_body(&s.func.params, &s.func.return_type, &s.func.body);
            }
            Decl::Test(t) => self.print_fn(&t.func, "@test "),
            Decl::Hook(h) => self.print_fn(&h.func, "@hook "),
            Decl::McpTool(m) => {
                self.write_indent();
                self.out.push_str(&format!("@mcp.tool(\"{}\") ", m.description));
                self.out.push_str("fn ");
                self.out.push_str(&m.func.name);
                self.print_fn_body(&m.func.params, &m.func.return_type, &m.func.body);
            }
            Decl::Routes(r) => {
                self.write_indent();
                self.out.push_str("routes:\n");
                self.indent();
                for entry in &r.entries {
                    self.write_indent();
                    self.out.push_str(&format!("\"{}\"", entry.path));
                    self.out.push_str(" to ");
                    self.out.push_str(&entry.component_name);
                    self.out.push('\n');
                }
                self.dedent();
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
            Decl::Workflow(w) => {
                self.write_indent();
                self.out.push_str("workflow ");
                self.out.push_str(&w.name);
                self.print_fn_body(&w.params, &w.return_type, &w.body);
            }
            Decl::Activity(a) => {
                self.write_indent();
                self.out.push_str("activity ");
                self.out.push_str(&a.name);
                self.print_fn_body(&a.params, &a.return_type, &a.body);
            }
            Decl::Actor(a) => {
                self.write_indent();
                self.out.push_str("actor ");
                self.out.push_str(&a.name);
                self.out.push_str(":\n");
                self.indent();
                for handler in &a.handlers {
                    self.write_indent();
                    self.out.push_str("on ");
                    self.out.push_str(&handler.event_name);
                    self.print_fn_body(&handler.params, &handler.return_type, &handler.body);
                    self.out.push('\n');
                }
                self.dedent();
            }
            Decl::Environment(e) => self.print_environment(e),
            _ => {}
        }
    }

    fn print_import(&mut self, i: &ImportDecl) {
        self.write_indent();
        self.out.push_str("import ");
        for (idx, path) in i.paths.iter().enumerate() {
            if idx > 0 { self.out.push_str(", "); }
            self.out.push_str(&path.segments.join("."));
        }
    }

    fn print_typedef(&mut self, t: &TypeDefDecl) {
        self.write_indent();
        if t.is_pub { self.out.push_str("pub "); }
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
                          if i > 0 { self.out.push_str(", "); }
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
        if f.is_pub { self.out.push_str("pub "); }
        if f.is_async { self.out.push_str("async "); }
        self.out.push_str("fn ");
        self.out.push_str(&f.name);
        self.print_fn_body(&f.params, &f.return_type, &f.body);
    }

    fn print_fn_body(&mut self, params: &[Param], ret: &Option<TypeExpr>, body: &[Stmt]) {
        self.out.push('(');
        for (i, p) in params.iter().enumerate() {
            if i > 0 { self.out.push_str(", "); }
            self.out.push_str(&p.name);
            if let Some(ref ty) = p.type_ann {
                self.out.push_str(": ");
                self.print_type(ty);
            }
        }
        self.out.push(')');

        if let Some(ref r) = ret {
            self.out.push_str(" to ");
            self.print_type(r);
        }

        if body.is_empty() {
            self.out.push_str(":\n");
            self.indent();
            self.write_indent();
            self.out.push_str("pass\n");
            self.dedent();
        } else {
            self.out.push_str(":\n");
            self.indent();
            for s in body {
                self.print_stmt(s);
            }
            self.dedent();
        }
    }

    fn print_stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { pattern, type_ann, value, mutable, .. } => {
                self.write_indent();
                self.out.push_str(if *mutable { "let mut " } else { "let " });
                self.print_pattern(pattern);
                if let Some(ref ty) = type_ann {
                    self.out.push_str(": ");
                    self.print_type(ty);
                }
                self.out.push_str(" = ");
                self.print_expr(value);
                self.out.push('\n');
            }
            Stmt::Assign { target, value, .. } => {
                self.write_indent();
                self.print_expr(target);
                self.out.push_str(" = ");
                self.print_expr(value);
                self.out.push('\n');
            }
            Stmt::Return { value, .. } => {
                self.write_indent();
                self.out.push_str("ret");
                if let Some(ref e) = value {
                    self.out.push(' ');
                    self.print_expr(e);
                }
                self.out.push('\n');
            }
            Stmt::Expr { expr, .. } => {
                self.write_indent();
                self.print_expr(expr);
                self.out.push('\n');
            }
            Stmt::For { binding, index_binding, iterable, body, .. } => {
                self.write_indent();
                if let Some(idx) = index_binding {
                    self.out.push_str(&format!("for {}, ", idx));
                } else {
                    self.out.push_str("for ");
                }
                self.print_pattern(binding);
                self.out.push_str(" in ");
                self.print_expr(iterable);
                self.out.push_str(":\n");
                self.indent();
                for s in body { self.print_stmt(s); }
                self.dedent();
            }
            Stmt::Break { value, .. } => {
                self.write_indent();
                if let Some(ref v) = value {
                    self.out.push_str("break ");
                    self.print_expr(v);
                    self.out.push('\n');
                } else {
                    self.out.push_str("break\n");
                }
            }
            Stmt::Continue { .. } => {
                self.write_indent();
                self.out.push_str("continue\n");
            }
            Stmt::Emit { value, .. } => {
                self.write_indent();
                self.out.push_str("emit ");
                self.print_expr(value);
                self.out.push('\n');
            }
            _ => {}
        }
    }

    fn print_expr(&mut self, e: &Expr) {
        match e {
            Expr::IntLit { value, .. } => self.out.push_str(&value.to_string()),
            Expr::FloatLit { value, .. } => self.out.push_str(&value.to_string()),
            Expr::StringLit { value, .. } => self.out.push_str(&format!("\"{}\"", value)),
            Expr::BoolLit { value, .. } => self.out.push_str(if *value { "true" } else { "false" }),
            Expr::Ident { name, .. } => self.out.push_str(name),
            Expr::Call { callee, args, .. } => {
                self.print_expr(callee);
                self.out.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { self.out.push_str(", "); }
                    if let Some(ref n) = arg.name {
                        self.out.push_str(n);
                        self.out.push_str(": ");
                    }
                    self.print_expr(&arg.value);
                }
                self.out.push(')');
            }
            Expr::Binary { op, left, right, .. } => {
                self.print_expr(left);
                self.out.push(' ');
                self.print_binop(*op);
                self.out.push(' ');
                self.print_expr(right);
            }
            Expr::If { condition, then_body, else_body, .. } => {
                self.out.push_str("if ");
                self.print_expr(condition);
                self.out.push_str(":\n");
                self.indent();
                for s in then_body { self.print_stmt(s); }
                self.dedent();
                if let Some(else_stmts) = else_body {
                    self.write_indent();
                    self.out.push_str("else:\n");
                    self.indent();
                    for s in else_stmts { self.print_stmt(s); }
                    self.dedent();
                }
            }
            Expr::Match { subject, arms, .. } => {
                self.out.push_str("match ");
                self.print_expr(subject);
                self.out.push_str(":\n");
                self.indent();
                for arm in arms {
                    self.write_indent();
                    self.print_pattern(&arm.pattern);
                    self.out.push_str(" -> ");
                    self.print_expr(&arm.body);
                    self.out.push('\n');
                }
                self.dedent();
            }
            _ => self.out.push_str("..."),
        }
    }

    fn print_binop(&mut self, op: BinOp) {
        self.out.push_str(match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Is => "is",
            BinOp::Isnt => "isnt",
            _ => "??",
        });
    }

    fn print_type(&mut self, t: &TypeExpr) {
        match t {
            TypeExpr::Named { name, .. } => self.out.push_str(name),
            TypeExpr::Generic { name, args, .. } => {
                self.out.push_str(name);
                self.out.push('[');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { self.out.push_str(", "); }
                    self.print_type(a);
                }
                self.out.push(']');
            }
            _ => self.out.push_str("Any"),
        }
    }

    fn print_pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Ident { name, .. } => self.out.push_str(name),
            Pattern::Wildcard { .. } => self.out.push('_'),
            _ => self.out.push_str("pat"),
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
                if i > 0 { self.out.push_str(", "); }
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
                if i > 0 { self.out.push_str(", "); }
                self.out.push_str(&p.to_string());
            }
            self.out.push_str("]\n");
        }

        if !env.volumes.is_empty() {
            self.write_indent();
            self.out.push_str("volumes: [");
            for (i, v) in env.volumes.iter().enumerate() {
                if i > 0 { self.out.push_str(", "); }
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
                if i > 0 { self.out.push_str(", "); }
                self.out.push_str(&format!("\"{}\"", cmd));
            }
            self.out.push_str("]\n");
        }

        if !env.cmd.is_empty() {
            self.write_indent();
            self.out.push_str("cmd: [");
            for (i, c) in env.cmd.iter().enumerate() {
                if i > 0 { self.out.push_str(", "); }
                self.out.push_str(&format!("\"{}\"", c));
            }
            self.out.push_str("]\n");
        }

        self.dedent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert formatting is idempotent: format(format(x)) == format(x)
    fn assert_idempotent(source: &str) {
        let once = format(source);
        let twice = format(&once);
        assert_eq!(once, twice, "Formatting is not idempotent for:\n{source}");
    }

    #[test]
    fn test_format_environment() {
        let source = r#"
@environment production:
    base: "node:22-alpine"
    workdir: "/app"
    packages: ["curl"]
    env:
        NODE_ENV: "production"
    expose: [3000]
    volumes: ["/data"]
    copy:
        "nginx.conf" to "/etc/nginx/nginx.conf"
    run: ["echo 'Building...'"]
    cmd: ["npm", "start"]
"#;
        let formatted = format(source);
        let expected = r#"@environment production:
    base: "node:22-alpine"
    workdir: "/app"
    packages: ["curl"]
    env:
        NODE_ENV: "production"
    expose: [3000]
    volumes: ["/data"]
    copy:
        "nginx.conf" to "/etc/nginx/nginx.conf"
    run: ["echo 'Building...'"]
    cmd: ["npm", "start"]
"#;
        assert_eq!(formatted.trim(), expected.trim());
    }

    #[test]
    fn idempotent_simple_fn() {
        assert_idempotent("fn add(x: int, y: int) to int:\n    ret x + y\n");
    }

    #[test]
    fn idempotent_table_decl() {
        assert_idempotent(
            "@table type Note:\n    title: str\n    content: str\n    created_at: str\n",
        );
    }

    #[test]
    fn idempotent_server_fn() {
        assert_idempotent(
            "@server fn greet(name: str) to str:\n    ret \"hello\"\n",
        );
    }

    #[test]
    fn idempotent_query_fn() {
        assert_idempotent(
            "@query fn list_items() to list[Item]:\n    ret []\n",
        );
    }

    #[test]
    fn idempotent_mutation_fn() {
        assert_idempotent(
            "@mutation fn add_item(name: str) to Result[str]:\n    ret Ok(name)\n",
        );
    }

    #[test]
    fn idempotent_import() {
        assert_idempotent("import react.use_state\n");
    }

    #[test]
    fn idempotent_const() {
        assert_idempotent("const MAX: int = 100\n");
    }

    #[test]
    fn idempotent_for_loop() {
        assert_idempotent(
            "fn process(items: list[str]) to int:\n    for item in items:\n        ret 0\n    ret 1\n",
        );
    }

    #[test]
    fn idempotent_workflow() {
        assert_idempotent(
            "workflow my_flow(input: str) to Result[str]:\n    ret Ok(input)\n",
        );
    }

    #[test]
    fn idempotent_actor() {
        assert_idempotent(
            "actor Counter:\n    on increment(n: int) to int:\n        ret n\n\n",
        );
    }

    #[test]
    fn idempotent_routes() {
        assert_idempotent("routes:\n    \"/\" to Home\n    \"/about\" to About\n");
    }

    #[test]
    fn table_uses_colon_syntax() {
        let out = format("@table type User:\n    name: str\n    age: int\n");
        assert!(out.contains("@table type User:"), "expected colon block, got: {out}");
        assert!(!out.contains('{'), "must not use brace syntax, got: {out}");
    }

    #[test]
    fn format_invalid_source_returns_original() {
        let broken = "fn ():\n    !!!";
        let out = format(broken);
        assert_eq!(out, broken, "Invalid source should be returned unchanged");
    }
}
