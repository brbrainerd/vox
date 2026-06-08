//! `vox-langtool fmt` — format a .vox file in place.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

fn atomic_write_file(path: &Path, contents: &str) -> Result<()> {
    use std::io::Write;

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let base = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot derive temp basename from path: {}", path.display()))?;
    let tmp: PathBuf = parent.join(format!("{base}.voxfmt.{stamp}.tmp"));
    std::fs::File::create(&tmp)
        .with_context(|| format!("create temp {}", tmp.display()))?
        .write_all(contents.as_bytes())
        .with_context(|| format!("write temp {}", tmp.display()))?;
    #[cfg(unix)]
    {
        std::fs::rename(&tmp, path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    }
    #[cfg(not(unix))]
    {
        if path.exists() {
            std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
        }
        std::fs::rename(&tmp, path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    }
    Ok(())
}

pub fn run(file: &Path, check: bool) -> Result<()> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read source file: {}", file.display()))?;

    let formatted = vox_compiler::fmt::try_format(&source).map_err(|e| {
        let lines: Vec<String> = e.iter().map(|pe| pe.to_string()).collect();
        anyhow::anyhow!(
            "{}: cannot format (parse or print round-trip failed):\n{}",
            file.display(),
            lines.join("\n")
        )
    })?;

    if check {
        if source != formatted {
            bail!(
                "{}: needs format (run `vox-langtool fmt` without `--check` to write)",
                file.display()
            );
        }
        return Ok(());
    }

    if source == formatted {
        return Ok(());
    }

    atomic_write_file(file, &formatted)?;
    println!("Formatted {}", file.display());
    Ok(())
}
