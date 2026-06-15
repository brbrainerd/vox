use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

    let aff = crate::affected::compute_affected(&changed, &graph.crates);
    let (full, list) = match aff {
        crate::affected::Affected::Full => (true, String::new()),
        crate::affected::Affected::None => (false, String::new()),
        crate::affected::Affected::Crates(set) => {
            let names: Vec<String> = set
                .into_iter()
                .filter(|c| {
                    c.chars()
                        .all(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_')
                })
                .collect();
            (false, names.join(" "))
        }
    };

    let p_args = if list.is_empty() {
        String::new()
    } else {
        list.split_whitespace()
            .map(|c| format!("-p {c}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let out_line = format!("full={full}\naffected_crates={list}\naffected_p_args={p_args}\n");

    if let Some(go) = get("--github-output") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&go)
            .unwrap();
        write!(f, "{out_line}").unwrap();
    } else {
        print!("{out_line}");
    }
    0
}
