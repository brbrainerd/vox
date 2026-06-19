//! Build script for `vox-corpus`.
//!
//! Dynamically walks the `vox-compiler` AST source tree via `syn` to extract every
//! enum variant from `Expr`, `BinOp`, `UnOp`, `TypeExpr`, `Pattern`, `Stmt`,
//! and `Decl`. These are emitted as `&[&str]` constants into `dynamic_registry.rs`,
//! which `codegen_vox.rs` and `coverage.rs` consume.
//!
//! When any vox-ast enum changes, these constants auto-update on next build.

use std::env;
use std::fs;
use std::path::PathBuf;
use syn::visit::{self, Visit};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert `CamelCase` to `snake_case`.
fn to_snake(name: &str) -> String {
    name.chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i > 0 {
                format!("_{}", c.to_lowercase())
            } else {
                c.to_lowercase().to_string()
            }
        })
        .collect()
}

/// Generic visitor: extract variant names from a named enum.
struct EnumVariantVisitor {
    target: &'static str,
    variants: Vec<String>,
    use_snake: bool,
}

impl<'ast> Visit<'ast> for EnumVariantVisitor {
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        if i.ident == self.target {
            for v in &i.variants {
                let raw = v.ident.to_string();
                // Check for #[serde(rename = "...")] — use that if present
                let mut name = raw.clone();
                for attr in &v.attrs {
                    if attr.path().is_ident("serde") {
                        let attr_str = quote::quote!(#attr).to_string();
                        if let Some(start) = attr_str.find("rename = \"") {
                            let start = start + 10;
                            if let Some(end) = attr_str[start..].find('"') {
                                name = attr_str[start..start + end].to_string();
                            }
                        }
                    }
                }
                // If no rename found, optionally snake-case the CamelCase name
                if name == raw && self.use_snake {
                    name = to_snake(&raw);
                }
                self.variants.push(name);
            }
        }
        visit::visit_item_enum(self, i);
    }
}

/// Parse a Rust source file and extract all variant names from the named enum.
fn walk_enum(source_path: &PathBuf, enum_name: &'static str, use_snake: bool) -> Vec<String> {
    let source = match fs::read_to_string(source_path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let file = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut visitor = EnumVariantVisitor {
        target: enum_name,
        variants: vec![],
        use_snake,
    };
    visit::visit_file(&mut visitor, &file);
    visitor.variants.sort();
    visitor.variants.dedup();
    visitor.variants
}

// ── CLI enum visitor (extracts name + doc comment) ────────────────────────────

struct CliVisitor {
    commands: Vec<(String, String)>,
}

impl<'ast> Visit<'ast> for CliVisitor {
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        if i.ident == "Cli" {
            for v in &i.variants {
                let snake = to_snake(&v.ident.to_string());
                let mut desc = String::new();
                for attr in &v.attrs {
                    if attr.path().is_ident("doc")
                        && let syn::Meta::NameValue(nv) = &attr.meta
                        && let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) = &nv.value
                    {
                        let text = s.value();
                        let text = text.trim();
                        if !text.is_empty() {
                            if !desc.is_empty() {
                                desc.push(' ');
                            }
                            desc.push_str(text);
                        }
                    }
                }
                if desc.is_empty() {
                    desc = format!("{snake} command");
                }
                // Truncate at first parenthesis for cleaner training data
                if let Some(pos) = desc.find('(') {
                    desc.truncate(pos);
                    desc = desc.trim_end().to_string();
                }
                self.commands.push((snake, desc));
            }
        }
        visit::visit_item_enum(self, i);
    }
}

// ── A2A visitor (serde-renamed unit variants) ─────────────────────────────────

struct A2AVisitor {
    variants: Vec<String>,
}

impl<'ast> Visit<'ast> for A2AVisitor {
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        if i.ident == "A2AMessageType" {
            for v in &i.variants {
                if let syn::Fields::Unit = v.fields {
                    let mut name = v.ident.to_string();
                    for attr in &v.attrs {
                        if attr.path().is_ident("serde") {
                            let attr_str = quote::quote!(#attr).to_string();
                            if let Some(start) = attr_str.find("rename = \"") {
                                let start = start + 10;
                                if let Some(end) = attr_str[start..].find('"') {
                                    name = attr_str[start..start + end].to_string();
                                }
                            }
                        }
                    }
                    if v.ident == name {
                        name = to_snake(&name);
                    }
                    self.variants.push(name);
                }
            }
        }
        visit::visit_item_enum(self, i);
    }
}

// ── MCP tool visitor ──────────────────────────────────────────────────────────

struct ToolRegistryVisitor {
    tools: Vec<String>,
}

impl<'ast> Visit<'ast> for ToolRegistryVisitor {
    fn visit_lit_str(&mut self, i: &'ast syn::LitStr) {
        let val = i.value();
        if val.starts_with("vox_")
            && val
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        {
            self.tools.push(val);
        }
        visit::visit_lit_str(self, i);
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(&out_dir).join("dynamic_registry.rs");

    // ── Watch triggers ────────────────────────────────────────────────────────
    for rel in &[
        "../vox-ast/src/expr.rs",
        "../vox-ast/src/types.rs",
        "../vox-ast/src/pattern.rs",
        "../vox-ast/src/stmt.rs",
        "../vox-ast/src/decl/types.rs",
        "../vox-cli/src/lib.rs",
        "../vox-orchestrator/src/types.rs",
        "../vox-mcp-registry/src/lib.rs",
        "../vox-mcp/src/tools/",
    ] {
        println!("cargo:rerun-if-changed={rel}");
    }

    // ── 1. Walk vox-ast enum files ────────────────────────────────────────────

    // expr.rs contains: Expr (snake), BinOp (as-is), UnOp (as-is)
    let expr_file = manifest_dir.join("../vox-ast/src/expr.rs");
    let expr_variants = walk_enum(&expr_file, "Expr", true); // snake_case names
    let binop_variants = walk_enum(&expr_file, "BinOp", false); // keep original
    let unop_variants = walk_enum(&expr_file, "UnOp", false);

    // types.rs: TypeExpr
    let types_file = manifest_dir.join("../vox-ast/src/types.rs");
    let type_variants = walk_enum(&types_file, "TypeExpr", false);

    // pattern.rs: Pattern
    let pattern_file = manifest_dir.join("../vox-ast/src/pattern.rs");
    let pattern_variants = walk_enum(&pattern_file, "Pattern", false);

    // stmt.rs: Stmt
    let stmt_file = manifest_dir.join("../vox-ast/src/stmt.rs");
    let stmt_variants = walk_enum(&stmt_file, "Stmt", false);

    // decl/types.rs: Decl → snake_case TAXONOMY
    let decl_file = manifest_dir.join("../vox-ast/src/decl/types.rs");
    let taxonomy = walk_enum(&decl_file, "Decl", true);

    // ── 2. Walk CLI enum ──────────────────────────────────────────────────────
    let cli_commands = {
        let cli_path = manifest_dir.join("../vox-cli/src/lib.rs");
        let source = fs::read_to_string(&cli_path).unwrap_or_default();
        let file = syn::parse_file(&source).unwrap_or_else(|_| syn::parse_str("").unwrap());
        let mut v = CliVisitor { commands: vec![] };
        visit::visit_file(&mut v, &file);
        if v.commands.is_empty() {
            default_cli_commands()
        } else {
            v.commands
        }
    };

    // ── 3. Walk A2A types ─────────────────────────────────────────────────────
    let a2a = {
        let path = manifest_dir.join("../vox-orchestrator/src/types.rs");
        if path.exists() {
            let src = fs::read_to_string(&path).unwrap_or_default();
            if let Ok(file) = syn::parse_file(&src) {
                let mut v = A2AVisitor { variants: vec![] };
                visit::visit_file(&mut v, &file);
                if v.variants.is_empty() {
                    default_a2a()
                } else {
                    v.variants
                }
            } else {
                default_a2a()
            }
        } else {
            default_a2a()
        }
    };

    // ── 4. Walk MCP tool registry ─────────────────────────────────────────────
    let mcp_tools = {
        let mut tools = vec![];
        for path in &[
            manifest_dir.join("../vox-mcp-registry/src/lib.rs"),
            manifest_dir.join("../vox-mcp/src/tools/input_schemas.rs"),
        ] {
            if path.exists() {
                let src = fs::read_to_string(path).unwrap_or_default();
                if let Ok(file) = syn::parse_file(&src) {
                    let mut v = ToolRegistryVisitor { tools: vec![] };
                    visit::visit_file(&mut v, &file);
                    tools.extend(v.tools);
                }
            }
        }
        let mut t = if tools.is_empty() {
            default_mcp_tools()
        } else {
            tools
        };
        t.sort();
        t.dedup();
        t
    };

    // ── 5. Emit dynamic_registry.rs ──────────────────────────────────────────
    let mut out = String::from("// AUTO-GENERATED by vox-corpus/build.rs — DO NOT EDIT\n");
    out.push_str("// Regenerated whenever any vox-ast enum file changes.\n\n");

    emit_str_slice(
        &mut out,
        "TAXONOMY_FROM_AST",
        &taxonomy,
        "Auto-derived from `vox-ast` `Decl` enum variants (snake_case).",
    );
    emit_str_slice(
        &mut out,
        "EXPR_VARIANTS",
        &expr_variants,
        "Auto-derived from `vox-ast` `Expr` enum variants (snake_case).",
    );
    emit_str_slice(
        &mut out,
        "BINOP_VARIANTS",
        &binop_variants,
        "Auto-derived from `vox-ast` `BinOp` enum variants (CamelCase).",
    );
    emit_str_slice(
        &mut out,
        "UNOP_VARIANTS",
        &unop_variants,
        "Auto-derived from `vox-ast` `UnOp` enum variants (CamelCase).",
    );
    emit_str_slice(
        &mut out,
        "TYPE_EXPR_VARIANTS",
        &type_variants,
        "Auto-derived from `vox-ast` `TypeExpr` enum variants (CamelCase).",
    );
    emit_str_slice(
        &mut out,
        "PATTERN_VARIANTS",
        &pattern_variants,
        "Auto-derived from `vox-ast` `Pattern` enum variants (CamelCase).",
    );
    emit_str_slice(
        &mut out,
        "STMT_VARIANTS",
        &stmt_variants,
        "Auto-derived from `vox-ast` `Stmt` enum variants (CamelCase).",
    );
    emit_str_slice(
        &mut out,
        "A2A_MESSAGE_TYPES",
        &a2a,
        "Auto-derived from `vox-orchestrator` `A2AMessageType` variants.",
    );
    emit_str_slice(
        &mut out,
        "TOOL_REGISTRY_SLIM",
        &mcp_tools,
        "Auto-derived from `vox-mcp-registry` tool registry.",
    );

    // CLI commands: (name, description) tuples
    out.push_str("/// CLI subcommands auto-derived from `vox-cli` `Cli` enum.\n");
    out.push_str("pub const CLI_COMMANDS: &[(&str, &str)] = &[\n");
    for (cmd, desc) in &cli_commands {
        out.push_str(&format!("    (\"{cmd}\", \"{desc}\"),\n"));
    }
    out.push_str("];\n\n");

    // Totals — used for coverage scoring without hardcoding counts
    out.push_str(&format!(
        "/// Total AST surface counts — used by coverage reporter.\n\
         pub const AST_EXPR_TOTAL: usize = {};\n\
         pub const AST_BINOP_TOTAL: usize = {};\n\
         pub const AST_UNOP_TOTAL: usize = {};\n\
         pub const AST_TYPE_EXPR_TOTAL: usize = {};\n\
         pub const AST_PATTERN_TOTAL: usize = {};\n\
         pub const AST_STMT_TOTAL: usize = {};\n\
         pub const AST_DECL_TOTAL: usize = {};\n",
        expr_variants.len(),
        binop_variants.len(),
        unop_variants.len(),
        type_variants.len(),
        pattern_variants.len(),
        stmt_variants.len(),
        taxonomy.len(),
    ));

    fs::write(&dest_path, out).unwrap();
}

fn emit_str_slice(out: &mut String, name: &str, items: &[String], doc: &str) {
    out.push_str(&format!("/// {doc}\npub const {name}: &[&str] = &[\n"));
    for item in items {
        out.push_str(&format!("    \"{item}\",\n"));
    }
    out.push_str("];\n\n");
}

// ── Fallback defaults ─────────────────────────────────────────────────────────

fn default_cli_commands() -> Vec<(String, String)> {
    vec![
        ("build", "Compile a .vox file"),
        ("check", "Type-check a .vox file"),
        ("run", "Build and run a .vox application"),
        ("test", "Run Vox test declarations"),
        ("mens", "Mens training and inference"),
        ("ci", "CI guards and checks"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect()
}

fn default_a2a() -> Vec<String> {
    [
        "task_started",
        "task_completed",
        "delegation_request",
        "delegation_response",
        "knowledge_share",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_mcp_tools() -> Vec<String> {
    ["vox_search_web", "vox_view_file", "vox_skill_install"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}
