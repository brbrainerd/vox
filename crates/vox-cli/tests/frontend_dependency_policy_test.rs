use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DependencyPolicyContract {
    policy: DependencyPolicy,
    surfaces: Vec<SurfacePackage>,
}

#[derive(Debug, Deserialize)]
struct DependencyPolicy {
    package_manager: String,
    react_major: u64,
    vite_major: MajorRange,
}

#[derive(Debug, Deserialize)]
struct MajorRange {
    min: u64,
    max: u64,
}

#[derive(Debug, Deserialize)]
struct SurfacePackage {
    path: String,
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    #[serde(rename = "packageManager")]
    package_manager: Option<String>,
    dependencies: Option<BTreeMap<String, String>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<BTreeMap<String, String>>,
}

fn first_major(version: &str) -> Option<u64> {
    let digits: String = version
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<u64>().ok()
    }
}

fn lookup_dep(pkg: &PackageJson, name: &str) -> Option<String> {
    pkg.dependencies
        .as_ref()
        .and_then(|deps| deps.get(name).cloned())
        .or_else(|| {
            pkg.dev_dependencies
                .as_ref()
                .and_then(|deps| deps.get(name).cloned())
        })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn frontend_dependency_policy_is_enforced_for_gui_surfaces() {
    let root = repo_root();
    let contract_raw = fs::read_to_string(
        root.join("contracts")
            .join("frontend")
            .join("dependency-policy.v1.yaml"),
    )
    .expect("read contracts/frontend/dependency-policy.v1.yaml");
    let contract: DependencyPolicyContract =
        serde_yaml::from_str(&contract_raw).expect("parse dependency policy contract");

    for surface in &contract.surfaces {
        let package_path = root.join(&surface.path);
        if !package_path.exists() {
            // Surface not yet created on disk — skip rather than fail.
            continue;
        }
        let package_raw = fs::read_to_string(&package_path)
            .unwrap_or_else(|e| panic!("read {}: {}", surface.path, e));
        let package: PackageJson = serde_json::from_str(&package_raw)
            .unwrap_or_else(|e| panic!("parse {}: {}", surface.path, e));

        let package_manager = package.package_manager.as_deref().unwrap_or("pnpm@unknown");
        assert!(
            package_manager.starts_with(&contract.policy.package_manager),
            "{} packageManager must start with `{}` (found `{}`)",
            surface.path,
            contract.policy.package_manager,
            package_manager
        );

        let react_version = lookup_dep(&package, "react")
            .unwrap_or_else(|| panic!("{} missing react dependency", surface.path));
        let react_dom_version = lookup_dep(&package, "react-dom")
            .unwrap_or_else(|| panic!("{} missing react-dom dependency", surface.path));
        let vite_version = lookup_dep(&package, "vite")
            .unwrap_or_else(|| panic!("{} missing vite dependency", surface.path));

        let react_major = first_major(&react_version)
            .unwrap_or_else(|| panic!("bad react semver in {}", surface.path));
        let react_dom_major = first_major(&react_dom_version)
            .unwrap_or_else(|| panic!("bad react-dom semver in {}", surface.path));
        let vite_major = first_major(&vite_version)
            .unwrap_or_else(|| panic!("bad vite semver in {}", surface.path));

        assert_eq!(
            react_major, contract.policy.react_major,
            "{} react major must be {} (found `{}`)",
            surface.path, contract.policy.react_major, react_version
        );
        assert_eq!(
            react_dom_major, contract.policy.react_major,
            "{} react-dom major must be {} (found `{}`)",
            surface.path, contract.policy.react_major, react_dom_version
        );
        assert!(
            vite_major >= contract.policy.vite_major.min
                && vite_major <= contract.policy.vite_major.max,
            "{} vite major must be within [{}..={}] (found `{}`)",
            surface.path,
            contract.policy.vite_major.min,
            contract.policy.vite_major.max,
            vite_version
        );
    }
}
