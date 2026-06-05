//! Stdlib-coverage parity check.
//!
//! Three-way diff between:
//! 1. **Binary** — symbols registered in
//!    [`crates/vox-compiler/src/eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs)
//!    (parsed via `syn` to walk `call_global_builtin` and `call_builtin_method`).
//! 2. **Docs** — symbols claimed in
//!    [`docs/src/reference/ref-builtins-stdlib.md`](../../../docs/src/reference/ref-builtins-stdlib.md)
//!    (parsed via regex over the markdown tables).
//! 3. **Corpus** — symbols invoked under `scripts/**/*.vox` (regex
//!    `<ident>.<ident>(`).
//!
//! Pattern mirrors [`crate::retirement_parity`] — a non-CR-L tooling gate
//! consumed by `vox-audit::subcommands::stdlib_coverage`. See
//! [`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md`](../../../docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md) §10
//! for the spec.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParityError {
    #[error("could not read binary registration source at {path}: {source}")]
    ReadBinarySource {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse binary registration source at {path}: {source}")]
    ParseBinarySource {
        path: PathBuf,
        #[source]
        source: syn::Error,
    },
    #[error("could not read docs source at {path}: {source}")]
    ReadDocsSource {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not walk corpus root {path}: {source}")]
    CorpusWalk {
        path: PathBuf,
        #[source]
        source: glob::PatternError,
    },
}

/// One symbol reference in the corpus.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CorpusSite {
    pub file: PathBuf,
    pub line: usize,
}

/// Severity of a mismatch (mirrors the audit doc §10 severity rules).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum Severity {
    /// Script calls a symbol that doesn't exist at runtime — script will fail.
    /// Or docs claim a symbol that doesn't exist — training corpus reproduces lie.
    Error,
    /// Binary has a symbol that's not documented — useful but invisible.
    Warn,
    /// Doc + binary agree but no script uses it — possibly dead or just unused.
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub enum MismatchKind {
    /// `corpus_locations` non-empty, `binary_location` is None.
    CorpusUsesUnregistered,
    /// `doc_locations` non-empty, `binary_location` is None.
    DocClaimsUnregistered,
    /// `binary_location` Some, `doc_locations` empty.
    RegisteredButUndocumented,
    /// `doc_locations` non-empty, `binary_location` Some, `corpus_locations` empty.
    DocumentedButUnused,
}

#[derive(Debug, Clone, Serialize)]
pub struct Mismatch {
    /// Fully-qualified symbol, e.g. `fs.read`, `regex.replace`, `print`.
    pub symbol: String,
    pub kind: MismatchKind,
    pub severity: Severity,
    pub binary_location: Option<String>,
    pub doc_locations: Vec<String>,
    pub corpus_locations: Vec<CorpusSite>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParityReport {
    pub symbols_registered: usize,
    pub symbols_documented: usize,
    pub symbols_used_in_corpus: usize,
    pub mismatches: Vec<Mismatch>,
}

impl ParityReport {
    /// True iff there are no error-severity mismatches.
    pub fn is_clean(&self) -> bool {
        !self
            .mismatches
            .iter()
            .any(|m| m.severity == Severity::Error)
    }

    /// Compact one-line summary for log lines / report notes.
    pub fn summary(&self) -> String {
        let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
        for m in &self.mismatches {
            let k = match m.kind {
                MismatchKind::CorpusUsesUnregistered => "corpus_uses_unregistered",
                MismatchKind::DocClaimsUnregistered => "doc_claims_unregistered",
                MismatchKind::RegisteredButUndocumented => "registered_but_undocumented",
                MismatchKind::DocumentedButUnused => "documented_but_unused",
            };
            *by_kind.entry(k).or_insert(0) += 1;
        }
        by_kind
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Total count of error-severity mismatches.
    pub fn error_count(&self) -> usize {
        self.mismatches
            .iter()
            .filter(|m| m.severity == Severity::Error)
            .count()
    }
}

// ── Binary side: parse eval/builtins.rs via syn ────────────────────────────

/// Set of `namespace.method` registrations recovered from
/// `crates/vox-compiler/src/eval/builtins.rs`.
///
/// The eval file follows a stable shape: `call_global_builtin` is a `match name`
/// where each arm is a string literal; `call_builtin_method` is a `match obj`
/// where each `Some("namespace") =>` branch is a nested `match method` of
/// string literals. We walk both shapes and collect every reachable name.
fn parse_binary_registrations(source_path: &Path) -> Result<BTreeSet<String>, ParityError> {
    use syn::visit::Visit;

    let source =
        std::fs::read_to_string(source_path).map_err(|e| ParityError::ReadBinarySource {
            path: source_path.to_path_buf(),
            source: e,
        })?;
    let file = syn::parse_file(&source).map_err(|e| ParityError::ParseBinarySource {
        path: source_path.to_path_buf(),
        source: e,
    })?;

    struct Visitor {
        symbols: BTreeSet<String>,
        /// Whether the current ExprMatch is dispatching on the namespace
        /// (the outer match in `call_builtin_method`). When true, every
        /// string-literal arm becomes a namespace prefix.
        ns_stack: Vec<String>,
    }

    impl<'ast> Visit<'ast> for Visitor {
        fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
            // Try to recognize the two interesting shapes:
            //   match name { "print" => ..., "assert" => ..., ... }       (globals)
            //   match obj { Some("fs") => match method { "read" => ..., ... }, Some("env") => ... }
            //
            // For globals we see string-literal patterns directly; for namespace
            // dispatch we see `Some(...)` patterns wrapping a string literal,
            // then the arm body itself contains another match on `method`.
            //
            // We use a heuristic: every string-literal arm pattern collects the
            // current namespace prefix.
            for arm in &node.arms {
                let lits = extract_str_lits_from_pat(&arm.pat);
                for name in &lits {
                    if let Some(ns) = self.ns_stack.last() {
                        // Per-namespace method.
                        self.symbols.insert(format!("{ns}.{name}"));
                    } else {
                        // Top-level — possibly a global builtin. Identifier
                        // shape gates against unrelated string-literal matches.
                        if is_identifier_shape(name) {
                            self.symbols.insert(name.to_string());
                        }
                    }
                }

                // Detect `Some("namespace") => { ... }` and push namespace.
                let ns = extract_some_str_lit_from_pat(&arm.pat);
                if let Some(ns_name) = ns {
                    self.ns_stack.push(ns_name.to_string());
                    self.visit_expr(&arm.body);
                    self.ns_stack.pop();
                } else {
                    self.visit_expr(&arm.body);
                }
            }
            // Don't call default visitor since we already walked the arms.
        }
    }

    /// Collect every string literal reachable from a pattern, descending
    /// through `|` (or-patterns). Returns owned strings to sidestep
    /// lifetime threading through the visitor.
    fn extract_str_lits_from_pat(pat: &syn::Pat) -> Vec<String> {
        let mut out = Vec::new();
        collect_str_lits(pat, &mut out);
        out
    }

    fn collect_str_lits(pat: &syn::Pat, out: &mut Vec<String>) {
        match pat {
            syn::Pat::Lit(syn::PatLit {
                lit: syn::Lit::Str(s),
                ..
            }) => out.push(s.value()),
            syn::Pat::Or(syn::PatOr { cases, .. }) => {
                for c in cases {
                    collect_str_lits(c, out);
                }
            }
            _ => {}
        }
    }

    /// Match `Pat::TupleStruct(Some("foo"))` (i.e. `Some("foo")` in a pattern).
    fn extract_some_str_lit_from_pat(pat: &syn::Pat) -> Option<String> {
        let ts = match pat {
            syn::Pat::TupleStruct(ts) => ts,
            _ => return None,
        };
        if !ts
            .path
            .segments
            .last()
            .map(|seg| seg.ident == "Some")
            .unwrap_or(false)
        {
            return None;
        }
        if ts.elems.len() != 1 {
            return None;
        }
        let lits = extract_str_lits_from_pat(&ts.elems[0]);
        lits.into_iter().next()
    }

    fn is_identifier_shape(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .next()
                .map(|c| c.is_ascii_alphabetic() || c == '_')
                .unwrap_or(false)
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    let mut visitor = Visitor {
        symbols: BTreeSet::new(),
        ns_stack: Vec::new(),
    };
    visitor.visit_file(&file);
    Ok(visitor.symbols)
}

/// Parse `crates/vox-compiler/src/builtin_registry.rs` and collect every
/// `BuiltinRegistryEntry { namespace: "...", name: "...", ... }` literal.
///
/// These symbols are wired into typecheck and codegen via
/// `builtin_entry_param_tys` / `builtin_entry_result_ty` and are dispatched
/// at runtime by `vox-actor-runtime` (the native script-execution mode).
/// The interp path in `eval/builtins.rs` does NOT carry them — so they
/// appear "registered" from the build/check perspective but unavailable in
/// `vox run --mode interp`. For the stdlib-coverage gate this still counts
/// as registered because the symbol IS callable in the production
/// (native) execution path.
fn parse_registry_entries(source_path: &Path) -> Result<BTreeSet<String>, ParityError> {
    let source =
        std::fs::read_to_string(source_path).map_err(|e| ParityError::ReadBinarySource {
            path: source_path.to_path_buf(),
            source: e,
        })?;
    // Lightweight regex pass — the file is auto-formatted with stable
    // `namespace: "<ns>"` / `name: "<name>"` field shapes inside each
    // `BuiltinRegistryEntry { ... }` literal. A full syn parse is
    // overkill for this shape.
    let entry_re = regex::Regex::new(r#"namespace:\s*"([^"]+)",\s*name:\s*"([^"]+)""#)
        .expect("registry entry regex compiles");
    let mut symbols: BTreeSet<String> = BTreeSet::new();
    for c in entry_re.captures_iter(&source) {
        let ns = &c[1];
        let name = &c[2];
        // Normalize to match the doc-side convention: the reference doc
        // uses `<ns>.method` headers (without `std.` prefix), so strip
        // `std.` from registry entries when present.
        // `std.http.get_text` → `http.get_text`
        // `std.uuid` → `uuid` (top-level, no namespace prefix)
        let canonical = if ns == "std" {
            // Top-level `std` entries become bare globals.
            name.to_string()
        } else if let Some(stripped) = ns.strip_prefix("std.") {
            format!("{stripped}.{name}")
        } else {
            // Non-std namespaces (Browser, OpenClaw) stay verbatim.
            format!("{ns}.{name}")
        };
        symbols.insert(canonical);
    }
    Ok(symbols)
}

// ── Docs side: parse ref-builtins-stdlib.md markdown tables ────────────────

/// Set of `namespace.method` (or `method` for globals) names extracted from
/// the reference doc's markdown tables. The doc uses a stable table-row shape:
///
/// ```markdown
/// ## Path Manipulation (`std.path.*`)
///
/// | `fn join(a: str, b: str) to str` | Joins two path parts. |
/// ```
///
/// We capture (a) the namespace from the `(std.<ns>.*)` header, and (b) the
/// function name from the first identifier after `fn ` in a row.
fn parse_documented_symbols(doc_path: &Path) -> Result<BTreeSet<String>, ParityError> {
    let source = std::fs::read_to_string(doc_path).map_err(|e| ParityError::ReadDocsSource {
        path: doc_path.to_path_buf(),
        source: e,
    })?;

    // A namespaced section header like `## Path Manipulation (`std.path.*`)`.
    let header_re =
        regex::Regex::new(r"^##\s+.*\(`(?:std\.)?(\w+)\.\*`\)").expect("static regex compiles");
    // A top-level `std.*` header (the `Cryptography and UUID (`std.*`)`
    // and `Time (`std.*`)` shapes) — methods here are bare globals
    // matching `BuiltinRegistryEntry { namespace: "std", name: "..." }`
    // after normalization.
    let std_bare_header_re =
        regex::Regex::new(r"^##\s+.*\(`std\.\*`\)").expect("static regex compiles");
    let global_header_re =
        regex::Regex::new(r"^##\s+Global Built-ins").expect("static regex compiles");
    let row_re = regex::Regex::new(r"\|\s*`fn\s+(\w+)\s*\(").expect("static regex compiles");

    let mut symbols: BTreeSet<String> = BTreeSet::new();
    let mut current_ns: Option<String> = None;
    let mut globals_section = false;

    for line in source.lines() {
        if line.starts_with("## ") {
            // New section — reset state.
            globals_section = global_header_re.is_match(line) || std_bare_header_re.is_match(line);
            // Skip namespace capture for bare `std.*` headers (they're
            // globals).
            current_ns = if std_bare_header_re.is_match(line) {
                None
            } else {
                header_re
                    .captures(line)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            };
            continue;
        }
        if let Some(c) = row_re.captures(line) {
            let fn_name = &c[1];
            let symbol = if globals_section {
                fn_name.to_string()
            } else if let Some(ns) = current_ns.as_ref() {
                format!("{ns}.{fn_name}")
            } else {
                // Function row outside any recognized section — skip.
                continue;
            };
            symbols.insert(symbol);
        }
    }
    Ok(symbols)
}

// ── Corpus side: regex over scripts/**/*.vox ───────────────────────────────

/// Map of `<ident>.<ident>(` call sites to the files/lines where they occur.
///
/// Only canonical namespace calls (lowercase prefix, no upper-case receivers
/// like `s.trim()`) are collected — those are the *free-function* calls that
/// must resolve to a registered builtin. Receiver method calls are out of
/// scope (covered by the method-dispatch type-check path).
fn scan_corpus_call_sites(root: &Path) -> Result<BTreeMap<String, Vec<CorpusSite>>, ParityError> {
    // Use the existing glob crate (already a vox-compiler dep, also workspace-deep).
    let pattern = format!("{}/**/*.vox", root.display());
    let entries = glob::glob(&pattern).map_err(|e| ParityError::CorpusWalk {
        path: root.to_path_buf(),
        source: e,
    })?;

    // Only match calls whose RECEIVER is a known free-namespace ident. This
    // avoids treating `s.trim()` (method call on a variable) as a namespace
    // call. The list mirrors namespaces registered in eval/mod.rs.
    let known_namespaces = [
        "fs", "process", "env", "path", "secrets", "json", "regex", "log", "csv", "toml", "yaml",
        "io", "time", "http", "agentos", "str", "list",
    ];
    let alt = known_namespaces.join("|");
    let call_re =
        regex::Regex::new(&format!(r"\b({alt})\.(\w+)\s*\(")).expect("namespace regex compiles");

    let mut sites: BTreeMap<String, Vec<CorpusSite>> = BTreeMap::new();
    for entry in entries.flatten() {
        let path: PathBuf = entry.clone();
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for (lineno, line) in content.lines().enumerate() {
            for c in call_re.captures_iter(line) {
                let ns = &c[1];
                let method = &c[2];
                let symbol = format!("{ns}.{method}");
                sites.entry(symbol).or_default().push(CorpusSite {
                    file: path.clone(),
                    line: lineno + 1,
                });
            }
        }
    }
    Ok(sites)
}

// ── Three-way diff ─────────────────────────────────────────────────────────

/// Run the parity check against absolute paths.
///
/// `binary_source_path` should point at
/// `crates/vox-compiler/src/eval/builtins.rs`. `doc_path` at
/// `docs/src/reference/ref-builtins-stdlib.md`. `corpus_root` at `scripts/`
/// (the audit only covers committed automation scripts today; extend to
/// `examples/` once the corpus is healthy).
pub fn check_parity_at_paths(
    binary_source_path: &Path,
    doc_path: &Path,
    corpus_root: &Path,
) -> Result<ParityReport, ParityError> {
    let mut binary = parse_binary_registrations(binary_source_path)?;

    // Also fold in the registry-driven entries from
    // `crates/vox-compiler/src/builtin_registry.rs`. These are dispatched
    // at codegen / vox-actor-runtime for the native execution path. The
    // file path is derived by walking up from eval/builtins.rs to the
    // crate root and then into the sibling module.
    if let Some(builtins_parent) = binary_source_path.parent() {
        let registry_path = builtins_parent
            .parent() // src/
            .map(|p| p.join("builtin_registry.rs"));
        if let Some(reg) = registry_path
            && reg.exists()
                && let Ok(reg_syms) = parse_registry_entries(&reg) {
                    binary.extend(reg_syms);
                }
    }

    let docs = parse_documented_symbols(doc_path)?;
    let corpus = scan_corpus_call_sites(corpus_root)?;

    let mut all_symbols: BTreeSet<String> = BTreeSet::new();
    all_symbols.extend(binary.iter().cloned());
    all_symbols.extend(docs.iter().cloned());
    all_symbols.extend(corpus.keys().cloned());

    let mut mismatches: Vec<Mismatch> = Vec::new();
    for symbol in all_symbols {
        let in_binary = binary.contains(&symbol);
        let in_docs = docs.contains(&symbol);
        let in_corpus = corpus.contains_key(&symbol);

        let kind = match (in_binary, in_docs, in_corpus) {
            (false, false, true) => Some(MismatchKind::CorpusUsesUnregistered),
            (false, true, _) => Some(MismatchKind::DocClaimsUnregistered),
            (true, false, true) => Some(MismatchKind::RegisteredButUndocumented),
            (true, true, false) => Some(MismatchKind::DocumentedButUnused),
            // (true, true, true): all aligned — no mismatch
            // (true, false, false): registered, undocumented, unused — covered by RegisteredButUndocumented
            // (false, false, false): unreachable since symbol came from a non-empty source
            _ => None,
        };
        let kind = match kind {
            Some(k) => k,
            None => {
                if in_binary && !in_docs {
                    MismatchKind::RegisteredButUndocumented
                } else {
                    continue;
                }
            }
        };

        let severity = match kind {
            MismatchKind::CorpusUsesUnregistered | MismatchKind::DocClaimsUnregistered => {
                Severity::Error
            }
            MismatchKind::RegisteredButUndocumented => Severity::Warn,
            MismatchKind::DocumentedButUnused => Severity::Info,
        };

        let recommendation = match kind {
            MismatchKind::CorpusUsesUnregistered => format!(
                "`{symbol}` is called from scripts but is not registered in eval/builtins.rs. \
                 Either implement it in the binary or rewrite the call sites."
            ),
            MismatchKind::DocClaimsUnregistered => format!(
                "`{symbol}` is documented as existing but is not registered in eval/builtins.rs. \
                 Either implement it or remove the doc claim."
            ),
            MismatchKind::RegisteredButUndocumented => format!(
                "`{symbol}` is registered in the binary but not documented in \
                 docs/src/reference/ref-builtins-stdlib.md. Add a row to the appropriate \
                 namespace table."
            ),
            MismatchKind::DocumentedButUnused => format!(
                "`{symbol}` is documented and registered but unused in the script corpus. \
                 Either it's reserve-for-future, or it's dead — verify before retiring."
            ),
        };

        mismatches.push(Mismatch {
            symbol: symbol.clone(),
            kind,
            severity,
            binary_location: if in_binary {
                Some(format!("{}", binary_source_path.display()))
            } else {
                None
            },
            doc_locations: if in_docs {
                vec![format!("{}", doc_path.display())]
            } else {
                Vec::new()
            },
            corpus_locations: corpus.get(&symbol).cloned().unwrap_or_default(),
            recommendation,
        });
    }

    Ok(ParityReport {
        symbols_registered: binary.len(),
        symbols_documented: docs.len(),
        symbols_used_in_corpus: corpus.len(),
        mismatches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        // CARGO_MANIFEST_DIR points at crates/vox-code-audit; the root is two levels up.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn parse_binary_registrations_finds_known_global_print() {
        let root = workspace_root();
        let binary = root.join("crates/vox-compiler/src/eval/builtins.rs");
        let symbols = parse_binary_registrations(&binary).expect("parse should succeed");
        assert!(
            symbols.contains("print"),
            "expected `print` global to be detected; got: {symbols:?}"
        );
    }

    #[test]
    fn parse_binary_registrations_finds_fs_read() {
        let root = workspace_root();
        let binary = root.join("crates/vox-compiler/src/eval/builtins.rs");
        let symbols = parse_binary_registrations(&binary).expect("parse should succeed");
        assert!(
            symbols.contains("fs.read"),
            "expected `fs.read` to be detected; got first 30: {:?}",
            symbols.iter().take(30).collect::<Vec<_>>()
        );
    }

    #[test]
    fn parse_binary_registrations_finds_regex_replace() {
        let root = workspace_root();
        let binary = root.join("crates/vox-compiler/src/eval/builtins.rs");
        let symbols = parse_binary_registrations(&binary).expect("parse should succeed");
        assert!(
            symbols.contains("regex.replace"),
            "expected `regex.replace` (added 2026-05-23) to be detected"
        );
    }

    #[test]
    fn parse_documented_symbols_finds_path_join() {
        let root = workspace_root();
        let doc = root.join("docs/src/reference/ref-builtins-stdlib.md");
        let symbols = parse_documented_symbols(&doc).expect("parse should succeed");
        assert!(
            symbols.contains("path.join"),
            "expected `path.join` to be documented; got: {symbols:?}"
        );
    }

    #[test]
    fn check_parity_at_paths_runs_against_workspace() {
        let root = workspace_root();
        let binary = root.join("crates/vox-compiler/src/eval/builtins.rs");
        let doc = root.join("docs/src/reference/ref-builtins-stdlib.md");
        let corpus = root.join("scripts");
        let report = check_parity_at_paths(&binary, &doc, &corpus).expect("parity should run");
        assert!(
            report.symbols_registered > 30,
            "expected ≥30 binary symbols"
        );
        assert!(
            report.symbols_documented > 5,
            "expected ≥5 documented symbols"
        );
        // Don't gate on "no mismatches" since the audit is mid-cleanup — we
        // expect some RegisteredButUndocumented warns.
        let summary = report.summary();
        assert!(!summary.is_empty(), "summary must be non-empty");
    }

    /// Diagnostic dump — surface every `corpus_uses_unregistered` mismatch
    /// with citations so a human can decide whether to implement in eval or
    /// migrate the corpus. Run with:
    /// `cargo test -p vox-code-audit dump_corpus_uses_unregistered -- --nocapture --ignored`
    #[test]
    #[ignore = "diagnostic dump; run on demand with --ignored; owner: vox-code-audit; sunset: 2027-05-27"]
    fn dump_corpus_uses_unregistered() {
        let root = workspace_root();
        let binary = root.join("crates/vox-compiler/src/eval/builtins.rs");
        let doc = root.join("docs/src/reference/ref-builtins-stdlib.md");
        let corpus = root.join("scripts");
        let report = check_parity_at_paths(&binary, &doc, &corpus).expect("parity should run");
        println!("\n=== CORPUS-USES-UNREGISTERED ===");
        for m in &report.mismatches {
            if matches!(m.kind, MismatchKind::CorpusUsesUnregistered) {
                println!("\n{}", m.symbol);
                for site in m.corpus_locations.iter().take(5) {
                    println!("  {}:{}", site.file.display(), site.line);
                }
                if m.corpus_locations.len() > 5 {
                    println!("  ... +{} more", m.corpus_locations.len() - 5);
                }
            }
        }
        println!("\n=== DOC-CLAIMS-UNREGISTERED ===");
        for m in &report.mismatches {
            if matches!(m.kind, MismatchKind::DocClaimsUnregistered) {
                println!("  {}", m.symbol);
            }
        }
    }
}
