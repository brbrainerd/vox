use vox_plugin_api::manifest::{PluginManifest, PluginPayload};

#[test]
fn parses_a_code_plugin_manifest() {
    let toml_src = r#"
[plugin]
id = "mens-candle-cuda"
name = "Mens (Candle + CUDA)"
version = "0.1.0"
description = "ML training backend."

[plugin.host]
min-vox-version = "0.5.0"

[plugin.payload]
kind = "code"
abi-version = 1

[plugin.payload.provides]
extension-points = ["MlBackend"]

[plugin.payload.artifacts]
// vox-arch-check: allow dynlib-ext
"linux-x86_64" = "libvox_plugin_mens_candle_cuda.so"
"#;
    let m: PluginManifest = toml::from_str(toml_src).expect("should parse");
    assert_eq!(m.plugin.id, "mens-candle-cuda");
    match m.plugin.payload {
        PluginPayload::Code(c) => {
            assert_eq!(c.abi_version, 1);
            assert_eq!(c.provides.extension_points, vec!["MlBackend".to_string()]);
            assert_eq!(
                c.artifacts.get("linux-x86_64").unwrap(),
                // vox-arch-check: allow dynlib-ext
                "libvox_plugin_mens_candle_cuda.so"
            );
        }
        other => panic!("expected Code variant, got {other:?}"),
    }
}

#[test]
fn parses_a_skill_plugin_manifest() {
    let toml_src = r#"
[plugin]
id = "skill-compiler"
name = "Compiler skill"
version = "0.1.0"
description = "Compiler skill."

[plugin.host]
min-vox-version = "0.5.0"

[plugin.payload]
kind = "skill"
format-version = 1
skill-md = "compiler.skill.md"

[plugin.payload.tools]
exposes = ["vox_validate_file"]
"#;
    let m: PluginManifest = toml::from_str(toml_src).expect("should parse");
    match m.plugin.payload {
        PluginPayload::Skill(s) => {
            assert_eq!(s.format_version, 1);
            assert_eq!(s.skill_md, "compiler.skill.md");
            assert_eq!(s.tools.exposes, vec!["vox_validate_file".to_string()]);
        }
        other => panic!("expected Skill variant, got {other:?}"),
    }
}

#[test]
fn parses_a_composite_plugin_manifest() {
    let toml_src = r#"
[plugin]
id = "populi-mesh"
name = "Populi mesh"
version = "0.1.0"
description = "Mesh transport + skill."

[plugin.host]
min-vox-version = "0.5.0"

[plugin.payload]
kind = "composite"

[plugin.payload.code]
abi-version = 1

[plugin.payload.code.provides]
extension-points = ["MeshDriver"]

[plugin.payload.code.artifacts]
// vox-arch-check: allow dynlib-ext
"linux-x86_64" = "libvox_plugin_populi_mesh.so"

[plugin.payload.skill]
format-version = 1
skill-md = "populi.skill.md"

[plugin.payload.skill.tools]
exposes = ["vox_populi_join"]
"#;
    let m: PluginManifest = toml::from_str(toml_src).expect("should parse");
    match m.plugin.payload {
        PluginPayload::Composite(c) => {
            assert_eq!(c.code.abi_version, 1);
            assert_eq!(c.skill.skill_md, "populi.skill.md");
        }
        other => panic!("expected Composite variant, got {other:?}"),
    }
}
