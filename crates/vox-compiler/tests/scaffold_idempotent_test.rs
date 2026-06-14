//! `write_scaffold_if_missing` skips existing paths (WS09 idempotency).

use std::fs;

use vox_codegen::codegen_ts::scaffold::write_scaffold_if_missing;

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn scaffold_write_skips_existing_user_files() {
    let root = std::env::temp_dir().join(format!(
        "vox_scaffold_idempotent_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("mkdir root");
    // Seed a config file we actually emit (`vite.config.ts`) — the scaffold is
    // config-only now; the app bootstrap (`entry.tsx`/`vox-app.tsx`) is emitted
    // on every build, not scaffolded.
    let marker = "// user-edited vite config\n";
    fs::write(root.join("vite.config.ts"), marker).expect("seed vite config");

    write_scaffold_if_missing(&root, "vox-app", &[]).expect("first write");
    let after = fs::read_to_string(root.join("vite.config.ts")).expect("read vite config");
    assert_eq!(
        after, marker,
        "existing vite.config.ts must not be overwritten"
    );

    write_scaffold_if_missing(&root, "vox-app", &[]).expect("second write");
    let after2 = fs::read_to_string(root.join("vite.config.ts")).expect("read vite config");
    assert_eq!(after2, marker);
    let _ = fs::remove_dir_all(&root);
}
