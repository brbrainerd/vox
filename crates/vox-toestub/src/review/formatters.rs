use std::path::Path;
use crate::rules::Severity;
use super::types::{ReviewFinding, ReviewResult, ReviewCategory};

/// Parse the structured `ISSUE|...` response format into findings.
pub fn parse_review_response(response: &str, file_path: &Path) -> Vec<ReviewFinding> {
    let mut findings = Vec::new();

    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed == "CLEAN" || trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with("ISSUE|") {
            continue;
        }

        // Format: ISSUE|line|severity|category|confidence|message|suggestion
        let parts: Vec<&str> = trimmed.splitn(7, '|').collect();
        if parts.len() < 6 {
            continue;
        }

        let line_num = parts[1].parse::<usize>().unwrap_or(0);
        let severity = match parts[2].trim().to_lowercase().as_str() {
            "critical" => Severity::Critical,
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            _ => Severity::Info,
        };
        let category = ReviewCategory::parse_category(parts[3].trim());
        let confidence = parts[4].trim().parse::<u8>().unwrap_or(50);
        let message = parts[5].trim().to_string();
        let suggestion = parts
            .get(6)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "-")
            .map(|s| s.to_string());

        findings.push(ReviewFinding {
            category,
            severity,
            file: file_path.to_path_buf(),
            line: line_num,
            message,
            suggestion,
            confidence,
        });
    }

    findings
}

/// Format review findings for terminal output with icons per severity.
pub fn format_terminal(result: &ReviewResult) -> String {
    let mut out = String::new();

    if result.findings.is_empty() {
        out.push_str("🔍 vox review: No issues found.\n");
        return out;
    }

    out.push_str(&format!(
        "🔍 vox review — {} issue(s) via {}\n\n",
        result.findings.len(),
        result.provider_used
    ));

    // Group by file for display
    let mut by_file: std::collections::BTreeMap<&Path, Vec<&ReviewFinding>> =
        std::collections::BTreeMap::new();
    for f in &result.findings {
        by_file.entry(f.file.as_path()).or_default().push(f);
    }

    for (file, findings) in &by_file {
        out.push_str(&format!("  📄 {}\n", file.display()));
        for f in findings {
            let icon = match f.severity {
                Severity::Critical => "🔴",
                Severity::Error => "🟠",
                Severity::Warning => "🟡",
                Severity::Info => "🔵",
            };
            let location = if f.line > 0 {
                format!("L{}", f.line)
            } else {
                "file".to_string()
            };
            out.push_str(&format!(
                "    {} [{}] [{} {}] {} (conf: {}%)\n",
                icon,
                f.severity,
                location,
                f.category.rule_prefix(),
                f.message,
                f.confidence
            ));
            if let Some(ref sug) = f.suggestion {
                out.push_str(&format!("      💡 {}\n", sug));
            }
        }
        out.push('\n');
    }

    if result.tokens_used > 0 {
        out.push_str(&format!(
            "  ℹ  Tokens used: {} | Estimated cost: ${:.4}\n",
            result.tokens_used, result.cost_estimate_usd
        ));
    }

    out
}

/// Format as SARIF 2.1.0 JSON (compatible with GitHub Code Scanning).
pub fn format_sarif(result: &ReviewResult) -> String {
    let rules: Vec<serde_json::Value> = result
        .findings
        .iter()
        .map(|f| f.category)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|cat| {
            serde_json::json!({
                "id": format!("vox-review/{}", cat.rule_prefix()),
                "name": cat.to_string(),
                "shortDescription": { "text": format!("vox review: {}", cat) },
                "helpUri": "https://github.com/brbrainerd/vox"
            })
        })
        .collect();

    let results: Vec<serde_json::Value> = result
        .findings
        .iter()
        .map(|f| {
            let mut loc = serde_json::json!({
                "physicalLocation": {
                    "artifactLocation": {
                        "uri": f.file.to_string_lossy().replace('\\', "/")
                    }
                }
            });
            if f.line > 0 {
                loc["physicalLocation"]["region"] = serde_json::json!({ "startLine": f.line });
            }
            serde_json::json!({
                "ruleId": format!("vox-review/{}", f.category.rule_prefix()),
                "level": sarif_level(f.severity),
                "message": { "text": f.message },
                "locations": [loc]
            })
        })
        .collect();

    let sarif = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "vox-review",
                    "version": env!("CARGO_PKG_VERSION"),
                    "rules": rules
                }
            },
            "results": results
        }]
    });

    serde_json::to_string_pretty(&sarif).unwrap_or_else(|_| "{}".to_string())
}

fn sarif_level(s: Severity) -> &'static str {
    match s {
        Severity::Info => "note",
        Severity::Warning => "warning",
        Severity::Error | Severity::Critical => "error",
    }
}

/// Format as Markdown for PR comment or file output.
pub fn format_markdown(result: &ReviewResult) -> String {
    let mut out = String::new();
    out.push_str("## 🔍 vox review\n\n");

    if result.findings.is_empty() {
        out.push_str("✅ No issues found.\n");
        return out;
    }

    out.push_str(&format!(
        "**{} issue(s)** detected via `{}`\n\n",
        result.findings.len(),
        result.provider_used
    ));

    // Table header
    out.push_str("| File | Line | Severity | Category | Issue |\n");
    out.push_str("|:-----|:-----|:---------|:---------|:------|\n");

    for f in &result.findings {
        let line = if f.line > 0 {
            format!("{}", f.line)
        } else {
            "—".to_string()
        };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            f.file.display(),
            line,
            f.severity,
            f.category,
            f.message.replace('|', "\\|")
        ));
    }

    out.push('\n');
    if result.tokens_used > 0 {
        out.push_str(&format!(
            "> Tokens: {} | Cost: ~${:.4}\n",
            result.tokens_used, result.cost_estimate_usd
        ));
    }

    out
}
