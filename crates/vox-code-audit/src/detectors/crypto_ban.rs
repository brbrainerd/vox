use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects imports/uses of banned cryptography crates (aegis, ring, md5, sha1, openssl, etc.).
pub struct CryptoBanDetector {
    /// Matches banned Vox import statements.
    vox_banned_import: Regex,
    /// Matches banned Rust `use` / `extern crate` statements.
    rust_banned_use: Regex,
    /// Matches banned Cargo.toml dependency lines.
    cargo_banned_dep: Regex,
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

            // Cargo.toml: dependency lines for banned crates or cmake+nasm combination
            cargo_banned_dep: Regex::new(
                r#"(?x)
                (?:
                    ^(?:aegis|ring|aws-lc-rs|md5|sha1|openssl)\s*=
                  | (?:aegis|ring|aws-lc-rs)\s*=\s*\{
                  | features\s*=\s*\[.*\bcmake\b.*\bnasm\b
                  | features\s*=\s*\[.*\bnasm\b.*\bcmake\b
                )
                "#,
            )
            .expect("valid cargo_banned_dep regex"),

            // Unknown is included so Cargo.toml/.lock (which map to Language::Unknown) are scanned.
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

    /// Returns true if this file path looks like a Cargo.toml or Cargo.lock.
    fn is_cargo_manifest(file: &SourceFile) -> bool {
        file.path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "Cargo.toml" || n == "Cargo.lock")
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
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        let is_cargo = Self::is_cargo_manifest(file);
        let is_rust = file.language == Language::Rust;
        let is_vox = file.language == Language::Vox;

        // Only process Cargo manifests, Rust, and Vox files.
        if !is_cargo && !is_rust && !is_vox {
            return findings;
        }

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

            let matched = if is_cargo {
                self.cargo_banned_dep.is_match(trimmed)
            } else if is_rust {
                self.rust_banned_use.is_match(line)
            } else {
                // Vox
                self.vox_banned_import.is_match(line)
            };

            if matched {
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
}
