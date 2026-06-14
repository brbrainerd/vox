//! Cross-crate exact-body split-brain detector.
//!
//! Flags functions whose **normalized body is byte-identical across two or more DIFFERENT
//! crates** — a split-brain risk: the copies will drift apart silently (one gets a bug fix,
//! the other doesn't). The body is normalized via syn's token stream (`block.to_token_stream()
//! .to_string()`), which strips comments and canonicalizes whitespace, so formatting and
//! comment differences don't hide a true duplicate.
//!
//! This is a **batch** detector (it needs the full file set to compare across crates), exposed
//! as a free function and invoked once per scan after the per-file rules. It is **Info**
//! severity (advisory): many cross-crate duplicates are *intentional* — most notably
//! platform-specific plugin backends (`*-cuda` vs `*-metal`) that share a body on purpose — so
//! we skip platform-sibling crates and trivial bodies to keep precision high.
//!
//! Source-of-truth example from the graphify audit: `deliver_a2a` duplicated in the populi
//! plugin vs populi core (a real split-brain), while `preflight_native_qlora` in the cuda vs
//! metal plugins is an intentional variant we deliberately do NOT flag.

use crate::diagnostics::catalog;
use crate::rules::{Finding, FindingConfidence, Language, Severity, SourceFile};
use crate::run_context::workspace_crate_key;
use quote::ToTokens;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use syn::visit::Visit;

/// Bodies shorter than this many whitespace-separated tokens are skipped — short bodies
/// (`Self::default()`, `Ok(())`, one-liners) collide trivially and aren't split-brain.
const MIN_BODY_TOKENS: usize = 40;

/// Function names that are trivially-duplicated by language convention, not split-brain.
const ALLOWED_FN_NAMES: &[&str] = &[
    "default", "new", "main", "noop", "no_op", "stub", "drop", "fmt", "clone", "deref", "as_ref",
    "as_mut", "from", "into", "try_from", "try_into",
];

/// Crate-name suffixes that denote a deliberate platform/backend variant. A body shared
/// across `foo-cuda` and `foo-metal` is an intentional variant, not split-brain, so we
/// collapse these to a single identity before comparing crates.
const PLATFORM_SUFFIXES: &[&str] = &[
    "-cuda", "-metal", "-cpu", "-rocm", "-wgpu", "-vulkan", "-native", "-gpu",
];

/// Collapse a platform-variant crate key to its base identity (`vox-…-candle-cuda` →
/// `vox-…-candle`) so intentional backend variants are not flagged against each other.
fn platform_base(crate_key: &str) -> &str {
    for suf in PLATFORM_SUFFIXES {
        if let Some(base) = crate_key.strip_suffix(suf) {
            return base;
        }
    }
    crate_key
}

/// True if any attribute is `#[test]` or `#[cfg(test)]` (skip test code).
fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        let p = a.path();
        p.is_ident("test")
            || (p.is_ident("cfg") && a.to_token_stream().to_string().contains("test"))
    })
}

fn hash_body(body: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    body.hash(&mut h);
    h.finish()
}

/// One collected function body, ready for cross-crate grouping.
struct FnRec {
    crate_key: String,
    file: PathBuf,
    line: usize,
    name: String,
    body_hash: u64,
}

struct Collector<'a> {
    recs: Vec<FnRec>,
    crate_key: String,
    file: PathBuf,
    lines: &'a [String],
}

impl<'a> Collector<'a> {
    fn consider(
        &mut self,
        name: String,
        line: usize,
        block: &syn::Block,
        attrs: &[syn::Attribute],
    ) {
        if has_test_attr(attrs) {
            return;
        }
        if ALLOWED_FN_NAMES.contains(&name.as_str()) || name.starts_with("default_") {
            return;
        }
        // Honor an explicit suppression on the signature line.
        if self
            .lines
            .get(line.saturating_sub(1))
            .is_some_and(|l| l.contains("toestub-ignore"))
        {
            return;
        }
        let body = block.to_token_stream().to_string();
        if body.split_whitespace().count() < MIN_BODY_TOKENS {
            return;
        }
        self.recs.push(FnRec {
            crate_key: self.crate_key.clone(),
            file: self.file.clone(),
            line,
            name,
            body_hash: hash_body(&body),
        });
    }
}

impl<'a, 'ast> Visit<'ast> for Collector<'a> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_test_attr(&node.attrs) {
            return;
        }
        syn::visit::visit_item_mod(self, node);
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let name = node.sig.ident.to_string();
        let line = node.sig.ident.span().start().line;
        self.consider(name, line, &node.block, &node.attrs);
        syn::visit::visit_item_fn(self, node);
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let name = node.sig.ident.to_string();
        let line = node.sig.ident.span().start().line;
        self.consider(name, line, &node.block, &node.attrs);
        syn::visit::visit_impl_item_fn(self, node);
    }
}

/// Detect functions with byte-identical normalized bodies across two or more crates.
/// Returns one Info finding per duplicate site beyond the first in each cluster.
pub fn detect_cross_crate_dup_in_batch(files: &[SourceFile]) -> Vec<Finding> {
    let mut recs: Vec<FnRec> = Vec::new();
    for file in files {
        if file.language != Language::Rust {
            continue;
        }
        let Some(crate_key) = workspace_crate_key(&file.path) else {
            continue;
        };
        let Ok(ast) = syn::parse_file(&file.content) else {
            continue;
        };
        let mut c = Collector {
            recs: Vec::new(),
            crate_key,
            file: file.path.clone(),
            lines: &file.lines,
        };
        c.visit_file(&ast);
        recs.extend(c.recs);
    }

    // Group record indices by body hash.
    let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, r) in recs.iter().enumerate() {
        by_hash.entry(r.body_hash).or_default().push(i);
    }

    let mut findings = Vec::new();
    for idxs in by_hash.values() {
        if idxs.len() < 2 {
            continue;
        }
        // Distinct crate identities (platform variants collapsed). Need >=2 to be split-brain.
        let mut identities: Vec<&str> = idxs
            .iter()
            .map(|&i| platform_base(&recs[i].crate_key))
            .collect();
        identities.sort_unstable();
        identities.dedup();
        if identities.len() < 2 {
            continue; // same crate, or only platform-sibling variants → intentional, skip
        }
        // Emit a finding at every site after the first, naming the sibling crates.
        let mut sites: Vec<usize> = idxs.clone();
        sites.sort_by_key(|&i| (recs[i].crate_key.clone(), recs[i].line));
        let cluster_crates: Vec<String> = {
            let mut c: Vec<String> = sites.iter().map(|&i| recs[i].crate_key.clone()).collect();
            c.sort_unstable();
            c.dedup();
            c
        };
        for &i in sites.iter().skip(1) {
            let r = &recs[i];
            findings.push(Finding {
                rule_id: "arch/cross-crate-dup".to_string(),
                diagnostic_id: Some(catalog::CROSS_CRATE_DUP.to_string()),
                rule_name: "Cross-Crate Split-Brain Detector".to_string(),
                severity: Severity::Info,
                file: r.file.clone(),
                line: r.line,
                column: 1,
                message: format!(
                    "`{}` has a byte-identical body in {} crates ({}) — split-brain risk: the \
                     copies will drift apart silently.",
                    r.name,
                    cluster_crates.len(),
                    cluster_crates.join(", ")
                ),
                suggestion: Some(
                    "Extract the shared logic into a common crate both depend on. If the \
                     duplication is intentional (a deliberate platform/backend variant), add \
                     `// toestub-ignore` on the fn signature line."
                        .to_string(),
                ),
                alternatives: vec![],
                rationale: Some(
                    "Two copies of the same logic in different crates diverge over time — a \
                     fix lands in one and not the other. Exact-body duplicates are the highest-\
                     confidence split-brain signal."
                        .to_string(),
                ),
                context: String::new(),
                confidence: Some(FindingConfidence::Medium),
                evidence: None,
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rs(path: &str, content: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(path), content.to_string())
    }

    // A body long enough (>= MIN_BODY_TOKENS) to be a real candidate.
    const BIG_BODY: &str = r#"{
        let mut total = 0i64;
        for item in items.iter() {
            if item.is_active() {
                total += item.weight() * 2;
            } else {
                total -= item.penalty();
            }
        }
        let avg = total / (items.len() as i64).max(1);
        Ok(Summary { total, avg, count: items.len() })
    }"#;

    #[test]
    fn flags_exact_body_across_two_real_crates() {
        let a = rs(
            "crates/vox-populi/src/a2a.rs",
            &format!("pub fn deliver_a2a(items: &[Item]) -> Result<Summary> {BIG_BODY}"),
        );
        let b = rs(
            "crates/vox-plugin-populi-mesh/src/a2a.rs",
            &format!("pub fn deliver_a2a(items: &[Item]) -> Result<Summary> {BIG_BODY}"),
        );
        let f = detect_cross_crate_dup_in_batch(&[a, b]);
        assert_eq!(f.len(), 1, "exactly one finding (the second site)");
        assert!(f[0].message.contains("deliver_a2a"));
        assert_eq!(f[0].severity, Severity::Info);
    }

    #[test]
    fn skips_intentional_platform_variants() {
        // cuda vs metal plugins sharing a body is a deliberate backend variant — not split-brain.
        let cuda = rs(
            "crates/vox-plugin-mens-candle-cuda/src/qlora.rs",
            &format!("pub fn preflight_native_qlora(items: &[Item]) -> Result<Summary> {BIG_BODY}"),
        );
        let metal = rs(
            "crates/vox-plugin-mens-candle-metal/src/qlora.rs",
            &format!("pub fn preflight_native_qlora(items: &[Item]) -> Result<Summary> {BIG_BODY}"),
        );
        assert!(
            detect_cross_crate_dup_in_batch(&[cuda, metal]).is_empty(),
            "platform-sibling crates must not be flagged"
        );
    }

    #[test]
    fn skips_same_crate_duplicates() {
        // Two copies in the SAME crate are dry_violation's job, not cross-crate split-brain.
        let a = rs(
            "crates/vox-populi/src/a.rs",
            &format!("pub fn f(items: &[Item]) -> Result<Summary> {BIG_BODY}"),
        );
        let b = rs(
            "crates/vox-populi/src/b.rs",
            &format!("pub fn g(items: &[Item]) -> Result<Summary> {BIG_BODY}"),
        );
        assert!(detect_cross_crate_dup_in_batch(&[a, b]).is_empty());
    }

    #[test]
    fn skips_trivial_bodies() {
        let a = rs("crates/vox-a/src/x.rs", "pub fn helper() -> i32 { 1 + 1 }");
        let b = rs("crates/vox-b/src/y.rs", "pub fn other() -> i32 { 1 + 1 }");
        assert!(
            detect_cross_crate_dup_in_batch(&[a, b]).is_empty(),
            "bodies below MIN_BODY_TOKENS must not be flagged"
        );
    }

    #[test]
    fn skips_test_functions_and_allowlisted_names() {
        let a = rs(
            "crates/vox-a/src/x.rs",
            &format!(
                "#[test]\nfn t(items: &[Item]) -> Result<Summary> {BIG_BODY}\npub fn new(items: &[Item]) -> Result<Summary> {BIG_BODY}"
            ),
        );
        let b = rs(
            "crates/vox-b/src/y.rs",
            &format!(
                "#[test]\nfn t(items: &[Item]) -> Result<Summary> {BIG_BODY}\npub fn new(items: &[Item]) -> Result<Summary> {BIG_BODY}"
            ),
        );
        assert!(
            detect_cross_crate_dup_in_batch(&[a, b]).is_empty(),
            "#[test] fns and allowlisted `new` must be skipped"
        );
    }

    #[test]
    fn honors_toestub_ignore() {
        // The suppression marker goes on the fn SIGNATURE line (the documented convention,
        // matching hollow_fn). `// toestub-ignore` is stripped by token-stream normalize, so
        // both bodies still hash identically — but site `a` is dropped at collection time.
        let a = rs(
            "crates/vox-a/src/x.rs",
            &format!(
                "pub fn shared(items: &[Item]) -> Result<Summary> {{ // toestub-ignore\n{}",
                &BIG_BODY[1..]
            ),
        );
        let b = rs(
            "crates/vox-b/src/y.rs",
            &format!("pub fn shared(items: &[Item]) -> Result<Summary> {BIG_BODY}"),
        );
        // 'a' is suppressed; only one site remains in the cluster → no finding.
        assert!(detect_cross_crate_dup_in_batch(&[a, b]).is_empty());
    }
}
