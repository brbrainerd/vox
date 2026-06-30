/// Extract the first comment block from the top of a `.vox` file as the prompt.
fn extract_file_doc(source: &str) -> Option<String> {
    let mut doc_lines = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            let text = trimmed
                .trim_start_matches('#')
                .trim_start_matches("//")
                .trim();
            if !text.is_empty() {
                doc_lines.push(text.to_string());
            }
        } else if trimmed.is_empty() {
            if !doc_lines.is_empty() {
                break; // End of leading comment block
            }
        } else {
            break; // Non-comment, non-empty line
        }
    }
    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join(" "))
    }
}

/// Infer a category from filename and content keywords.
fn infer_vox_category(path: &Path, source: &str) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Check for specific construct keywords in source
    let content_lower = source.to_lowercase();
    if content_lower.contains("@workflow") || content_lower.contains("workflow fn") {
        return "workflow".to_string();
    }
    if content_lower.contains("actor ") {
        return "actor".to_string();
    }
    if content_lower.contains("@mcp.tool") {
        return "mcp_tool".to_string();
    }
    let line_starts = |kw: &str| source.lines().any(|l| l.trim_start().starts_with(kw));
    if content_lower.contains("@table") || line_starts("table ") || line_starts("table(") {
        return "table".to_string();
    }
    if content_lower.contains("@component") || content_lower.contains("component fn") {
        return "component".to_string();
    }
    if content_lower.contains("@query") || line_starts("query ") {
        return "query".to_string();
    }
    if content_lower.contains("@mutation") || line_starts("mutation ") {
        return "mutation".to_string();
    }

    // Fall back to filename heuristics
    match stem {
        s if s.contains("test") => "test".to_string(),
        s if s.contains("workflow") || s.contains("durable") => "workflow".to_string(),
        s if s.contains("agent") => "agent_def".to_string(),
        s if s.contains("component") || s.contains("dashboard") => "component".to_string(),
        s if s.contains("server") => "server_fn".to_string(),
        s if s.contains("route") => "http_route".to_string(),
        _ => "function".to_string(), // Default: most .vox files contain functions
    }
}

/// Extract top-level declaration slices: AST path when `ast-extract` is enabled, else heuristics.
fn extract_construct_blocks(source: &str) -> Vec<(String, String, String)> {
    #[cfg(feature = "ast-extract")]
    if let Some(v) = part_ast::extract_decl_blocks_ast(source) {
        return v;
    }
    extract_construct_blocks_heuristic(source)
}

/// Legacy line scanner (brace-depth). Used when `ast-extract` is off or parse fails upstream.
fn extract_construct_blocks_heuristic(source: &str) -> Vec<(String, String, String)> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Detect construct starts
        let (construct_type, name): (&str, String) = if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ")
        {
            let n = extract_vox_name(trimmed, "fn ");
            ("fn", n)
        } else if trimmed.starts_with("actor ") {
            let n = extract_vox_name(trimmed, "actor ");
            ("actor", n)
        } else if trimmed.starts_with("@workflow") || trimmed.contains("workflow fn") {
            let n = extract_vox_name(trimmed, "fn ");
            ("workflow", n)
        } else if trimmed.starts_with("@table")
            || trimmed.starts_with("table ")
            || trimmed.starts_with("table(")
        {
            let n = extract_vox_type_name(trimmed);
            ("table", n)
        } else if trimmed.starts_with("type ") {
            let n = extract_vox_name(trimmed, "type ");
            ("type", n)
        } else if trimmed.starts_with("@component") || trimmed.starts_with("component fn") {
            let n = extract_vox_name(trimmed, "fn ");
            ("component", n)
        } else if trimmed.starts_with("@mcp.tool") {
            let n = extract_vox_name(trimmed, "fn ");
            ("mcp_tool", n)
        } else if trimmed.starts_with("@query") || trimmed.starts_with("query ") {
            let n = extract_vox_name(trimmed, if trimmed.starts_with("query ") { "query " } else { "fn " });
            ("query", n)
        } else if trimmed.starts_with("@mutation") || trimmed.starts_with("mutation ") {
            let n = extract_vox_name(trimmed, if trimmed.starts_with("mutation ") { "mutation " } else { "fn " });
            ("mutation", n)
        } else if trimmed.starts_with("@test") || trimmed.starts_with("test fn") {
            let n = extract_vox_name(trimmed, "fn ");
            ("test", n)
        } else {
            i += 1;
            continue;
        };

        // Collect the construct block by brace depth
        let block_start = i;
        let mut depth: i32 = 0;
        let mut found_open = false;
        let mut block_lines: Vec<&str> = Vec::new();

        while i < lines.len() {
            let line = lines[i];
            block_lines.push(line);
            for c in line.chars() {
                if c == '{' {
                    depth += 1;
                    found_open = true;
                } else if c == '}' {
                    depth -= 1;
                }
            }
            i += 1;
            if found_open && depth <= 0 {
                break;
            }
            // Also stop if we hit a new top-level construct (no braces found after 2 lines)
            if !found_open
                && i > block_start + 2
                && !lines.get(i).is_none_or(|l| {
                    l.starts_with(' ') || l.starts_with('\t') || l.trim().is_empty()
                })
            {
                break;
            }
        }

        if block_lines.len() >= 2 {
            blocks.push((construct_type.to_string(), name, block_lines.join("\n")));
        }
    }

    blocks
}

fn extract_vox_name(line: &str, after_kw: &str) -> String {
    if let Some(pos) = line.find(after_kw) {
        let rest = &line[pos + after_kw.len()..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if end > 0 {
            return rest[..end].to_string();
        }
    }
    "unnamed".to_string()
}

fn extract_vox_type_name(line: &str) -> String {
    let t = line.trim_start();
    // Soft-keyword form: `table Name {` or `table(args) Name {`.
    let kw_rest = t
        .strip_prefix("table(")
        .and_then(|r| r.split_once(')').map(|(_, after)| after))
        .or_else(|| t.strip_prefix("table "));
    if let Some(rest) = kw_rest {
        let rest = rest.trim_start();
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if end > 0 {
            return rest[..end].to_string();
        }
    }
    // Legacy decorator form: `@table type Name`.
    if let Some(pos) = line.find("type ") {
        let rest = &line[pos + 5..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if end > 0 {
            return rest[..end].to_string();
        }
    }
    "unnamed".to_string()
}

/// Get a prompt template for a construct type and name.
fn construct_prompt(construct_type: &str, name: &str, seed: usize) -> String {
    for &(ct, templates) in CONSTRUCT_PROMPTS {
        if ct == construct_type {
            let tmpl = templates[seed % templates.len()];
            return tmpl.replace("{name}", name);
        }
    }
    format!("Write a Vox {construct_type} called `{name}`")
}

/// Golden examples: `// @training_prompt:` wins; else `description:` inside `// ---` frontmatter.
pub(super) fn extract_golden_prompt_summary(source: &str) -> Option<String> {
    for line in source.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("//").map(str::trim)
            && let Some(p) = rest.strip_prefix("@training_prompt:") {
                let p = p.trim();
                if !p.is_empty() {
                    return Some(p.to_string());
                }
            }
    }
    let mut in_fm = false;
    let mut desc: Option<String> = None;
    for line in source.lines() {
        let t = line.trim();
        if t == "// ---" {
            in_fm = !in_fm;
            continue;
        }
        if in_fm
            && let Some(rest) = t.strip_prefix("//").map(str::trim)
                && let Some(d) = rest.strip_prefix("description:") {
                    let d = d.trim().trim_matches('"').trim_matches('\'').trim();
                    if !d.is_empty() {
                        desc = Some(d.to_string());
                    }
                }
    }
    desc
}

/// Check if content contains frontmatter indicating it should be excluded from training.
fn is_eligible_for_training(content: &str) -> bool {
    // Vox golden examples have frontmatter commented out with //
    if content.contains("training_eligible: false") || content.contains("training_eligible:false") {
        return false;
    }
    if content.contains("status: deprecated")
        || content.contains("status: \"deprecated\"")
        || content.contains("status: 'deprecated'")
    {
        return false;
    }
    true
}
