use super::super::common::Check;
use vox_cli_ci::sync_ignore_files;

pub async fn run(auto_heal: bool, checks: &mut Vec<Check>) {
    let cwd = std::env::current_dir().unwrap_or_default();

    // 1. Check .voxignore SSOT
    let voxignore_path = cwd.join(".voxignore");
    let has_voxignore = voxignore_path.exists();

    if !has_voxignore {
        checks.push(Check::fail(
            ".voxignore (SSOT)",
            "not found in workspace root — required for AI context exclusion policy".to_string(),
        ));
        return;
    }

    // 2. Check derived files sync
    // We run the verify pass from sync_ignore_files
    match sync_ignore_files::run(&cwd, true) {
        Ok(_) => {
            checks.push(Check::pass(
                "AI Ignore Sync (.voxignore derived)",
                "all derived files (.cursorignore, .aiignore, .aiexclude) are in sync".to_string(),
            ));
        }
        Err(e) => {
            let mut detail = e.to_string();
            let mut pass = false;

            if auto_heal {
                println!("  [auto-heal] Synchronizing ignore files from .voxignore...");
                if sync_ignore_files::run(&cwd, false).is_ok() {
                    pass = true;
                    detail = "synchronized via auto-heal".to_string();
                } else {
                    detail = format!("auto-heal failed: {}", detail);
                }
            }

            checks.push(Check {
                name: "AI Ignore Sync (.voxignore derived)".to_string(),
                pass,
                detail,
            });
        }
    }
}
