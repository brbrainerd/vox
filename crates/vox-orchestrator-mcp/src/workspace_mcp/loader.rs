//! Scan workspace Vox (.vox) files and merge AppContractModule.mcp_tools into a federated surface.

use std::collections::HashSet;
use std::path::Path;

use glob::glob;
use vox_compiler::app_contract::project_app_contract;
use vox_compiler::hir::HirModule;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_mcp_registry::TOOL_REGISTRY;

use super::schema::{hir_type_json_schema_property, param_required_in_schema};
use super::{
    WorkspaceMcpLoadError, WorkspaceMcpLoadResult, WorkspaceMcpResourceEntry, WorkspaceMcpSurface,
    WorkspaceMcpToolEntry,
};

/// Scan configuration loaded from contracts/mcp/workspace-mcp-surface.v1.yaml.
#[derive(Debug, Clone)]
pub struct WorkspaceMcpScanConfig {
    pub scan_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
}

impl Default for WorkspaceMcpScanConfig {
    fn default() -> Self {
        Self {
            scan_globs: vec![
                "examples/golden/**/*.vox".to_string(),
                "src/**/*.vox".to_string(),
            ],
            exclude_globs: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/.vox/**".to_string(),
            ],
        }
    }
}

/// Load scan config from the repo contract file, falling back to defaults.
pub fn load_scan_config(repo: &Path) -> WorkspaceMcpScanConfig {
    let path = repo.join("contracts/mcp/workspace-mcp-surface.v1.yaml");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return WorkspaceMcpScanConfig::default();
    };
    #[derive(serde::Deserialize)]
    struct Contract {
        #[serde(default)]
        scan_globs: Vec<String>,
        #[serde(default)]
        exclude_globs: Vec<String>,
    }
    match serde_yaml::from_str::<Contract>(&raw) {
        Ok(c) if !c.scan_globs.is_empty() => WorkspaceMcpScanConfig {
            scan_globs: c.scan_globs,
            exclude_globs: if c.exclude_globs.is_empty() {
                WorkspaceMcpScanConfig::default().exclude_globs
            } else {
                c.exclude_globs
            },
        },
        _ => WorkspaceMcpScanConfig::default(),
    }
}

pub struct WorkspaceMcpLoader;

impl WorkspaceMcpLoader {
    /// Walk configured globs under repo, compile each .vox file, and merge MCP tools/resources.
    ///
    /// Per-file compile failures are recorded in [`WorkspaceMcpLoadResult::errors`]; the scan
    /// continues for remaining files.
    pub fn load_repo(
        repo: &Path,
        config: &WorkspaceMcpScanConfig,
    ) -> Result<WorkspaceMcpLoadResult, String> {
        let static_names: HashSet<&str> = TOOL_REGISTRY.iter().map(|t| t.name.as_ref()).collect();
        let mut surface = WorkspaceMcpSurface::default();
        let mut errors = Vec::new();
        let mut seen_tools: HashSet<String> = HashSet::new();
        let mut seen_resources: HashSet<String> = HashSet::new();

        let exclude = build_exclude_matcher(&config.exclude_globs);

        for pattern in &config.scan_globs {
            let full_pattern = repo.join(pattern.replace('/', std::path::MAIN_SEPARATOR_STR));
            let pattern_str = full_pattern.to_string_lossy().to_string();
            for entry in glob(&pattern_str).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?;
                if !path.is_file() {
                    continue;
                }
                if exclude.as_ref().is_some_and(|m| m.is_match(&path)) {
                    continue;
                }
                let source = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        errors.push(WorkspaceMcpLoadError {
                            path: path.clone(),
                            message: e.to_string(),
                        });
                        continue;
                    }
                };
                let hir = match compile_source_to_hir(&source) {
                    Ok(h) => h,
                    Err(msg) => {
                        errors.push(WorkspaceMcpLoadError {
                            path: path.clone(),
                            message: msg,
                        });
                        continue;
                    }
                };
                merge_hir_into_surface(
                    repo,
                    &path,
                    &hir,
                    &static_names,
                    &mut surface,
                    &mut seen_tools,
                    &mut seen_resources,
                );
            }
        }

        surface.tools.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(WorkspaceMcpLoadResult { surface, errors })
    }
}

fn build_exclude_matcher(globs: &[String]) -> Option<globset::GlobSet> {
    if globs.is_empty() {
        return None;
    }
    let mut builder = globset::GlobSetBuilder::new();
    for g in globs {
        if let Ok(glob) = globset::Glob::new(g) {
            builder.add(glob);
        }
    }
    builder.build().ok()
}

fn compile_source_to_hir(source: &str) -> Result<HirModule, String> {
    let tokens = lex(source);
    let module = parse(tokens).map_err(|errs| {
        errs.iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    Ok(lower_module(&module))
}

fn merge_hir_into_surface(
    repo: &Path,
    path: &Path,
    hir: &HirModule,
    static_names: &HashSet<&str>,
    surface: &mut WorkspaceMcpSurface,
    seen_tools: &mut HashSet<String>,
    seen_resources: &mut HashSet<String>,
) {
    let contract = project_app_contract(hir);

    for tool in &hir.mcp_tools {
        let name = tool.func.name.clone();
        if static_names.contains(name.as_str()) {
            tracing::warn!(tool = %name, "workspace MCP tool shadowed by static catalog");
            surface.shadowed.push(name);
            continue;
        }
        if !seen_tools.insert(name.clone()) {
            tracing::warn!(tool = %name, path = %path.display(), "duplicate workspace MCP tool");
            surface.duplicate_tools.push(name);
            continue;
        }
        let rel = path
            .strip_prefix(repo)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for param in &tool.func.params {
            properties.insert(
                param.name.clone(),
                hir_type_json_schema_property(param.type_ann.as_ref()),
            );
            if param_required_in_schema(param) {
                required.push(param.name.clone());
            }
        }
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        });
        let signature = contract
            .mcp_tools
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.signature.clone())
            .unwrap_or_default();
        surface.tools.push(WorkspaceMcpToolEntry {
            name,
            description: tool.description.clone(),
            signature,
            source_path: path.to_path_buf(),
            repo_relative: rel,
            input_schema,
        });
    }

    for res in &hir.mcp_resources {
        let uri = res.uri.clone();
        if !seen_resources.insert(uri.clone()) {
            tracing::warn!(uri = %uri, path = %path.display(), "duplicate workspace MCP resource URI");
            surface.duplicate_resources.push(uri);
            continue;
        }
        let rel = path
            .strip_prefix(repo)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        surface.resources.push(WorkspaceMcpResourceEntry {
            uri,
            description: res.description.clone(),
            source_path: path.to_path_buf(),
            repo_relative: rel,
            func_name: res.func.name.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_skips_invalid_file_and_keeps_valid_tools() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.vox"), "this is not vox {{{").unwrap();
        std::fs::write(
            dir.path().join("good.vox"),
            r#"@tool "ping: ping"
fn ping() to str { return "pong" }
"#,
        )
        .unwrap();
        let config = WorkspaceMcpScanConfig {
            scan_globs: vec!["*.vox".to_string()],
            exclude_globs: vec![],
        };
        let result = WorkspaceMcpLoader::load_repo(dir.path(), &config).expect("partial load");
        assert!(result.surface.tool_by_name("ping").is_some());
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].path.ends_with("bad.vox"));
    }

    #[test]
    fn load_fixture_golden_mcp_tools_vox() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let result = WorkspaceMcpLoader::load_repo(repo, &load_scan_config(repo)).expect("load");
        let names: Vec<_> = result
            .surface
            .tools
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert!(names.iter().any(|n| *n == "read_file"));
        assert!(
            result
                .surface
                .resource_by_uri("vox://golden/mcp-status")
                .is_some()
        );
    }
}
