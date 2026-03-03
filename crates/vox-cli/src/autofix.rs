use anyhow::Result;
use std::path::Path;
use vox_toestub::{ReviewClient, auto_discover_providers};
use vox_typeck::diagnostics::Diagnostic;

/// Attempt to fix type errors in a source file using an LLM.
pub async fn autofix_file(path: &Path, source: &str, diagnostics: &[Diagnostic]) -> Result<String> {
    if diagnostics.is_empty() {
        return Ok(source.to_string());
    }

    let providers = auto_discover_providers();
    if providers.is_empty() {
        return Err(anyhow::anyhow!("No AI providers configured. Set OPENROUTER_API_KEY, OPENAI_API_KEY, or GEMINI_API_KEY."));
    }

    let client = ReviewClient::new(providers.clone());
    let diagnostics_summary = diagnostics.iter()
        .map(|d| format!("- L{}: {}", d.span.start, d.message)) // simplistic line mapping
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        r#"You are a Vox compiler expert. Fix the following type errors in the source code.

FILE: {}
ERRORS:
{}

SOURCE CODE:
```vox
{}
```

Respond ONLY with the complete, corrected source code. Do not include any explanations, markdown headers (except the code block), or conversational text.
If you cannot fix it, return the original source code unchanged."#,
        path.display(),
        diagnostics_summary,
        source
    );

    // Try providers in sequence
    for provider in &providers {
        match client.call_provider(provider, &prompt).await {
            Ok((fixed_source, _)) => {
                // Strip potential markdown code blocks if the LLM included them
                let mut stripped = fixed_source.clone();
                if stripped.contains("```") {
                    if let Some(code) = stripped.split("```").find(|s| s.contains("fn ") || s.contains("let ") || s.contains("import ")) {
                        stripped = code.trim().to_string();
                    }
                }

                // If it looks like it just returned the prompt or empty, skip
                if stripped.is_empty() || stripped == source {
                    continue;
                }

                return Ok(stripped);
            }
            Err(_) => continue,
        }
    }

    Err(anyhow::anyhow!("All AI providers failed to fix the code."))
}
