use anyhow::Result;

/// Advise the caller on sccache setup. Returns an empty vec when healthy.
pub fn advise(
    sccache_on_path: bool,
    rustc_wrapper: Option<&str>,
    cargo_incremental: Option<&str>,
) -> Vec<String> {
    if !sccache_on_path {
        return vec![
            "sccache is not installed — install it with: cargo install sccache".to_string(),
        ];
    }
    // `rustc-wrapper` is commonly set to an absolute path (a Homebrew or cargo bin
    // directory, via `~/.cargo/config.toml`), and on Windows it carries a `.exe`
    // suffix. Compare the file stem so a correctly-configured wrapper is not
    // reported as missing.
    let wrapper_name = rustc_wrapper.map(|v| {
        std::path::Path::new(v)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(v)
            .to_lowercase()
    });
    if wrapper_name.as_deref() != Some("sccache") {
        return vec![
            "sccache is installed but RUSTC_WRAPPER is not set to 'sccache'".to_string(),
            "Add to ~/.cargo/config.toml (NOT the tracked .cargo/config.toml):".to_string(),
            "  [build]\n  rustc-wrapper = \"sccache\"".to_string(),
        ];
    }
    if cargo_incremental.unwrap_or("1") != "0" {
        return vec![
            "CARGO_INCREMENTAL must be 0 when using sccache (incremental builds bypass the cache)"
                .to_string(),
            "Add to ~/.cargo/config.toml:".to_string(),
            "  [build]\n  incremental = false".to_string(),
        ];
    }
    vec![]
}

pub fn run() -> Result<()> {
    let sccache_on_path = which::which("sccache").is_ok();
    let rustc_wrapper = std::env::var("RUSTC_WRAPPER").ok();
    let cargo_incremental = std::env::var("CARGO_INCREMENTAL").ok();

    let advices = advise(
        sccache_on_path,
        rustc_wrapper.as_deref(),
        cargo_incremental.as_deref(),
    );

    if advices.is_empty() {
        println!("build-cache-doctor: sccache is configured correctly ✓");
    } else {
        println!("build-cache-doctor: issues found:");
        for a in &advices {
            println!("  • {a}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_config_returns_empty() {
        let advice = advise(true, Some("sccache"), Some("0"));
        assert!(advice.is_empty(), "expected no advice, got: {advice:?}");
    }

    #[test]
    fn missing_sccache_reports_install_hint() {
        let advice = advise(false, None, None);
        assert!(!advice.is_empty());
        assert!(advice[0].contains("not installed"), "got: {}", advice[0]);
    }

    #[test]
    fn wrapper_not_set_reports_config_hint() {
        let advice = advise(true, None, None);
        assert!(!advice.is_empty());
        assert!(
            advice.iter().any(|a| a.contains("RUSTC_WRAPPER")),
            "got: {advice:?}"
        );
    }

    #[test]
    fn incremental_not_zero_reports_hint() {
        let advice = advise(true, Some("sccache"), Some("1"));
        assert!(!advice.is_empty());
        assert!(
            advice.iter().any(|a| a.contains("CARGO_INCREMENTAL")),
            "got: {advice:?}"
        );
    }
}
