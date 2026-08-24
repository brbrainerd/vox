use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Joins the `use` statement starting at `start` (0-based) up to its `;`.
///
/// Returns `None` unless the line actually starts a `use` item. Bounded to 16 lines
/// so a missing semicolon in unparseable input cannot walk the whole file.
fn join_use_statement(code_lines: &[String], start: usize) -> Option<String> {
    let first = code_lines.get(start)?.trim_start();
    if !(first == "use" || first.starts_with("use ") || first.starts_with("use{")) {
        return None;
    }
    let mut stmt = String::new();
    for line in code_lines.iter().skip(start).take(16) {
        stmt.push(' ');
        stmt.push_str(line.trim());
        if line.contains(';') {
            return Some(stmt);
        }
    }
    None
}

/// The banned crate a `use` statement imports, if any.
///
/// Handles the forms a single-line regex misses: a path wrapped across lines
/// (`use\n    ring::digest;`), a braced group (`use {md5::Md5, std::io};`), and a
/// nested group (`use ring::{digest, hmac};`). Only the *root* segment of each path
/// is a crate name, so `use foo::ring::x` is correctly ignored.
fn banned_use_root(stmt: &str) -> Option<&'static str> {
    let body = stmt.trim().strip_prefix("use")?;
    for segment in body.split([',', '{', '}', ';']) {
        let root = segment
            .trim()
            .split("::")
            .next()
            .unwrap_or("")
            .split_whitespace()
            .next()
            .unwrap_or("");
        if let Some(hit) = CryptoBanDetector::BANNED_CRATES
            .iter()
            .find(|b| **b == root)
        {
            return Some(hit);
        }
    }
    None
}

/// Detects imports/uses of banned cryptography crates (aegis, ring, md5, sha1, openssl, etc.).
pub struct CryptoBanDetector {
    /// Matches banned Vox import statements.
    vox_banned_import: Regex,
    /// Matches banned Rust `use` / `extern crate` statements.
    rust_banned_use: Regex,
    /// Languages this detector supports (includes Unknown so Cargo.toml files are accepted).
    supported_langs: Vec<Language>,
}

impl Default for CryptoBanDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CryptoBanDetector {
    pub fn new() -> Self {
        Self {
            // Vox: `import aegis`, `import ring`, any import with `nasm` or `cmake` in path
            vox_banned_import: Regex::new(
                r#"(?x)
                \bimport\b.*
                (?:
                    \baegis\b
                  | \bring\b
                  | \bnasm\b
                  | \bcmake\b
                  | \bmd5\b
                  | \bsha1\b
                  | \bopenssl\b
                )
                "#,
            )
            .expect("valid vox_banned_import regex"),

            // Rust: `use aegis::`, `use ring::`, `extern crate ring`, `extern crate aegis`,
            //        `use md5`, `use sha1`, `use openssl`
            rust_banned_use: Regex::new(
                r#"(?x)
                (?:
                    \buse\s+(?:aegis|ring|md5|sha1|openssl)\b
                  | \bextern\s+crate\s+(?:aegis|ring|md5|sha1|openssl)\b
                )
                "#,
            )
            .expect("valid rust_banned_use regex"),

            // Unknown is included so `Cargo.toml` (which maps to Language::Unknown) is
            // scanned. NOTE: `Cargo.lock` is *not* covered — `Scanner::walk_root` admits
            // only files named `Cargo.toml`. A lockfile would parse as TOML, but its
            // `[[package]] dependencies = [...]` is an array, not a dep table, so the
            // walk below finds nothing in it regardless. Whole-graph
            // (transitive) crypto arrivals are reviewed in cryptography-ssot-2026.md — there
            // is deliberately no `[[bans.deny]]` gate (see the comment in deny.toml).
            supported_langs: vec![Language::Vox, Language::Rust, Language::Unknown],
        }
    }

    fn crate_name_from_line(&self, line: &str) -> &'static str {
        let line_lower = line.to_lowercase();
        if line_lower.contains("aws-lc-rs") {
            "aws-lc-rs"
        } else if line_lower.contains("aegis") {
            "aegis"
        } else if line_lower.contains("ring") {
            "ring"
        } else if line_lower.contains("openssl") {
            "openssl"
        } else if line_lower.contains("sha1") {
            "sha1"
        } else if line_lower.contains("md5") {
            "md5"
        } else if line_lower.contains("nasm") || line_lower.contains("cmake") {
            "cmake/nasm-dependent crate"
        } else {
            "(banned crate)"
        }
    }

    /// Crates banned as direct Cargo dependencies.
    const BANNED_CRATES: &'static [&'static str] =
        &["aegis", "ring", "aws-lc-rs", "md5", "sha1", "openssl"];

    /// Collects `(declaration key, resolved crate name)` for every banned direct
    /// dependency in a parsed manifest.
    ///
    /// A real TOML parse (rather than line regexes) is what makes the renamed form
    /// `crypto = { package = "ring" }`, the workspace-inheritance idiom
    /// `openssl.workspace = true`, quoted keys, and the `[dependencies.md5]` table
    /// form all visible. `[patch.*]` / `[replace]` are simply never walked — a patch
    /// substitutes a source, it cannot introduce a crate into the graph.
    fn banned_cargo_deps(manifest: &toml::Table) -> Vec<(String, String)> {
        fn scan(table: Option<&toml::Value>, out: &mut Vec<(String, String)>) {
            let Some(t) = table.and_then(|v| v.as_table()) else {
                return;
            };
            for (key, value) in t {
                let renamed = value.get("package").and_then(|p| p.as_str());
                let name = renamed.unwrap_or(key);
                if CryptoBanDetector::BANNED_CRATES.contains(&name) {
                    out.push((key.clone(), name.to_string()));
                    continue;
                }
                // A crate pulling in both cmake and nasm build tooling is banned by
                // the same policy even when its name is not on the list.
                let feats = value.get("features").and_then(|f| f.as_array());
                if let Some(feats) = feats {
                    let has = |want: &str| {
                        feats
                            .iter()
                            .filter_map(|f| f.as_str())
                            .any(|f| f.eq_ignore_ascii_case(want))
                    };
                    if has("cmake") && has("nasm") {
                        out.push((key.clone(), "cmake/nasm-dependent crate".to_string()));
                    }
                }
            }
        }

        let mut out = Vec::new();
        let mut scan_dep_tables = |root: Option<&toml::Value>| {
            let Some(root) = root else { return };
            for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
                scan(root.get(kind), &mut out);
            }
            // target.<cfg>.<kind>
            if let Some(targets) = root.get("target").and_then(|t| t.as_table()) {
                for cfg in targets.values() {
                    for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
                        scan(cfg.get(kind), &mut out);
                    }
                }
            }
        };

        let root = toml::Value::Table(manifest.clone());
        scan_dep_tables(Some(&root));
        // `[workspace.dependencies]` — the table members inherit from via
        // `foo.workspace = true`, and the place a banned crate actually enters.
        scan_dep_tables(manifest.get("workspace"));
        out
    }

    /// Best-effort line number for a dependency declared under `key`.
    ///
    /// Matches either an inline key (`key = ...`, `"key" = ...`, `key.workspace = ...`)
    /// or the last segment of a table header (`[dependencies.key]`). Falls back to
    /// line 1 so a finding is never dropped for want of a location.
    fn declaration_line(lines: &[String], key: &str) -> usize {
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            let candidate = match t.strip_prefix('[') {
                Some(rest) => rest
                    .split(']')
                    .next()
                    .unwrap_or("")
                    .rsplit('.')
                    .next()
                    .unwrap_or(""),
                None => t
                    .split('=')
                    .next()
                    .unwrap_or("")
                    .split('.')
                    .next()
                    .unwrap_or(""),
            };
            if candidate.trim().trim_matches(['"', '\'']) == key {
                return i + 1;
            }
        }
        1
    }

    fn make_finding(&self, file: &SourceFile, line_num: usize, crate_name: &str) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: Some(catalog::CRYPTO_BANNED_CRATE_IMPORT.to_string()),
            rule_name: self.name().to_string(),
            severity: Severity::Error,
            file: file.path.clone(),
            line: line_num,
            column: 0,
            message: format!(
                "Banned cryptography crate `{crate_name}` imported or declared as a dependency. \
                 Use `vox-crypto` or `chacha20poly1305` instead."
            ),
            suggestion: Some(
                "Use the `vox-crypto` crate. For AEAD encryption, use `chacha20poly1305` \
                 (pure-Rust). See docs/src/architecture/cryptography-ssot-2026.md."
                    .to_string(),
            ),
            alternatives: vec![],
            rationale: Some(
                "Vox policy bans aegis, ring, and any crate dragging in cmake/nasm for \
                 C-assembly optimization on Windows. Pure-Rust chacha20poly1305 is the standard \
                 AEAD. See AGENTS.md §Cryptography Policy."
                    .to_string(),
            ),
            context: file.context_around(line_num, 2),
            confidence: Some(FindingConfidence::High),
            evidence: None,
        }
    }

    /// Returns true for `crates/workspace-hack/Cargo.toml`, which `cargo hakari`
    /// generates from the union of transitive third-party dependencies.
    fn is_generated_hack_manifest(file: &SourceFile) -> bool {
        file.path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|n| n == "workspace-hack")
            .unwrap_or(false)
    }

    /// Returns true if this file path is a Cargo manifest.
    ///
    /// Only `Cargo.toml` — `Scanner::walk_root` never yields `Cargo.lock`, and the
    /// `cargo_banned_dep` patterns match dependency *declarations* (`ring = ...`),
    /// not a lockfile's `name = "ring"`. Claiming `.lock` coverage here would
    /// overstate what this security rule actually checks.
    fn is_cargo_manifest(file: &SourceFile) -> bool {
        file.path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "Cargo.toml")
            .unwrap_or(false)
    }
}

impl DetectionRule for CryptoBanDetector {
    fn id(&self) -> &'static str {
        "vox/crypto/banned-crate-import"
    }

    fn name(&self) -> &'static str {
        "Crypto Banned Crate Import Detector"
    }

    fn description(&self) -> &'static str {
        "Detects imports or Cargo dependencies for banned cryptography crates (aegis, ring, \
         aws-lc-rs, md5, sha1, openssl) that violate Vox cryptography policy."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::CRYPTO_BANNED_CRATE_IMPORT)
    }

    fn explain(&self) -> &'static str {
        "Vox bans aegis, ring, aws-lc-rs, md5, sha1, and openssl. The ring and aegis crates \
         drag in cmake/nasm for C-assembly optimization paths that fail on Windows CI. The md5 \
         and sha1 crates are cryptographically broken for most uses. The openssl crate requires \
         native libraries that complicate cross-compilation; prefer rustls.\n\n\
         BAD (Cargo.toml):\n  ring = \"0.17\"\n\n\
         GOOD:\n  chacha20poly1305 = \"0.10\"\n  # or: vox-crypto = { path = \"crates/vox-crypto\" }"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "# VIOLATION — banned crypto crate in Cargo.toml\n\
             [dependencies]\n\
             ring = \"0.17\"  # banned: drags in cmake/nasm, fails on Windows CI\n\
             \n\
             # FIX — use a pure-Rust crate or vox-crypto\n\
             [dependencies]\n\
             chacha20poly1305 = \"0.10\"\n\
             # or: vox-crypto = { path = \"crates/vox-crypto\" }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        let is_cargo = Self::is_cargo_manifest(file);
        let is_rust = file.language == Language::Rust;
        let is_vox = file.language == Language::Vox;

        // Only process Cargo manifests, Rust, and Vox files.
        if !is_cargo && !is_rust && !is_vox {
            return findings;
        }

        // `crates/workspace-hack/Cargo.toml` is generated by `cargo hakari` and lists
        // the union of *transitive* third-party deps by construction. The policy is
        // first-party scope (AGENTS.md §Cryptography Policy), so flagging it reports
        // other people's dependency choices as our violations.
        if is_cargo && Self::is_generated_hack_manifest(file) {
            return findings;
        }

        if is_cargo {
            // A manifest that does not parse is a manifest cargo itself rejects;
            // there is nothing to report on it.
            let Ok(manifest) = file.content.parse::<toml::Table>() else {
                return findings;
            };
            for (key, crate_name) in Self::banned_cargo_deps(&manifest) {
                let line_num = Self::declaration_line(&file.lines, &key);
                findings.push(self.make_finding(file, line_num, &crate_name));
            }
            return findings;
        }

        // Code-only projection of every line, computed once. Used to join `use`
        // statements that span lines without dragging comment or literal text in.
        let code_lines: Vec<String> = if is_rust {
            (1..=file.lines.len())
                .map(|n| {
                    rust_ctx
                        .map(|ctx| ctx.code_only_line(&file.content, n))
                        .unwrap_or_else(|| file.lines[n - 1].clone())
                })
                .collect()
        } else {
            Vec::new()
        };

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;

            // Skip comment lines
            let trimmed = line.trim();
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                continue;
            }

            // Match against code only. A banned crate named inside a string literal or
            // comment is a mention (test fixture, error message, this file's own regex
            // sources), not an import. `rust_ctx` carries the file-wide TokenMap, so
            // multi-line and raw literals are handled; when it is absent (Vox files, or
            // a Rust file the engine did not pre-parse) fall back to the raw line, which
            // is the previous behaviour.
            let scanned = rust_ctx
                .map(|ctx| ctx.code_only_line(&file.content, line_num))
                .unwrap_or_else(|| line.to_string());

            if is_rust {
                // A `use` may wrap across lines (`use\n    ring::digest;`) or brace a
                // group (`use {md5::Md5, std::io};`), and a single-line regex sees
                // neither. Join to the terminating `;` and inspect each path root.
                if let Some(stmt) = join_use_statement(&code_lines, i)
                    && let Some(name) = banned_use_root(&stmt)
                {
                    findings.push(self.make_finding(file, line_num, name));
                    continue;
                }
                if self.rust_banned_use.is_match(&scanned) {
                    let crate_name = self.crate_name_from_line(line).to_string();
                    findings.push(self.make_finding(file, line_num, &crate_name));
                }
            } else if self.vox_banned_import.is_match(&scanned) {
                let crate_name = self.crate_name_from_line(line).to_string();
                findings.push(self.make_finding(file, line_num, &crate_name));
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lang: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{lang}")), code.to_string())
    }

    fn cargo_source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("Cargo.toml"), code.to_string())
    }

    /// Run the detector on Rust source the way the engine does — with a
    /// `RustFileContext`, so comment and string spans are known.
    fn detect_rust(code: &str) -> Vec<Finding> {
        let f = source("rs", code);
        let ctx = crate::analysis::RustFileContext::parse(&f.content);
        CryptoBanDetector::new().detect(&f, Some(&ctx))
    }

    #[test]
    fn detects_use_ring_in_rust() {
        let d = CryptoBanDetector::new();
        let code = "use ring::digest;\n";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect `use ring::`");
        assert!(findings[0].message.contains("ring"));
        assert_eq!(
            findings[0].diagnostic_id.as_deref(),
            Some("vox/crypto/banned-crate-import")
        );
    }

    #[test]
    fn detects_extern_crate_aegis() {
        let d = CryptoBanDetector::new();
        let code = "extern crate aegis;\n";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect `extern crate aegis`");
        assert!(findings[0].message.contains("aegis"));
    }

    #[test]
    fn detects_ring_in_cargo_toml() {
        let d = CryptoBanDetector::new();
        let code = "[dependencies]\nring = \"0.17\"\nserde = \"1\"\n";
        let f = cargo_source(code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect ring in Cargo.toml");
    }

    #[test]
    fn detects_aws_lc_rs_in_cargo_toml() {
        let d = CryptoBanDetector::new();
        let code = "[dependencies]\naws-lc-rs = { version = \"1\", features = [] }\n";
        let f = cargo_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should detect aws-lc-rs in Cargo.toml"
        );
    }

    #[test]
    fn detects_vox_import_aegis() {
        let d = CryptoBanDetector::new();
        let code = "import aegis\n";
        let f = source("vox", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect `import aegis` in Vox");
    }

    #[test]
    fn detects_use_md5() {
        let d = CryptoBanDetector::new();
        let code = "use md5::Md5;\n";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect use md5");
    }

    #[test]
    fn detects_use_sha1() {
        let d = CryptoBanDetector::new();
        let code = "use sha1::Sha1;\n";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(!findings.is_empty(), "should detect use sha1");
    }

    #[test]
    fn ignores_allowed_chacha20() {
        let d = CryptoBanDetector::new();
        let code = "use chacha20poly1305::ChaCha20Poly1305;\n";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "chacha20poly1305 is allowed and should not be flagged"
        );
    }

    #[test]
    fn ignores_comment_lines() {
        let d = CryptoBanDetector::new();
        let code = "// use ring::digest; // old approach\n";
        let f = source("rs", code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "comment lines should not be flagged");
    }

    #[test]
    fn ignores_non_crypto_cargo_deps() {
        let d = CryptoBanDetector::new();
        let code =
            "[dependencies]\nserde = \"1\"\ntokio = { version = \"1\", features = [\"full\"] }\n";
        let f = cargo_source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "clean deps should not fire");
    }

    /// `keyring` contains the substring `ring`. Before word boundaries were added
    /// to `cargo_banned_dep`, `keyring = { workspace = true }` in
    /// `crates/vox-cli/Cargo.toml` matched the `ring = {` arm and reported an
    /// Error — a CI-breaking false positive that only surfaced once the scanner
    /// started handing `Cargo.toml` to detectors at all.
    #[test]
    fn cargo_dep_substring_of_banned_crate_does_not_fire() {
        let d = CryptoBanDetector::new();
        for line in [
            "keyring = { workspace = true, optional = true }",
            "keyring = \"3\"",
            "stringly = { workspace = true }",
        ] {
            let f = cargo_source(&format!("[dependencies]\n{line}\n"));
            assert!(
                d.detect(&f, None).is_empty(),
                "substring match should not fire: {line}"
            );
        }
        // ...but the real thing still must.
        assert!(
            !d.detect(
                &cargo_source("[dependencies]\nring = { version = \"0.17\" }\n"),
                None
            )
            .is_empty(),
            "genuine `ring = {{` must still fire"
        );
    }

    /// `[patch.crates-io] aegis = { path = "patches/aegis-0.9.8" }` is how the repo
    /// forces aegis's pure-Rust backend so MSVC needs no clang-cl. Flagging it would
    /// report the mitigation as the violation.
    #[test]
    fn patch_section_entries_are_not_violations() {
        let d = CryptoBanDetector::new();
        let manifest = "[dependencies]\n\
                        serde = \"1\"\n\
                        \n\
                        [patch.crates-io]\n\
                        aegis = { path = \"patches/aegis-0.9.8\" }\n";
        assert!(
            d.detect(&cargo_source(manifest), None).is_empty(),
            "patch entries are mitigations, not adoptions"
        );

        // A real dependency after the patch table must still fire — proves the
        // section tracker resets on the next header rather than muting the rest.
        let after = "[patch.crates-io]\n\
                     aegis = { path = \"patches/aegis-0.9.8\" }\n\
                     \n\
                     [dependencies]\n\
                     ring = \"0.17\"\n";
        assert!(
            !d.detect(&cargo_source(after), None).is_empty(),
            "a genuine dep after a [patch] table must still fire"
        );
    }

    /// A continuation line beginning with `[` (a wrapped `features = [` value) must
    /// not be mistaken for a table header, or the rest of the patch table becomes
    /// scannable again and its mitigations get reported as violations.
    #[test]
    fn multiline_value_inside_patch_table_does_not_end_the_section() {
        let d = CryptoBanDetector::new();
        let manifest = "[patch.crates-io]\n\
                        aegis = { path = \"patches/aegis-0.9.8\", features = [\n\
                        \"pure-rust\",\n\
                        ] }\n\
                        ring = { path = \"patches/ring\" }\n";
        assert!(
            d.detect(&cargo_source(manifest), None).is_empty(),
            "wrapped value must not terminate the [patch] table"
        );
    }

    /// Regression: the old regex arms only saw `^name =` and `name = {`, so every
    /// other legal TOML spelling of the same dependency evaded the rule. Each of
    /// these produced 0 findings before the manifest was actually parsed.
    #[test]
    fn evading_cargo_dep_spellings_now_fire() {
        let d = CryptoBanDetector::new();
        for manifest in [
            // renamed dep — the crate name only appears in `package = "..."`
            "[dependencies]\ncrypto = { package = \"ring\", version = \"0.17\" }\n",
            // workspace inheritance — this workspace's dominant idiom
            "[dependencies]\nopenssl.workspace = true\n",
            // quoted key
            "[dependencies]\n\"aegis\" = \"0.9\"\n",
            // table form
            "[dependencies.md5]\nversion = \"0.7\"\n",
            // non-default dependency kinds and target-specific tables
            "[dev-dependencies]\nring = \"0.17\"\n",
            "[build-dependencies]\nsha1 = \"0.10\"\n",
            "[target.'cfg(windows)'.dependencies]\naws-lc-rs = \"1\"\n",
            // workspace root table members inherit from
            "[workspace.dependencies]\nring = \"0.17\"\n",
        ] {
            assert!(
                !d.detect(&cargo_source(manifest), None).is_empty(),
                "should fire on: {manifest}"
            );
        }
    }

    /// Regression (Bug 1b): the hand-rolled `[patch]` tracker required a header line
    /// to *end* with `]`, so `[dev-dependencies] # test deps` was not recognised and
    /// `in_patch_section` stayed stuck, muting the rest of the manifest.
    #[test]
    fn table_header_with_trailing_comment_does_not_mute_the_manifest() {
        let d = CryptoBanDetector::new();
        let manifest = "[patch.crates-io]\n\
                        aegis = { path = \"patches/aegis-0.9.8\" }\n\
                        \n\
                        [dev-dependencies] # test deps\n\
                        ring = \"0.17\"\n";
        let findings = d.detect(&cargo_source(manifest), None);
        assert_eq!(findings.len(), 1, "commented header must not mute the rest");
        assert!(findings[0].message.contains("ring"));
    }

    /// The reported line must point at the declaration, not line 1.
    #[test]
    fn cargo_finding_points_at_the_declaration_line() {
        let d = CryptoBanDetector::new();
        let manifest = "[dependencies]\nserde = \"1\"\nopenssl.workspace = true\n";
        let findings = d.detect(&cargo_source(manifest), None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
    }

    /// A banned crate named inside a string literal, a comment, a raw string, or a
    /// multi-line literal is a mention — not an import. This is how the detector was
    /// reporting its own test fixtures and regex sources.
    #[test]
    fn banned_crate_named_only_in_non_code_does_not_fire() {
        for code in [
            "let code = \"use ring::digest;\";",
            "assert!(msg.contains(\"use aegis\"));",
            "// use openssl here one day",
            "let re = r#\"use aegis::x; use openssl;\"#;",
            "let re = r##\"extern crate md5;\"##;",
            "let s = \"use openssl;\nuse md5;\";",
        ] {
            assert!(
                detect_rust(code).is_empty(),
                "non-code mention should not fire: {code}"
            );
        }
    }

    /// The flip side: real imports must survive, including alongside non-code that
    /// mentions another banned crate on the same line.
    #[test]
    fn real_imports_still_fire_alongside_non_code() {
        assert!(!detect_rust("use ring::digest;").is_empty());
        assert!(!detect_rust("extern crate aegis;").is_empty());
        assert!(
            !detect_rust("use ring::digest; // see \"docs\"").is_empty(),
            "import with a trailing comment must still fire"
        );
        assert!(
            !detect_rust("let re = r#\"noop\"#; use ring::digest;").is_empty(),
            "import after a closed raw string must still fire"
        );
    }

    /// A `use` wrapped across lines or braced into a group evaded the single-line
    /// regex entirely — a real evasion for a security rule.
    #[test]
    fn multiline_and_grouped_use_statements_are_caught() {
        assert!(
            !detect_rust("use\n    ring::digest;").is_empty(),
            "wrapped use"
        );
        assert!(
            !detect_rust("use {md5::Md5, std::io};").is_empty(),
            "braced group"
        );
        assert!(
            !detect_rust("use ring::{digest, hmac};").is_empty(),
            "nested group"
        );
        assert!(!detect_rust("use openssl as ssl;").is_empty(), "rename");
        assert!(!detect_rust("use md5;").is_empty(), "bare name");
    }

    /// Only the ROOT path segment is a crate name, and a mention inside a literal
    /// still must not fire.
    #[test]
    fn use_root_matching_does_not_overreach() {
        assert!(
            detect_rust("use foo::ring::digest;").is_empty(),
            "not a root segment"
        );
        assert!(
            detect_rust("use keyring::Entry;").is_empty(),
            "substring of a real crate"
        );
        assert!(
            detect_rust("let s = \"use ring::digest;\";").is_empty(),
            "literal mention"
        );
        assert!(detect_rust("use std::io;").is_empty(), "unrelated import");
    }
}
