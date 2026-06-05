//! Static import-cycle detector for Vox files.
//!
//! # Design
//!
//! The [`DetectionRule`] per-file interface only sees one file at a time, so
//! `ImportCyclesDetector::detect` handles the one case that is detectable
//! per-file: a file that directly imports itself (cycle of length 1).
//!
//! For multi-file cycles (A → B → A, or longer chains) the crate also exposes
//! [`detect_import_cycles_in_batch`], a free function that accepts the full
//! `SourceFile` slice from a workspace scan, builds the directed import graph,
//! runs iterative DFS cycle detection, and returns [`Finding`]s for each
//! import edge that closes a cycle.  The engine / CLI should call it as a
//! post-scan step when the complete file set is available.
//!
//! # Vox import syntax recognised
//!
//! ```vox
//! import "./relative/path.vox"
//! import "./relative/path.vox" as alias
//! ```
//!
//! Only relative (`.`-prefixed) paths are tracked; bare module names or
//! `@`-prefixed package imports do not participate in the local cycle graph.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};

// ---------------------------------------------------------------------------
// Shared regex (compile once in the detector, once in the free function)
// ---------------------------------------------------------------------------

/// Matches a Vox relative import statement.  Group 1 = the quoted path
/// (without quotes); group 2 = optional alias identifier.
///
/// ```text
/// import "./foo/bar.vox"
/// import "./baz.vox" as baz
/// ```
const IMPORT_RE: &str = r#"^\s*import\s+"(\./[^"]+)""#;

// ---------------------------------------------------------------------------
// Per-file detector (self-import only)
// ---------------------------------------------------------------------------

/// Detects import cycles in Vox files.
///
/// The per-file `detect` method catches **direct self-imports** (a file that
/// imports itself).  For the full multi-file cycle check call
/// [`detect_import_cycles_in_batch`] with all workspace files.
pub struct ImportCyclesDetector {
    import_re: Regex,
    supported_langs: Vec<Language>,
}

impl ImportCyclesDetector {
    pub fn new() -> Self {
        Self {
            import_re: Regex::new(IMPORT_RE).expect("valid IMPORT_RE"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl Default for ImportCyclesDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DetectionRule for ImportCyclesDetector {
    fn id(&self) -> &'static str {
        "import/cycle"
    }

    fn name(&self) -> &'static str {
        "Import Cycle Detector"
    }

    fn description(&self) -> &'static str {
        "Detects circular `import` dependencies in Vox files. \
         Self-imports are caught per-file; multi-file cycles require the batch runner."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::IMPORT_CYCLE)
    }

    fn explain(&self) -> &'static str {
        "Import cycles prevent the Vox interpreter from resolving module boundaries and \
         cause `ImportCycle` runtime errors at startup.\n\n\
         Bad — self-import:\n\
         \x20 // a.vox\n\
         \x20 import \"./a.vox\"   // imports itself\n\n\
         Bad — mutual cycle:\n\
         \x20 // a.vox\n\
         \x20 import \"./b.vox\"\n\
         \x20 // b.vox\n\
         \x20 import \"./a.vox\"   // closes the cycle\n\n\
         Good — shared module breaks the cycle:\n\
         \x20 // shared.vox  (no imports back into a.vox or b.vox)\n\
         \x20 // a.vox\n\
         \x20 import \"./shared.vox\"\n\
         \x20 // b.vox\n\
         \x20 import \"./shared.vox\""
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// a.vox — VIOLATION: file imports itself\n\
             import \"./a.vox\"\n\
             \n\
             pub fn greet() to str {\n\
             \x20   \"hello\"\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Vox {
            return vec![];
        }

        let file_name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if file_name.is_empty() {
            return vec![];
        }

        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                continue;
            }

            if let Some(caps) = self.import_re.captures(line) {
                let raw_path = caps.get(1).map_or("", |m| m.as_str());
                let import_file_name = Path::new(raw_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                if import_file_name == file_name {
                    let col = trimmed.find("import").unwrap_or(0) + 1;
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Error,
                        file: file.path.clone(),
                        line: line_num,
                        column: col,
                        message: format!(
                            "Self-import cycle: `{raw_path}` resolves to the current file `{file_name}`."
                        ),
                        suggestion: Some(
                            "Remove the self-import. A file cannot import itself.".to_string(),
                        ),
                        alternatives: vec![],
                        rationale: Some(
                            "Import cycles prevent the Vox interpreter from resolving \
                             module boundaries and cause `ImportCycle` runtime errors at startup."
                                .into(),
                        ),
                        context: file.context_around(line_num, 2),
                        confidence: Some(FindingConfidence::High),
                        evidence: None,
                    });
                }
            }
        }

        findings
    }
}

// ---------------------------------------------------------------------------
// Helpers shared by the free batch function
// ---------------------------------------------------------------------------

/// Extract all relative import paths from a Vox source file.
///
/// Returns `(line_number_1indexed, raw_path_string)` pairs for every
/// `import "./…"` statement in the file.
pub fn extract_vox_imports(file: &SourceFile) -> Vec<(usize, String)> {
    if file.language != Language::Vox {
        return vec![];
    }
    let re = Regex::new(IMPORT_RE).expect("valid IMPORT_RE");
    let mut out = Vec::new();
    for (i, line) in file.lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        if let Some(caps) = re.captures(line) {
            let raw = caps.get(1).map_or("", |m| m.as_str()).to_string();
            out.push((i + 1, raw));
        }
    }
    out
}

/// Lexically normalise a path: collapse `CurDir` (`.`) components without
/// hitting the filesystem. Does **not** collapse `..` (ParentDir) because we
/// can't safely do that without knowing the real directory tree.
fn normalize_path(path: &Path) -> PathBuf {
    path.components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect()
}

/// Resolve a relative Vox import path against the importing file's directory.
///
/// Returns `None` if `importer` has no parent directory or if the resolved
/// path escapes the root (contains `..` before any real component — we leave
/// those for a future resolver).
fn resolve_import(importer: &Path, raw_path: &str) -> Option<PathBuf> {
    let dir = importer.parent()?;
    Some(normalize_path(&dir.join(raw_path)))
}

// ---------------------------------------------------------------------------
// Batch multi-file cycle detection
// ---------------------------------------------------------------------------

/// Run import-cycle detection across a batch of Vox source files.
///
/// Builds a directed import graph (`importer → importee`) using lexical path
/// resolution, then performs an iterative DFS with white/gray/black colouring
/// to detect back edges (which imply cycles).  Returns one [`Finding`] per
/// back-edge import line that closes a cycle.
///
/// **Call this after a full workspace scan** — it needs the complete set of
/// `.vox` files to resolve and cross-reference import paths.
///
/// Non-Vox files in the slice are silently skipped.
pub fn detect_import_cycles_in_batch(files: &[SourceFile]) -> Vec<Finding> {
    // Map canonical path → files[] index (Vox files only).
    let path_to_idx: HashMap<PathBuf, usize> = files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.language == Language::Vox)
        .map(|(i, f)| (normalize_path(&f.path), i))
        .collect();

    if path_to_idx.len() < 2 {
        // Need at least two files for a multi-file cycle; self-import is
        // handled by the per-file detector.
        return vec![];
    }

    let n = files.len();

    // Build adjacency list: files[src_idx] → Vec<(target_idx, line_num, raw_path)>
    let mut edges: Vec<Vec<(usize, usize, String)>> = vec![vec![]; n];
    for (canon, &src_idx) in &path_to_idx {
        for (line_num, raw_path) in extract_vox_imports(&files[src_idx]) {
            if let Some(target_canon) = resolve_import(canon, &raw_path)
                && let Some(&tgt_idx) = path_to_idx.get(&target_canon) {
                    edges[src_idx].push((tgt_idx, line_num, raw_path));
                }
        }
    }

    // Iterative DFS with white (0) / gray (1) / black (2) colouring.
    // A gray-to-gray edge is a back edge → cycle.
    let mut color = vec![0u8; n];
    let mut findings: Vec<Finding> = Vec::new();
    // Track reported source nodes to emit at most one finding per file.
    let mut reported: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for start in 0..n {
        if color[start] != 0 || files[start].language != Language::Vox {
            continue;
        }

        // (node_idx, edge_cursor)
        let mut dfs_stack: Vec<(usize, usize)> = vec![(start, 0)];
        color[start] = 1;

        while let Some(top) = dfs_stack.last_mut() {
            let node = top.0;
            if top.1 < edges[node].len() {
                let cursor = top.1;
                top.1 += 1;
                let (tgt, line_num, ref raw_path) = edges[node][cursor];
                let raw_path = raw_path.clone();

                if color[tgt] == 1 {
                    // Back edge → cycle found. Emit one finding at the
                    // importing file (node), at the line that closes the cycle.
                    if !reported.contains(&node) {
                        reported.insert(node);
                        let file = &files[node];
                        findings.push(Finding {
                            rule_id: "import/cycle".to_string(),
                            diagnostic_id: Some(catalog::IMPORT_CYCLE.to_string()),
                            rule_name: "Import Cycle Detector".to_string(),
                            severity: Severity::Error,
                            file: file.path.clone(),
                            line: line_num,
                            column: 1,
                            message: format!(
                                "Import cycle: `{raw_path}` closes a circular dependency chain."
                            ),
                            suggestion: Some(
                                "Extract shared logic into a new module that neither side of \
                                 the cycle imports back."
                                    .to_string(),
                            ),
                            alternatives: vec![],
                            rationale: Some(
                                "Import cycles prevent the Vox interpreter from resolving \
                                 module boundaries and cause `ImportCycle` runtime errors at startup."
                                    .into(),
                            ),
                            context: file.context_around(line_num, 2),
                            confidence: Some(FindingConfidence::High),
                            evidence: None,
                        });
                    }
                } else if color[tgt] == 0 {
                    color[tgt] = 1;
                    dfs_stack.push((tgt, 0));
                }
                // color[tgt] == 2 → already fully explored, no cycle through here
            } else {
                color[node] = 2;
                dfs_stack.pop();
            }
        }
    }

    findings
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn vox_file(name: &str, content: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(name), content.to_string())
    }

    // ── Per-file detector ──────────────────────────────────────────────────

    #[test]
    fn fires_on_self_import() {
        let d = ImportCyclesDetector::new();
        let f = vox_file("src/a.vox", r#"import "./a.vox""#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1, "should fire on self-import");
        assert!(findings[0].message.contains("Self-import"));
        assert_eq!(findings[0].severity, Severity::Error);
    }

    #[test]
    fn no_fire_on_regular_import() {
        let d = ImportCyclesDetector::new();
        let f = vox_file(
            "src/a.vox",
            r#"import "./b.vox"
pub fn hello() to str { "hi" }"#,
        );
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "regular import of different file should not fire"
        );
    }

    #[test]
    fn skips_comment_lines() {
        let d = ImportCyclesDetector::new();
        let f = vox_file("src/a.vox", r#"// import "./a.vox""#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "commented import should not fire");
    }

    #[test]
    fn does_not_fire_on_rust_files() {
        let d = ImportCyclesDetector::new();
        let f = SourceFile::new(
            PathBuf::from("src/main.rs"),
            r#"import "./main.rs""#.to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "Rust files are ignored");
    }

    // ── Batch cycle detection ──────────────────────────────────────────────

    #[test]
    fn batch_detects_mutual_cycle() {
        // a.vox imports b.vox; b.vox imports a.vox → mutual cycle.
        let a = vox_file(
            "src/a.vox",
            "import \"./b.vox\"\npub fn fa() to str { \"a\" }",
        );
        let b = vox_file(
            "src/b.vox",
            "import \"./a.vox\"\npub fn fb() to str { \"b\" }",
        );
        let findings = detect_import_cycles_in_batch(&[a, b]);
        assert!(
            !findings.is_empty(),
            "mutual import cycle should be detected"
        );
        assert!(findings.iter().any(|f| f.message.contains("cycle")));
    }

    #[test]
    fn batch_detects_three_node_cycle() {
        // a → b → c → a
        let a = vox_file("src/a.vox", "import \"./b.vox\"");
        let b = vox_file("src/b.vox", "import \"./c.vox\"");
        let c = vox_file("src/c.vox", "import \"./a.vox\"");
        let findings = detect_import_cycles_in_batch(&[a, b, c]);
        assert!(!findings.is_empty(), "three-node cycle should be detected");
    }

    #[test]
    fn batch_no_fire_on_dag() {
        // a → b → c  (no cycle)
        let a = vox_file("src/a.vox", "import \"./b.vox\"");
        let b = vox_file("src/b.vox", "import \"./c.vox\"");
        let c = vox_file("src/c.vox", "pub fn root() to str { \"leaf\" }");
        let findings = detect_import_cycles_in_batch(&[a, b, c]);
        assert!(findings.is_empty(), "DAG should not produce cycle findings");
    }

    #[test]
    fn batch_no_fire_on_diamond() {
        // a → b, a → c, b → d, c → d  (diamond, no cycle)
        let a = vox_file("src/a.vox", "import \"./b.vox\"\nimport \"./c.vox\"");
        let b = vox_file("src/b.vox", "import \"./d.vox\"");
        let c = vox_file("src/c.vox", "import \"./d.vox\"");
        let d = vox_file("src/d.vox", "pub fn leaf() to str { \"leaf\" }");
        let findings = detect_import_cycles_in_batch(&[a, b, c, d]);
        assert!(
            findings.is_empty(),
            "diamond import graph should not produce cycle findings"
        );
    }

    #[test]
    fn batch_ignores_non_vox_files() {
        let rs = SourceFile::new(
            PathBuf::from("src/main.rs"),
            "import \"./main.rs\"".to_string(),
        );
        let ts = SourceFile::new(
            PathBuf::from("src/index.ts"),
            "import \"./index.ts\"".to_string(),
        );
        let findings = detect_import_cycles_in_batch(&[rs, ts]);
        assert!(
            findings.is_empty(),
            "non-Vox files are ignored by batch detector"
        );
    }

    // ── extract_vox_imports ────────────────────────────────────────────────

    #[test]
    fn extract_imports_picks_up_relative_paths() {
        let f = vox_file(
            "src/a.vox",
            "import \"./b.vox\"\nimport \"./sub/c.vox\" as c\nfn x() {}",
        );
        let imports = extract_vox_imports(&f);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0], (1, "./b.vox".to_string()));
        assert_eq!(imports[1], (2, "./sub/c.vox".to_string()));
    }

    #[test]
    fn extract_imports_skips_non_relative() {
        let f = vox_file("src/a.vox", "import \"@stdlib/io\"\nimport \"http\"");
        let imports = extract_vox_imports(&f);
        assert!(
            imports.is_empty(),
            "non-relative imports should not be extracted"
        );
    }
}
