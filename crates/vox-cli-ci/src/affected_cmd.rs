use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Default)]
pub struct CrateGraph {
    pub schema_version: u32,
    pub crates: BTreeMap<String, Vec<String>>,
}

pub fn graph_from_metadata(meta: &serde_json::Value) -> CrateGraph {
    let empty_vec = vec![];
    let members: std::collections::BTreeSet<String> = meta["workspace_members"]
        .as_array()
        .unwrap_or(&empty_vec)
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let mut id_name = BTreeMap::new();
    for p in meta["packages"].as_array().unwrap_or(&empty_vec) {
        if let (Some(id), Some(name)) = (p["id"].as_str(), p["name"].as_str()) {
            id_name.insert(id.to_string(), name.to_string());
        }
    }
    let mut crates: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in meta["resolve"]["nodes"].as_array().unwrap_or(&empty_vec) {
        let id = node["id"].as_str().unwrap_or("");
        if !members.contains(id) {
            continue;
        }
        let name = match id_name.get(id) {
            Some(n) => n.clone(),
            None => continue,
        };
        let mut deps: Vec<String> = node["deps"]
            .as_array()
            .unwrap_or(&empty_vec)
            .iter()
            .filter_map(|d| d["pkg"].as_str())
            .filter(|pid| members.contains(*pid))
            .filter_map(|pid| id_name.get(pid).cloned())
            .collect();
        deps.sort();
        deps.dedup();
        crates.insert(name, deps);
    }
    CrateGraph {
        schema_version: 1,
        crates,
    }
}

pub fn regen_graph(out_path: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let graph = graph_from_metadata(&meta);
    let json = serde_json::to_string_pretty(&graph).map_err(|e| e.to_string())? + "\n";
    std::fs::create_dir_all(out_path.parent().unwrap_or(std::path::Path::new("."))).ok();
    std::fs::write(out_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn check_graph(committed: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .map_err(|e| e.to_string())?;
    let meta: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let fresh = serde_json::to_string_pretty(&graph_from_metadata(&meta))
        .map_err(|e| e.to_string())?
        + "\n";
    let on_disk = std::fs::read_to_string(committed).unwrap_or_default();
    if fresh != on_disk {
        return Err(format!(
            "crate-graph drift: run `vox ci affected-crates --regen --out {}`",
            committed.display()
        ));
    }
    Ok(())
}

fn valid_crate_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_')
}

fn write_github_output(path: &str, lines: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    write!(f, "{lines}")
}

pub fn run_affected_cmd(args: &[String]) -> i32 {
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };

    if args.iter().any(|a| a == "--regen") {
        let out = get("--out").unwrap_or_else(|| "contracts/ci/crate-graph.v1.json".into());
        return match regen_graph(std::path::Path::new(&out)) {
            Ok(_) => {
                eprintln!("wrote {out}");
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
    }

    if args.iter().any(|a| a == "--check") {
        return match check_graph(std::path::Path::new("contracts/ci/crate-graph.v1.json")) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("::error::{e}");
                1
            }
        };
    }

    if args.iter().any(|a| a == "--shadow-junit") {
        let junit = match get("--junit") {
            Some(p) => p,
            None => {
                eprintln!("--junit required with --shadow-junit");
                return 1;
            }
        };
        let affected_list = get("--affected-crates").unwrap_or_default();
        let affected: BTreeSet<String> = affected_list
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let xml = std::fs::read_to_string(&junit).unwrap_or_default();
        if affected.is_empty() && (xml.contains("<failure") || xml.contains("<error")) {
            eprintln!(
                "::warning title=affected-ci shadow::junit has failures but affected set is empty — \
                 shadow comparison skipped (check merge_group setup / affected_crates output)"
            );
        }
        let misses = crate::affected::shadow_misses(&xml, &affected);
        for c in &misses {
            eprintln!(
                "::warning title=affected-ci shadow-miss::{c} failed but was not in the PR affected set"
            );
        }
        return if misses.is_empty() { 0 } else { 1 };
    }

    let changed_path = match get("--changed") {
        Some(p) => p,
        None => {
            eprintln!("--changed required");
            return 1;
        }
    };
    let graph_path = get("--graph").unwrap_or_else(|| "contracts/ci/crate-graph.v1.json".into());

    let changed: Vec<String> = std::fs::read_to_string(&changed_path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let graph: CrateGraph =
        serde_json::from_str(&std::fs::read_to_string(&graph_path).unwrap_or_default())
            .unwrap_or_default();

    let path_flags = crate::affected::compute_path_flags(&changed);
    let aff = crate::affected::compute_affected(&changed, &graph.crates);

    let seeds: BTreeSet<String> = changed
        .iter()
        .filter_map(|f| crate::affected::file_to_crate(f))
        .map(String::from)
        .collect();
    let closure = if seeds.is_empty() {
        BTreeSet::new()
    } else {
        crate::affected::reverse_closure(&graph.crates, &seeds)
    };

    for c in &closure {
        if !valid_crate_name(c) {
            eprintln!("::error::invalid crate name in affected set: {c}");
            return 1;
        }
    }

    let full = matches!(aff, crate::affected::Affected::Full);
    let list: String = closure.iter().cloned().collect::<Vec<_>>().join(" ");
    let affects_compiler =
        full || crate::affected::set_includes_compiler(&closure) || path_flags.affects_golden;

    let p_args = if list.is_empty() {
        String::new()
    } else {
        list.split_whitespace()
            .map(|c| format!("-p {c}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let out_line = format!(
        "full={full}\n\
         affected_crates={list}\n\
         affected_p_args={p_args}\n\
         affects_compiler={affects_compiler}\n\
         affects_golden={}\n\
         affects_contracts={}\n\
         affects_scripts={}\n\
         affects_gui={}\n\
         affects_web={}\n\
         affects_plugins={}\n",
        path_flags.affects_golden,
        path_flags.affects_contracts,
        path_flags.affects_scripts,
        path_flags.affects_gui,
        path_flags.affects_web,
        path_flags.affects_plugins,
    );

    if let Some(go) = get("--github-output") {
        if let Err(e) = write_github_output(&go, &out_line) {
            eprintln!("::error::failed to write github output: {e}");
            return 1;
        }
    } else {
        print!("{out_line}");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::run_affected_cmd;

    #[test]
    fn shadow_junit_exits_nonzero_when_failure_outside_affected_set() {
        let dir = tempfile::tempdir().expect("tempdir");
        let junit = dir.path().join("junit.xml");
        std::fs::write(
            &junit,
            r#"<testcase classname="vox-compiler::tests::foo" name="bar"><failure message="boom"/></testcase>"#,
        )
        .expect("write junit");
        let code = run_affected_cmd(&[
            "affected-crates".into(),
            "--shadow-junit".into(),
            "--junit".into(),
            junit.to_string_lossy().into_owned(),
            "--affected-crates".into(),
            "vox-db".into(),
        ]);
        assert_eq!(code, 1, "shadow miss must exit non-zero");
    }

    #[test]
    fn shadow_junit_exits_zero_when_failure_inside_affected_set() {
        let dir = tempfile::tempdir().expect("tempdir");
        let junit = dir.path().join("junit.xml");
        std::fs::write(
            &junit,
            r#"<testcase classname="vox-compiler::tests::foo" name="bar"><failure message="boom"/></testcase>"#,
        )
        .expect("write junit");
        let code = run_affected_cmd(&[
            "affected-crates".into(),
            "--shadow-junit".into(),
            "--junit".into(),
            junit.to_string_lossy().into_owned(),
            "--affected-crates".into(),
            "vox-compiler vox-db".into(),
        ]);
        assert_eq!(code, 0);
    }

    #[test]
    fn shadow_junit_exits_zero_when_affected_set_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let junit = dir.path().join("junit.xml");
        std::fs::write(
            &junit,
            r#"<testcase classname="vox-compiler::t" name="x"><failure/></testcase>"#,
        )
        .expect("write junit");
        let code = run_affected_cmd(&[
            "affected-crates".into(),
            "--shadow-junit".into(),
            "--junit".into(),
            junit.to_string_lossy().into_owned(),
            "--affected-crates".into(),
            String::new(),
        ]);
        assert_eq!(code, 0);
    }
}
