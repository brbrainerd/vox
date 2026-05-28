use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};
use regex::Regex;
use std::path::Path;

/// Detects modules/files that are declared but never imported or referenced.
///
/// Catches the classic AI pattern: create a helper module, forget to wire it in.
#[allow(dead_code)]
pub struct UnwiredModuleDetector {
    rust_mod_decl: Regex,
    rust_use_stmt: Regex,
    rust_include: Regex,
    rust_path_attr: Regex,
    ts_export_re: Regex,
}

impl Default for UnwiredModuleDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl UnwiredModuleDetector {
    /// Regexes for `mod foo;` vs `use crate::foo` and TS `export` declarations for wiring checks.
    pub fn new() -> Self {
        Self {
            // File-backed modules: private `mod foo;` plus `pub`, `pub(crate)`, `pub(super)`, `pub(in ...)`.
            rust_mod_decl: Regex::new(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;")
                .expect("valid rust mod decl regex"),
            rust_use_stmt: Regex::new(r"\buse\s+(?:crate|super|self)::(\w+)").expect("valid regex"),
            rust_include: Regex::new(r#"include!\(\s*"([^"]+)"\s*\)"#).expect("include regex"),
            rust_path_attr: Regex::new(r#"#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]"#)
                .expect("path attr regex"),
            ts_export_re: Regex::new(
                r"export\s+(?:default\s+)?(?:function|class|const|let|type|interface|enum)\s+(\w+)",
            )
            .expect("valid regex"),
        }
    }

    /// `include!(\"…\")` bodies are not in `file.content` on disk; merge them (one hop, capped reads)
    /// so `foo::bar` references inside includes count as wiring `mod foo;`.
    fn rust_content_with_includes(rust_include: &Regex, path: &Path, content: &str) -> String {
        let mut out = content.to_string();
        let Some(parent) = path.parent() else {
            return out;
        };
        for caps in rust_include.captures_iter(content) {
            let Some(rel) = caps.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let inc_path = parent.join(rel);
            if let Ok(body) = vox_bounded_fs::read_utf8_path_capped(&inc_path) {
                out.push('\n');
                out.push_str(&body);
            }
        }
        out
    }

    fn preceding_has_cfg_test(lines: &[String], mod_line_idx: usize) -> bool {
        let mut i = mod_line_idx;
        while i > 0 {
            i -= 1;
            let t = lines[i].trim();
            if t.is_empty() {
                continue;
            }
            if t.starts_with("#[") && t.contains("cfg(test)") {
                return true;
            }
            if !t.starts_with("#[") {
                break;
            }
        }
        false
    }

    /// `mod foo;` resolves to an on-disk module if any canonical layout matches.
    fn module_backing_exists(
        base: &Path,
        declaring_file: &Path,
        mod_name: &str,
        path_override: Option<&str>,
    ) -> bool {
        if let Some(rel) = path_override {
            return base.join(rel).is_file();
        }
        let n = mod_name;
        let mut paths = vec![base.join(format!("{n}.rs")), base.join(n).join("mod.rs")];
        let fname = declaring_file.file_name().and_then(|s| s.to_str());
        if fname != Some("mod.rs")
            && let Some(stem) = declaring_file.file_stem().and_then(|s| s.to_str())
            && stem != "lib"
        {
            paths.push(base.join(stem).join(format!("{n}.rs")));
            paths.push(base.join(stem).join(n).join("mod.rs"));
        }
        paths.iter().any(|p| p.is_file())
    }

    fn detect_rust(&self, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();
        let blob = Self::rust_content_with_includes(&self.rust_include, &file.path, &file.content);

        // Collect all `mod name;` declarations
        let mut declared_mods: Vec<(String, usize)> = Vec::new();
        // Collect all `use crate::name` / `use super::name` references
        let mut used_mods: Vec<String> = Vec::new();

        let base = file.path.parent().unwrap_or_else(|| Path::new("."));
        let mut pending_path: Option<String> = None;

        for (i, line) in file.lines.iter().enumerate() {
            let trim_line = line.trim();
            if trim_line.starts_with("#[") && trim_line.contains("path") {
                if let Some(caps) = self.rust_path_attr.captures(trim_line)
                    && let Some(m) = caps.get(1)
                {
                    pending_path = Some(m.as_str().to_string());
                }
                continue;
            }
            if let Some(caps) = self.rust_mod_decl.captures(line)
                && let Some(name) = caps.get(1)
            {
                // Removed early return for 'pub mod' to enforce reachability checks on public exports.
                // We rely on workspace_crate_refs_mod to ensure they are used elsewhere.
                if line.trim_start().starts_with("pub") {
                    // pending_path = None; // Still consume path attr if needed? Yes, below logic handles it.
                }
                // `#[cfg(test)] mod tests;` (possibly after `#[path = "..."]`) — not referenced in-lib.
                if name.as_str() == "tests" && Self::preceding_has_cfg_test(&file.lines, i) {
                    pending_path = None;
                    continue;
                }
                let n = name.as_str();
                let po = pending_path.take();
                if Self::module_backing_exists(base, &file.path, n, po.as_deref()) {
                    continue;
                }
                declared_mods.push((n.to_string(), i + 1));
            }
            for caps in self.rust_use_stmt.captures_iter(line) {
                if let Some(name) = caps.get(1) {
                    used_mods.push(name.as_str().to_string());
                }
            }
        }
        for line in blob.lines() {
            for caps in self.rust_use_stmt.captures_iter(line) {
                if let Some(name) = caps.get(1) {
                    used_mods.push(name.as_str().to_string());
                }
            }
        }

        // Also check for inline `name::` usage patterns
        for (mod_name, line_num) in &declared_mods {
            // Check if `mod_name` appears anywhere in the file as `mod_name::` or in a `use`
            let mut is_used = used_mods.contains(mod_name)
                || file.lines.iter().enumerate().any(|(j, line)| {
                    j + 1 != *line_num // skip the declaration line itself
                    && (line.contains(&format!("{}::", mod_name))
                        || line.contains(&format!("use {}", mod_name))
                        || line.contains(&format!("{mod_name} as ")))
                })
                // `include!(...)` text is merged into `blob` so sibling `mod foo;` + `foo::` in inc counts.
                || blob.contains(&format!("{}::", mod_name));
            if crate::run_context::feature_enabled("unwired-graph") {
                let z = mod_name.as_str();
                if blob.contains(&format!("crate::{z}::")) || blob.contains(&format!("crate::{z};"))
                {
                    is_used = true;
                }
            }
            if crate::run_context::workspace_crate_refs_mod(&file.path, mod_name.as_str()) {
                is_used = true;
            }

            if !is_used {
                findings.push(Finding {
                    rule_id: "unwired/module".to_string(),
                    diagnostic_id: None,
                    rule_name: self.name().to_string(),
                    severity: self.severity(),
                    file: file.path.clone(),
                    line: *line_num,
                    column: 0,
                    message: format!(
                        "Module `{}` is declared but never referenced in this file",
                        mod_name
                    ),
                    suggestion: Some(format!(
                        "Add `use crate::{}::...;` or remove the module declaration if unused.",
                        mod_name
                    )),
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(*line_num, 1),
                    confidence: None,
                    evidence: None,
                });
            }
        }

        findings
    }
}

impl DetectionRule for UnwiredModuleDetector {
    fn id(&self) -> &'static str {
        "arch/unwired"
    }
    fn name(&self) -> &'static str {
        "Unwired Module Detector"
    }
    fn description(&self) -> &'static str {
        "Detects modules declared but never imported or referenced"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — module declared but never imported elsewhere\n\
             // In lib.rs:\n\
             mod analytics;  // declared — but nothing imports it!\n\
             mod payments;   // declared and used ✓\n\
             \n\
             // FIX — either use the module or remove the declaration\n\
             mod analytics;   // now used in routes\n\
             pub use analytics::track_event;  // wired in",
        )
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        match file.language {
            Language::Rust => self.detect_rust(file),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
    }

    #[test]
    fn detects_unwired_mod() {
        let d = UnwiredModuleDetector::new();
        let f = source(
            "rs",
            "mod helpers;\nmod utils;\n\nfn main() {\n    helpers::do_thing();\n}",
        );
        let findings = d.detect(&f, None);
        // `utils` is declared but never used, `helpers` is used via `helpers::do_thing()`
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("utils"));
    }

    #[test]
    fn no_findings_when_all_used() {
        let d = UnwiredModuleDetector::new();
        let f = source(
            "rs",
            "mod engine;\nmod rules;\n\nuse crate::engine::Engine;\nuse crate::rules::Rule;\n",
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn no_findings_when_wired_as_underscore() {
        let d = UnwiredModuleDetector::new();
        let f = source(
            "rs",
            "mod helpers;\nuse self::helpers as _;\n\nfn main() {}\n",
        );
        assert!(d.detect(&f, None).is_empty());
    }

    #[test]
    fn pub_mods_produce_findings_if_unwired() {
        let d = UnwiredModuleDetector::new();
        let f = source(
            "rs",
            "pub mod alpha;\npub mod beta;\npub(crate) mod gamma;\n",
        );
        assert_eq!(
            d.detect(&f, None).len(),
            3,
            "public module declarations are now checked for wiring"
        );
    }

    #[test]
    fn skips_path_attr_backed_mod() {
        let root = std::env::temp_dir().join(format!("vox_unwired_path_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("expr.rs"), "// expr module\n").unwrap();
        let lower = root.join("lower.rs");
        std::fs::write(
            &lower,
            "#[path = \"expr.rs\"]\nmod lowering_expr;\nfn _x() { lowering_expr::marker(); }\n",
        )
        .unwrap();
        let f = SourceFile::new(lower.clone(), std::fs::read_to_string(&lower).unwrap());
        let d = UnwiredModuleDetector::new();
        assert!(
            d.detect(&f, None).is_empty(),
            "path = points at real file — not unwired"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_stem_subdirectory_file_backed_mod() {
        let root = std::env::temp_dir().join(format!("vox_unwired_stem_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("orchestrator")).unwrap();
        std::fs::write(root.join("orchestrator").join("agent.rs"), "\n").unwrap();
        let pf = root.join("orchestrator.rs");
        std::fs::write(&pf, "mod agent;\n").unwrap();
        let f = SourceFile::new(pf.clone(), std::fs::read_to_string(&pf).unwrap());
        let d = UnwiredModuleDetector::new();
        assert!(
            d.detect(&f, None).is_empty(),
            "orchestrator/agent.rs backs `mod agent` in orchestrator.rs"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_cfg_test_mod_tests_with_path_attr() {
        let d = UnwiredModuleDetector::new();
        let f = source(
            "rs",
            "#[cfg(test)]\n#[path = \"snap_tests.rs\"]\nmod tests;\n",
        );
        assert!(d.detect(&f, None).is_empty());
    }
}
