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
        println!("DRIFT {}:{} {} = {} (want {})",
            x.declaration.file.display(), x.declaration.line,
            x.declaration.what, x.declaration.version, x.expected);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.iter().any(|a| a == "--write");
    if let Some(next) = args.iter().find(|a| !a.starts_with("--")) {
        println!("\n-- bump to {next}{} --", if write { "" } else { " (dry run)" });
        let (out, n) = v::rewrite(&root, Path::new("Cargo.toml"), next, false);
        println!("Cargo.toml: {n} line(s)");
        if write { std::fs::write("Cargo.toml", out).unwrap(); }
        for p in NPM {
            if let Ok(t) = std::fs::read_to_string(p) {
                let (out, n) = v::rewrite(&t, Path::new(p), next, true);
                println!("{p}: {n} line(s)");
                if write { std::fs::write(p, out).unwrap(); }
            }
        }
    } else if !d.is_empty() {
        std::process::exit(1);
    }
}
