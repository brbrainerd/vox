use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::Path;

const MAPPED_IDS: &[(&str, &str)] = &[
    ("agent_id", "DbAgentId"),
    ("session_id", "DbSessionId"),
    ("task_id", "DbTaskId"),
    ("correlation_id", "DbCorrelationId"),
    ("user_id", "DbUserId"),
    ("plan_session_id", "DbPlanSessionId"),
];

pub fn run(root: &Path, report_only: bool) -> Result<()> {
    let dir = root.join("crates/vox-db-types/src/store_types");
    let mut violations = Vec::new();
    walk(&dir, &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return Ok(());
        }
        let body = fs::read_to_string(path)?;
        for (field, ty) in MAPPED_IDS {
            let re = Regex::new(&format!(r"\bpub\s+{field}\s*:\s*(?:Option<)?String")).unwrap();
            for m in re.find_iter(&body) {
                let line_no = body[..m.start()].lines().count() + 1;
                violations.push(format!(
                    "  {}:{}  field `{}: String` should use `{}`",
                    path.display(),
                    line_no,
                    field,
                    ty,
                ));
            }
        }
        Ok(())
    })?;
    if !violations.is_empty() {
        let msg = format!(
            "string-id-lint: {} stringly-typed ID field(s) where a newtype exists:\n{}",
            violations.len(),
            violations.join("\n"),
        );
        if report_only {
            eprintln!("WARN: {msg}");
            println!("string-id-lint REPORT-ONLY ({} findings)", violations.len());
            return Ok(());
        } else {
            return Err(anyhow::anyhow!(msg));
        }
    }
    println!("string-id-lint OK");
    Ok(())
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            f(entry.path())?;
        }
    }
    Ok(())
}
