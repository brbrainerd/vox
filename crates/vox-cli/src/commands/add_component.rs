//! `vox component <name>` — shadcn/ui vendor codegen mode (Phase-5 sub-spec §10).
//!
//! shadcn/ui is **not** an npm package; it is a registry that copies `.tsx`
//! source into your repo. This command fetches a registry item from
//! `ui.shadcn.com` at build time, resolves its `registryDependencies`
//! transitively, and writes each `files[].content` into the project at the path
//! its `components.json` aliases dictate. The vendored component is then used
//! through an ordinary local `import react Button from "./components/ui/button"`.
//!
//! The pure planning logic (`plan_files`) is network-free and unit-tested
//! against a fixture; only `run` touches the network and disk.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// Default shadcn style (registry path segment). The scaffolded `components.json`
/// has no `style` field, so this is the fallback.
const DEFAULT_STYLE: &str = "new-york-v4";
const REGISTRY_BASE: &str = "https://ui.shadcn.com/r/styles";

/// Canonical aliases the shadcn registry content is authored against.
const CANON_UI: &str = "@/components/ui";
const CANON_UTILS: &str = "@/lib/utils";
const CANON_COMPONENTS: &str = "@/components";

/// One registry item (`<name>.json`) returned by the shadcn registry.
#[derive(Debug, Deserialize)]
pub struct RegistryItem {
    /// Item name (e.g. `button`).
    #[serde(default)]
    pub name: String,
    /// npm runtime dependencies the consumer must install (advisory).
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Other registry items this one depends on (resolved transitively).
    #[serde(default, rename = "registryDependencies")]
    pub registry_dependencies: Vec<String>,
    /// Source files, each carrying the literal `.tsx`/`.ts` content.
    #[serde(default)]
    pub files: Vec<RegistryFile>,
}

/// One file inside a registry item.
#[derive(Debug, Deserialize)]
pub struct RegistryFile {
    /// Registry-relative path (e.g. `registry/new-york-v4/ui/button.tsx`).
    pub path: String,
    /// Literal source to write.
    pub content: String,
    /// `registry:ui` | `registry:lib` | `registry:component` | `registry:hook` | …
    #[serde(default, rename = "type")]
    pub file_type: Option<String>,
}

/// Subset of `components.json` that drives placement and content transforms.
#[derive(Debug, Deserialize)]
pub struct ComponentsConfig {
    /// `false` ⇒ emit JavaScript (`.jsx`). Not yet supported (see `plan_files`).
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub tsx: bool,
    /// `true` ⇒ inject the `"use client"` directive into client components.
    #[serde(default)]
    pub rsc: bool,
    /// Import-path aliases.
    #[serde(default)]
    pub aliases: Aliases,
}

impl Default for ComponentsConfig {
    fn default() -> Self {
        Self {
            tsx: true,
            rsc: false,
            aliases: Aliases::default(),
        }
    }
}

/// `components.json` `aliases` block (defaults match the Vox scaffold).
#[derive(Debug, Deserialize)]
pub struct Aliases {
    #[serde(default = "default_ui")]
    pub ui: String,
    #[serde(default = "default_utils")]
    pub utils: String,
    #[serde(default = "default_components")]
    pub components: String,
    #[serde(default = "default_hooks")]
    pub hooks: String,
}

fn default_ui() -> String {
    CANON_UI.to_string()
}
fn default_utils() -> String {
    CANON_UTILS.to_string()
}
fn default_components() -> String {
    CANON_COMPONENTS.to_string()
}
fn default_hooks() -> String {
    "@/hooks".to_string()
}

impl Default for Aliases {
    fn default() -> Self {
        Self {
            ui: default_ui(),
            utils: default_utils(),
            components: default_components(),
            hooks: default_hooks(),
        }
    }
}

/// A file the command will write: project-relative path + final content.
#[derive(Debug, PartialEq, Eq)]
pub struct PlannedFile {
    /// Path relative to the project root (e.g. `app/components/ui/button.tsx`).
    pub rel_path: PathBuf,
    /// Final file content (after any rsc / alias transforms).
    pub content: String,
}

/// Source directory the `@` alias maps to (scaffold tsconfig: `@/* → ./app/*`).
const SRC_DIR: &str = "app";

/// Resolve an alias string (`@/components/ui`) to a project-relative dir
/// (`app/components/ui`).
///
/// Segments are validated: only `Normal` path components are accepted, so a
/// malicious `components.json` alias (`@/../../etc`, absolute paths, `..`) can
/// never produce a write target outside `project_root`.
fn alias_dir(alias: &str) -> Result<PathBuf> {
    let rest = alias.strip_prefix("@/").unwrap_or(alias);
    let mut p = PathBuf::from(SRC_DIR);
    for seg in rest.split('/') {
        if seg.is_empty() {
            continue;
        }
        match Path::new(seg).components().next() {
            Some(std::path::Component::Normal(s)) if Path::new(seg).components().count() == 1 => {
                p.push(s);
            }
            _ => bail!("unsafe alias segment `{seg}` in components.json alias `{alias}`"),
        }
    }
    Ok(p)
}

/// Parent alias of `@/lib/utils` → `@/lib` (where `registry:lib` files land).
fn lib_dir(utils_alias: &str) -> Result<PathBuf> {
    let dir = alias_dir(utils_alias)?;
    Ok(dir.parent().map_or(dir.clone(), Path::to_path_buf))
}

/// Pick the target directory for a file based on its registry type.
fn target_dir(file_type: Option<&str>, cfg: &ComponentsConfig) -> Result<PathBuf> {
    match file_type {
        Some("registry:ui") => alias_dir(&cfg.aliases.ui),
        Some("registry:lib") => lib_dir(&cfg.aliases.utils),
        Some("registry:hook") => alias_dir(&cfg.aliases.hooks),
        // registry:component, registry:page, registry:block, or unknown.
        _ => alias_dir(&cfg.aliases.components),
    }
}

/// Rewrite canonical aliases in `content` to the configured ones (identity when
/// the config uses the defaults). Most-specific first to avoid prefix clobber.
fn rewrite_aliases(content: &str, cfg: &ComponentsConfig) -> String {
    let mut out = content.to_string();
    if cfg.aliases.ui != CANON_UI {
        out = out.replace(CANON_UI, &cfg.aliases.ui);
    }
    if cfg.aliases.utils != CANON_UTILS {
        out = out.replace(CANON_UTILS, &cfg.aliases.utils);
    }
    if cfg.aliases.components != CANON_COMPONENTS {
        out = out.replace(CANON_COMPONENTS, &cfg.aliases.components);
    }
    out
}

/// Whether a file is a client component that should carry `"use client"` under RSC.
fn is_client_component(file_type: Option<&str>) -> bool {
    matches!(
        file_type,
        Some("registry:ui") | Some("registry:component") | Some("registry:page")
    )
}

/// Plan the on-disk writes for one registry item — network-free and deterministic.
///
/// Honors `components.json` aliases (file placement + in-content rewrite), injects
/// `"use client"` for client components when `rsc` is set, and keeps the file's
/// own extension. Returns an error when `tsx: false` (JS output requires type
/// stripping, which is not implemented — an honest limitation, not a silent stub).
pub fn plan_files(item: &RegistryItem, cfg: &ComponentsConfig) -> Result<Vec<PlannedFile>> {
    if !cfg.tsx {
        bail!(
            "components.json has `tsx: false` (JavaScript output). Vox's shadcn vendor mode \
             only supports TypeScript output today — set `tsx: true`."
        );
    }
    let mut planned = Vec::new();
    for f in &item.files {
        let basename = Path::new(&f.path)
            .file_name()
            .and_then(|s| s.to_str())
            .with_context(|| format!("registry file has no basename: {}", f.path))?;
        let rel_path = target_dir(f.file_type.as_deref(), cfg)?.join(basename);

        let mut content = rewrite_aliases(&f.content, cfg);
        if cfg.rsc
            && is_client_component(f.file_type.as_deref())
            && !content.trim_start().starts_with("\"use client\"")
            && !content.trim_start().starts_with("'use client'")
        {
            content = format!("\"use client\"\n\n{content}");
        }
        planned.push(PlannedFile { rel_path, content });
    }
    Ok(planned)
}

/// Load `components.json` from `<root>/app/components.json` (Vox scaffold) or
/// `<root>/components.json`. Falls back to defaults only when neither file
/// exists; a present-but-malformed file is a hard error (rather than silently
/// vendoring into unexpected locations).
fn load_components_config(project_root: &Path) -> Result<ComponentsConfig> {
    for candidate in ["app/components.json", "components.json"] {
        let path = project_root.join(candidate);
        if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let cfg = serde_json::from_str::<ComponentsConfig>(&text)
                .with_context(|| format!("parse {}", path.display()))?;
            return Ok(cfg);
        }
    }
    Ok(ComponentsConfig::default())
}

/// `vox component <name>` — fetch the named shadcn registry item (and its
/// registry dependencies) and vendor the source into the current project.
pub async fn run(name: &str) -> Result<()> {
    let project_root = std::env::current_dir().context("resolve current directory")?;
    let cfg = load_components_config(&project_root)?;
    let client = vox_http_client::client();

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: Vec<String> = vec![name.to_string()];
    let mut npm_deps: BTreeSet<String> = BTreeSet::new();
    let mut written = 0usize;

    while let Some(item_name) = queue.pop() {
        if !visited.insert(item_name.clone()) {
            continue;
        }
        let url = format!("{REGISTRY_BASE}/{DEFAULT_STYLE}/{item_name}.json");
        let resp = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("fetch shadcn registry item `{item_name}` ({url})"))?;
        if !resp.status().is_success() {
            bail!(
                "shadcn registry returned {} for `{item_name}` ({url})",
                resp.status()
            );
        }
        let item: RegistryItem = resp
            .json()
            .await
            .with_context(|| format!("parse registry item `{item_name}`"))?;

        for d in &item.registry_dependencies {
            queue.push(d.clone());
        }
        npm_deps.extend(item.dependencies.iter().cloned());

        for pf in plan_files(&item, &cfg)? {
            let abs = project_root.join(&pf.rel_path);
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create dir {}", parent.display()))?;
            }
            std::fs::write(&abs, &pf.content)
                .with_context(|| format!("write {}", pf.rel_path.display()))?;
            println!("✓ wrote {}", pf.rel_path.display());
            written += 1;
        }
    }

    println!("✓ vendored `{name}` ({written} file(s)).");
    if !npm_deps.is_empty() {
        let list: Vec<String> = npm_deps.into_iter().collect();
        println!(
            "→ ensure these npm deps are installed (app-owned package.json): {} + tailwindcss",
            list.join(", ")
        );
    }
    println!("→ use it from Vox: `import react <Name> from \"./components/ui/{name}\"`");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_button() -> RegistryItem {
        // Minimal shape mirroring https://ui.shadcn.com/r/styles/new-york-v4/button.json
        let json = r#"{
          "name": "button",
          "dependencies": ["radix-ui", "class-variance-authority"],
          "registryDependencies": ["utils"],
          "files": [
            { "path": "registry/new-york-v4/ui/button.tsx",
              "content": "import { Slot } from \"radix-ui\"\nimport { cn } from \"@/lib/utils\"\nexport function Button() { return null }\n",
              "type": "registry:ui" }
          ]
        }"#;
        serde_json::from_str(json).expect("fixture parses")
    }

    #[test]
    fn ui_file_lands_under_components_ui_with_content_preserved() {
        let item = fixture_button();
        let planned = plan_files(&item, &ComponentsConfig::default()).unwrap();
        assert_eq!(planned.len(), 1);
        assert_eq!(
            planned[0].rel_path,
            PathBuf::from("app")
                .join("components")
                .join("ui")
                .join("button.tsx")
        );
        // Default config matches canonical aliases → content is verbatim.
        assert!(
            planned[0]
                .content
                .contains("import { cn } from \"@/lib/utils\"")
        );
        assert!(!planned[0].content.contains("use client"));
    }

    #[test]
    fn traversal_alias_is_rejected_no_escape_outside_project_root() {
        // A malicious components.json alias must not be able to write outside
        // the project root via `..`.
        let item = fixture_button();
        let cfg = ComponentsConfig {
            aliases: Aliases {
                ui: "@/../../etc".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            plan_files(&item, &cfg).is_err(),
            "alias containing `..` must be rejected"
        );
    }

    #[test]
    fn registry_dependencies_are_exposed() {
        let item = fixture_button();
        assert_eq!(item.registry_dependencies, vec!["utils".to_string()]);
        assert!(item.dependencies.iter().any(|d| d == "radix-ui"));
    }

    #[test]
    fn rsc_injects_use_client_into_ui_components() {
        let item = fixture_button();
        let cfg = ComponentsConfig {
            rsc: true,
            ..Default::default()
        };
        let planned = plan_files(&item, &cfg).unwrap();
        assert!(
            planned[0].content.starts_with("\"use client\""),
            "rsc must inject use client, got:\n{}",
            planned[0].content
        );
    }

    #[test]
    fn lib_file_lands_in_lib_dir() {
        let json = r#"{
          "name": "utils",
          "files": [
            { "path": "registry/new-york-v4/lib/utils.ts",
              "content": "export function cn() {}\n",
              "type": "registry:lib" }
          ]
        }"#;
        let item: RegistryItem = serde_json::from_str(json).unwrap();
        let planned = plan_files(&item, &ComponentsConfig::default()).unwrap();
        assert_eq!(
            planned[0].rel_path,
            PathBuf::from("app").join("lib").join("utils.ts")
        );
    }

    #[test]
    fn custom_aliases_are_rewritten_in_content_and_path() {
        let item = fixture_button();
        let cfg = ComponentsConfig {
            aliases: Aliases {
                ui: "@/ui".to_string(),
                utils: "@/util".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let planned = plan_files(&item, &cfg).unwrap();
        assert_eq!(
            planned[0].rel_path,
            PathBuf::from("app").join("ui").join("button.tsx")
        );
        assert!(planned[0].content.contains("@/util"));
        assert!(!planned[0].content.contains("@/lib/utils"));
    }

    #[test]
    fn tsx_false_is_an_honest_error_not_a_stub() {
        let item = fixture_button();
        let cfg = ComponentsConfig {
            tsx: false,
            ..Default::default()
        };
        assert!(plan_files(&item, &cfg).is_err());
    }
}
