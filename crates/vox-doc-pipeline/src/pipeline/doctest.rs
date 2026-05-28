use std::path::Path;
use vox_compiler::pipeline::check_file;

use crate::pipeline::types::{LintError, LintKind};

pub fn check_doctests(path: &Path, content: &str, errors: &mut Vec<LintError>) {
    let mut in_fence = false;
    let mut current_block = String::new();
    let mut has_skip = false;
    let mut is_vox = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_fence {
                in_fence = false;
                if is_vox && !has_skip {
                    // Collect and add a newline
                    current_block.push('\n');
                }
            } else {
                in_fence = true;
                has_skip = false;

                let count = trimmed.chars().take_while(|&c| c == '`').count();
                let lang = trimmed[count..].trim();
                // Only ```vox is Vox source. Other fences (tsx, rust, …) are illustrative;
                // parsing them as Vox produces noisy false positives (e.g. `from` imports).
                is_vox = lang == "vox";
            }
        } else if in_fence && is_vox {
            if trimmed.contains("vox:skip")
                || trimmed.contains("Skip-Test")
                || trimmed.contains("{{#include")
            {
                has_skip = true;
            }
            if !has_skip {
                current_block.push_str(line);
                current_block.push('\n');
            }
        }
    }

    if !current_block.trim().is_empty() {
        if path.to_string_lossy().contains("expl-rosetta-inventory") {
            let _ = std::fs::write("scratch_extract.vox", &current_block);
        }
        let path_str = path.to_string_lossy();
        let diagnostics = check_file(&current_block, &path_str);
        if !diagnostics.is_empty() {
            let mut err_msg = format!("DocTest error in {}:\n", path.display());
            for diag in diagnostics {
                err_msg.push_str(&format!(
                    "  - [{:?}] {} at line {}\n",
                    diag.severity, diag.message, diag.span.start_line
                ));
            }
            errors.push(LintError {
                file: path.to_owned(),
                line: 1,
                kind: LintKind::DocTestFailed { msg: err_msg },
            });
        }
    }
}
