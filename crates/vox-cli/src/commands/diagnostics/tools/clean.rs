use anyhow::Result;

/// `vox clean` — clean build artifacts and caches.
pub async fn run(all: bool) -> Result<()> {
    let mut cleaned = 0;

    let target_dir = std::path::PathBuf::from("target");
    if tokio::fs::try_exists(&target_dir).await.unwrap_or(false) {
        crate::diagnostics::print_info("Cleaning target/...");
        tokio::fs::remove_dir_all(&target_dir).await?;
        cleaned += 1;
    }

    let dist_dir = std::path::PathBuf::from("dist");
    if tokio::fs::try_exists(&dist_dir).await.unwrap_or(false) {
        crate::diagnostics::print_info("Cleaning dist/...");
        tokio::fs::remove_dir_all(&dist_dir).await?;
        cleaned += 1;
    }

    let cache_dir = std::path::PathBuf::from(vox_config::paths::REPO_DOT_VOX_CACHE_DIR);
    if tokio::fs::try_exists(&cache_dir).await.unwrap_or(false) {
        crate::diagnostics::print_info("Cleaning .vox-cache/...");
        tokio::fs::remove_dir_all(&cache_dir).await?;
        cleaned += 1;
    }

    let _ = crate::fs_utils::gc_script_cache(100, 500);

    if all {
        let modules_dir = std::path::PathBuf::from(".vox_modules");
        if tokio::fs::try_exists(&modules_dir).await.unwrap_or(false) {
            crate::diagnostics::print_info("Cleaning .vox_modules/...");
            tokio::fs::remove_dir_all(&modules_dir).await?;
            cleaned += 1;
        }

        let lock_path = std::path::PathBuf::from("vox.lock");
        if tokio::fs::try_exists(&lock_path).await.unwrap_or(false) {
            crate::diagnostics::print_info("Removing vox.lock...");
            tokio::fs::remove_file(&lock_path).await?;
            cleaned += 1;
        }

        let vox_home = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".vox");

        let global_dirs = [
            "cache",
            "script-cache",
            "script-cache-wasi",
            "script-target",
            "script-cache-target",
            "script-cache-wasi-target",
        ];

        for d in global_dirs {
            let path = vox_home.join(d);
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                crate::diagnostics::print_info(&format!("Cleaning ~/.vox/{d}..."));
                let _ = tokio::fs::remove_dir_all(&path).await;
                cleaned += 1;
            }
        }
    }

    if cleaned > 0 {
        crate::diagnostics::print_success(&format!("Cleaned {cleaned} artifact(s)."));
    } else {
        crate::diagnostics::print_info("Nothing to clean.");
    }

    Ok(())
}
