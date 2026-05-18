//! One-shot regenerator for `contracts/eval/humaneval-vox/held-out.v1.json`.
//! Run with `cargo test -p vox-audit --test regen_held_out -- --ignored`
//! when the seed corpus changes.

#[test]
#[ignore]
fn regenerate_held_out_v1_json() {
    let workspace = vox_audit::workspace_root();
    let problems_dir = workspace.join("contracts/eval/humaneval-vox/problems");
    let out_path = workspace.join("contracts/eval/humaneval-vox/held-out.v1.json");
    let manifest = vox_audit::subcommands::humaneval::build_held_out_manifest(&problems_dir)
        .expect("build_held_out_manifest");
    let text = serde_json::to_string_pretty(&manifest).expect("to_string_pretty");
    std::fs::write(&out_path, text + "\n").expect("write");
    println!("wrote {}", out_path.display());
}
