use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A registry mapping logical domains (e.g., 'rust-expert', 'vox-lang') to their
/// compiled adapter weights on disk, enabling multi-LoRA inference multiplexing.
#[derive(Debug, Clone, Default)]
pub struct DomainRouter {
    adapters: HashMap<String, PathBuf>,
}

impl DomainRouter {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Registers a domain with its compiled artifact path.
    pub fn register(&mut self, domain: &str, adapter_path: impl AsRef<Path>) {
        self.adapters
            .insert(domain.to_string(), adapter_path.as_ref().to_path_buf());
    }

    /// Returns the adapter path for the given domain, if registered.
    pub fn route(&self, domain: &str) -> Option<&PathBuf> {
        self.adapters.get(domain)
    }

    /// Attempts to auto-discover adapters in the given artifacts directory.
    /// Expects directories matching domain names (e.g., `artifacts/vox-lang/adapter_model.safetensors`).
    pub fn discover(artifacts_dir: impl AsRef<Path>) -> Result<Self> {
        let mut router = Self::new();
        let artifacts_dir = artifacts_dir.as_ref();

        if !artifacts_dir.exists() {
            return Ok(router);
        }

        for entry in std::fs::read_dir(artifacts_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                let adapter_file = path.join("adapter_model.safetensors");
                if adapter_file.exists() {
                    router.register(name, adapter_file);
                }
            }
        }

        Ok(router)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_router() {
        let mut router = DomainRouter::new();
        router.register("rust-expert", "/fake/path/adapter_model.safetensors");
        assert!(router.route("rust-expert").is_some());
        assert!(router.route("rocks").is_none());
    }
}

#[cfg(test)]
mod semcov_wave26_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    // ── Basic routing ────────────────────────────────────────────────────────

    #[test]
    fn route_returns_none_for_empty_router() {
        // Catches: HashMap::get panicking or returning a non-None sentinel on
        // an empty map (shouldn't happen, but guards the contract explicitly).
        let router = DomainRouter::new();
        assert!(router.route("anything").is_none());
    }

    #[test]
    fn register_overwrites_existing_domain() {
        // Catches: register() inserting a duplicate key and leaving the old path
        // in some secondary store, so route() returns the stale path.
        let mut router = DomainRouter::new();
        router.register("domain", "/old/adapter_model.safetensors");
        router.register("domain", "/new/adapter_model.safetensors");
        let path = router.route("domain").unwrap();
        assert!(
            path.to_string_lossy().contains("new"),
            "second register must overwrite the first; got {path:?}"
        );
    }

    #[test]
    fn route_is_case_sensitive() {
        // Catches: case-insensitive HashMap key lookup that would return a path
        // for "Rust-Expert" when only "rust-expert" was registered.
        let mut router = DomainRouter::new();
        router.register("rust-expert", "/path/adapter_model.safetensors");
        assert!(
            router.route("Rust-Expert").is_none(),
            "domain lookup must be case-sensitive"
        );
        assert!(router.route("rust-expert").is_some());
    }

    #[test]
    fn register_empty_domain_string() {
        // Catches: an assert or panic in register() when the domain key is empty,
        // which could happen if a caller passes an unset field default.
        let mut router = DomainRouter::new();
        router.register("", "/path/adapter_model.safetensors");
        // Must not panic; empty-string domain is a valid (if unusual) key.
        assert!(router.route("").is_some());
        assert!(router.route("nonempty").is_none());
    }

    #[test]
    fn route_path_matches_registered_path() {
        // Catches: register() storing a canonicalized or modified path that
        // doesn't match the raw input, breaking downstream file-open calls.
        let mut router = DomainRouter::new();
        let raw = "/models/vox-lang/adapter_model.safetensors";
        router.register("vox-lang", raw);
        let stored = router.route("vox-lang").unwrap();
        assert_eq!(stored, &PathBuf::from(raw));
    }

    // ── Discovery ────────────────────────────────────────────────────────────

    #[test]
    fn discover_nonexistent_dir_returns_empty_router() {
        // Catches: discover() returning Err (or panicking) for a missing
        // directory instead of returning an empty Ok(router).
        let result = DomainRouter::discover("/nonexistent/__fake_dir__");
        let router = result.expect("nonexistent dir should yield Ok, not Err");
        assert!(
            router.adapters.is_empty(),
            "no adapters should be registered for a missing dir"
        );
    }

    #[test]
    fn discover_ignores_dirs_without_adapter_file() {
        // Catches: discover() registering a domain even when adapter_model.safetensors
        // is absent (e.g., it checks is_dir() but not the sentinel file).
        let tmp = std::env::temp_dir().join("vox_wave26_discover_no_adapter");
        let domain_dir = tmp.join("my-domain");
        fs::create_dir_all(&domain_dir).unwrap();
        // No adapter_model.safetensors inside domain_dir.

        let router = DomainRouter::discover(&tmp).unwrap();
        assert!(
            router.route("my-domain").is_none(),
            "domain dir without adapter file must NOT be registered"
        );

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn discover_registers_domain_with_adapter_file() {
        // Catches: discover() skipping directories that DO have the sentinel file,
        // e.g., due to an inverted is_file() condition.
        let tmp = std::env::temp_dir().join("vox_wave26_discover_with_adapter");
        let domain_dir = tmp.join("code-gen");
        fs::create_dir_all(&domain_dir).unwrap();
        let adapter = domain_dir.join("adapter_model.safetensors");
        fs::write(&adapter, b"fake").unwrap();

        let router = DomainRouter::discover(&tmp).unwrap();
        assert!(
            router.route("code-gen").is_some(),
            "domain dir with adapter_model.safetensors must be registered"
        );

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn discover_registered_path_points_to_adapter_file() {
        // Catches: discover() storing the domain directory path instead of the
        // adapter file path, causing a subsequent file-open to fail.
        let tmp = std::env::temp_dir().join("vox_wave26_discover_path_check");
        let domain_dir = tmp.join("math");
        fs::create_dir_all(&domain_dir).unwrap();
        let adapter = domain_dir.join("adapter_model.safetensors");
        fs::write(&adapter, b"fake").unwrap();

        let router = DomainRouter::discover(&tmp).unwrap();
        let stored = router.route("math").unwrap();
        assert!(
            stored
                .file_name()
                .map(|n| n == "adapter_model.safetensors")
                .unwrap_or(false),
            "stored path must point to adapter_model.safetensors, not the dir; got {stored:?}"
        );

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn discover_multiple_domains() {
        // Catches: discover() short-circuiting after the first successful entry
        // (e.g., a wrong `return Ok(router)` inside the loop body).
        let tmp = std::env::temp_dir().join("vox_wave26_discover_multi");
        for name in &["alpha", "beta", "gamma"] {
            let d = tmp.join(name);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("adapter_model.safetensors"), b"fake").unwrap();
        }

        let router = DomainRouter::discover(&tmp).unwrap();
        for name in &["alpha", "beta", "gamma"] {
            assert!(
                router.route(name).is_some(),
                "domain '{name}' should be discovered"
            );
        }

        fs::remove_dir_all(&tmp).ok();
    }
}
