//! Layout + documentation drift guards driven by `examples/examples.ssot.v1.yaml`.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct ExamplesSsot {
    schema_version: u32,
    golden_roots: Vec<String>,
    #[serde(default)]
    sandbox_roots: Vec<String>,
    negative_roots: Vec<String>,
    doc_roots: Vec<String>,
    /// In-progress / planned-feature examples; parse-tested but not golden.
    /// Added 2026-05-25 for `examples/aspirational/**`.
    #[serde(default)]
    aspirational_roots: Vec<String>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn load_ssot() -> ExamplesSsot {
    let path = repo_root().join("examples/examples.ssot.v1.yaml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read examples SSOT {}: {e}", path.display()));
    serde_yaml::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn collect_vox_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let read = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for ent in read.flatten() {
        let p = ent.path();
        if p.is_dir() {
            collect_vox_files(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("vox") {
            out.push(p);
        }
    }
}

fn posix_rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or_else(|e| panic!("{} not under {}: {e}", path.display(), root.display()))
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_under_any(rel: &str, roots: &[String]) -> bool {
    roots.iter().any(|r| {
        let r = r.trim_end_matches('/');
        rel == r || rel.starts_with(&format!("{r}/"))
    })
}

#[test]
fn examples_ssot_manifest_loads() {
    let ssot = load_ssot();
    assert_eq!(ssot.schema_version, 1);
    assert!(
        !ssot.golden_roots.is_empty(),
        "examples SSOT must list at least one golden root"
    );
    assert!(
        !ssot.negative_roots.is_empty(),
        "examples SSOT must list parser-inventory (or successor) under negative_roots"
    );
}

#[test]
fn examples_tree_has_no_orphan_vox_files() {
    let root = repo_root();
    let ssot = load_ssot();
    let examples_dir = root.join("examples");
    assert!(
        examples_dir.is_dir(),
        "missing examples dir {}",
        examples_dir.display()
    );

    let mut vox_files = Vec::new();
    collect_vox_files(&examples_dir, &mut vox_files);

    for path in vox_files {
        let rel = posix_rel(&path, &root);
        if is_under_any(&rel, &ssot.negative_roots) {
            continue;
        }
        if is_under_any(&rel, &ssot.golden_roots) {
            continue;
        }
        if is_under_any(&rel, &ssot.sandbox_roots) {
            continue;
        }
        if is_under_any(&rel, &ssot.aspirational_roots) {
            continue;
        }
        panic!(
            "orphan `examples/**/*.vox`: {rel}\n\
             Move the file under a `golden_roots` directory, list its tree under `negative_roots` if it is intentionally invalid,\n\
             or update `examples/examples.ssot.v1.yaml` when adding a new policy bucket."
        );
    }
}

fn for_each_doc_pipeline_include(text: &str, mut f: impl FnMut(&str)) {
    let needle = "{{#include ";
    let mut search = 0usize;
    while let Some(found) = text[search..].find(needle) {
        let abs = search + found + needle.len();
        let rest = &text[abs..];
        let close = rest.find("}}").unwrap_or_else(|| {
            panic!("unclosed doc pipeline `{{{{#include` starting at byte {abs}")
        });
        let token = rest[..close].trim();
        let path_part = token.split(':').next().unwrap_or(token).trim();
        if !path_part.is_empty() {
            f(path_part);
        }
        search = abs + close + 2;
    }
}

fn walk_md_files(dir: &Path, f: &mut impl FnMut(&Path)) {
    let read = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for ent in read.flatten() {
        let p = ent.path();
        if p.is_dir() {
            walk_md_files(&p, f);
        } else if p.extension().and_then(|s| s.to_str()) == Some("md") {
            f(&p);
        }
    }
}

#[test]
fn doc_pipeline_includes_resolve_and_vox_are_golden() {
    let root = repo_root();
    let ssot = load_ssot();
    let golden_dir = root.join("examples/golden");
    let golden_canon = golden_dir
        .canonicalize()
        .unwrap_or_else(|e| panic!("canonicalize golden dir {}: {e}", golden_dir.display()));

    for doc_root in &ssot.doc_roots {
        let base = root.join(doc_root);
        assert!(base.is_dir(), "doc root missing: {}", base.display());

        walk_md_files(&base, &mut |md_path| {
            let text = fs::read_to_string(md_path)
                .unwrap_or_else(|e| panic!("read {}: {e}", md_path.display()));
            let parent = md_path.parent().unwrap_or_else(|| Path::new("."));
            for_each_doc_pipeline_include(&text, |rel_raw| {
                let joined = parent.join(rel_raw);
                let resolved = joined.canonicalize().unwrap_or_else(|e| {
                    panic!(
                        "{}: doc pipeline include path {:?} does not resolve (from {}): {e}",
                        md_path.display(),
                        rel_raw,
                        md_path.display()
                    )
                });
                if resolved.extension().and_then(|s| s.to_str()) == Some("vox") {
                    assert!(
                        resolved.starts_with(&golden_canon),
                        "{}: doc pipeline .vox include must target a path under {} (got {})",
                        md_path.display(),
                        golden_dir.display(),
                        resolved.display()
                    );
                }
            });
        });
    }
}
