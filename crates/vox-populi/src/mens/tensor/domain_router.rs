use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::adapter_card::AdapterCard;

/// A registry mapping logical domains (e.g., 'rust-expert', 'vox-lang') to their
/// compiled adapter weights on disk and the associated provenance card, enabling
/// multi-LoRA inference multiplexing.
#[derive(Debug, Clone, Default)]
pub struct DomainRouter {
    adapters: HashMap<String, (PathBuf, AdapterCard)>,
}

impl DomainRouter {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Registers a domain with its compiled artifact path and provenance card.
    /// Fail-closed: returns Err if the card fails validation (missing required fields).
    pub fn register(
        &mut self,
        domain: &str,
        adapter_path: impl AsRef<Path>,
        card: AdapterCard,
    ) -> Result<()> {
        card.validate()?; // fail-closed: error on missing provenance
        self.adapters.insert(
            domain.to_string(),
            (adapter_path.as_ref().to_path_buf(), card),
        );
        Ok(())
    }

    /// Returns the adapter path and card for the given domain, if registered.
    pub fn route(&self, domain: &str) -> Option<&(PathBuf, AdapterCard)> {
        self.adapters.get(domain)
    }

    /// Attempts to auto-discover adapters in the given artifacts directory.
    /// Expects directories matching domain names (e.g., `artifacts/vox-lang/adapter_model.safetensors`).
    /// Requires an `adapter_card.json` sidecar next to the adapter file — adapters without one are skipped.
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
                    match AdapterCard::read_sidecar(&adapter_file) {
                        Ok(Some(card)) => {
                            if let Err(e) = router.register(name, &adapter_file, card) {
                                // Missing or invalid provenance — skip, don't panic
                                eprintln!("skip {name}: {e}");
                            }
                        }
                        Ok(None) => {
                            // No sidecar — legacy adapter, skip
                            eprintln!("skip {name}: no adapter_card.json sidecar");
                        }
                        Err(e) => {
                            eprintln!("skip {name}: card parse error: {e}");
                        }
                    }
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
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        router
            .register("rust-expert", "/fake/path/adapter_model.safetensors", card)
            .unwrap();
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
        router
            .register(
                "domain",
                "/old/adapter_model.safetensors",
                AdapterCard::for_test("qwen3_16g", "qlora"),
            )
            .unwrap();
        router
            .register(
                "domain",
                "/new/adapter_model.safetensors",
                AdapterCard::for_test("qwen3_16g", "qlora"),
            )
            .unwrap();
        let (path, _card) = router.route("domain").unwrap();
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
        router
            .register(
                "rust-expert",
                "/path/adapter_model.safetensors",
                AdapterCard::for_test("qwen3_16g", "qlora"),
            )
            .unwrap();
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
        router
            .register(
                "",
                "/path/adapter_model.safetensors",
                AdapterCard::for_test("qwen3_16g", "qlora"),
            )
            .unwrap();
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
        router
            .register(
                "vox-lang",
                raw,
                AdapterCard::for_test("qwen3_16g", "qlora"),
            )
            .unwrap();
        let (stored, _card) = router.route("vox-lang").unwrap();
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
        // Catches: discover() skipping directories that DO have the sentinel file.
        // Now also writes an adapter_card.json sidecar (required for registration).
        let tmp = std::env::temp_dir().join("vox_wave26_discover_with_adapter");
        let domain_dir = tmp.join("code-gen");
        fs::create_dir_all(&domain_dir).unwrap();
        let adapter = domain_dir.join("adapter_model.safetensors");
        fs::write(&adapter, b"fake").unwrap();
        AdapterCard::for_test("qwen3_16g", "qlora")
            .write_sidecar(&adapter)
            .unwrap();

        let router = DomainRouter::discover(&tmp).unwrap();
        assert!(
            router.route("code-gen").is_some(),
            "domain dir with adapter_model.safetensors + sidecar must be registered"
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
        AdapterCard::for_test("qwen3_16g", "qlora")
            .write_sidecar(&adapter)
            .unwrap();

        let router = DomainRouter::discover(&tmp).unwrap();
        let (stored, _card) = router.route("math").unwrap();
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
        // Catches: discover() short-circuiting after the first successful entry.
        let tmp = std::env::temp_dir().join("vox_wave26_discover_multi");
        for name in &["alpha", "beta", "gamma"] {
            let d = tmp.join(name);
            fs::create_dir_all(&d).unwrap();
            let adapter = d.join("adapter_model.safetensors");
            fs::write(&adapter, b"fake").unwrap();
            AdapterCard::for_test("qwen3_16g", "qlora")
                .write_sidecar(&adapter)
                .unwrap();
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

    #[test]
    fn discover_skips_adapter_without_sidecar() {
        // Ensures fail-closed: adapter dir without sidecar is silently skipped.
        let tmp = std::env::temp_dir().join("vox_wave26_discover_no_sidecar");
        let domain_dir = tmp.join("nosidecar-domain");
        fs::create_dir_all(&domain_dir).unwrap();
        let adapter = domain_dir.join("adapter_model.safetensors");
        fs::write(&adapter, b"fake").unwrap();
        // Intentionally no sidecar written.

        let router = DomainRouter::discover(&tmp).unwrap();
        assert!(
            router.route("nosidecar-domain").is_none(),
            "adapter without sidecar must be skipped (fail-closed)"
        );

        fs::remove_dir_all(&tmp).ok();
    }
}

#[cfg(test)]
mod provenance_tests {
    use super::*;

    #[test]
    fn register_requires_card_and_fails_on_empty_rung() {
        let mut router = DomainRouter::new();
        let mut card = AdapterCard::for_test("", "qlora"); // empty rung
        card.base_revision = "abc".to_string();
        let result = router.register("test", "/fake/adapter.safetensors", card);
        assert!(result.is_err(), "empty base_rung must fail registration");
    }

    #[test]
    fn register_requires_card_and_fails_on_empty_revision() {
        let mut router = DomainRouter::new();
        let mut card = AdapterCard::for_test("qwen3_16g", "qlora");
        card.base_revision = "".to_string(); // empty revision
        let result = router.register("test", "/fake/adapter.safetensors", card);
        assert!(result.is_err(), "empty base_revision must fail registration");
    }

    #[test]
    fn register_succeeds_with_valid_card() {
        let mut router = DomainRouter::new();
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        router
            .register("vox-lang", "/fake/adapter.safetensors", card)
            .unwrap();
        assert!(router.route("vox-lang").is_some());
    }

    #[test]
    fn is_compatible_with_matches() {
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        assert!(card.is_compatible_with("qwen3_16g", "qlora"));
        assert!(!card.is_compatible_with("qwen3_24g", "qlora"));
        assert!(!card.is_compatible_with("qwen3_16g", "lora"));
    }

    #[test]
    fn load_rejects_rung_mismatch() {
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        // Simulates serve-side checking before loading adapter weights
        assert!(
            !card.is_compatible_with("qwen3_24g", "qlora"),
            "loading must be rejected when serve_rung mismatches"
        );
    }

    #[test]
    fn route_returns_card_alongside_path() {
        let mut router = DomainRouter::new();
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        router
            .register("vox-lang", "/fake/adapter.safetensors", card)
            .unwrap();
        let (path, card) = router.route("vox-lang").unwrap();
        assert_eq!(path, &std::path::PathBuf::from("/fake/adapter.safetensors"));
        assert_eq!(card.base_rung, "qwen3_16g");
        assert_eq!(card.quantization, "qlora");
    }
}

#[cfg(feature = "mens-train")]
use crate::mens::tensor::domain_profiles::DomainProfilesFile;

/// Pick the spoke whose router.triggers substring-match `signal`, by highest
/// priority, breaking ties on the lexicographically smaller spoke name
/// (profiles is a HashMap -> name tie-break required for determinism). A
/// trigger's leading `*` is stripped before matching.
///
/// Note: Substring matching also means `.rs` matches things like `foo.rsync`,
/// which is acceptable for v1.
#[cfg(feature = "mens-train")]
pub fn route_by_signal(file: &DomainProfilesFile, signal: &str) -> Option<String> {
    let mut best: Option<(i32, &str)> = None;
    for (name, p) in &file.profiles {
        let Some(r) = &p.router else { continue };
        let hit = r.triggers.iter().any(|t| {
            let n = t.trim_start_matches('*');
            !n.is_empty() && signal.contains(n)
        });
        if hit {
            let cand = (r.priority, name.as_str());
            let better = match best {
                None => true,
                Some((bp, bn)) => cand.0 > bp || (cand.0 == bp && cand.1 < bn),
            };
            if better {
                best = Some(cand);
            }
        }
    }
    best.map(|(_, n)| n.to_string())
}

#[cfg(all(test, feature = "mens-train"))]
mod signal_tests {
    use super::*;

    fn file() -> DomainProfilesFile {
        serde_yaml::from_str(r#"
profiles:
  rust-expert:     { description: x, router: { triggers: ["*.rs", "lane:vox_rust_authoring"], priority: 10 } }
  tool-selection:  { description: x, router: { triggers: ["lane:vox_tool_selection", "lane:vox_tooling"], priority: 5 } }
"#).unwrap()
    }

    #[test]
    fn rs_routes_rust() {
        assert_eq!(
            route_by_signal(&file(), "src/main.rs").as_deref(),
            Some("rust-expert")
        );
    }

    #[test]
    fn tool_routes_tool_selection() {
        // lane:vox_tooling now routes to tool-selection (agents profile retired by B0.1)
        assert_eq!(
            route_by_signal(&file(), "lane:vox_tooling x").as_deref(),
            Some("tool-selection")
        );
    }

    #[test]
    fn no_match_none() {
        assert_eq!(route_by_signal(&file(), "zzz"), None);
    }

    #[test]
    fn equal_priority_name_tiebreak() {
        let f: DomainProfilesFile = serde_yaml::from_str(
            "profiles:\n  zeta:  { description: x, router: { triggers: [\"x\"], priority: 5 } }\n  alpha: { description: x, router: { triggers: [\"x\"], priority: 5 } }\n"
        ).unwrap();
        for _ in 0..20 {
            assert_eq!(route_by_signal(&f, "x").as_deref(), Some("alpha"));
        }
    }
}
