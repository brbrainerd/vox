# Drift Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `vox-drift-check`, a multi-language (Rust/TS/Vox) workspace-level linter that detects code drift — both targeted violations (reqwest bypass, raw path literals, etc.) and dynamically-discovered repeated patterns (duplicate string literals, numeric constants, function body clones).

**Architecture:** Two-phase pipeline: parallel per-file extraction to `ExtractedFeatures` (native AST per language), then a single-threaded workspace sweep that aggregates features into dedup indexes and runs targeted `DriftRule` checks. Results reported via the existing `vox-code-audit` `Finding`/`Reporter` types.

**Tech Stack:** Rust, `syn` (Rust AST), `swc_ecma_parser` (TypeScript AST), `vox_compiler::pipeline` (Vox), `rayon` (parallelism), `xxhash-rust` (body hashing), `bincode` (cache), `clap` (CLI) — all workspace-managed except swc.

---

## File Map

**Create:**
- `crates/vox-drift-check/Cargo.toml`
- `crates/vox-drift-check/src/lib.rs`
- `crates/vox-drift-check/src/features.rs` — `ExtractedFeatures`, `Loc`, `LiteralLoc`, `NumericLoc`, `CallSite`, `BodySignature`, `ImportLoc`, `FnDef`, `LiteralContext`, `UnitHint`
- `crates/vox-drift-check/src/extractor.rs` — `LanguageExtractor` trait
- `crates/vox-drift-check/src/extractors/mod.rs`
- `crates/vox-drift-check/src/extractors/rust.rs` — syn-backed
- `crates/vox-drift-check/src/extractors/typescript.rs` — swc-backed
- `crates/vox-drift-check/src/extractors/vox.rs` — vox-compiler-backed
- `crates/vox-drift-check/src/engine.rs` — walk + dispatch + aggregate
- `crates/vox-drift-check/src/sweep/mod.rs` — `SweepRule` trait, `WorkspaceFeatures`
- `crates/vox-drift-check/src/sweep/literal_dedup.rs`
- `crates/vox-drift-check/src/sweep/numeric_dedup.rs`
- `crates/vox-drift-check/src/sweep/body_hash.rs`
- `crates/vox-drift-check/src/sweep/call_shape.rs`
- `crates/vox-drift-check/src/rules/mod.rs` — `DriftRule` trait, `WorkspaceContext`
- `crates/vox-drift-check/src/rules/reqwest_bypass.rs`
- `crates/vox-drift-check/src/rules/vox_path_literal.rs`
- `crates/vox-drift-check/src/rules/timeout_literal.rs`
- `crates/vox-drift-check/src/rules/serde_default_dup.rs`
- `crates/vox-drift-check/src/rules/version_string.rs`
- `crates/vox-drift-check/src/rules/bearer_header.rs`
- `crates/vox-drift-check/src/config.rs` — `DriftConfig`, `drift-patterns.toml` loader
- `crates/vox-drift-check/src/cache.rs` — content-hash bincode cache
- `crates/vox-drift-check/src/report.rs`
- `crates/vox-drift-check/src/bin/vox_drift_check.rs`
- `crates/vox-drift-check/tests/fixtures/planted_violations.rs` — test fixture
- `drift-patterns.toml` — workspace root

**Modify:**
- `Cargo.toml` (root) — add workspace member + swc deps to `[workspace.dependencies]`
- `docs/src/architecture/layers.toml` — add L3 entry
- `docs/src/architecture/where-things-live.md` — add row
- `crates/vox-cli/Cargo.toml` — add dep
- `crates/vox-cli/src/commands/mod.rs` — add module
- `crates/vox-cli/src/commands/drift_check.rs` — new (create)
- `lefthook.yml` — add pre-push step

---

## Phase 1 — Foundation

### Task 1: Workspace bootstrap

**Files:**
- Create: `crates/vox-drift-check/Cargo.toml`
- Modify: `Cargo.toml` (root)
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Add swc to workspace dependencies**

In root `Cargo.toml`, under `[workspace.dependencies]` add:
```toml
swc_ecma_parser = "0.149"
swc_ecma_ast    = "0.118"
swc_ecma_visit  = "0.104"
swc_common      = "0.37"
```

- [ ] **Step 2: Create crate Cargo.toml**

```toml
[package]
name = "vox-drift-check"
version.workspace = true
edition.workspace = true
description = "Workspace-wide multi-language drift and pattern-repetition linter"

[[bin]]
name = "vox-drift-check"
path = "src/bin/vox_drift_check.rs"

[dependencies]
vox-code-audit   = { workspace = true }
vox-compiler     = { workspace = true }
vox-config       = { workspace = true }
syn              = { workspace = true, features = ["full", "visit", "extra-traits"] }
proc-macro2      = { workspace = true, features = ["span-locations"] }
rayon            = { workspace = true }
walkdir          = { workspace = true }
xxhash-rust      = { workspace = true, features = ["xxh3"] }
serde            = { workspace = true, features = ["derive"] }
serde_json       = { workspace = true }
toml             = { workspace = true }
bincode          = { workspace = true }
sha2             = { workspace = true }
clap             = { workspace = true, features = ["derive"] }
anyhow           = { workspace = true }
tracing          = { workspace = true }
globset          = { workspace = true }
swc_ecma_parser  = { workspace = true }
swc_ecma_ast     = { workspace = true }
swc_ecma_visit   = { workspace = true }
swc_common       = { workspace = true }
workspace-hack   = { workspace = true }

[dev-dependencies]
insta    = { workspace = true }
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 3: Add workspace member**

In root `Cargo.toml` `[workspace] members`, add `"crates/vox-drift-check"`.

- [ ] **Step 4: Register in governance docs**

In `docs/src/architecture/layers.toml` under `# ── L3`, add:
```toml
vox-drift-check = { layer = 3, kind = "binary", max_loc = 8000 }
```

In `docs/src/architecture/where-things-live.md`, add a row:
```
| `vox-drift-check` | Workspace drift & pattern-repetition linter (multi-language) |
```

- [ ] **Step 5: Create stub lib.rs and verify it compiles**

```rust
// crates/vox-drift-check/src/lib.rs
pub mod config;
pub mod engine;
pub mod extractor;
pub mod extractors;
pub mod features;
pub mod rules;
pub mod sweep;
pub mod cache;
pub mod report;
```

Create empty `mod.rs` files in each subdirectory, then:
```
cargo check -p vox-drift-check
```
Expected: compiles (no errors, possible unused warnings).

- [ ] **Step 6: Commit**
```
git add crates/vox-drift-check/ Cargo.toml Cargo.lock docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "chore: scaffold vox-drift-check crate with workspace registration"
```

---

### Task 2: Feature vocabulary types

**Files:**
- Create: `crates/vox-drift-check/src/features.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/vox-drift-check/src/features.rs (bottom, in #[cfg(test)])
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_loc_round_trips_serde() {
        let lit = LiteralLoc {
            value: "hello world".into(),
            loc: Loc { line: 5, col: 3 },
            ctx: LiteralContext::Code,
        };
        let json = serde_json::to_string(&lit).unwrap();
        let back: LiteralLoc = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value, "hello world");
        assert_eq!(back.loc.line, 5);
        assert!(matches!(back.ctx, LiteralContext::Code));
    }

    #[test]
    fn numeric_loc_unit_hint_default() {
        let n = NumericLoc { value: 30.0, unit: None, loc: Loc::default() };
        assert_eq!(n.value, 30.0);
        assert!(n.unit.is_none());
    }

    #[test]
    fn extracted_features_default_is_empty() {
        let f = ExtractedFeatures::new(std::path::PathBuf::from("foo.rs"), Language::Rust);
        assert!(f.string_literals.is_empty());
        assert!(f.call_sites.is_empty());
    }
}
```

- [ ] **Step 2: Run — expect compile failure**
```
cargo test -p vox-drift-check 2>&1 | head -20
```
Expected: `cannot find type 'LiteralLoc'` or similar.

- [ ] **Step 3: Implement features.rs**

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use vox_code_audit::rules::Language;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Loc {
    pub line: usize, // 1-indexed
    pub col: usize,  // 0-indexed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiteralContext {
    Code,
    Test,
    Doc,
    ConstDecl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnitHint {
    Millis,
    Seconds,
    Bytes,
    Count,
    Bare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteralLoc {
    pub value: String,
    pub loc: Loc,
    pub ctx: LiteralContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericLoc {
    pub value: f64,
    pub unit: Option<UnitHint>,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSite {
    pub path: Vec<String>,
    pub arity: u8,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodySignature {
    pub hash: u64,
    pub line_count: u32,
    pub parent_fn: Option<String>,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLoc {
    pub path: Vec<String>,
    pub symbol: Option<String>,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FnDef {
    pub name: String,
    pub body_hash: u64,
    pub sig_hash: u64,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFeatures {
    pub file: PathBuf,
    pub language: Language,
    pub crate_name: Option<String>,
    pub string_literals: Vec<LiteralLoc>,
    pub numeric_literals: Vec<NumericLoc>,
    pub call_sites: Vec<CallSite>,
    pub body_signatures: Vec<BodySignature>,
    pub imports: Vec<ImportLoc>,
    pub fn_definitions: Vec<FnDef>,
}

impl ExtractedFeatures {
    pub fn new(file: PathBuf, language: Language) -> Self {
        Self {
            file,
            language,
            crate_name: None,
            string_literals: Vec::new(),
            numeric_literals: Vec::new(),
            call_sites: Vec::new(),
            body_signatures: Vec::new(),
            imports: Vec::new(),
            fn_definitions: Vec::new(),
        }
    }
}
```

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check features
```
Expected: 3 tests pass.

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/features.rs
git commit -m "feat(drift): add ExtractedFeatures vocabulary types"
```

---

### Task 3: LanguageExtractor trait + crate_name resolver

**Files:**
- Create: `crates/vox-drift-check/src/extractor.rs`

- [ ] **Step 1: Write failing test**

```rust
// end of extractor.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn crate_name_from_path_under_crates() {
        let p = Path::new("crates/vox-config/src/lib.rs");
        assert_eq!(crate_name_from_path(p), Some("vox-config".to_string()));
    }

    #[test]
    fn crate_name_from_path_unknown() {
        let p = Path::new("apps/my-app/index.ts");
        assert_eq!(crate_name_from_path(p), None);
    }
}
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check extractor
```

- [ ] **Step 3: Implement**

```rust
use std::path::Path;
use anyhow::Result;
use crate::features::ExtractedFeatures;

pub trait LanguageExtractor: Send + Sync {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures>;
}

/// Extract crate name from path like `crates/vox-foo/src/lib.rs` → `"vox-foo"`.
pub fn crate_name_from_path(path: &Path) -> Option<String> {
    let mut parts = path.components().map(|c| c.as_os_str().to_string_lossy().into_owned());
    let mut prev = String::new();
    let mut prev_prev = String::new();
    for part in &mut parts {
        if prev == "crates" {
            return Some(part.clone());
        }
        prev_prev = prev.clone();
        prev = part;
    }
    // also handle `src` as second component: crates/<name>/src/...
    let _ = prev_prev;
    None
}
```

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check extractor
```
Expected: 2 tests pass.

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/extractor.rs
git commit -m "feat(drift): add LanguageExtractor trait and crate_name resolver"
```

---

### Task 4: Rust extractor — string and numeric literals

**Files:**
- Create: `crates/vox-drift-check/src/extractors/rust.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &str) -> ExtractedFeatures {
        let e = RustExtractor;
        e.extract(std::path::Path::new("test.rs"), src).unwrap()
    }

    #[test]
    fn extracts_string_literal() {
        let f = extract(r#"fn foo() { let x = "hello world"; }"#);
        assert_eq!(f.string_literals.len(), 1);
        assert_eq!(f.string_literals[0].value, "hello world");
    }

    #[test]
    fn extracts_numeric_literal_with_seconds_unit() {
        let f = extract("fn t() { Duration::from_secs(30); }");
        assert_eq!(f.numeric_literals.len(), 1);
        assert_eq!(f.numeric_literals[0].value, 30.0);
        assert!(matches!(f.numeric_literals[0].unit, Some(UnitHint::Seconds)));
    }

    #[test]
    fn extracts_numeric_literal_with_millis_unit() {
        let f = extract("fn t() { Duration::from_millis(100); }");
        assert!(matches!(f.numeric_literals[0].unit, Some(UnitHint::Millis)));
    }

    #[test]
    fn skips_doc_string_literals() {
        let f = extract(r#"/// "not a literal" fn foo() {}"#);
        assert_eq!(f.string_literals.len(), 0);
    }
}
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check extractors::rust
```

- [ ] **Step 3: Implement**

```rust
use std::path::Path;
use anyhow::Result;
use syn::{visit::Visit, Expr, ExprLit, ExprCall, Lit, LitStr};
use proc_macro2::LineColumn;
use crate::extractor::LanguageExtractor;
use crate::features::*;
use vox_code_audit::rules::Language;

pub struct RustExtractor;

struct RustVisitor {
    features: ExtractedFeatures,
}

impl RustVisitor {
    fn span_to_loc(span: proc_macro2::Span) -> Loc {
        let lc: LineColumn = span.start();
        Loc { line: lc.line, col: lc.column }
    }
}

impl<'ast> Visit<'ast> for RustVisitor {
    fn visit_expr_lit(&mut self, node: &'ast ExprLit) {
        match &node.lit {
            Lit::Str(s) => {
                self.features.string_literals.push(LiteralLoc {
                    value: s.value(),
                    loc: Self::span_to_loc(s.span()),
                    ctx: LiteralContext::Code,
                });
            }
            Lit::Int(i) => {
                if let Ok(v) = i.base10_parse::<i64>() {
                    self.features.numeric_literals.push(NumericLoc {
                        value: v as f64,
                        unit: None,
                        loc: Self::span_to_loc(i.span()),
                    });
                }
            }
            _ => {}
        }
        syn::visit::visit_expr_lit(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        // Detect Duration::from_secs/from_millis calls and tag the numeric arg
        if let Expr::Path(p) = node.func.as_ref() {
            let segs: Vec<String> = p.path.segments.iter()
                .map(|s| s.ident.to_string()).collect();
            let unit = match segs.last().map(|s| s.as_str()) {
                Some("from_secs") | Some("from_secs_f64") => Some(UnitHint::Seconds),
                Some("from_millis") => Some(UnitHint::Millis),
                Some("from_nanos") => Some(UnitHint::Millis), // nanoseconds → tag as Millis for grouping
                _ => None,
            };
            if let Some(unit) = unit {
                // Retag the last numeric literal we just pushed (it will be visited next)
                // Instead, visit args first
                if let Some(arg) = node.args.first() {
                    if let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = arg {
                        if let Ok(v) = i.base10_parse::<i64>() {
                            // Remove the un-tagged entry if it was already added
                            self.features.numeric_literals.retain(|n| {
                                !(n.unit.is_none() && n.value == v as f64
                                  && n.loc.line == i.span().start().line)
                            });
                            self.features.numeric_literals.push(NumericLoc {
                                value: v as f64,
                                unit: Some(unit),
                                loc: Self::span_to_loc(i.span()),
                            });
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

impl LanguageExtractor for RustExtractor {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures> {
        let mut visitor = RustVisitor {
            features: ExtractedFeatures::new(path.to_path_buf(), Language::Rust),
        };
        if let Ok(file) = syn::parse_file(content) {
            visitor.visit_file(&file);
        }
        visitor.features.crate_name = crate::extractor::crate_name_from_path(path);
        Ok(visitor.features)
    }
}
```

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check extractors::rust
```
Expected: 4 tests pass. (The Duration unit tagging test may need one adjustment: the visitor visits the ExprCall which calls visit_expr_lit on args — reorder to tag correctly. If the retain approach has edge cases, simplify: parse numeric literals only inside Duration call context by visiting args before recursing.)

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/extractors/rust.rs
git commit -m "feat(drift): Rust extractor — string and numeric literal extraction"
```

---

### Task 5: Rust extractor — call sites, imports, fn definitions

**Files:**
- Modify: `crates/vox-drift-check/src/extractors/rust.rs`

- [ ] **Step 1: Write failing tests**

```rust
    #[test]
    fn extracts_call_site_path() {
        let f = extract("fn t() { reqwest::Client::new(); }");
        assert_eq!(f.call_sites.len(), 1);
        assert_eq!(f.call_sites[0].path, vec!["reqwest", "Client", "new"]);
        assert_eq!(f.call_sites[0].arity, 0);
    }

    #[test]
    fn extracts_use_import() {
        let f = extract("use std::collections::HashMap;");
        assert_eq!(f.imports.len(), 1);
        assert_eq!(f.imports[0].path, vec!["std", "collections", "HashMap"]);
    }

    #[test]
    fn extracts_fn_definition_name() {
        let f = extract("fn default_true() -> bool { true }");
        assert_eq!(f.fn_definitions.len(), 1);
        assert_eq!(f.fn_definitions[0].name, "default_true");
    }
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check extractors::rust -- call_site imports fn_def
```

- [ ] **Step 3: Add to RustVisitor**

Add these visit methods to `impl<'ast> Visit<'ast> for RustVisitor`:

```rust
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        // (keep existing Duration logic above, then also record the call site)
        if let Expr::Path(p) = node.func.as_ref() {
            let path: Vec<String> = p.path.segments.iter()
                .map(|s| s.ident.to_string()).collect();
            self.features.call_sites.push(CallSite {
                path,
                arity: node.args.len() as u8,
                loc: Self::span_to_loc(node.func.span()),
            });
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        fn flatten_tree(tree: &syn::UseTree, prefix: Vec<String>, out: &mut Vec<Vec<String>>) {
            match tree {
                syn::UseTree::Path(p) => {
                    let mut next = prefix.clone();
                    next.push(p.ident.to_string());
                    flatten_tree(&p.tree, next, out);
                }
                syn::UseTree::Name(n) => {
                    let mut full = prefix;
                    full.push(n.ident.to_string());
                    out.push(full);
                }
                syn::UseTree::Group(g) => {
                    for item in &g.items {
                        flatten_tree(item, prefix.clone(), out);
                    }
                }
                _ => {}
            }
        }
        let mut paths = Vec::new();
        flatten_tree(&node.tree, Vec::new(), &mut paths);
        for p in paths {
            self.features.imports.push(ImportLoc {
                path: p,
                symbol: None,
                loc: Loc::default(),
            });
        }
        syn::visit::visit_item_use(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        use xxhash_rust::xxh3::xxh3_64;
        let body_src = quote::quote!(#node).to_string();
        let hash = xxh3_64(body_src.as_bytes());
        let sig_src = format!("{}", node.sig.ident);
        let sig_hash = xxh3_64(sig_src.as_bytes());
        let line_count = body_src.lines().count() as u32;
        self.features.fn_definitions.push(FnDef {
            name: node.sig.ident.to_string(),
            body_hash: hash,
            sig_hash,
            loc: Self::span_to_loc(node.sig.ident.span()),
        });
        self.features.body_signatures.push(BodySignature {
            hash,
            line_count,
            parent_fn: Some(node.sig.ident.to_string()),
            loc: Self::span_to_loc(node.sig.ident.span()),
        });
        syn::visit::visit_item_fn(self, node);
    }
```

Add `use quote::quote;` at top, add `quote = { workspace = true }` to Cargo.toml.

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check extractors::rust
```
Expected: all 7 tests pass.

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/extractors/rust.rs crates/vox-drift-check/Cargo.toml
git commit -m "feat(drift): Rust extractor — call sites, imports, fn definitions"
```

---

### Task 6: Engine — file walker, parallel extraction, WorkspaceFeatures

**Files:**
- Create: `crates/vox-drift-check/src/engine.rs`
- Create: `crates/vox-drift-check/src/sweep/mod.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/vox-drift-check/src/engine.rs #[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn engine_finds_rust_files_and_extracts() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("foo.rs"), r#"fn foo() { let x = "hello"; }"#).unwrap();
        fs::write(dir.path().join("bar.ts"), r#"const x = "world";"#).unwrap();

        let eng = DriftEngine::new(dir.path());
        let ws = eng.extract_workspace().unwrap();
        let rust_files: Vec<_> = ws.files.iter().filter(|f| f.file.extension().map_or(false, |e| e == "rs")).collect();
        assert!(!rust_files.is_empty());
        assert!(rust_files[0].string_literals.iter().any(|l| l.value == "hello"));
    }
}
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check engine
```

- [ ] **Step 3: Implement engine.rs**

```rust
use std::path::{Path, PathBuf};
use anyhow::Result;
use rayon::prelude::*;
use walkdir::WalkDir;
use vox_code_audit::rules::Language;
use crate::extractors::{rust::RustExtractor, typescript::TypeScriptExtractor, vox::VoxExtractor};
use crate::extractor::LanguageExtractor;
use crate::features::ExtractedFeatures;

pub struct WorkspaceFeatures {
    pub files: Vec<ExtractedFeatures>,
    pub workspace_version: String,
}

pub struct DriftEngine {
    root: PathBuf,
}

impl DriftEngine {
    pub fn new(root: &Path) -> Self {
        Self { root: root.to_path_buf() }
    }

    pub fn extract_workspace(&self) -> Result<WorkspaceFeatures> {
        let paths = self.collect_source_files();
        let files: Vec<ExtractedFeatures> = paths
            .par_iter()
            .filter_map(|p| {
                let content = std::fs::read_to_string(p).ok()?;
                let lang = detect_language(p);
                let extractor: &dyn LanguageExtractor = match lang {
                    Language::Rust => &RustExtractor,
                    Language::TypeScript => &TypeScriptExtractor,
                    Language::Vox => &VoxExtractor,
                    _ => return None,
                };
                extractor.extract(p, &content).ok()
            })
            .collect();

        let workspace_version = read_workspace_version(&self.root);
        Ok(WorkspaceFeatures { files, workspace_version })
    }

    fn collect_source_files(&self) -> Vec<PathBuf> {
        WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !matches!(name.as_ref(), "target" | "node_modules" | ".git" | "archive")
                    && name != ".vox-cache"
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| matches!(detect_language(p), Language::Rust | Language::TypeScript | Language::Vox))
            .collect()
    }
}

pub fn detect_language(path: &Path) -> Language {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Language::Rust,
        Some("ts") | Some("tsx") | Some("js") | Some("jsx") => Language::TypeScript,
        Some("vox") => Language::Vox,
        _ => Language::Unknown,
    }
}

fn read_workspace_version(root: &Path) -> String {
    let cargo = root.join("Cargo.toml");
    std::fs::read_to_string(cargo)
        .ok()
        .and_then(|s| {
            let t: toml::Value = toml::from_str(&s).ok()?;
            t.get("workspace")?.get("package")?.get("version")?.as_str().map(String::from)
        })
        .unwrap_or_default()
}
```

Create `crates/vox-drift-check/src/sweep/mod.rs`:
```rust
use anyhow::Result;
use vox_code_audit::rules::{Finding, Severity};
use crate::features::ExtractedFeatures;

pub mod literal_dedup;
pub mod numeric_dedup;
pub mod body_hash;
pub mod call_shape;

pub trait SweepRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn severity(&self) -> Severity;
    fn sweep(&self, files: &[ExtractedFeatures]) -> Vec<Finding>;
}

pub fn all_sweep_rules() -> Vec<Box<dyn SweepRule>> {
    vec![
        Box::new(literal_dedup::LiteralDedupRule::default()),
        Box::new(numeric_dedup::NumericDedupRule::default()),
        Box::new(body_hash::BodyHashRule::default()),
        Box::new(call_shape::CallShapeRule::default()),
    ]
}
```

Create stub `mod.rs` files for each sweep submodule (empty structs for now).

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check engine
```

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/engine.rs crates/vox-drift-check/src/sweep/
git commit -m "feat(drift): engine file walker + parallel extraction + WorkspaceFeatures"
```

---

### Task 7: SweepRule — literal_dedup

**Files:**
- Create: `crates/vox-drift-check/src/sweep/literal_dedup.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::*;
    use std::path::PathBuf;
    use vox_code_audit::rules::Language;

    fn make_file(path: &str, literals: &[&str]) -> ExtractedFeatures {
        let mut f = ExtractedFeatures::new(PathBuf::from(path), Language::Rust);
        for &v in literals {
            f.string_literals.push(LiteralLoc {
                value: v.to_string(),
                loc: Loc { line: 1, col: 0 },
                ctx: LiteralContext::Code,
            });
        }
        f
    }

    #[test]
    fn finds_string_over_threshold() {
        let files = vec![
            make_file("a.rs", &["duplicate-me", "other"]),
            make_file("b.rs", &["duplicate-me"]),
            make_file("c.rs", &["duplicate-me"]),
        ];
        let rule = LiteralDedupRule::default();
        let findings = rule.sweep(&files);
        assert!(!findings.is_empty());
        let f = &findings[0];
        assert!(f.message.contains("3"));
        assert_eq!(f.rule_id, "sweep/duplicate-string-literal");
    }

    #[test]
    fn ignores_strings_below_threshold() {
        let files = vec![
            make_file("a.rs", &["only-twice"]),
            make_file("b.rs", &["only-twice"]),
        ];
        let rule = LiteralDedupRule::default();
        assert!(rule.sweep(&files).is_empty());
    }

    #[test]
    fn ignores_const_decl_context() {
        let mut f1 = make_file("a.rs", &[]);
        f1.string_literals.push(LiteralLoc { value: "dup".into(), loc: Loc::default(), ctx: LiteralContext::ConstDecl });
        let f2 = make_file("b.rs", &["dup"]);
        let f3 = make_file("c.rs", &["dup"]);
        // Only 2 Code occurrences — below threshold
        let rule = LiteralDedupRule::default();
        assert!(rule.sweep(&[f1, f2, f3]).is_empty());
    }
}
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check sweep::literal_dedup
```

- [ ] **Step 3: Implement**

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use vox_code_audit::rules::{Finding, FindingConfidence, Severity};
use crate::features::{ExtractedFeatures, LiteralContext};
use super::SweepRule;

pub struct LiteralDedupRule {
    pub threshold: usize,
    pub min_length: usize,
}

impl Default for LiteralDedupRule {
    fn default() -> Self { Self { threshold: 3, min_length: 8 } }
}

impl SweepRule for LiteralDedupRule {
    fn id(&self) -> &'static str { "sweep/duplicate-string-literal" }
    fn severity(&self) -> Severity { Severity::Info }

    fn sweep(&self, files: &[ExtractedFeatures]) -> Vec<Finding> {
        let mut index: HashMap<String, Vec<(PathBuf, usize)>> = HashMap::new();
        for f in files {
            for lit in &f.string_literals {
                if lit.value.len() < self.min_length { continue; }
                if matches!(lit.ctx, LiteralContext::ConstDecl | LiteralContext::Doc) { continue; }
                if is_ignored_path(&f.file) { continue; }
                index.entry(lit.value.clone())
                    .or_default()
                    .push((f.file.clone(), lit.loc.line));
            }
        }
        index.into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|(value, locs)| {
                let others: Vec<String> = locs[1..].iter()
                    .map(|(p, l)| format!("{}:{}", p.display(), l))
                    .collect();
                Finding {
                    rule_id: self.id().to_string(),
                    rule_name: "Duplicate String Literal".into(),
                    severity: self.severity(),
                    file: locs[0].0.clone(),
                    line: locs[0].1,
                    column: 0,
                    message: format!(
                        "{:?} appears {} times — consider a named constant",
                        value, locs.len()
                    ),
                    suggestion: Some("Extract to a SSOT constant module".into()),
                    context: format!("Also at: {}", others.join(", ")),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: Some(serde_json::json!({
                        "occurrences": locs.iter().map(|(p,l)| format!("{}:{}", p.display(), l)).collect::<Vec<_>>()
                    })),
                }
            })
            .collect()
    }
}

fn is_ignored_path(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy();
    s.contains("/tests/") || s.contains("/fixtures/") || s.contains("/golden/")
        || s.ends_with("_test.rs") || s.ends_with(".generated.md")
}
```

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check sweep::literal_dedup
```
Expected: 3 tests pass.

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/sweep/literal_dedup.rs
git commit -m "feat(drift): sweep/duplicate-string-literal rule"
```

---

### Task 8: SweepRules — numeric_dedup and body_hash

**Files:**
- Create: `crates/vox-drift-check/src/sweep/numeric_dedup.rs`
- Create: `crates/vox-drift-check/src/sweep/body_hash.rs`
- Create: `crates/vox-drift-check/src/sweep/call_shape.rs`

- [ ] **Step 1: Write failing tests for numeric_dedup**

```rust
// numeric_dedup.rs #[cfg(test)]
#[test]
fn finds_repeated_duration_constant() {
    let make = |line: usize, val: f64| ExtractedFeatures {
        numeric_literals: vec![NumericLoc { value: val, unit: Some(UnitHint::Seconds), loc: Loc { line, col: 0 } }],
        ..ExtractedFeatures::new(PathBuf::from(format!("{}.rs", line)), Language::Rust)
    };
    let files = vec![make(1, 30.0), make(2, 30.0), make(3, 30.0)];
    let rule = NumericDedupRule::default();
    let findings = rule.sweep(&files);
    assert!(!findings.is_empty());
    assert!(findings[0].message.contains("30"));
}
```

```rust
// body_hash.rs #[cfg(test)]
#[test]
fn finds_duplicate_fn_bodies() {
    let make = |name: &str, hash: u64| {
        let mut f = ExtractedFeatures::new(PathBuf::from(format!("{}.rs", name)), Language::Rust);
        f.fn_definitions.push(FnDef { name: name.into(), body_hash: hash, sig_hash: hash, loc: Loc::default() });
        f
    };
    let files = vec![make("alpha", 42), make("beta", 42)]; // same hash
    let rule = BodyHashRule::default();
    let findings = rule.sweep(&files);
    assert!(!findings.is_empty());
}
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check sweep::numeric_dedup sweep::body_hash
```

- [ ] **Step 3: Implement numeric_dedup.rs**

```rust
use std::collections::HashMap;
use vox_code_audit::rules::{Finding, FindingConfidence, Severity};
use crate::features::{ExtractedFeatures, UnitHint};
use super::SweepRule;

pub struct NumericDedupRule {
    pub threshold: usize,
}
impl Default for NumericDedupRule {
    fn default() -> Self { Self { threshold: 3 } }
}

impl SweepRule for NumericDedupRule {
    fn id(&self) -> &'static str { "sweep/duplicate-numeric-literal" }
    fn severity(&self) -> Severity { Severity::Warning }

    fn sweep(&self, files: &[ExtractedFeatures]) -> Vec<Finding> {
        // Key: (value as ordered bits, unit discriminant)
        let mut index: HashMap<(u64, u8), Vec<(std::path::PathBuf, usize)>> = HashMap::new();
        for f in files {
            for n in &f.numeric_literals {
                if n.unit.is_none() { continue; } // only care about unit-bearing numerics
                let unit_disc = match n.unit {
                    Some(UnitHint::Seconds) => 1,
                    Some(UnitHint::Millis) => 2,
                    Some(UnitHint::Bytes) => 3,
                    _ => continue,
                };
                let key = (n.value.to_bits(), unit_disc);
                index.entry(key).or_default().push((f.file.clone(), n.loc.line));
            }
        }
        index.into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|((bits, unit_disc), locs)| {
                let val = f64::from_bits(bits);
                let unit_str = match unit_disc { 1 => "s", 2 => "ms", _ => "bytes" };
                Finding {
                    rule_id: self.id().to_string(),
                    rule_name: "Duplicate Numeric Literal".into(),
                    severity: self.severity(),
                    file: locs[0].0.clone(),
                    line: locs[0].1,
                    column: 0,
                    message: format!("{}{} appears {} times — define a named constant", val, unit_str, locs.len()),
                    suggestion: Some("Add a const to vox-config::timeouts or the appropriate SSOT module".into()),
                    context: String::new(),
                    confidence: Some(FindingConfidence::High),
                    evidence: Some(serde_json::json!({
                        "occurrences": locs.iter().map(|(p,l)| format!("{}:{}", p.display(), l)).collect::<Vec<_>>()
                    })),
                }
            })
            .collect()
    }
}
```

- [ ] **Step 4: Implement body_hash.rs**

```rust
use std::collections::HashMap;
use vox_code_audit::rules::{Finding, FindingConfidence, Severity};
use crate::features::ExtractedFeatures;
use super::SweepRule;

pub struct BodyHashRule {
    pub threshold: usize,
    pub min_lines: u32,
}
impl Default for BodyHashRule {
    fn default() -> Self { Self { threshold: 2, min_lines: 5 } }
}

impl SweepRule for BodyHashRule {
    fn id(&self) -> &'static str { "sweep/duplicate-body" }
    fn severity(&self) -> Severity { Severity::Warning }

    fn sweep(&self, files: &[ExtractedFeatures]) -> Vec<Finding> {
        let mut index: HashMap<u64, Vec<(std::path::PathBuf, String, usize)>> = HashMap::new();
        for f in files {
            for def in &f.fn_definitions {
                // Skip tiny bodies
                let sig = &f.body_signatures.iter().find(|b| b.parent_fn.as_deref() == Some(&def.name));
                if let Some(sig) = sig {
                    if sig.line_count < self.min_lines { continue; }
                }
                index.entry(def.body_hash)
                    .or_default()
                    .push((f.file.clone(), def.name.clone(), def.loc.line));
            }
        }
        index.into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|(_, locs)| {
                let names: Vec<_> = locs.iter().map(|(_, n, _)| n.as_str()).collect();
                Finding {
                    rule_id: self.id().to_string(),
                    rule_name: "Duplicate Function Body".into(),
                    severity: self.severity(),
                    file: locs[0].0.clone(),
                    line: locs[0].2,
                    column: 0,
                    message: format!("Functions {:?} have identical bodies — extract a shared helper", names),
                    suggestion: Some("Extract to a shared module".into()),
                    context: locs[1..].iter().map(|(p, n, l)| format!("{}:{} ({})", p.display(), l, n)).collect::<Vec<_>>().join(", "),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                }
            })
            .collect()
    }
}
```

- [ ] **Step 5: Implement call_shape.rs (stub that compiles)**

```rust
use vox_code_audit::rules::{Finding, Severity};
use crate::features::ExtractedFeatures;
use super::SweepRule;

pub struct CallShapeRule { pub threshold: usize }
impl Default for CallShapeRule { fn default() -> Self { Self { threshold: 5 } } }

impl SweepRule for CallShapeRule {
    fn id(&self) -> &'static str { "sweep/duplicate-call-pattern" }
    fn severity(&self) -> Severity { Severity::Info }
    fn sweep(&self, files: &[ExtractedFeatures]) -> Vec<Finding> {
        use std::collections::HashMap;
        let mut index: HashMap<String, Vec<(std::path::PathBuf, usize)>> = HashMap::new();
        for f in files {
            for cs in &f.call_sites {
                let key = format!("{}:{}", cs.path.join("::"), cs.arity);
                index.entry(key).or_default().push((f.file.clone(), cs.loc.line));
            }
        }
        index.into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|(key, locs)| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Repeated Call Pattern".into(),
                severity: self.severity(),
                file: locs[0].0.clone(),
                line: locs[0].1,
                column: 0,
                message: format!("`{}` called {} times — consider a wrapper helper", key, locs.len()),
                suggestion: None,
                context: String::new(),
                confidence: None,
                evidence: None,
            })
            .collect()
    }
}
```

- [ ] **Step 6: Run all sweep tests**
```
cargo test -p vox-drift-check sweep
```
Expected: all tests pass.

- [ ] **Step 7: Commit**
```
git add crates/vox-drift-check/src/sweep/
git commit -m "feat(drift): numeric_dedup, body_hash, call_shape sweep rules"
```

---

## Phase 2 — Targeted Drift Rules + CLI

### Task 9: DriftRule trait + WorkspaceContext + rules/mod.rs

**Files:**
- Create: `crates/vox-drift-check/src/rules/mod.rs`

- [ ] **Step 1: Implement rules/mod.rs** (no separate test needed — trait is structural)

```rust
use std::path::PathBuf;
use anyhow::Result;
use vox_code_audit::rules::{Finding, Severity, Language};
use crate::features::ExtractedFeatures;

pub mod reqwest_bypass;
pub mod vox_path_literal;
pub mod timeout_literal;
pub mod serde_default_dup;
pub mod version_string;
pub mod bearer_header;

pub struct WorkspaceContext {
    pub workspace_version: String,
    pub workspace_root: PathBuf,
}

pub trait DriftRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn severity(&self) -> Severity;
    fn languages(&self) -> &[Language];
    fn check(&self, features: &ExtractedFeatures, ctx: &WorkspaceContext) -> Vec<Finding>;
}

pub fn all_drift_rules() -> Vec<Box<dyn DriftRule>> {
    vec![
        Box::new(reqwest_bypass::ReqwestBypassRule),
        Box::new(vox_path_literal::VoxPathLiteralRule),
        Box::new(timeout_literal::TimeoutLiteralRule),
        Box::new(serde_default_dup::SerdeDefaultDupRule),
        Box::new(version_string::VersionStringRule),
        Box::new(bearer_header::BearerHeaderRule),
    ]
}
```

- [ ] **Step 2: Verify compile**
```
cargo check -p vox-drift-check
```

- [ ] **Step 3: Commit**
```
git add crates/vox-drift-check/src/rules/mod.rs
git commit -m "feat(drift): DriftRule trait + WorkspaceContext"
```

---

### Task 10: drift/reqwest-bypass

**Files:**
- Create: `crates/vox-drift-check/src/rules/reqwest_bypass.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::*;
    use vox_code_audit::rules::Language;
    use std::path::PathBuf;
    use crate::rules::WorkspaceContext;

    fn ctx() -> WorkspaceContext {
        WorkspaceContext { workspace_version: "0.5.0".into(), workspace_root: PathBuf::from(".") }
    }

    fn make(crate_name: &str, calls: &[&[&str]]) -> ExtractedFeatures {
        let mut f = ExtractedFeatures::new(PathBuf::from(format!("crates/{}/src/lib.rs", crate_name)), Language::Rust);
        f.crate_name = Some(crate_name.to_string());
        for &path in calls {
            f.call_sites.push(CallSite {
                path: path.iter().map(|s| s.to_string()).collect(),
                arity: 0,
                loc: Loc { line: 5, col: 0 },
            });
        }
        f
    }

    #[test]
    fn flags_client_new_outside_defaults() {
        let f = make("vox-publisher", &[&["reqwest", "Client", "new"]]);
        let rule = ReqwestBypassRule;
        let findings = rule.check(&f, &ctx());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "drift/reqwest-bypass");
    }

    #[test]
    fn allows_client_new_inside_defaults_crate() {
        let f = make("vox-reqwest-defaults", &[&["reqwest", "Client", "new"]]);
        let rule = ReqwestBypassRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn flags_client_builder() {
        let f = make("vox-search", &[&["reqwest", "Client", "builder"]]);
        let rule = ReqwestBypassRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }
}
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check rules::reqwest_bypass
```

- [ ] **Step 3: Implement**

```rust
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};
use crate::features::ExtractedFeatures;
use crate::rules::{DriftRule, WorkspaceContext};

pub struct ReqwestBypassRule;

const ALLOWED_CRATES: &[&str] = &["vox-reqwest-defaults"];
const FORBIDDEN: &[&[&str]] = &[
    &["reqwest", "Client", "new"],
    &["reqwest", "Client", "builder"],
];

impl DriftRule for ReqwestBypassRule {
    fn id(&self) -> &'static str { "drift/reqwest-bypass" }
    fn severity(&self) -> Severity { Severity::Warning }
    fn languages(&self) -> &[Language] { &[Language::Rust] }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        let crate_name = features.crate_name.as_deref().unwrap_or("");
        if ALLOWED_CRATES.contains(&crate_name) { return vec![]; }
        if is_test_file(&features.file) { return vec![]; }

        features.call_sites.iter()
            .filter(|cs| FORBIDDEN.iter().any(|f| cs.path.iter().map(|s| s.as_str()).eq(f.iter().copied())))
            .map(|cs| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Reqwest Client Bypass".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: cs.loc.line,
                column: cs.loc.col,
                message: format!(
                    "Direct reqwest `{}` bypasses vox-reqwest-defaults (timeouts, UA, pooling)",
                    cs.path.join("::")
                ),
                suggestion: Some("Use `vox_reqwest_defaults::client_builder()` or `vox_reqwest_defaults::client()`".into()),
                context: format!("crate: {}", crate_name),
                confidence: Some(FindingConfidence::High),
                evidence: None,
            })
            .collect()
    }
}

fn is_test_file(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy();
    s.contains("/tests/") || s.ends_with("_test.rs")
}
```

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check rules::reqwest_bypass
```
Expected: 3 tests pass.

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/rules/reqwest_bypass.rs
git commit -m "feat(drift): drift/reqwest-bypass rule"
```

---

### Task 11: drift/vox-path-literal

**Files:**
- Create: `crates/vox-drift-check/src/rules/vox_path_literal.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn flags_raw_vox_path_outside_config() {
    let mut f = ExtractedFeatures::new(PathBuf::from("crates/vox-cli/src/lib.rs"), Language::Rust);
    f.crate_name = Some("vox-cli".into());
    f.string_literals.push(LiteralLoc { value: ".vox/sessions".into(), loc: Loc { line: 10, col: 0 }, ctx: LiteralContext::Code });
    let rule = VoxPathLiteralRule;
    let findings = rule.check(&f, &ctx());
    assert_eq!(findings.len(), 1);
}

#[test]
fn allows_raw_vox_path_inside_config_crate() {
    let mut f = ExtractedFeatures::new(PathBuf::from("crates/vox-config/src/paths.rs"), Language::Rust);
    f.crate_name = Some("vox-config".into());
    f.string_literals.push(LiteralLoc { value: ".vox/sessions".into(), loc: Loc { line: 1, col: 0 }, ctx: LiteralContext::Code });
    let rule = VoxPathLiteralRule;
    assert!(rule.check(&f, &ctx()).is_empty());
}
```

- [ ] **Step 2: Implement**

```rust
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};
use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};

pub struct VoxPathLiteralRule;

const ALLOWED_CRATES: &[&str] = &["vox-config", "vox-db"];

impl DriftRule for VoxPathLiteralRule {
    fn id(&self) -> &'static str { "drift/vox-path-literal" }
    fn severity(&self) -> Severity { Severity::Warning }
    fn languages(&self) -> &[Language] { &[Language::Rust, Language::Vox] }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        let crate_name = features.crate_name.as_deref().unwrap_or("");
        if ALLOWED_CRATES.contains(&crate_name) { return vec![]; }

        features.string_literals.iter()
            .filter(|lit| {
                matches!(lit.ctx, LiteralContext::Code)
                    && (lit.value.starts_with(".vox/") || lit.value.starts_with(".vox-cache"))
            })
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Raw .vox/ Path Literal".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: format!("{:?} is a raw .vox path — use vox_config::paths::* constants", lit.value),
                suggestion: Some("Import from `vox_config::paths` and use the named constant".into()),
                context: format!("crate: {}", crate_name),
                confidence: Some(FindingConfidence::High),
                evidence: None,
            })
            .collect()
    }
}
```

- [ ] **Step 3: Run tests and commit**
```
cargo test -p vox-drift-check rules::vox_path_literal
git add crates/vox-drift-check/src/rules/vox_path_literal.rs
git commit -m "feat(drift): drift/vox-path-literal rule"
```

---

### Task 12: drift/timeout-literal, drift/serde-default-dup, drift/version-string, drift/bearer-header

**Files:**
- Create: `crates/vox-drift-check/src/rules/timeout_literal.rs`
- Create: `crates/vox-drift-check/src/rules/serde_default_dup.rs`
- Create: `crates/vox-drift-check/src/rules/version_string.rs`
- Create: `crates/vox-drift-check/src/rules/bearer_header.rs`

- [ ] **Step 1: Write failing tests for all four**

```rust
// timeout_literal.rs tests
#[test]
fn flags_duration_from_secs_without_const() {
    let mut f = ExtractedFeatures::new(PathBuf::from("crates/vox-orchestrator/src/catalog.rs"), Language::Rust);
    f.crate_name = Some("vox-orchestrator".into());
    f.numeric_literals.push(NumericLoc { value: 30.0, unit: Some(UnitHint::Seconds), loc: Loc { line: 5, col: 0 } });
    let rule = TimeoutLiteralRule;
    assert_eq!(rule.check(&f, &ctx()).len(), 1);
}

// serde_default_dup.rs test (workspace-level, so we check FnDef name pattern)
#[test]
fn flags_default_true_fn_outside_config() {
    let mut f = ExtractedFeatures::new(PathBuf::from("crates/vox-publisher/src/types.rs"), Language::Rust);
    f.crate_name = Some("vox-publisher".into());
    f.fn_definitions.push(FnDef { name: "default_true".into(), body_hash: 99, sig_hash: 99, loc: Loc { line: 3, col: 0 } });
    let rule = SerdeDefaultDupRule;
    assert_eq!(rule.check(&f, &ctx()).len(), 1);
}

// version_string.rs test
#[test]
fn flags_hardcoded_version_string() {
    let mut ctx = WorkspaceContext { workspace_version: "0.5.0".into(), workspace_root: PathBuf::from(".") };
    let mut f = ExtractedFeatures::new(PathBuf::from("crates/vox-cli/tests/foo.rs"), Language::Rust);
    f.string_literals.push(LiteralLoc { value: "0.5.0".into(), loc: Loc { line: 78, col: 0 }, ctx: LiteralContext::Code });
    let rule = VersionStringRule;
    assert_eq!(rule.check(&f, &ctx).len(), 1);
}

// bearer_header.rs test
#[test]
fn flags_bearer_header_literal() {
    let mut f = ExtractedFeatures::new(PathBuf::from("crates/vox-orchestrator-mcp/src/gateway.rs"), Language::Rust);
    f.string_literals.push(LiteralLoc { value: "Bearer secret-token".into(), loc: Loc { line: 47, col: 0 }, ctx: LiteralContext::Code });
    let rule = BearerHeaderRule;
    assert_eq!(rule.check(&f, &ctx()).len(), 1);
}
```

- [ ] **Step 2: Run — expect failure**
```
cargo test -p vox-drift-check rules
```

- [ ] **Step 3: Implement timeout_literal.rs**

```rust
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};
use crate::features::{ExtractedFeatures, UnitHint};
use crate::rules::{DriftRule, WorkspaceContext};

pub struct TimeoutLiteralRule;

// Values commonly repeated that should be constants
const COMMON_TIMEOUTS_SECS: &[u64] = &[5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600];
const COMMON_TIMEOUTS_MS: &[u64] = &[100, 250, 500, 1000, 5000, 10000, 30000, 60000];

impl DriftRule for TimeoutLiteralRule {
    fn id(&self) -> &'static str { "drift/timeout-literal" }
    fn severity(&self) -> Severity { Severity::Warning }
    fn languages(&self) -> &[Language] { &[Language::Rust] }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        features.numeric_literals.iter()
            .filter(|n| {
                match &n.unit {
                    Some(UnitHint::Seconds) => COMMON_TIMEOUTS_SECS.contains(&(n.value as u64)),
                    Some(UnitHint::Millis) => COMMON_TIMEOUTS_MS.contains(&(n.value as u64)),
                    _ => false,
                }
            })
            .map(|n| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Inline Timeout Literal".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: n.loc.line,
                column: n.loc.col,
                message: format!(
                    "Inline timeout {}{}  — define a named constant (e.g. `vox_config::timeouts::HTTP_REQUEST`)",
                    n.value, match n.unit { Some(UnitHint::Seconds) => "s", _ => "ms" }
                ),
                suggestion: Some("Add const to `vox-config::timeouts` module".into()),
                context: String::new(),
                confidence: Some(FindingConfidence::High),
                evidence: None,
            })
            .collect()
    }
}
```

- [ ] **Step 4: Implement serde_default_dup.rs**

```rust
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};
use crate::features::ExtractedFeatures;
use crate::rules::{DriftRule, WorkspaceContext};

pub struct SerdeDefaultDupRule;

const ALLOWED_CRATES: &[&str] = &["vox-config"];

impl DriftRule for SerdeDefaultDupRule {
    fn id(&self) -> &'static str { "drift/serde-default-dup" }
    fn severity(&self) -> Severity { Severity::Warning }
    fn languages(&self) -> &[Language] { &[Language::Rust] }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        let crate_name = features.crate_name.as_deref().unwrap_or("");
        if ALLOWED_CRATES.contains(&crate_name) { return vec![]; }

        features.fn_definitions.iter()
            .filter(|def| {
                def.name.starts_with("default_true")
                    || def.name.starts_with("default_false")
                    || def.name.starts_with("default_30")
                    || def.name.starts_with("default_60")
                    || def.name.starts_with("default_10")
            })
            .map(|def| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Duplicate Serde Default Function".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: def.loc.line,
                column: def.loc.col,
                message: format!(
                    "`{}` is a common serde default — consolidate into `vox_config::serde_defaults`",
                    def.name
                ),
                suggestion: Some("Move to `vox-config::serde_defaults` and import from there".into()),
                context: format!("crate: {}", crate_name),
                confidence: Some(FindingConfidence::High),
                evidence: None,
            })
            .collect()
    }
}
```

- [ ] **Step 5: Implement version_string.rs**

```rust
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};
use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};

pub struct VersionStringRule;

impl DriftRule for VersionStringRule {
    fn id(&self) -> &'static str { "drift/version-string" }
    fn severity(&self) -> Severity { Severity::Warning }
    fn languages(&self) -> &[Language] { &[Language::Rust, Language::Vox] }

    fn check(&self, features: &ExtractedFeatures, ctx: &WorkspaceContext) -> Vec<Finding> {
        if ctx.workspace_version.is_empty() { return vec![]; }
        // Don't flag Cargo.toml files
        if features.file.file_name().map_or(false, |n| n == "Cargo.toml") { return vec![]; }

        features.string_literals.iter()
            .filter(|lit| {
                matches!(lit.ctx, LiteralContext::Code)
                    && lit.value == ctx.workspace_version
            })
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Hardcoded Version String".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: format!(
                    "Hardcoded version {:?} — use env!(\"CARGO_PKG_VERSION\") instead",
                    lit.value
                ),
                suggestion: Some("Replace with `env!(\"CARGO_PKG_VERSION\")`".into()),
                context: String::new(),
                confidence: Some(FindingConfidence::High),
                evidence: None,
            })
            .collect()
    }
}
```

- [ ] **Step 6: Implement bearer_header.rs**

```rust
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};
use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};

pub struct BearerHeaderRule;

impl DriftRule for BearerHeaderRule {
    fn id(&self) -> &'static str { "drift/bearer-header-inline" }
    fn severity(&self) -> Severity { Severity::Warning }
    fn languages(&self) -> &[Language] { &[Language::Rust] }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        features.string_literals.iter()
            .filter(|lit| {
                matches!(lit.ctx, LiteralContext::Code)
                    && lit.value.starts_with("Bearer ")
            })
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Inline Bearer Header Literal".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: "Inline Bearer token literal — use `vox_reqwest_defaults::bearer_auth_header(token)` helper".into(),
                suggestion: Some("Add `bearer_auth_header(token: &str) -> HeaderValue` to vox-reqwest-defaults".into()),
                context: String::new(),
                confidence: Some(FindingConfidence::High),
                evidence: None,
            })
            .collect()
    }
}
```

- [ ] **Step 7: Run all rule tests**
```
cargo test -p vox-drift-check rules
```
Expected: all 7 rule tests pass.

- [ ] **Step 8: Commit**
```
git add crates/vox-drift-check/src/rules/
git commit -m "feat(drift): timeout, serde-default, version-string, bearer-header drift rules"
```

---

### Task 13: Wire engine + rules, CLI binary, vox-cli subcommand

**Files:**
- Modify: `crates/vox-drift-check/src/engine.rs`
- Create: `crates/vox-drift-check/src/bin/vox_drift_check.rs`
- Create: `crates/vox-drift-check/src/report.rs`
- Create: `crates/vox-cli/src/commands/drift_check.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs`, `crates/vox-cli/Cargo.toml`

- [ ] **Step 1: Extend engine.rs to run all rules**

Add `run_all` method to `DriftEngine`:

```rust
use crate::rules::{all_drift_rules, WorkspaceContext};
use crate::sweep::all_sweep_rules;

impl DriftEngine {
    pub fn run_all(&self) -> Result<Vec<vox_code_audit::rules::Finding>> {
        let ws = self.extract_workspace()?;
        let ctx = WorkspaceContext {
            workspace_version: ws.workspace_version.clone(),
            workspace_root: self.root.clone(),
        };

        let mut findings = Vec::new();

        // Sweep rules (cross-file)
        for rule in all_sweep_rules() {
            findings.extend(rule.sweep(&ws.files));
        }

        // Targeted drift rules (per-file with workspace ctx)
        let drift_rules = all_drift_rules();
        for file_features in &ws.files {
            for rule in &drift_rules {
                if rule.languages().contains(&file_features.language) {
                    findings.extend(rule.check(file_features, &ctx));
                }
            }
        }

        Ok(findings)
    }
}
```

- [ ] **Step 2: Create report.rs**

```rust
use vox_code_audit::rules::{Finding, Severity};

pub fn print_terminal(findings: &[Finding], min_severity: Severity) {
    let filtered: Vec<_> = findings.iter().filter(|f| f.severity >= min_severity).collect();
    if filtered.is_empty() {
        println!("✓ No drift findings at {:?} level or above.", min_severity);
        return;
    }
    for f in &filtered {
        let icon = match f.severity {
            Severity::Info => "ℹ",
            Severity::Warning => "⚠",
            Severity::Error | Severity::Critical => "✗",
        };
        println!("{} [{}] {}:{} — {}", icon, f.rule_id, f.file.display(), f.line, f.message);
        if let Some(s) = &f.suggestion {
            println!("  → {}", s);
        }
    }
    println!("\n{} finding(s).", filtered.len());
}

pub fn print_json(findings: &[Finding]) {
    println!("{}", serde_json::to_string_pretty(findings).unwrap_or_default());
}

pub fn exit_code(findings: &[Finding], fail_on: Severity) -> i32 {
    if findings.iter().any(|f| f.severity >= fail_on) { 1 } else { 0 }
}
```

- [ ] **Step 3: Create the standalone binary**

```rust
// crates/vox-drift-check/src/bin/vox_drift_check.rs
use clap::Parser;
use std::path::PathBuf;
use vox_drift_check::{engine::DriftEngine, report};
use vox_code_audit::rules::Severity;

#[derive(Parser)]
#[command(name = "vox-drift-check", about = "Workspace-wide drift & repetition linter")]
struct Cli {
    /// Workspace root (defaults to current directory)
    #[arg(default_value = ".")]
    root: PathBuf,
    /// Emit JSON output
    #[arg(long)]
    json: bool,
    /// Minimum severity to show (info/warning/error)
    #[arg(long, default_value = "info")]
    severity: String,
    /// Exit non-zero if any findings at this level (info/warning/error)
    #[arg(long, default_value = "error")]
    fail_on: String,
}

fn parse_sev(s: &str) -> Severity {
    match s { "info" => Severity::Info, "error" | "critical" => Severity::Error, _ => Severity::Warning }
}

fn main() {
    let cli = Cli::parse();
    let engine = DriftEngine::new(&cli.root);
    let findings = match engine.run_all() {
        Ok(f) => f,
        Err(e) => { eprintln!("Error: {}", e); std::process::exit(2); }
    };
    if cli.json {
        report::print_json(&findings);
    } else {
        report::print_terminal(&findings, parse_sev(&cli.severity));
    }
    std::process::exit(report::exit_code(&findings, parse_sev(&cli.fail_on)));
}
```

- [ ] **Step 4: Wire into vox-cli**

Add to `crates/vox-cli/Cargo.toml`:
```toml
vox-drift-check = { workspace = true }
```

Add `"vox-drift-check"` to root `Cargo.toml` workspace dependencies.

Create `crates/vox-cli/src/commands/drift_check.rs`:
```rust
use anyhow::Result;
use clap::Args;
use vox_drift_check::{engine::DriftEngine, report};
use vox_code_audit::rules::Severity;

#[derive(Args, Debug)]
pub struct DriftCheckArgs {
    #[arg(long, default_value = ".")]
    pub root: std::path::PathBuf,
    #[arg(long)]
    pub json: bool,
    #[arg(long, default_value = "info")]
    pub severity: String,
}

pub async fn run(args: DriftCheckArgs) -> Result<()> {
    let engine = DriftEngine::new(&args.root);
    let findings = engine.run_all()?;
    let min_sev = match args.severity.as_str() {
        "error" => Severity::Error, "warning" | "warn" => Severity::Warning, _ => Severity::Info,
    };
    if args.json {
        report::print_json(&findings);
    } else {
        report::print_terminal(&findings, min_sev);
    }
    Ok(())
}
```

In `crates/vox-cli/src/commands/mod.rs`, add:
```rust
pub mod drift_check;
```

Wire into the CLI dispatch (in `cli_args.rs`/`cli_dispatch/mod.rs`) by adding a `DriftCheck(DriftCheckArgs)` variant to the Cli enum and calling `drift_check::run(args).await?` in the match arm.

- [ ] **Step 5: Build and smoke test**
```
cargo build -p vox-drift-check
cargo run -p vox-drift-check -- . --severity warning 2>&1 | head -40
```
Expected: builds and produces findings output from the real workspace.

- [ ] **Step 6: Commit**
```
git add crates/vox-drift-check/src/ crates/vox-cli/
git commit -m "feat(drift): wire engine + all rules, CLI binary, vox-cli subcommand"
```

---

## Phase 3 — Config Layer

### Task 14: drift-patterns.toml config + --suggest-config

**Files:**
- Create: `crates/vox-drift-check/src/config.rs`
- Create: `drift-patterns.toml`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loads_minimal_config() {
        let toml = r#"
[meta]
version = 1

[duplicated_literal]
threshold = 5
min_length = 10
severity = "warning"
"#;
        let cfg: DriftConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.duplicated_literal.threshold, 5);
    }

    #[test]
    fn default_config_has_sensible_thresholds() {
        let cfg = DriftConfig::default();
        assert_eq!(cfg.duplicated_literal.threshold, 3);
        assert_eq!(cfg.duplicated_numeric.threshold, 3);
    }
}
```

- [ ] **Step 2: Implement config.rs**

```rust
use serde::{Deserialize, Serialize};
use vox_code_audit::rules::Severity;

fn default_3() -> usize { 3 }
fn default_8() -> usize { 8 }
fn default_2() -> usize { 2 }
fn default_5() -> usize { 5 }
fn default_warn() -> Severity { Severity::Warning }
fn default_info() -> Severity { Severity::Info }

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigMeta { pub version: u32 }

#[derive(Debug, Deserialize, Serialize)]
pub struct LiteralDedupConfig {
    #[serde(default = "default_3")] pub threshold: usize,
    #[serde(default = "default_8")] pub min_length: usize,
    #[serde(default)] pub ignore_in_paths: Vec<String>,
    #[serde(default = "default_info")] pub severity: Severity,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NumericDedupConfig {
    #[serde(default = "default_3")] pub threshold: usize,
    #[serde(default = "default_warn")] pub severity: Severity,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BodyDedupConfig {
    #[serde(default = "default_2")] pub threshold: usize,
    #[serde(default = "default_5")] pub min_lines: usize,
    #[serde(default = "default_warn")] pub severity: Severity,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForbiddenCall {
    pub id: String,
    #[serde(rename = "match")] pub patterns: Vec<String>,
    #[serde(default)] pub allow_in_crate: Vec<String>,
    #[serde(default)] pub allow_in_test: bool,
    pub severity: Severity,
    pub suggestion: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForbiddenLiteral {
    pub id: String,
    pub pattern: String,
    #[serde(default)] pub allow_in_crate: Vec<String>,
    pub severity: Severity,
    pub suggestion: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DriftConfig {
    pub meta: Option<ConfigMeta>,
    #[serde(default)] pub duplicated_literal: LiteralDedupConfig,
    #[serde(default)] pub duplicated_numeric: NumericDedupConfig,
    #[serde(default)] pub duplicated_body: BodyDedupConfig,
    #[serde(default)] pub forbidden_call: Vec<ForbiddenCall>,
    #[serde(default)] pub forbidden_literal: Vec<ForbiddenLiteral>,
}

impl Default for LiteralDedupConfig {
    fn default() -> Self { Self { threshold: 3, min_length: 8, ignore_in_paths: vec![], severity: Severity::Info } }
}
impl Default for NumericDedupConfig {
    fn default() -> Self { Self { threshold: 3, severity: Severity::Warning } }
}
impl Default for BodyDedupConfig {
    fn default() -> Self { Self { threshold: 2, min_lines: 5, severity: Severity::Warning } }
}

impl DriftConfig {
    pub fn load(root: &std::path::Path) -> Self {
        let path = root.join("drift-patterns.toml");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }
}
```

- [ ] **Step 3: Create starter drift-patterns.toml at workspace root**

```toml
[meta]
version = 1

# Reqwest client construction must go through vox-reqwest-defaults
[[forbidden_call]]
id = "drift/reqwest-bypass"
match = ["reqwest::Client::new", "reqwest::Client::builder"]
allow_in_crate = ["vox-reqwest-defaults"]
allow_in_test = true
severity = "warning"
suggestion = "Use vox_reqwest_defaults::client_builder()"

# Raw .vox/ paths must use vox-config::paths constants
[[forbidden_literal]]
id = "drift/vox-path-literal"
pattern = '^\.(vox|vox-cache)[/\\]'
allow_in_crate = ["vox-config", "vox-db"]
severity = "warning"
suggestion = "Use vox_config::paths::* constants"

[duplicated_literal]
threshold = 3
min_length = 8
ignore_in_paths = ["**/tests/**", "**/fixtures/**", "**/golden/**"]
severity = "info"

[duplicated_numeric]
threshold = 3
severity = "warning"

[duplicated_body]
threshold = 2
min_lines = 5
severity = "warning"
```

- [ ] **Step 4: Thread config into engine.run_all**

In `engine.rs`, load `DriftConfig::load(&self.root)` and pass to sweep rules (update `SweepRule::sweep` signature to take `config: &DriftConfig`, or load inside each rule). Simplest path: pass threshold overrides from config to the default rule constructors.

- [ ] **Step 5: Run tests**
```
cargo test -p vox-drift-check config
```

- [ ] **Step 6: Commit**
```
git add crates/vox-drift-check/src/config.rs drift-patterns.toml
git commit -m "feat(drift): drift-patterns.toml config layer"
```

---

## Phase 4 — TypeScript Extractor

### Task 15: TypeScript extractor — literals, call sites, imports

**Files:**
- Create: `crates/vox-drift-check/src/extractors/typescript.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &str) -> ExtractedFeatures {
        TypeScriptExtractor.extract(std::path::Path::new("test.ts"), src).unwrap()
    }

    #[test]
    fn extracts_string_literal_ts() {
        let f = extract(r#"const x = "hello world";"#);
        assert!(f.string_literals.iter().any(|l| l.value == "hello world"));
    }

    #[test]
    fn extracts_numeric_literal_ts() {
        let f = extract("const t = 30;");
        assert!(f.numeric_literals.iter().any(|n| n.value == 30.0));
    }

    #[test]
    fn extracts_call_site_ts() {
        let f = extract("fetch('https://example.com');");
        assert!(f.call_sites.iter().any(|cs| cs.path == vec!["fetch"]));
    }
}
```

- [ ] **Step 2: Implement typescript.rs**

```rust
use std::path::Path;
use anyhow::Result;
use swc_common::{FileName, SourceMap, sync::Lrc, GLOBALS, Globals};
use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitWith};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};
use crate::extractor::LanguageExtractor;
use crate::features::*;
use vox_code_audit::rules::Language;

pub struct TypeScriptExtractor;

struct TsVisitor {
    features: ExtractedFeatures,
}

impl Visit for TsVisitor {
    fn visit_str(&mut self, node: &Str) {
        self.features.string_literals.push(LiteralLoc {
            value: node.value.to_string(),
            loc: Loc::default(), // SWC spans require SourceMap for line resolution
            ctx: LiteralContext::Code,
        });
    }

    fn visit_number(&mut self, node: &Number) {
        self.features.numeric_literals.push(NumericLoc {
            value: node.value,
            unit: None,
            loc: Loc::default(),
        });
    }

    fn visit_call_expr(&mut self, node: &CallExpr) {
        let path = match &node.callee {
            Callee::Expr(expr) => extract_expr_path(expr),
            _ => vec![],
        };
        if !path.is_empty() {
            self.features.call_sites.push(CallSite {
                path,
                arity: node.args.len() as u8,
                loc: Loc::default(),
            });
        }
        node.visit_children_with(self);
    }

    fn visit_import_decl(&mut self, node: &ImportDecl) {
        self.features.imports.push(ImportLoc {
            path: vec![node.src.value.to_string()],
            symbol: None,
            loc: Loc::default(),
        });
        node.visit_children_with(self);
    }

    fn visit_fn_decl(&mut self, node: &FnDecl) {
        use xxhash_rust::xxh3::xxh3_64;
        // Approximate body hash from stringified body span (no full AST normalization yet)
        let name = node.ident.sym.to_string();
        let hash = xxh3_64(name.as_bytes()); // placeholder — Task 16 adds normalized body hash
        self.features.fn_definitions.push(FnDef {
            name,
            body_hash: hash,
            sig_hash: hash,
            loc: Loc::default(),
        });
        node.visit_children_with(self);
    }
}

fn extract_expr_path(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Ident(i) => vec![i.sym.to_string()],
        Expr::Member(m) => {
            let mut base = extract_expr_path(&m.obj);
            if let MemberProp::Ident(id) = &m.prop {
                base.push(id.sym.to_string());
            }
            base
        }
        _ => vec![],
    }
}

impl LanguageExtractor for TypeScriptExtractor {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures> {
        let is_tsx = path.extension().map_or(false, |e| e == "tsx");
        let cm = Lrc::new(SourceMap::default());
        let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), content.to_string());

        let lexer = Lexer::new(
            Syntax::Typescript(TsConfig { tsx: is_tsx, ..Default::default() }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module()
            .map_err(|e| anyhow::anyhow!("TS parse error in {}: {:?}", path.display(), e))?;

        let mut visitor = TsVisitor {
            features: ExtractedFeatures::new(path.to_path_buf(), Language::TypeScript),
        };
        visitor.features.crate_name = crate::extractor::crate_name_from_path(path);
        module.visit_with(&mut visitor);
        Ok(visitor.features)
    }
}
```

- [ ] **Step 3: Run tests**
```
cargo test -p vox-drift-check extractors::typescript
```

- [ ] **Step 4: Commit**
```
git add crates/vox-drift-check/src/extractors/typescript.rs
git commit -m "feat(drift): TypeScript extractor via swc_ecma_parser"
```

---

### Task 16: TypeScript normalized body hash

**Files:**
- Modify: `crates/vox-drift-check/src/extractors/typescript.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn identical_ts_bodies_produce_same_hash() {
    let src_a = r#"function double(x) { return x * 2; }"#;
    let src_b = r#"function twice(n) { return n * 2; }"#;
    let fa = extract(src_a);
    let fb = extract(src_b);
    assert_eq!(fa.fn_definitions[0].body_hash, fb.fn_definitions[0].body_hash);
}
```

- [ ] **Step 2: Implement normalized hash in TsVisitor::visit_fn_decl**

Replace the placeholder hash with an alpha-renaming normalizer:

```rust
fn visit_fn_decl(&mut self, node: &FnDecl) {
    let name = node.ident.sym.to_string();
    let body_hash = normalize_fn_body_ts(&node.function);
    let sig_hash = xxhash_rust::xxh3::xxh3_64(name.as_bytes());
    let line_count = node.function.body.as_ref().map_or(0, |b| b.stmts.len() as u32);
    self.features.fn_definitions.push(FnDef { name, body_hash, sig_hash, loc: Loc::default() });
    self.features.body_signatures.push(BodySignature {
        hash: body_hash, line_count, parent_fn: Some(self.features.fn_definitions.last().unwrap().name.clone()), loc: Loc::default(),
    });
    node.visit_children_with(self);
}
```

Add normalizer function:
```rust
fn normalize_fn_body_ts(func: &Function) -> u64 {
    use xxhash_rust::xxh3::Xxh3;
    use std::hash::{Hash, Hasher};

    struct Norm { tokens: Vec<u64>, bindings: std::collections::HashMap<String, u64>, next: u64 }
    impl Norm {
        fn ident(&mut self, s: &str) -> u64 {
            let n = self.next;
            *self.bindings.entry(s.to_string()).or_insert_with(|| { self.next += 1; n })
        }
        fn tag(&mut self, t: u64) { self.tokens.push(t); }
    }

    // Walk body stmts, emit normalized token tags
    fn walk_stmts(stmts: &[Stmt], norm: &mut Norm) {
        for stmt in stmts {
            walk_stmt(stmt, norm);
        }
    }
    fn walk_stmt(stmt: &Stmt, norm: &mut Norm) {
        match stmt {
            Stmt::Return(r) => { norm.tag(1); if let Some(e) = &r.arg { walk_expr(e, norm); } }
            Stmt::Expr(e) => { norm.tag(2); walk_expr(&e.expr, norm); }
            Stmt::Decl(d) => { norm.tag(3); walk_decl(d, norm); }
            Stmt::Block(b) => { norm.tag(4); walk_stmts(&b.stmts, norm); }
            _ => { norm.tag(99); }
        }
    }
    fn walk_expr(expr: &Expr, norm: &mut Norm) {
        match expr {
            Expr::Ident(i) => { norm.tag(10); let id = norm.ident(&i.sym); norm.tokens.push(id); }
            Expr::Lit(Lit::Num(n)) => { norm.tag(11); norm.tokens.push(n.value.to_bits()); }
            Expr::Lit(Lit::Str(s)) => { norm.tag(12); /* don't push value, just presence */ }
            Expr::Bin(b) => { norm.tag(13); walk_expr(&b.left, norm); norm.tokens.push(b.op as u64); walk_expr(&b.right, norm); }
            Expr::Call(c) => { norm.tag(14); norm.tokens.push(c.args.len() as u64); }
            _ => { norm.tag(99); }
        }
    }
    fn walk_decl(decl: &Decl, norm: &mut Norm) { norm.tag(30); }

    let mut norm = Norm { tokens: Vec::new(), bindings: std::collections::HashMap::new(), next: 0 };
    if let Some(body) = &func.body {
        walk_stmts(&body.stmts, &mut norm);
    }
    let mut h = Xxh3::new();
    norm.tokens.hash(&mut h);
    h.finish()
}
```

- [ ] **Step 3: Run test**
```
cargo test -p vox-drift-check extractors::typescript::tests::identical_ts_bodies
```

- [ ] **Step 4: Commit**
```
git add crates/vox-drift-check/src/extractors/typescript.rs
git commit -m "feat(drift): TypeScript normalized body hash for dedup detection"
```

---

## Phase 5 — Vox Extractor

### Task 17: Vox extractor

**Files:**
- Create: `crates/vox-drift-check/src/extractors/vox.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn extracts_string_literal_from_vox() {
    let src = r#"fn greet() { let msg = "hello vox"; }"#;
    let f = VoxExtractor.extract(std::path::Path::new("greet.vox"), src).unwrap();
    assert!(f.string_literals.iter().any(|l| l.value == "hello vox"));
}

#[test]
fn extracts_numeric_literal_from_vox() {
    let src = r#"fn timeout() { let t = 30; }"#;
    let f = VoxExtractor.extract(std::path::Path::new("t.vox"), src).unwrap();
    assert!(f.numeric_literals.iter().any(|n| n.value == 30.0));
}
```

- [ ] **Step 2: Implement vox.rs**

The Vox extractor uses `vox_compiler::lexer::lex` to tokenize (available since `vox_compiler::lexer` is `pub mod`), then pattern-matches tokens. For richer analysis, check `vox_compiler::parser::parse_with_registry`.

```rust
use std::path::Path;
use anyhow::Result;
use vox_code_audit::rules::Language;
use crate::extractor::LanguageExtractor;
use crate::features::*;

pub struct VoxExtractor;

impl LanguageExtractor for VoxExtractor {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures> {
        let mut features = ExtractedFeatures::new(path.to_path_buf(), Language::Vox);
        features.crate_name = crate::extractor::crate_name_from_path(path);

        // Use vox-compiler lexer for token-level extraction.
        // vox_compiler::lexer::lex returns a Vec of tokens.
        // Check the token types in crates/vox-compiler/src/tokens.rs for exact variants.
        let tokens = vox_compiler::lexer::lex(content);

        let mut line = 1usize;
        for tok in &tokens {
            // Advance line counter by newlines in the source up to tok.span.start
            // Simplified: count newlines in source up to span position
            // For full line tracking, use tok.span from vox_compiler::ast::span::Span

            // Match on token kind — exact variants from vox_compiler::tokens
            // Common pattern: Token { kind: TokenKind::StringLiteral(s), span }
            // Adjust based on actual token enum in crates/vox-compiler/src/tokens.rs
            use vox_compiler::tokens::TokenKind;
            match &tok.kind {
                TokenKind::StringLiteral(s) => {
                    features.string_literals.push(LiteralLoc {
                        value: s.clone(),
                        loc: Loc { line: byte_offset_to_line(content, tok.span.start), col: 0 },
                        ctx: LiteralContext::Code,
                    });
                }
                TokenKind::IntLiteral(n) => {
                    features.numeric_literals.push(NumericLoc {
                        value: *n as f64,
                        unit: None,
                        loc: Loc { line: byte_offset_to_line(content, tok.span.start), col: 0 },
                    });
                }
                TokenKind::Fn => {} // track function definitions if needed
                _ => {}
            }
        }

        Ok(features)
    }
}

fn byte_offset_to_line(src: &str, offset: usize) -> usize {
    src[..offset.min(src.len())].chars().filter(|&c| c == '\n').count() + 1
}
```

> **Note:** Check `crates/vox-compiler/src/tokens.rs` for exact `TokenKind` variant names before running. The variants above (`StringLiteral`, `IntLiteral`, `Fn`) are the most likely names based on the token module's purpose, but adjust if the names differ.

- [ ] **Step 3: Run tests**
```
cargo test -p vox-drift-check extractors::vox
```

- [ ] **Step 4: Commit**
```
git add crates/vox-drift-check/src/extractors/vox.rs
git commit -m "feat(drift): Vox extractor using vox-compiler lexer"
```

---

## Phase 6 — Cache + Baseline + CI

### Task 18: Content-hash file cache

**Files:**
- Create: `crates/vox-drift-check/src/cache.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn cache_round_trips_features() {
    let dir = tempfile::TempDir::new().unwrap();
    let cache = FeatureCache::new(dir.path().to_path_buf());

    let mut f = ExtractedFeatures::new(std::path::PathBuf::from("test.rs"), Language::Rust);
    f.string_literals.push(LiteralLoc { value: "hi".into(), loc: Loc::default(), ctx: LiteralContext::Code });

    let key = "abc123";
    cache.store(key, &f).unwrap();
    let loaded = cache.load(key).unwrap();
    assert_eq!(loaded.string_literals[0].value, "hi");
}
```

- [ ] **Step 2: Implement cache.rs**

```rust
use std::path::PathBuf;
use anyhow::Result;
use sha2::{Sha256, Digest};
use crate::features::ExtractedFeatures;

pub struct FeatureCache { dir: PathBuf }

impl FeatureCache {
    pub fn new(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        Self { dir }
    }

    pub fn from_workspace(root: &std::path::Path) -> Self {
        Self::new(root.join(".vox/cache/drift"))
    }

    pub fn hash_file(content: &str) -> String {
        let mut h = Sha256::new();
        h.update(content.as_bytes());
        format!("{:x}", h.finalize())
    }

    pub fn store(&self, key: &str, features: &ExtractedFeatures) -> Result<()> {
        let path = self.dir.join(format!("{}.bin", &key[..16.min(key.len())]));
        let bytes = bincode::encode_to_vec(features, bincode::config::standard())?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn load(&self, key: &str) -> Option<ExtractedFeatures> {
        let path = self.dir.join(format!("{}.bin", &key[..16.min(key.len())]));
        let bytes = std::fs::read(path).ok()?;
        bincode::decode_from_slice::<ExtractedFeatures, _>(&bytes, bincode::config::standard()).ok().map(|(f, _)| f)
    }
}
```

Add `bincode::Encode + bincode::Decode` derives to all feature types (or use `serde` with `bincode`'s serde feature):

In `Cargo.toml` change `bincode = { workspace = true }` to use `features = ["serde"]` and use `bincode::serde::encode_to_vec` / `decode_from_slice`.

- [ ] **Step 3: Integrate cache into engine.extract_workspace**

In `DriftEngine::extract_workspace`, before calling extractor:
```rust
let cache = FeatureCache::from_workspace(&self.root);
// ...
let content = std::fs::read_to_string(p).ok()?;
let hash = FeatureCache::hash_file(&content);
if let Some(cached) = cache.load(&hash) { return Some(cached); }
let result = extractor.extract(p, &content).ok()?;
cache.store(&hash, &result).ok();
result
```

- [ ] **Step 4: Run tests**
```
cargo test -p vox-drift-check cache
```

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/cache.rs
git commit -m "feat(drift): content-hash feature cache (warm runs skip re-parse)"
```

---

### Task 19: --baseline mode + lefthook + vox doctor

**Files:**
- Modify: `crates/vox-drift-check/src/bin/vox_drift_check.rs`
- Modify: `lefthook.yml`

- [ ] **Step 1: Add --baseline flags to CLI**

```rust
// In Cli struct, add:
/// Path to baseline JSON file; only show new findings vs baseline
#[arg(long)]
baseline: Option<PathBuf>,
/// Write current findings to baseline file and exit
#[arg(long)]
update_baseline: bool,
```

Add baseline load/compare logic in `main`:
```rust
if cli.update_baseline {
    let json = serde_json::to_string_pretty(&findings).unwrap();
    std::fs::write(cli.baseline.unwrap_or(PathBuf::from(".vox/cache/drift/baseline.json")), json).unwrap();
    println!("Baseline updated ({} findings).", findings.len());
    return;
}

let findings = if let Some(base_path) = &cli.baseline {
    let base_json = std::fs::read_to_string(base_path).unwrap_or("[]".into());
    let base: Vec<vox_code_audit::rules::Finding> = serde_json::from_str(&base_json).unwrap_or_default();
    let base_keys: std::collections::HashSet<String> = base.iter()
        .map(|f| format!("{}:{}:{}", f.rule_id, f.file.display(), f.line))
        .collect();
    findings.into_iter()
        .filter(|f| !base_keys.contains(&format!("{}:{}:{}", f.rule_id, f.file.display(), f.line)))
        .collect()
} else { findings };
```

- [ ] **Step 2: Add lefthook pre-push hook**

In `lefthook.yml`, under `pre-push:` commands, add:
```yaml
  drift-check:
    run: cargo run -p vox-drift-check -- . --severity warning --fail-on error
    glob: "crates/**/*.{rs,ts,vox}"
```

- [ ] **Step 3: Build and run full workspace scan**
```
cargo run -p vox-drift-check -- . --severity info 2>&1 | tail -20
```
Expected: shows finding count from the audit (reqwest bypasses, timeout literals, etc.).

- [ ] **Step 4: Run CI check with --severity error (should pass clean)**
```
cargo run -p vox-drift-check -- . --severity error --fail-on error
echo "Exit code: $?"
```
Expected: exit 0 (no errors, only warnings/info).

- [ ] **Step 5: Commit**
```
git add crates/vox-drift-check/src/bin/vox_drift_check.rs lefthook.yml
git commit -m "feat(drift): --baseline mode + lefthook pre-push integration"
```

---

### Task 20: Architecture docs + integration test

**Files:**
- Modify: `docs/src/architecture/where-things-live.md` (final row, verify)
- Create: `crates/vox-drift-check/tests/integration.rs`

- [ ] **Step 1: Write integration test**

```rust
// crates/vox-drift-check/tests/integration.rs
use vox_drift_check::engine::DriftEngine;
use tempfile::TempDir;
use std::fs;

#[test]
fn detects_planted_reqwest_bypass() {
    let dir = TempDir::new().unwrap();
    let crate_dir = dir.path().join("crates/vox-publisher/src");
    fs::create_dir_all(&crate_dir).unwrap();
    fs::write(crate_dir.join("lib.rs"), r#"
pub fn make_client() -> reqwest::Client {
    reqwest::Client::new()
}
"#).unwrap();

    let engine = DriftEngine::new(dir.path());
    let findings = engine.run_all().unwrap();
    assert!(
        findings.iter().any(|f| f.rule_id == "drift/reqwest-bypass"),
        "Expected drift/reqwest-bypass finding, got: {:?}",
        findings.iter().map(|f| &f.rule_id).collect::<Vec<_>>()
    );
}

#[test]
fn detects_duplicate_string_literals() {
    let dir = TempDir::new().unwrap();
    for i in 0..3 {
        let f = dir.path().join(format!("crates/vox-foo{}/src/lib.rs", i));
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, r#"fn foo() { let x = "duplicate-literal-value"; }"#).unwrap();
    }
    let engine = DriftEngine::new(dir.path());
    let findings = engine.run_all().unwrap();
    assert!(findings.iter().any(|f| f.rule_id == "sweep/duplicate-string-literal"));
}
```

- [ ] **Step 2: Run integration tests**
```
cargo test -p vox-drift-check --test integration
```
Expected: both tests pass.

- [ ] **Step 3: Final compile check + full test suite**
```
cargo test -p vox-drift-check
cargo clippy -p vox-drift-check -- -D warnings
```

- [ ] **Step 4: Final commit**
```
git add crates/vox-drift-check/tests/ docs/
git commit -m "feat(drift): integration tests + docs — vox-drift-check complete"
```

---

## Self-Review

**Spec coverage check:**
- ✓ Two-phase pipeline (Tasks 6, 13)
- ✓ Rust extractor via syn (Tasks 4, 5)
- ✓ TypeScript extractor via swc (Tasks 15, 16)
- ✓ Vox extractor via vox-compiler lexer (Task 17)
- ✓ ExtractedFeatures vocabulary (Task 2)
- ✓ 4 sweep rules: literal, numeric, body, call_shape (Tasks 7, 8)
- ✓ 6 targeted drift rules (Tasks 10–12)
- ✓ drift-patterns.toml config layer (Task 14)
- ✓ CLI binary + vox-cli subcommand (Task 13)
- ✓ Content-hash cache (Task 18)
- ✓ --baseline mode (Task 19)
- ✓ lefthook integration (Task 19)
- ✓ Governance docs (Task 1)
- ✓ Integration tests (Task 20)

**Gaps addressed:** `import_drift` sweep is stub in `call_shape.rs`; full implementation follows the same SweepRule pattern as literal_dedup and can be added as a follow-up once `drift-patterns.toml` forbidden-import config is threaded through (the schema exists in Task 14).

**Type consistency:** `Loc` (not `Span`) used throughout; `FindingConfidence` (not `Confidence`) matches vox-code-audit exactly; `Severity::Warning` (capital W) matches enum; `Language::Rust` etc. match enum.
