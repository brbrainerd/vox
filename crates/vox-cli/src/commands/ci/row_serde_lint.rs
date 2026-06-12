use anyhow::{Result, anyhow};
use regex::Regex;
use std::fs;
use std::path::Path;

pub fn run(root: &Path) -> Result<()> {
    let dir = root.join("crates/vox-db-types/src/store_types");
    let mut violations = Vec::new();
    walk(&dir, &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return Ok(());
        }
        let body = fs::read_to_string(path)?;
        check_file(path, &body, &mut violations);
        Ok(())
    })?;
    if !violations.is_empty() {
        return Err(anyhow!(
            "row-serde-lint: {} type(s) missing serde derives:\n{}",
            violations.len(),
            violations.join("\n"),
        ));
    }
    println!("row-serde-lint OK");
    Ok(())
}

fn check_file(path: &Path, body: &str, out: &mut Vec<String>) {
    let struct_re = Regex::new(
        r"(?ms)#\[derive\(([^)]*)\)\]\s*pub\s+struct\s+([A-Z][A-Za-z0-9]*(?:Row|Entry|Result|Summary|Pair|Report|Rollup|Snapshot|Profile|Job))\b"
    )
    .unwrap();
    for cap in struct_re.captures_iter(body) {
        let derives = cap.get(1).unwrap().as_str();
        let name = cap.get(2).unwrap().as_str();
        let has_ser = derives.contains("Serialize");
        let has_de = derives.contains("Deserialize");
        if !(has_ser && has_de) {
            out.push(format!(
                "  {}: struct `{}` missing {}{}{}",
                path.display(),
                name,
                if !has_ser { "Serialize" } else { "" },
                if !has_ser && !has_de { ", " } else { "" },
                if !has_de { "Deserialize" } else { "" },
            ));
        }
    }
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
