use crate::extractor::LanguageExtractor;
use crate::features::{
    CallSite, ExtractedFeatures, ImportLoc, LiteralContext, LiteralLoc, Loc, NumericLoc,
};
use anyhow::Result;
use std::path::Path;
use swc_common::{FileName, SourceMap, sync::Lrc};
use swc_ecma_ast::{Callee, Expr, Lit, Module, ModuleDecl, ModuleItem};
use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax, lexer::Lexer};
use swc_ecma_visit::{Visit, VisitWith};
use vox_code_audit::rules::Language;

pub struct TypeScriptExtractor;

struct TsVisitor {
    features: ExtractedFeatures,
}

impl Visit for TsVisitor {
    fn visit_lit(&mut self, node: &Lit) {
        match node {
            Lit::Str(s) => {
                self.features.string_literals.push(LiteralLoc {
                    value: s.value.as_str().unwrap_or_default().to_string(),
                    loc: Loc::default(),
                    ctx: LiteralContext::Code,
                });
            }
            Lit::Num(n) => {
                self.features.numeric_literals.push(NumericLoc {
                    value: n.value,
                    unit: None,
                    loc: Loc::default(),
                });
            }
            _ => {}
        }
    }

    fn visit_call_expr(&mut self, node: &swc_ecma_ast::CallExpr) {
        if let Callee::Expr(expr) = &node.callee
            && let Expr::Member(m) = expr.as_ref()
        {
            let mut path = Vec::new();
            collect_member_path(&m.obj, &mut path);
            if let swc_ecma_ast::MemberProp::Ident(id) = &m.prop {
                path.push(id.sym.as_str().to_string());
            }
            if !path.is_empty() {
                self.features.call_sites.push(CallSite {
                    path,
                    arity: node.args.len() as u8,
                    loc: Loc::default(),
                });
            }
        }
        node.visit_children_with(self);
    }
}

fn collect_member_path(expr: &Expr, path: &mut Vec<String>) {
    match expr {
        Expr::Member(m) => {
            collect_member_path(&m.obj, path);
            if let swc_ecma_ast::MemberProp::Ident(id) = &m.prop {
                path.push(id.sym.as_str().to_string());
            }
        }
        Expr::Ident(id) => {
            path.push(id.sym.as_str().to_string());
        }
        _ => {}
    }
}

fn extract_imports(module: &Module, features: &mut ExtractedFeatures) {
    for item in &module.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = item {
            let module_path = import.src.value.as_str().unwrap_or_default().to_string();
            for spec in &import.specifiers {
                let symbol = match spec {
                    swc_ecma_ast::ImportSpecifier::Named(n) => Some(n.local.sym.to_string()),
                    swc_ecma_ast::ImportSpecifier::Default(d) => Some(d.local.sym.to_string()),
                    swc_ecma_ast::ImportSpecifier::Namespace(ns) => Some(ns.local.sym.to_string()),
                };
                features.imports.push(ImportLoc {
                    path: module_path.split('/').map(String::from).collect(),
                    symbol,
                    loc: Loc::default(),
                });
            }
        }
    }
}

impl LanguageExtractor for TypeScriptExtractor {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures> {
        let is_tsx = path.extension().is_some_and(|e| e == "tsx" || e == "jsx");
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(
            FileName::Real(path.to_path_buf()).into(),
            content.to_string(),
        );

        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax {
                tsx: is_tsx,
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );
        let mut parser = Parser::new_from(lexer);

        let mut features = ExtractedFeatures::new(path.to_path_buf(), Language::TypeScript);
        features.crate_name = crate::extractor::crate_name_from_path(path);

        if let Ok(module) = parser.parse_module() {
            extract_imports(&module, &mut features);
            let mut visitor = TsVisitor { features };
            module.visit_with(&mut visitor);
            Ok(visitor.features)
        } else {
            Ok(features)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &str) -> ExtractedFeatures {
        TypeScriptExtractor
            .extract(Path::new("test.ts"), src)
            .unwrap()
    }

    #[test]
    fn extracts_string_literal_ts() {
        let f = extract(r#"const x = "hello world";"#);
        assert!(f.string_literals.iter().any(|l| l.value == "hello world"));
    }

    #[test]
    fn extracts_import_path() {
        let f = extract(r#"import { foo } from "some-module";"#);
        assert!(f.imports.iter().any(|i| i.symbol.as_deref() == Some("foo")));
    }
}
