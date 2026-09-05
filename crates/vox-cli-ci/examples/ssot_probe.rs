//! Dry-run probe: report every version restatement, and optionally show what a
//! bump would rewrite. Never writes to the repo.
use std::path::Path;
use vox_cli_ci::version_ssot as v;

const NPM: &[&str] = &[
    "clients/runtime-types/package.json",
    "clients/runtime-rn/package.json",
    "clients/runtime-web/package.json",
];

fn main() {
    let root = std::fs::read_to_string("Cargo.toml").unwrap();
    let expected = v::workspace_version(&root).unwrap();
    println!("SSOT version: {expected}");

    let mut decls = v::path_dependency_versions(&root, Path::new("Cargo.toml"));

    // Every member crate carries a hakari-generated workspace-hack pin on
    // major.minor. All 113 break at once on a minor bump.
    let members: Vec<std::path::PathBuf> = std::fs::read_dir("crates")
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path().join("Cargo.toml"))
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default();
    let mm = v::major_minor(&expected);
    let mut hack_stale = 0usize;
    for m in &members {
        if let Ok(t) = std::fs::read_to_string(m)
            && let Some(d) = v::workspace_hack_pin(&t, m)
            && d.version != mm
        {
            hack_stale += 1;
            println!(
                "DRIFT {}:{} workspace-hack pin = {} (want {})",
                d.file.display(),
                d.line,
                d.version,
                mm
            );
        }
    }
    if hack_stale == 0 {
        println!("workspace-hack pins: all {} agree ({mm})", members.len());
    }
    for p in NPM {
        if let Ok(t) = std::fs::read_to_string(p) {
            decls.extend(v::npm_versions(&t, Path::new(p)));
        }
    }
    println!("restatements: {}", decls.len());
    let d = v::drift(&expected, &decls);
    if d.is_empty() {
        println!("✅ no drift");
    }
    for x in &d {
        println!(
            "DRIFT {}:{} {} = {} (want {})",
            x.declaration.file.display(),
            x.declaration.line,
            x.declaration.what,
            x.declaration.version,
            x.expected
        );
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.iter().any(|a| a == "--write");
    if let Some(next) = args.iter().find(|a| !a.starts_with("--")) {
        if !v::is_hakari_pinnable(next) {
            eprintln!(
                "refusing to bump to {next}: hakari pins members on major.minor, and cargo's \
                 caret semantics exclude prereleases — every workspace-hack pin would stop \
                 matching and the tree would not resolve. Bump to a release version first."
            );
            std::process::exit(2);
        }
        println!(
            "\n-- bump to {next}{} --",
            if write { "" } else { " (dry run)" }
        );
        let (out, n) = v::rewrite(&root, Path::new("Cargo.toml"), next, false);
        println!("Cargo.toml: {n} line(s)");
        if write {
            std::fs::write("Cargo.toml", out).unwrap();
        }
        for p in NPM {
            if let Ok(t) = std::fs::read_to_string(p) {
                let (out, n) = v::rewrite(&t, Path::new(p), next, true);
                println!("{p}: {n} line(s)");
                if write {
                    std::fs::write(p, out).unwrap();
                }
            }
        }
        // The 113 workspace-hack pins, without which the bumped tree does not
        // resolve at all.
        let next_mm = v::major_minor(next);
        let mut hacked = 0usize;
        for m in &members {
            let Ok(t) = std::fs::read_to_string(m) else {
                continue;
            };
            let Some(d) = v::workspace_hack_pin(&t, m) else {
                continue;
            };
            if d.version == next_mm {
                continue;
            }
            hacked += 1;
            if write {
                let out = t.replace(
                    &format!("workspace-hack = {{ version = \"{}\"", d.version),
                    &format!("workspace-hack = {{ version = \"{next_mm}\""),
                );
                std::fs::write(m, out).unwrap();
            }
        }
        println!("workspace-hack pins: {hacked} crate(s)");
    } else if !d.is_empty() {
        std::process::exit(1);
    }
}
