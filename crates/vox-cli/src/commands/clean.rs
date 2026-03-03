use anyhow::Result;

/// `vox clean` — clean build artifacts and caches.
pub async fn run(all: bool) -> Result<()> {
    let mut cleaned = 0;

    // Clean the target directory
    let target_dir = std::path::PathBuf::from("target");
    if target_dir.exists() {
        println!("Cleaning target/...");
        std::fs::remove_dir_all(&target_dir)?;
        cleaned += 1;
    }

    // Clean the dist directory
    let dist_dir = std::path::PathBuf::from("dist");
    if dist_dir.exists() {
        println!("Cleaning dist/...");
        std::fs::remove_dir_all(&dist_dir)?;
        cleaned += 1;
    }

    // Clean the project-local artifact cache
    let cache_dir = std::path::PathBuf::from(".vox-cache");
    if cache_dir.exists() {
        println!("Cleaning .vox-cache/...");
        std::fs::remove_dir_all(&cache_dir)?;
        cleaned += 1;
    }

    if all {
        // Also clean .vox_modules and lockfile
        let modules_dir = std::path::PathBuf::from(".vox_modules");
        if modules_dir.exists() {
            println!("Cleaning .vox_modules/...");
            std::fs::remove_dir_all(&modules_dir)?;
            cleaned += 1;
        }

        let lock_path = std::path::PathBuf::from("vox.lock");
        if lock_path.exists() {
            println!("Removing vox.lock...");
            std::fs::remove_file(&lock_path)?;
            cleaned += 1;
        }

        // Clean global cache
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let global_cache = std::path::PathBuf::from(home).join(".vox").join("cache");
        if global_cache.exists() {
            println!("Cleaning global cache (~/.vox/cache/)...");
            std::fs::remove_dir_all(&global_cache)?;
            cleaned += 1;
        }
    }

    if cleaned > 0 {
        println!("\n✓ Cleaned {cleaned} artifact(s).");
    } else {
        println!("Nothing to clean.");
    }

    Ok(())
}
