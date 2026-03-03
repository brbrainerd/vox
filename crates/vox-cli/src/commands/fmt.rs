use anyhow::{Context, Result};
use std::{fs, path};

pub fn run(file: &path::Path, check: bool) -> Result<()> {
    let source = fs::read_to_string(file)
        .with_context(|| format!("Failed to read source file: {}", file.display()))?;

    let formatted = vox_fmt::format(&source);

    if source != formatted {
        if check {
            println!("Diff for {}:", file.display());
            // ... diff logic remains similar or simplified ...
            anyhow::bail!("{} is unformatted", file.display());
        }

        fs::write(file, &formatted)
            .with_context(|| format!("Failed to write formatted file: {}", file.display()))?;
        println!("Formatted {}", file.display());
    } else {
        println!("{} is already formatted", file.display());
    }

    Ok(())
}
