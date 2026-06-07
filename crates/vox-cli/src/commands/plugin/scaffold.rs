//! `vox plugin scaffold` — generate a starter plugin directory.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;

/// Plugin payload kind for scaffolding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ScaffoldKind {
    /// Native Rust plugin that implements one or more extension-point traits.
    Code,
    /// Markdown skill spec exposed to agents via MCP tools.
    Skill,
    /// Both a Rust code plugin and an agent skill spec.
    Composite,
}

impl std::fmt::Display for ScaffoldKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "code"),
            Self::Skill => write!(f, "skill"),
            Self::Composite => write!(f, "composite"),
        }
    }
}

/// Scaffold a new plugin directory.
///
/// Creates `<output_dir>/vox-plugin-<id>/` (or `<output_dir>/<id>/` when `id`
/// already starts with `vox-plugin-`) with a valid `Plugin.toml` and starter
/// source files for the requested payload kind.
pub fn run(id: &str, kind: ScaffoldKind, output_dir: &Path) -> Result<()> {
    let dir_name = if id.starts_with("vox-plugin-") {
        id.to_string()
    } else {
        format!("vox-plugin-{id}")
    };
    let plugin_dir = output_dir.join(&dir_name);
    if plugin_dir.exists() {
        bail!("Directory already exists: {}", plugin_dir.display());
    }
    std::fs::create_dir_all(&plugin_dir)
        .with_context(|| format!("Failed to create {}", plugin_dir.display()))?;

    write_plugin_toml(&plugin_dir, id, kind)?;

    match kind {
        ScaffoldKind::Code => write_code_scaffold(&plugin_dir, id)?,
        ScaffoldKind::Skill => write_skill_scaffold(&plugin_dir, id)?,
        ScaffoldKind::Composite => {
            write_code_scaffold(&plugin_dir, id)?;
            write_skill_scaffold(&plugin_dir, id)?;
        }
    }

    println!("Scaffolded plugin at: {}", plugin_dir.display());
    println!("Next steps:");
    println!(
        "  1. Edit {}/Plugin.toml — fill in description, host.min-vox-version",
        dir_name
    );
    match kind {
        ScaffoldKind::Code | ScaffoldKind::Composite => {
            println!("  2. Edit {dir_name}/src/lib.rs — implement your extension-point trait");
            println!("  3. Add {dir_name} to your workspace Cargo.toml");
        }
        ScaffoldKind::Skill => {
            println!("  2. Edit {dir_name}/SKILL.md — describe tools agents can call");
        }
    }
    println!(
        "  Run `vox plugin install --path {}` to test locally.",
        dir_name
    );
    Ok(())
}

fn write_plugin_toml(dir: &Path, id: &str, kind: ScaffoldKind) -> Result<()> {
    // Stamp the host's *current* ABI version so a freshly scaffolded plugin loads as-is
    // (a hard-coded value would silently drift from VOX_PLUGIN_ABI_VERSION and be rejected).
    let abi = VOX_PLUGIN_ABI_VERSION;
    let payload_section = match kind {
        ScaffoldKind::Code => format!(
            "[plugin.payload]\nkind = \"code\"\nabi-version = {abi}\n\
             \n[plugin.payload.provides]\nextension-points = []\n\
             \n[plugin.payload.artifacts]\n# \"linux-x86_64\" = \"libvox_plugin_PLACEHOLDER.so\"\n"
        ),
        ScaffoldKind::Skill => {
            "[plugin.payload]\nkind = \"skill\"\nformat-version = 1\nskill-md = \"SKILL.md\"\n\
             \n[plugin.payload.tools]\nexposes = []\n"
                .to_string()
        }
        ScaffoldKind::Composite => format!(
            "[plugin.payload]\nkind = \"composite\"\n\
             \n[plugin.payload.code]\nabi-version = {abi}\n\
             \n[plugin.payload.code.provides]\nextension-points = []\n\
             \n[plugin.payload.skill]\nformat-version = 1\nskill-md = \"SKILL.md\"\n\
             \n[plugin.payload.skill.tools]\nexposes = []\n"
        ),
    };
    let content = format!(
        "[plugin]\n\
         id = {:?}\n\
         name = {:?}\n\
         version = \"0.1.0\"\n\
         description = \"TODO: describe your plugin.\"\n\
         status = \"alpha\"\n\
         \n\
         [plugin.host]\n\
         min-vox-version = \"0.5.0\"\n\
         \n\
         {payload_section}",
        id, id
    );
    write_file(&dir.join("Plugin.toml"), &content)
}

fn write_code_scaffold(dir: &Path, id: &str) -> Result<()> {
    std::fs::create_dir_all(dir.join("src")).with_context(|| "Failed to create src/")?;

    let crate_name = id.replace('-', "_");
    let struct_name: String = id
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>()
        + "Plugin";

    let lib_rs = format!(
        r##"//! # {crate_name}
//!
//! TODO: describe what this plugin does.

use vox_plugin_sdk::prelude::*;

// The SDK's `declare_plugin!` emits the dylib export glue (root_module / manifest_json /
// init), stamped with the host's current ABI version. Implement only the `VoxPlugin` trait
// plus whichever extension-point accessors your plugin provides.
vox_plugin_sdk::declare_plugin! {{
    id: "{id}",
    version: "0.1.0",
    init: |_host| ROk(wrap({struct_name})),
}}

#[derive(Clone)]
struct {struct_name};

impl VoxPlugin for {struct_name} {{
    fn id(&self) -> RString {{
        RString::from("{id}")
    }}

    fn shutdown(&self) -> RResult<(), RBoxError> {{
        ROk(())
    }}

    // TODO: provide an extension point by overriding its accessor, e.g.:
    //
    //   fn as_hardware_probe(&self) -> ROption<HardwareProbe_TO<'static, RBox<()>>> {{
    //       RSome(HardwareProbe_TO::from_value(self.clone(), TD_Opaque))
    //   }}
    //
    // (import the trait + `_TO` type from `vox_plugin_sdk::extensions::*` and
    //  `TD_Opaque` from `vox_plugin_sdk::abi_stable::erased_types`), and list it under
    // `[plugin.payload.provides] extension-points` in Plugin.toml.
}}
"##,
        crate_name = crate_name,
        id = id,
        struct_name = struct_name
    );

    let cargo_toml = format!(
        r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2021"
description = "TODO: describe your plugin."

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# The SDK re-exports the stable plugin ABI + the abi_stable types you need.
vox-plugin-sdk = {{ workspace = true }}
# `declare_plugin!` uses abi_stable's `#[export_root_module]` / `#[sabi_extern_fn]`
# attribute macros, which emit `abi_stable::` paths — so it must be a direct dependency.
abi_stable = {{ workspace = true }}
"#,
        crate_name = if id.starts_with("vox-plugin-") {
            id.replace('-', "_")
        } else {
            format!("vox_plugin_{}", id.replace('-', "_"))
        }
    );

    write_file(&dir.join("src").join("lib.rs"), &lib_rs)?;
    write_file(&dir.join("Cargo.toml"), &cargo_toml)?;
    Ok(())
}

fn write_skill_scaffold(dir: &Path, id: &str) -> Result<()> {
    let skill_md = format!(
        "# {id}\n\
         \n\
         TODO: describe what this skill does and when an agent should use it.\n\
         \n\
         ## Tools\n\
         \n\
         ### `{id}_example`\n\
         \n\
         TODO: describe the tool, its inputs, and its outputs.\n\
         \n\
         **Parameters:**\n\
         - `input` (string, required): TODO\n\
         \n\
         **Returns:** TODO\n\
         \n\
         **Example:**\n\
         ```json\n\
         {{ \"input\": \"hello\" }}\n\
         ```\n"
    );
    write_file(&dir.join("SKILL.md"), &skill_md)
}

fn write_file(path: &PathBuf, content: &str) -> Result<()> {
    std::fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_code_creates_expected_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        run("my-plugin", ScaffoldKind::Code, tmp.path()).unwrap();
        let dir = tmp.path().join("vox-plugin-my-plugin");
        assert!(dir.join("Plugin.toml").exists());
        assert!(dir.join("src").join("lib.rs").exists());
        assert!(dir.join("Cargo.toml").exists());
        assert!(!dir.join("SKILL.md").exists());

        // Scaffolded code uses the SDK macro (not hand-written export glue)…
        let lib = std::fs::read_to_string(dir.join("src").join("lib.rs")).unwrap();
        assert!(lib.contains("vox_plugin_sdk::declare_plugin!"));
        assert!(!lib.contains("export_root_module"));
        let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("vox-plugin-sdk"));
        // …and the manifest stamps the host's *current* ABI version (so it loads as-is).
        let manifest = std::fs::read_to_string(dir.join("Plugin.toml")).unwrap();
        assert!(manifest.contains(&format!("abi-version = {VOX_PLUGIN_ABI_VERSION}")));
    }

    #[test]
    fn scaffold_skill_creates_expected_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        run("my-skill", ScaffoldKind::Skill, tmp.path()).unwrap();
        let dir = tmp.path().join("vox-plugin-my-skill");
        assert!(dir.join("Plugin.toml").exists());
        assert!(dir.join("SKILL.md").exists());
        assert!(!dir.join("src").exists());
    }

    #[test]
    fn scaffold_composite_creates_both() {
        let tmp = tempfile::TempDir::new().unwrap();
        run("my-composite", ScaffoldKind::Composite, tmp.path()).unwrap();
        let dir = tmp.path().join("vox-plugin-my-composite");
        assert!(dir.join("Plugin.toml").exists());
        assert!(dir.join("SKILL.md").exists());
        assert!(dir.join("src").join("lib.rs").exists());
    }

    #[test]
    fn scaffold_existing_dir_returns_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        run("clash", ScaffoldKind::Code, tmp.path()).unwrap();
        let err = run("clash", ScaffoldKind::Code, tmp.path());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn id_with_vox_plugin_prefix_not_doubled() {
        let tmp = tempfile::TempDir::new().unwrap();
        run("vox-plugin-prefixed", ScaffoldKind::Skill, tmp.path()).unwrap();
        assert!(tmp.path().join("vox-plugin-prefixed").exists());
        assert!(!tmp.path().join("vox-plugin-vox-plugin-prefixed").exists());
    }
}
