use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Three iroh patterns that silently defeat the mesh's security model (ADR-046).
///
/// Each is a compile-clean, test-passing change that removes a guarantee the
/// rest of `vox-mesh-transport` assumes, which is exactly why a detector and
/// not a code review is the right control:
///
/// - `presets::N0` configures pkarr publishing, DNS address lookup, and n0's
///   relay servers. The mesh must contact no third party and must keep working
///   with both machines off the internet.
/// - `N0DisableRelay` reads as "no n0" but still installs n0's DNS and pkarr
///   discovery — it disables only the relays, so the name invites the mistake.
/// - `into_0rtt()` makes `remote_id()` fallible. The `?` someone then adds to
///   satisfy the compiler turns every trust check in the accept loop advisory,
///   with no error, no failing test, and no reviewer prompt.
pub struct MeshTransportUnsafeDetector {
    pattern: Regex,
    supported_langs: Vec<Language>,
}

impl Default for MeshTransportUnsafeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshTransportUnsafeDetector {
    pub fn new() -> Self {
        Self {
            // Word-bounded so `presets::N0` does not also match `presets::N0`-prefixed
            // identifiers, and `N0DisableRelay` is matched as a whole symbol.
            pattern: Regex::new(r"\b(presets::N0|N0DisableRelay|into_0rtt)\b")
                .expect("valid mesh transport pattern"),
            supported_langs: vec![Language::Rust],
        }
    }

    /// This detector's own source and its test fixtures name the patterns in
    /// order to match them; exempting the file by path avoids a self-report
    /// that would make the gate permanently red.
    fn is_self(path: &std::path::Path) -> bool {
        let s = path.to_string_lossy().replace('\\', "/");
        s.ends_with("detectors/mesh_transport_unsafe.rs")
    }

    fn why(matched: &str) -> &'static str {
        match matched {
            "presets::N0" => {
                "`presets::N0` installs a PkarrPublisher, a PkarrResolver, a DnsAddressLookup \
                 and n0's default relays. The mesh must contact no third party and must work \
                 with both machines off the internet. Use `presets::Minimal`."
            }
            "N0DisableRelay" => {
                "`N0DisableRelay` disables only the relays — it still installs n0's DNS and \
                 pkarr discovery, so the name promises more isolation than it delivers. \
                 Use `presets::Minimal`."
            }
            _ => {
                "`into_0rtt()` makes `Connection::remote_id()` fallible. Every trust check in \
                 the accept loop assumes it is infallible, so the `?` added to satisfy the \
                 compiler silently downgrades authentication to advisory. Await the \
                 `Incoming` instead and complete the handshake."
            }
        }
    }

    fn make_finding(&self, file: &SourceFile, line_num: usize, matched: &str) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: Some(catalog::MESH_TRANSPORT_UNSAFE_IROH.to_string()),
            rule_name: self.name().to_string(),
            severity: Severity::Error,
            file: file.path.clone(),
            line: line_num,
            column: 0,
            message: format!("`{matched}` defeats the mesh transport security model (ADR-046)."),
            suggestion: Some(Self::why(matched).to_string()),
            alternatives: vec![
                "iroh::endpoint::presets::Minimal".to_string(),
                "Endpoint::builder(presets::Minimal).alpns(...).bind()".to_string(),
            ],
            rationale: Some(Self::why(matched).to_string()),
            context: file.context_around(line_num, 2),
            confidence: Some(FindingConfidence::High),
            evidence: None,
        }
    }
}

impl DetectionRule for MeshTransportUnsafeDetector {
    fn id(&self) -> &'static str {
        "vox/mesh/unsafe-iroh-pattern"
    }

    fn name(&self) -> &'static str {
        "Mesh Transport Unsafe iroh Pattern Detector"
    }

    fn description(&self) -> &'static str {
        "Detects iroh patterns that defeat the mesh security model: presets::N0, \
         N0DisableRelay, and into_0rtt."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::MESH_TRANSPORT_UNSAFE_IROH)
    }

    fn explain(&self) -> &'static str {
        "Three iroh patterns silently defeat the mesh security model (ADR-046). All three \
         compile, pass tests, and pass review.\n\n\
         BAD:\n  Endpoint::builder(presets::N0)          // contacts n0 DNS, pkarr, relays\n\
         \x20 Endpoint::builder(presets::N0DisableRelay) // still contacts n0 DNS and pkarr\n\
         \x20 let conn = incoming.into_0rtt()?;          // remote_id() becomes fallible\n\n\
         GOOD:\n  Endpoint::builder(presets::Minimal).alpns(vec![ALPN.to_vec()]).bind().await?"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — contacts n0 infrastructure\n\
             let ep = Endpoint::builder(presets::N0).bind().await?;\n\
             \n\
             // FIX — no relay, no discovery, no third party\n\
             let ep = Endpoint::builder(presets::Minimal).bind().await?;",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if Self::is_self(&file.path) {
            return Vec::new();
        }

        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            let trimmed = line.trim();
            // Doc comments name these patterns to warn about them; that is the
            // point of the warning, not a violation of it.
            if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
                continue;
            }
            if let Some(m) = self.pattern.find(line) {
                findings.push(self.make_finding(file, i + 1, m.as_str()));
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scan(src: &str) -> Vec<Finding> {
        // `.rs` so the language is inferred exactly as the scanner infers it —
        // constructing a SourceFile with a hand-set language is how the crypto
        // detector's tests passed for months against a branch that never ran.
        let file = SourceFile::new(
            PathBuf::from("crates/vox-mesh-transport/src/endpoint.rs"),
            src.to_string(),
        );
        assert_eq!(file.language, Language::Rust, "fixture must scan as Rust");
        MeshTransportUnsafeDetector::new().detect(&file, None)
    }

    #[test]
    fn presets_n0_fires() {
        let f = scan("let ep = Endpoint::builder(presets::N0).bind().await?;");
        assert_eq!(f.len(), 1, "presets::N0 must be an Error");
        assert_eq!(f[0].severity, Severity::Error);
    }

    #[test]
    fn n0_disable_relay_fires() {
        let f = scan("let ep = Endpoint::builder(presets::N0DisableRelay).bind().await?;");
        assert!(
            !f.is_empty(),
            "N0DisableRelay still installs n0 DNS and pkarr"
        );
    }

    #[test]
    fn into_0rtt_fires() {
        let f = scan("let (conn, _) = incoming.into_0rtt()?;");
        assert_eq!(f.len(), 1, "into_0rtt makes remote_id() fallible");
        assert!(f[0].message.contains("into_0rtt"));
    }

    #[test]
    fn the_sanctioned_preset_is_clean() {
        let f = scan(
            "let ep = Endpoint::builder(presets::Minimal)\n    .alpns(vec![ALPN.to_vec()])\n    .bind()\n    .await?;",
        );
        assert!(f.is_empty(), "presets::Minimal is the correct call: {f:?}");
    }

    #[test]
    fn a_comment_warning_about_the_pattern_is_not_a_violation() {
        let f = scan("// Never call into_0rtt(): remote_id() becomes fallible.");
        assert!(
            f.is_empty(),
            "documenting the hazard must not trip the gate"
        );
    }
}
