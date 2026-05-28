use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects `@capacitor/*` imports and `npx cap sync` invocations.
///
/// Covers two contract rows:
///   - `capacitor-imports`: `@capacitor/*` → `@tauri-apps/plugin-*`
///   - `cap-sync-cli`: `npx cap sync` → `cargo tauri build` / `cargo tauri dev`
///
/// File coverage:
///   - TypeScript / JavaScript (`.ts`/`.tsx`/`.js`/...): `import ... from '@capacitor/...'`
///   - `package.json`: `"@capacitor/...": "version"` dependency entries
///   - Shell / generic text (`.sh` / `Unknown`): `npx cap sync` invocations
///
/// Severity: `Warning`. Tracked under
/// [`tauri-convergence-migration-plan-2026.md`](../../../../../docs/src/architecture/tauri-convergence-migration-plan-2026.md);
/// vestigial call-sites during migration are expected to carry
/// `// vox-deprecated-since=...` annotations per AGENTS.md §Deprecation Annotations.
pub struct RetiredCapacitorDetector {
    /// TypeScript / JavaScript import pattern.
    ts_import_pattern: Regex,
    /// package.json dependency-entry pattern.
    package_json_pattern: Regex,
    /// `npx cap sync` (any subcommand) pattern.
    cap_cli_pattern: Regex,
}

impl Default for RetiredCapacitorDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RetiredCapacitorDetector {
    pub fn new() -> Self {
        Self {
            // `import * from "@capacitor/anything"` or `from '@capacitor/anything'`.
            ts_import_pattern: Regex::new(
                r#"(?:import|from)\s*[^"';]*['"]@capacitor/([a-zA-Z0-9_-]+)['"]"#,
            )
            .expect("valid regex"),
            // `"@capacitor/anything": "..."` in package.json dep blocks.
            package_json_pattern: Regex::new(r#""@capacitor/([a-zA-Z0-9_-]+)"\s*:\s*"[^"]*""#)
                .expect("valid regex"),
            // `npx cap sync`, `npx cap run`, etc. — any cap subcommand counts.
            cap_cli_pattern: Regex::new(r"\bnpx\s+cap\s+([a-z]+)\b").expect("valid regex"),
        }
    }

    fn is_ts_like(lang: Language) -> bool {
        matches!(lang, Language::TypeScript)
    }
}

impl DetectionRule for RetiredCapacitorDetector {
    fn id(&self) -> &'static str {
        "retired/capacitor"
    }

    fn name(&self) -> &'static str {
        "Retired Capacitor Detector"
    }

    fn description(&self) -> &'static str {
        "Detects retired `@capacitor/*` imports and `npx cap` CLI invocations."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        // Inspects TypeScript directly; package.json and shell scripts are
        // matched by filename / extension overrides in `detect()`.
        &[Language::TypeScript]
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::RETIRED_CAPACITOR)
    }

    fn explain(&self) -> &'static str {
        "AGENTS.md §Retired Surfaces lists Capacitor-era plugins and CLI as retired in favor of \
the Tauri 2 runtime.\n\n\
Retired → Canonical:\n\
  @capacitor/*       →  @tauri-apps/plugin-*\n\
  npx cap sync       →  cargo tauri build (or `cargo tauri dev` for hot-reload)\n\n\
The Tauri convergence is tracked in \
`docs/src/architecture/tauri-convergence-migration-plan-2026.md`. Vestigial call-sites \
during migration MUST carry a `// vox-deprecated-since=...` annotation."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — importing from retired @capacitor/* namespace\n\
             import { Camera } from '@capacitor/camera';        // retired\n\
             import { Filesystem } from '@capacitor/filesystem'; // retired\n\
             \n\
             // VIOLATION — using retired npx cap CLI\n\
             // npx cap sync\n\
             \n\
             // FIX — use Tauri 2 equivalents\n\
             import { camera } from '@tauri-apps/plugin-camera';      // current\n\
             import { fs }     from '@tauri-apps/plugin-fs';          // current\n\
             // Build: cargo tauri build   (not npx cap sync)",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let file_name = file.path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ext = file.path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let is_package_json = file_name == "package.json";
        let is_shell_or_script = matches!(ext, "sh" | "bash" | "zsh" | "ps1" | "cmd" | "bat");
        let in_scope = Self::is_ts_like(file.language) || is_package_json || is_shell_or_script;
        if !in_scope {
            return vec![];
        }

        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comments (TS `//`, package.json doesn't allow comments, shell `#`).
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with("REM ")
                || trimmed.starts_with("::")
            {
                continue;
            }

            // Skip lines explicitly about the retirement.
            if trimmed.contains("retired") || trimmed.contains("vox-deprecated-since") {
                continue;
            }

            if Self::is_ts_like(file.language)
                && let Some(caps) = self.ts_import_pattern.captures(line)
            {
                let plugin = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                let m = caps.get(0).expect("group 0");
                findings.push(self.build_finding(
                    file,
                    line_num,
                    m.start() + 1,
                    format!(
                        "Retired import `@capacitor/{plugin}` — use `@tauri-apps/plugin-{plugin}` instead."
                    ),
                    "Replace the @capacitor/* import with the matching @tauri-apps/plugin-* \
                     package per docs/src/architecture/tauri-convergence-migration-plan-2026.md."
                        .to_string(),
                ));
                continue;
            }

            if is_package_json && let Some(caps) = self.package_json_pattern.captures(line) {
                let plugin = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                let m = caps.get(0).expect("group 0");
                findings.push(self.build_finding(
                    file,
                    line_num,
                    m.start() + 1,
                    format!(
                        "Retired npm dep `@capacitor/{plugin}` — replace with `@tauri-apps/plugin-{plugin}`."
                    ),
                    "Remove the @capacitor/* dependency from package.json and add the matching \
                     @tauri-apps/plugin-* entry. Run `pnpm install` after the change."
                        .to_string(),
                ));
                continue;
            }

            if (is_shell_or_script || is_package_json)
                && let Some(caps) = self.cap_cli_pattern.captures(line)
            {
                let sub = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                let m = caps.get(0).expect("group 0");
                findings.push(self.build_finding(
                    file,
                    line_num,
                    m.start() + 1,
                    format!("Retired CLI `npx cap {sub}` — use `cargo tauri` instead."),
                    "Replace `npx cap sync` with `cargo tauri build` (or `cargo tauri dev` for \
                     hot-reload). See docs/src/architecture/tauri-convergence-migration-plan-2026.md."
                        .to_string(),
                ));
            }
        }

        findings
    }
}

impl RetiredCapacitorDetector {
    fn build_finding(
        &self,
        file: &SourceFile,
        line: usize,
        column: usize,
        message: String,
        suggestion: String,
    ) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: self.diagnostic_id().map(str::to_string),
            rule_name: self.name().to_string(),
            severity: Severity::Warning,
            file: file.path.clone(),
            line,
            column,
            message,
            suggestion: Some(suggestion),
            alternatives: vec![],
            rationale: Some(
                "The Tauri convergence (2026-Q2 migration plan) replaces Capacitor as the \
                 desktop/mobile runtime. @tauri-apps/plugin-* packages provide drop-in \
                 equivalents for most Capacitor plugins."
                    .to_string(),
            ),
            context: file.context_around(line, 2),
            confidence: Some(FindingConfidence::High),
            evidence: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ts(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("app.ts"), code.to_string())
    }

    fn package_json(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("package.json"), code.to_string())
    }

    fn shell(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("build.sh"), code.to_string())
    }

    #[test]
    fn flags_capacitor_import_in_typescript() {
        let d = RetiredCapacitorDetector::new();
        let f = ts(r#"import { Foo } from "@capacitor/filesystem";"#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0]
                .message
                .contains("@tauri-apps/plugin-filesystem")
        );
    }

    #[test]
    fn flags_capacitor_dep_in_package_json() {
        let d = RetiredCapacitorDetector::new();
        let f = package_json(r#"  "@capacitor/camera": "^5.0.0","#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("@tauri-apps/plugin-camera"));
    }

    #[test]
    fn flags_npx_cap_sync_in_shell_script() {
        let d = RetiredCapacitorDetector::new();
        let f = shell(r#"npx cap sync ios"#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("cargo tauri"));
    }

    #[test]
    fn flags_npx_cap_run_in_shell_script() {
        let d = RetiredCapacitorDetector::new();
        let f = shell(r#"npx cap run android"#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("npx cap run"));
    }

    #[test]
    fn does_not_flag_tauri_canonical_import() {
        let d = RetiredCapacitorDetector::new();
        let f = ts(r#"import { fs } from "@tauri-apps/plugin-filesystem";"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_typescript_comment_lines() {
        let d = RetiredCapacitorDetector::new();
        let f = ts(r#"// import { Foo } from "@capacitor/filesystem";"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_shell_comment_lines() {
        let d = RetiredCapacitorDetector::new();
        let f = shell("# npx cap sync ios");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_lines_explaining_the_retirement() {
        let d = RetiredCapacitorDetector::new();
        let f = ts(r#"const note = "@capacitor/camera is retired";"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn does_not_fire_on_rust_files() {
        let d = RetiredCapacitorDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.rs"),
            r#"// import "@capacitor/foo" — not a TS file"#.to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn finds_multiple_capacitor_deps_on_separate_lines() {
        let d = RetiredCapacitorDetector::new();
        let f = package_json(
            r#"  "@capacitor/camera": "^5",
  "@capacitor/filesystem": "^5""#,
        );
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 2);
    }
}
