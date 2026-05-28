//! Generate `TOOL_WIRE_ALIASES` from `contracts/mcp/tool-wire-aliases.v1.yaml`.

use std::fs;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct Root {
    aliases: Vec<AliasRow>,
}

#[derive(serde::Deserialize)]
struct AliasRow {
    alias: String,
    canonical: String,
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("MANIFEST_DIR"));
    let yaml_path = manifest_dir.join("../../contracts/mcp/tool-wire-aliases.v1.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let raw = fs::read_to_string(&yaml_path).unwrap_or_else(|e| {
        panic!("read {}: {e}", yaml_path.display());
    });
    let root: Root = serde_yaml::from_str(&raw).unwrap_or_else(|e| {
        panic!("parse {}: {e}", yaml_path.display());
    });

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let mut buf = String::new();
    buf.push_str("// @generated from contracts/mcp/tool-wire-aliases.v1.yaml\n");
    buf.push_str("pub const TOOL_WIRE_ALIASES: &[(&str, &str)] = &[\n");
    for row in root.aliases {
        buf.push_str(&format!("    ({:?}, {:?}),\n", row.alias, row.canonical));
    }
    buf.push_str("];\n");
    let dest = out_dir.join("tool_aliases_wire.rs");
    fs::write(&dest, buf).unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
}
