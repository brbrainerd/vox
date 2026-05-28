use anyhow::Result;

/// README written by `vox_tauri_codegen::emit_tauri_packaging_hints` next to `tauri.conf.json`
/// (project root — not under `target/`).
pub const TAURI_PACKAGING_HINT_README_REL: &str = "tauri-packaging/README.md";

/// `vox init` — scaffold a new Vox project with a `Vox.toml` manifest.
pub async fn run(name: Option<&str>, kind: Option<&str>, template: Option<&str>) -> Result<()> {
    let project_name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .as_deref()
            .unwrap_or("my-project")
            .to_string()
            .leak()
    });
    let package_kind = kind.unwrap_or("application");

    let cwd = std::env::current_dir()?;
    let scaffold_path = cwd.join(project_name);
    let summary = vox_project_scaffold::scaffold_vox_project_at(
        &scaffold_path,
        project_name,
        package_kind,
        template,
    )?;

    if matches!(template, Some("mobile-pwa") | Some("tauri-mobile")) {
        let slug = project_name.to_lowercase().replace('-', "");
        let params = vox_tauri_codegen::TauriEmitParams {
            identifier: &format!("com.vox.{slug}"),
            display_name: project_name,
            frontend_dist_relative: "dist",
        };
        let contracts_repo = vox_tauri_codegen::find_contracts_repo_root(&scaffold_path);
        vox_tauri_codegen::emit_tauri_packaging_hints(
            &scaffold_path,
            &params,
            contracts_repo.as_deref(),
            None,
        )?;
    }

    if summary.package_kind == "skill" {
        let skill_file_name = vox_repository::skill_markdown_filename(project_name);
        println!("✓ Initialized Vox skill `{}`", project_name);
        println!("  Created {}", skill_file_name);
        println!();
        println!("  Next steps:");
        println!("    1. Edit {}", skill_file_name);
        println!("    2. Install with: vox skill install {}", skill_file_name);
        return Ok(());
    }

    let template_note = if let Some(ref t) = template {
        format!(" (template: {t})")
    } else {
        String::new()
    };

    println!(
        "✓ Initialized Vox {} `{}`{}",
        summary.package_kind, summary.project_name, template_note
    );
    println!("  Created Vox.toml");
    println!("  Created src/main.vox");
    println!("  Created .vox_modules/");
    println!();
    match (package_kind, template) {
        (_, Some("chatbot")) | ("chatbot", _) => {
            println!("  Next steps:");
            println!("    1. Add OPENROUTER_API_KEY to .env");
            println!("    2. vox build src/main.vox -o dist");
            println!("    3. POST to http://localhost:3001/api/chat");
        }
        (_, Some("dashboard")) | (_, Some("web")) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. Open http://localhost:3000");
        }
        (_, Some("api")) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. curl http://localhost:3001/health");
        }
        ("application", _) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. vox run src/main.vox");
            println!("    3. Open http://localhost:3000");
        }
        ("workflow", _) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. Edit activities and workflow steps");
        }
        (_, Some("mobile-pwa")) | (_, Some("tauri-mobile")) => {
            println!("  Next steps:");
            println!("    1. pnpm install");
            println!("    2. vox build src/main.vox -o dist");
            println!(
                "    3. Follow `{}` (Tauri 2 packaging hints from `vox init`)",
                TAURI_PACKAGING_HINT_README_REL
            );
            println!(
                "    4. `vox compile --target mobile-android` or `mobile-ios` when tooling is wired"
            );
        }
        _ => {
            println!("  Get started with: vox build src/main.vox");
        }
    }

    Ok(())
}
