use crate::jsx::emit_expr;
use vox_ast::decl::{Decl, HttpMethod, HttpRouteDecl, Module, ServerFnDecl};

/// Generate Express.js route handlers from Vox http route and @server fn declarations.
pub fn generate_routes(module: &Module) -> String {
    let routes: Vec<&HttpRouteDecl> = module
        .declarations
        .iter()
        .filter_map(|d| {
            if let Decl::HttpRoute(r) = d {
                Some(r)
            } else {
                None
            }
        })
        .collect();

    let server_fns: Vec<&ServerFnDecl> = module
        .declarations
        .iter()
        .filter_map(|d| {
            if let Decl::ServerFn(sf) = d {
                Some(sf)
            } else {
                None
            }
        })
        .collect();

    if routes.is_empty() && server_fns.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("import express, { Request, Response } from \"express\";\n");
    out.push_str("import cors from \"cors\";\n\n");
    out.push_str("const app = express();\n");
    out.push_str("app.use(cors());\n");
    out.push_str("app.use(express.json());\n\n");

    // Only emit mock actor when there are HTTP routes that might use actors
    if !routes.is_empty() {
        out.push_str("// Mock LLM actor (replace with real API key for production)\n");
        out.push_str("class ClaudeActor {\n");
        out.push_str("  async send(message: string): Promise<string> {\n");
        out.push_str("    const apiKey = process.env.ANTHROPIC_API_KEY;\n");
        out.push_str("    if (apiKey) {\n");
        out.push_str(
            "      const response = await fetch(\"https://api.anthropic.com/v1/messages\", {\n",
        );
        out.push_str("        method: \"POST\",\n");
        out.push_str("        headers: {\n");
        out.push_str("          \"Content-Type\": \"application/json\",\n");
        out.push_str("          \"x-api-key\": apiKey,\n");
        out.push_str("          \"anthropic-version\": \"2023-06-01\",\n");
        out.push_str("        },\n");
        out.push_str("        body: JSON.stringify({\n");
        out.push_str("          model: \"claude-sonnet-4-20250514\",\n");
        out.push_str("          max_tokens: 256,\n");
        out.push_str("          messages: [{ role: \"user\", content: message }],\n");
        out.push_str("        }),\n");
        out.push_str("      });\n");
        out.push_str("      const data = await response.json() as any;\n");
        out.push_str("      return data.content?.[0]?.text || \"No response from Claude\";\n");
        out.push_str("    }\n");
        out.push_str("    // Mock response when no API key is set\n");
        out.push_str("    return `Vox AI Echo: ${message}`;\n");
        out.push_str("  }\n");
        out.push_str("}\n\n");
    }

    for route in &routes {
        let method = match route.method {
            HttpMethod::Get => "get",
            HttpMethod::Post => "post",
            HttpMethod::Put => "put",
            HttpMethod::Delete => "delete",
        };
        let path = &route.path;

        out.push_str(&format!(
            "app.{method}(\"{path}\", async (req: Request, res: Response) => {{\n"
        ));
        out.push_str("  try {\n");
        out.push_str("    const request = req;\n");

        for stmt in &route.body {
            out.push_str(&format!("    {}", emit_route_stmt(stmt)));
        }

        out.push_str("  } catch (err) {\n");
        out.push_str("    res.status(500).json({ error: String(err) });\n");
        out.push_str("  }\n");
        out.push_str("});\n\n");
    }

    // Generate @server fn endpoints as POST /api/{name}
    for sf in &server_fns {
        let name = &sf.func.name;
        let route_path = format!("/api/{}", name);
        out.push_str(&format!(
            "app.post(\"{route_path}\", async (req: Request, res: Response) => {{\n"
        ));
        out.push_str("  try {\n");
        // Destructure params from request body
        for param in &sf.func.params {
            out.push_str(&format!(
                "    const {} = req.body.{};\n",
                param.name, param.name
            ));
        }
        // Emit function body
        for stmt in &sf.func.body {
            out.push_str(&format!("    {}", emit_route_stmt(stmt)));
        }
        out.push_str("  } catch (err) {\n");
        out.push_str("    res.status(500).json({ error: String(err) });\n");
        out.push_str("  }\n");
        out.push_str("});\n\n");
    }

    out.push_str("const PORT = process.env.PORT || 3001;\n");
    out.push_str("app.listen(PORT, () => {\n");
    out.push_str("  console.log(`Vox server running on port ${PORT}`);\n");
    out.push_str("});\n");

    out
}

/// Emit a statement in the context of a route handler.
/// Uses emit_route_expr to add `await` on actor method calls.
fn emit_route_stmt(stmt: &vox_ast::stmt::Stmt) -> String {
    match stmt {
        vox_ast::stmt::Stmt::Let {
            pattern,
            value,
            mutable,
            ..
        } => {
            let keyword = if *mutable { "let" } else { "const" };
            let pat = crate::jsx::emit_pattern_public(pattern);
            let val = emit_route_expr(value);
            format!("{keyword} {pat} = {val};\n")
        }
        vox_ast::stmt::Stmt::Return {
            value: Some(expr), ..
        } => {
            let result = emit_route_expr(expr);
            format!("const result = {result};\n    res.json({{ text: result }});\n")
        }
        vox_ast::stmt::Stmt::Return { value: None, .. } => "res.sendStatus(204);\n".to_string(),
        vox_ast::stmt::Stmt::Assign { target, value, .. } => {
            format!(
                "{} = {};\n",
                emit_route_expr(target),
                emit_route_expr(value)
            )
        }
        vox_ast::stmt::Stmt::Expr { expr, .. } => {
            format!("{};\n", emit_route_expr(expr))
        }
    }
}

/// Emit an expression in the context of a route handler.
fn emit_route_expr(expr: &vox_ast::expr::Expr) -> String {
    match expr {
        vox_ast::expr::Expr::MethodCall {
            object,
            method,
            args,
            ..
        } => {
            let obj = emit_route_expr(object);
            let args_str: Vec<String> = args.iter().map(|a| emit_expr(&a.value)).collect();
            if method == "send" {
                // spawn(Claude).send(msg) -> await new ClaudeActor().send(msg)
                format!("await {obj}.send({})", args_str.join(", "))
            } else {
                format!("{obj}.{method}({})", args_str.join(", "))
            }
        }
        vox_ast::expr::Expr::Spawn { target, .. } => {
            format!("new {}Actor()", emit_expr(target))
        }
        vox_ast::expr::Expr::FieldAccess { object, field, .. } => {
            let obj = emit_route_expr(object);
            format!("{obj}.{field}")
        }
        vox_ast::expr::Expr::Call { callee, args, .. } => {
            let callee_str = emit_route_expr(callee);
            let args_str: Vec<String> = args.iter().map(|a| emit_expr(&a.value)).collect();
            format!("{callee_str}({})", args_str.join(", "))
        }
        _ => emit_expr(expr),
    }
}
