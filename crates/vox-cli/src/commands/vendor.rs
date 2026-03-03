use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::{fs, io};

/// Recursively copy a directory from src to dst
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        if entry_type.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub async fn run() -> Result<()> {
    let modules_dir = PathBuf::from(".vox_modules");
    if !modules_dir.exists() {
        println!("No `.vox_modules` directory found. Try running `vox install` first.");
        return Ok(());
    }

    let vendor_dir = PathBuf::from("vendor");
    if vendor_dir.exists() {
        println!("Cleaning existing vendor directory...");
        fs::remove_dir_all(&vendor_dir).context("Failed to clean existing vendor directory")?;
    }

    println!("Vendoring dependencies into `vendor/`...");
    copy_dir_all(&modules_dir, &vendor_dir).context("Failed to copy `.vox_modules` to `vendor`")?;

    println!("✓ Successfully vendored dependencies. You can now build offline.");
    Ok(())
}
