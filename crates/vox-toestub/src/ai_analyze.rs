//! Optional AI-powered analysis layer.
//!
//! TOESTUB can optionally use an AI model to perform deeper semantic analysis
//! beyond what static regex/AST patterns can catch. This module supports:
//!
//! 1. **Ollama (local)** — Zero auth, fully redistributable, runs on user's machine
//! 2. **Gemini Flash (free tier)** — Requires a free API key (no credit card)
//! 3. **OpenRouter free models** — Aggregator with some free models
//!
//! The AI layer is **entirely optional** — TOESTUB works fully offline with
//! just the static detectors. AI analysis enhances detection for subtle patterns
//! that regexes miss: semantic dead code, inconsistent naming, logic gaps, etc.

use serde::{Deserialize, Serialize};

use crate::rules::{Finding, Severity, SourceFile};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Which AI backend to use for enhanced analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
#[derive(Default)]
pub enum AiProvider {
    /// Local Ollama instance — zero auth, the recommended default.
    /// Requires Ollama installed and a model pulled (e.g. `ollama pull codellama`).
    Ollama {
        /// Ollama API endpoint (default: http://localhost:11434)
        #[serde(default = "default_ollama_url")]
        url: String,
        /// Model name (default: codellama)
        #[serde(default = "default_ollama_model")]
        model: String,
    },
    /// Google Gemini Flash free tier — requires a free API key from
    /// https://aistudio.google.com/apikey (no credit card needed).
    Gemini {
        /// API key (can also be set via GEMINI_API_KEY env var)
        #[serde(default)]
        api_key: String,
        /// Model name (default: gemini-2.5-flash)
        #[serde(default = "default_gemini_model")]
        model: String,
    },
    /// Disabled — no AI analysis, pure static detection.
    #[default]
    Disabled,
    /// Pollinations.ai — zero API key, zero signup, free.
    Pollinations {
        /// Model to request (default: openai)
        #[serde(default = "default_pollinations_model")]
        model: String,
    },
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_ollama_model() -> String {
    "codellama".to_string()
}

fn default_gemini_model() -> String {
    "gemini-2.5-flash".to_string()
}

fn default_pollinations_model() -> String {
    "openai".to_string()
}

// ---------------------------------------------------------------------------
// AI Analyzer
// ---------------------------------------------------------------------------

/// Performs AI-enhanced analysis on source files.
///
/// This is intentionally synchronous and blocking for simplicity —
/// AI analysis is opt-in and expected to be slower than static detection.
pub struct AiAnalyzer {
    provider: AiProvider,
}

impl AiAnalyzer {
    pub fn new(provider: AiProvider) -> Self {
        Self { provider }
    }

    /// Check if AI analysis is available and configured.
    pub fn is_available(&self) -> bool {
        !matches!(self.provider, AiProvider::Disabled)
    }

    /// Build the analysis prompt for a source file and its existing findings.
    pub fn build_prompt(file: &SourceFile, existing_findings: &[Finding]) -> String {
        let findings_summary = if existing_findings.is_empty() {
            "No issues detected by static analysis.".to_string()
        } else {
            existing_findings
                .iter()
                .map(|f| format!("- L{}: [{}] {}", f.line, f.rule_id, f.message))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"You are a code quality reviewer. Analyze this source file for issues that static analysis might miss.

FILE: {}
LANGUAGE: {}

EXISTING FINDINGS:
{}

SOURCE CODE:
```
{}
```

Look for:
1. Functions that appear to be stubs (have a body but don't actually do anything meaningful)
2. Values that should be configurable but are hardcoded
3. Logic that is duplicated across functions
4. Missing error handling or edge cases
5. Incomplete implementations that were started but not finished
6. References to functions, types, or modules that don't exist in this file

For each issue found, respond with EXACTLY this format (one per line):
FINDING|line_number|severity(info/warning/error)|rule_id|message

If no additional issues are found, respond with: CLEAN

Do not explain your reasoning. Only output findings or CLEAN."#,
            file.path.display(),
            file.language,
            findings_summary,
            // Limit source code to avoid token limits
            if file.content.len() > 8000 {
                &file.content[..8000]
            } else {
                &file.content
            },
        )
    }

    /// Parse an AI response into findings.
    pub fn parse_response(response: &str, file: &SourceFile) -> Vec<Finding> {
        let mut findings = Vec::new();

        for line in response.lines() {
            let trimmed = line.trim();
            if trimmed == "CLEAN" || trimmed.is_empty() {
                continue;
            }
            if !trimmed.starts_with("FINDING|") {
                continue;
            }

            let parts: Vec<&str> = trimmed.splitn(5, '|').collect();
            if parts.len() != 5 {
                continue;
            }

            let line_num = parts[1].parse::<usize>().unwrap_or(0);
            let severity = match parts[2] {
                "error" => Severity::Error,
                "warning" => Severity::Warning,
                "info" => Severity::Info,
                _ => Severity::Warning,
            };

            findings.push(Finding {
                rule_id: format!("ai/{}", parts[3]),
                rule_name: "AI-Enhanced Detector".to_string(),
                severity,
                file: file.path.clone(),
                line: line_num,
                column: 0,
                message: parts[4].to_string(),
                suggestion: Some(
                    "This issue was detected by AI analysis. Review and fix if valid.".to_string(),
                ),
                context: if line_num > 0 {
                    file.context_around(line_num, 2)
                } else {
                    String::new()
                },
            });
        }

        findings
    }

    /// Generate the curl-compatible request body for Ollama.
    pub fn ollama_request_body(prompt: &str, model: &str) -> String {
        serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.1,
                "num_predict": 2048
            }
        })
        .to_string()
    }

    /// Generate the request body for Gemini API.
    pub fn gemini_request_body(prompt: &str) -> String {
        serde_json::json!({
            "contents": [{
                "parts": [{
                    "text": prompt
                }]
            }],
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 2048
            }
        })
        .to_string()
    }

    /// Get the API endpoint URL for the configured provider.
    pub fn endpoint_url(&self) -> Option<String> {
        match &self.provider {
            AiProvider::Ollama { url, .. } => Some(format!("{}/api/generate", url)),
            AiProvider::Gemini { api_key, model } => {
                let key = if api_key.is_empty() {
                    std::env::var("GEMINI_API_KEY").unwrap_or_default()
                } else {
                    api_key.clone()
                };
                if key.is_empty() {
                    None
                } else {
                    Some(format!(
                        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                        model, key
                    ))
                }
            }
            AiProvider::Disabled => None,
            AiProvider::Pollinations { model } => {
                // Pollinations uses a GET endpoint; we return a base URL.
                // The caller encodes the prompt into the path.
                Some(format!(
                    "https://text.pollinations.ai/?model={}&nologo=true",
                    model
                ))
            }
        }
    }

    /// Get the provider description for display purposes.
    pub fn provider_name(&self) -> &str {
        match &self.provider {
            AiProvider::Ollama { .. } => "Ollama (local)",
            AiProvider::Gemini { .. } => "Gemini Flash (free tier)",
            AiProvider::Disabled => "Disabled",
            AiProvider::Pollinations { .. } => "Pollinations.ai (free)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_clean_response() {
        let file = SourceFile::new(PathBuf::from("test.rs"), "fn main() {}".to_string());
        let findings = AiAnalyzer::parse_response("CLEAN", &file);
        assert!(findings.is_empty());
    }

    #[test]
    fn parse_finding_response() {
        let file = SourceFile::new(
            PathBuf::from("test.rs"),
            "fn foo() {}\nfn bar() {}".to_string(),
        );
        let response = "FINDING|1|warning|empty-stub|Function foo has an empty body\nFINDING|2|error|missing-logic|Function bar does nothing meaningful";
        let findings = AiAnalyzer::parse_response(response, &file);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule_id, "ai/empty-stub");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[1].severity, Severity::Error);
    }

    #[test]
    fn build_prompt_includes_file_info() {
        let file = SourceFile::new(
            PathBuf::from("src/handler.rs"),
            "fn process() { todo!() }".to_string(),
        );
        let prompt = AiAnalyzer::build_prompt(&file, &[]);
        assert!(prompt.contains("handler.rs"));
        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("FINDING|"));
    }

    #[test]
    fn ollama_endpoint_url() {
        let analyzer = AiAnalyzer::new(AiProvider::Ollama {
            url: "http://localhost:11434".to_string(),
            model: "codellama".to_string(),
        });
        assert_eq!(
            analyzer.endpoint_url().unwrap(),
            "http://localhost:11434/api/generate"
        );
    }

    #[test]
    fn disabled_has_no_endpoint() {
        let analyzer = AiAnalyzer::new(AiProvider::Disabled);
        assert!(analyzer.endpoint_url().is_none());
        assert!(!analyzer.is_available());
    }
}
