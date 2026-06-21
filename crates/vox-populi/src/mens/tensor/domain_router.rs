use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::adapter_card::AdapterCard;

// ── Signal-to-domain mapping ─────────────────────────────────────────────────

/// Known domain names for signal routing.
const DOMAIN_VOX_LANG: &str = "vox-lang";
const DOMAIN_RUST: &str = "rust-expert";
const DOMAIN_TOOL_SELECTION: &str = "tool-selection";
const DOMAIN_ARGUMENT_GENERATION: &str = "argument-generation";

/// A reference to an adapter returned by routing.
#[derive(Debug, Clone)]
pub struct AdapterRef {
    /// Adapter name / domain key.
    pub name: String,
    /// Path to the adapter weights on disk.
    pub path: PathBuf,
    /// Provenance card for the adapter.
    pub card: AdapterCard,
    /// Whether this was a champion or challenger slot.
    pub is_challenger: bool,
}

/// Telemetry record emitted alongside an [`AdapterRef`].
#[derive(Debug, Clone)]
pub struct RoutingTelemetry {
    /// The adapter name that was selected.
    pub adapter_name: String,
    /// The domain that was matched.
    pub domain: String,
    /// True if no adapter was registered for the domain (caller falls back to base model).
    pub is_fallback: bool,
    /// Number of LRU evictions that have occurred in the associated VllmLoraClient
    /// (populated by the caller; defaults to 0 here).
    pub eviction_count: u64,
    /// Unix epoch milliseconds at routing time.
    pub routed_at_ms: u64,
}

impl RoutingTelemetry {
    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn for_adapter(adapter_name: &str, domain: &str, _is_challenger: bool) -> Self {
        Self {
            adapter_name: adapter_name.to_string(),
            domain: domain.to_string(),
            is_fallback: false,
            eviction_count: 0,
            routed_at_ms: Self::now_ms(),
        }
    }

    fn fallback(domain: &str) -> Self {
        Self {
            adapter_name: String::new(),
            domain: domain.to_string(),
            is_fallback: true,
            eviction_count: 0,
            routed_at_ms: Self::now_ms(),
        }
    }
}

/// Per-domain champion/challenger state.
#[derive(Debug, Clone)]
struct DomainSlot {
    champion: (PathBuf, AdapterCard),
    /// Previous champion, kept for rollback after `promote_challenger`.
    prev_champion: Option<(PathBuf, AdapterCard)>,
    challenger: Option<(PathBuf, AdapterCard)>,
}

/// A registry mapping logical domains (e.g., 'rust-expert', 'vox-lang') to their
/// compiled adapter weights on disk and the associated provenance card, enabling
/// multi-LoRA inference multiplexing.
#[derive(Debug, Clone, Default)]
pub struct DomainRouter {
    /// Legacy flat map — kept for backward compatibility with existing `route()` callers.
    adapters: HashMap<String, (PathBuf, AdapterCard)>,
    /// Champion/challenger slots per domain.
    slots: HashMap<String, DomainSlot>,
}

impl DomainRouter {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
            slots: HashMap::new(),
        }
    }

    /// Registers a domain with its compiled artifact path and provenance card.
    ///
    /// Also sets the domain's champion slot. Fail-closed: returns Err if the card
    /// fails validation (missing required fields).
    pub fn register(
        &mut self,
        domain: &str,
        adapter_path: impl AsRef<Path>,
        card: AdapterCard,
    ) -> Result<()> {
        card.validate()?; // fail-closed: error on missing provenance
        let path = adapter_path.as_ref().to_path_buf();
        // Update legacy flat map.
        self.adapters
            .insert(domain.to_string(), (path.clone(), card.clone()));
        // Update champion slot — preserve existing challenger.
        let slot = self
            .slots
            .entry(domain.to_string())
            .or_insert_with(|| DomainSlot {
                champion: (path.clone(), card.clone()),
                prev_champion: None,
                challenger: None,
            });
        slot.prev_champion = None; // new explicit register clears rollback target
        slot.champion = (path, card);
        Ok(())
    }

    /// Returns the adapter path and card for the given domain, if registered.
    pub fn route(&self, domain: &str) -> Option<&(PathBuf, AdapterCard)> {
        self.adapters.get(domain)
    }

    // ── Champion / challenger ────────────────────────────────────────────────

    /// Register a challenger adapter for a domain without making it the default.
    ///
    /// The challenger is served only when the env var `VOX_MENS_SERVE_CHALLENGER`
    /// is set to `"1"` or `"true"`. Fail-closed: the domain must already have a
    /// champion registered, and the card must pass validation.
    pub fn register_challenger(
        &mut self,
        domain: &str,
        adapter_path: impl AsRef<Path>,
        card: AdapterCard,
    ) -> Result<()> {
        card.validate()?;
        let slot = self.slots.get_mut(domain).ok_or_else(|| {
            anyhow::anyhow!(
                "no champion registered for domain '{}'; register champion first",
                domain
            )
        })?;
        slot.challenger = Some((adapter_path.as_ref().to_path_buf(), card));
        Ok(())
    }

    /// Promote the challenger to champion for `domain`.
    ///
    /// The current champion is saved for rollback. Returns Err if there is no
    /// challenger registered for the domain.
    pub fn promote_challenger(&mut self, domain: &str) -> Result<()> {
        let slot = self
            .slots
            .get_mut(domain)
            .ok_or_else(|| anyhow::anyhow!("domain '{}' is not registered", domain))?;
        let challenger = slot
            .challenger
            .take()
            .ok_or_else(|| anyhow::anyhow!("no challenger registered for domain '{}'", domain))?;
        let old_champion = std::mem::replace(&mut slot.champion, challenger);
        slot.prev_champion = Some(old_champion.clone());
        // Keep challenger as a copy of the new champion so the rollback target
        // is the old champion, and challenger slot is cleared (it's now champion).
        // Also update the legacy flat map.
        self.adapters
            .insert(domain.to_string(), slot.champion.clone());
        Ok(())
    }

    /// Rollback the champion to the previous champion (set before the last `promote_challenger`).
    ///
    /// Returns Err if there is no previous champion to roll back to.
    pub fn rollback(&mut self, domain: &str) -> Result<()> {
        let slot = self
            .slots
            .get_mut(domain)
            .ok_or_else(|| anyhow::anyhow!("domain '{}' is not registered", domain))?;
        let prev = slot.prev_champion.take().ok_or_else(|| {
            anyhow::anyhow!(
                "no previous champion to roll back to for domain '{}'",
                domain
            )
        })?;
        slot.champion = prev;
        // Update legacy flat map.
        self.adapters
            .insert(domain.to_string(), slot.champion.clone());
        Ok(())
    }

    // ── Signal routing ───────────────────────────────────────────────────────

    /// Map a signal (file path suffix or lane tag) to a domain name.
    ///
    /// Returns `None` for unknown signals — callers fall back to the base model.
    fn signal_to_domain(signal: &str) -> Option<&'static str> {
        // Lane tags take priority.
        if signal.contains("lane:vox_lang")
            || signal.ends_with(".vox")
            || signal.contains("lane:vox_authoring")
        {
            return Some(DOMAIN_VOX_LANG);
        }
        if signal.contains("lane:vox_rust")
            || signal.ends_with(".rs")
            || signal.contains("lane:rust")
        {
            return Some(DOMAIN_RUST);
        }
        if signal.contains("lane:vox_tool_selection")
            || signal.contains("lane:vox_tooling")
            || signal.contains("lane:tool_selection")
        {
            return Some(DOMAIN_TOOL_SELECTION);
        }
        if signal.contains("lane:vox_argument_generation")
            || signal.contains("lane:argument_generation")
        {
            return Some(DOMAIN_ARGUMENT_GENERATION);
        }
        None
    }

    /// Route a signal to the champion (or challenger if `VOX_MENS_SERVE_CHALLENGER` is set)
    /// adapter for its domain.
    ///
    /// Returns `(None, telemetry)` when:
    /// - the signal doesn't map to a known domain, OR
    /// - no adapter is registered for the matched domain.
    ///
    /// Never errors — unknown signals are gracefully handled.
    pub fn route_by_signal(&self, signal: &str) -> (Option<AdapterRef>, RoutingTelemetry) {
        let Some(domain) = Self::signal_to_domain(signal) else {
            return (None, RoutingTelemetry::fallback("<no-domain>"));
        };

        let Some(slot) = self.slots.get(domain) else {
            return (None, RoutingTelemetry::fallback(domain));
        };

        // Check whether challenger mode is active.
        let serve_challenger = std::env::var("VOX_MENS_SERVE_CHALLENGER")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let (path, card, is_challenger) = if serve_challenger {
            if let Some((p, c)) = &slot.challenger {
                (p, c, true)
            } else {
                // No challenger — fall back to champion silently.
                let (p, c) = &slot.champion;
                (p, c, false)
            }
        } else {
            let (p, c) = &slot.champion;
            (p, c, false)
        };

        let adapter_ref = AdapterRef {
            name: domain.to_string(),
            path: path.clone(),
            card: card.clone(),
            is_challenger,
        };
        let telemetry = RoutingTelemetry::for_adapter(&adapter_ref.name, domain, is_challenger);
        (Some(adapter_ref), telemetry)
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
            .register("vox-lang", raw, AdapterCard::for_test("qwen3_16g", "qlora"))
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
        assert!(
            result.is_err(),
            "empty base_revision must fail registration"
        );
    }

    #[test]
    fn register_requires_card_and_fails_on_empty_quantization() {
        let mut router = DomainRouter::new();
        let mut card = AdapterCard::for_test("qwen3_16g", ""); // empty quantization
        card.base_revision = "abc".to_string();
        let result = router.register("test", "/fake/adapter.safetensors", card);
        assert!(result.is_err(), "empty quantization must fail registration");
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

// ── B6.2 champion/challenger + route_by_signal tests ─────────────────────────

#[cfg(test)]
mod b6_champion_challenger_tests {
    use super::*;

    fn make_router_with_champion(domain: &str, path: &str) -> DomainRouter {
        let mut router = DomainRouter::new();
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        router.register(domain, path, card).unwrap();
        router
    }

    // ── Test 1: route_unknown_signal_returns_none ────────────────────────────

    #[test]
    fn route_unknown_signal_returns_none() {
        let router = DomainRouter::new(); // no adapters
        let (adapter_ref, telemetry) = router.route_by_signal("lane:unknown");
        assert!(
            adapter_ref.is_none(),
            "unknown signal must return None, not panic"
        );
        assert!(
            telemetry.is_fallback,
            "telemetry must mark is_fallback=true for unknown signal"
        );
    }

    // ── Test 2: route_known_lane_returns_champion ────────────────────────────

    #[test]
    fn route_known_lane_returns_champion() {
        let router =
            make_router_with_champion("rust-expert", "/models/rust/adapter_model.safetensors");
        let (adapter_ref, telemetry) = router.route_by_signal("src/main.rs");
        let adapter_ref = adapter_ref.expect("*.rs signal must route to rust-expert");
        assert_eq!(adapter_ref.name, "rust-expert");
        assert!(
            !adapter_ref.is_challenger,
            "must be champion, not challenger"
        );
        assert!(!telemetry.is_fallback);
    }

    // ── Test 3: challenger_not_served_without_flag ───────────────────────────

    #[test]
    fn challenger_not_served_without_flag() {
        // Ensure the env var is not set for this test.
        // SAFETY: test-only; serial_test not imported so we use best-effort.
        unsafe { std::env::remove_var("VOX_MENS_SERVE_CHALLENGER") };

        let mut router =
            make_router_with_champion("vox-lang", "/models/vox-lang/adapter_model.safetensors");

        let challenger_card = AdapterCard::for_test("qwen3_16g", "qlora");
        router
            .register_challenger(
                "vox-lang",
                "/models/vox-lang-v2/adapter_model.safetensors",
                challenger_card,
            )
            .unwrap();

        let (adapter_ref, _tel) = router.route_by_signal("foo.vox");
        let adapter_ref = adapter_ref.expect("vox-lang domain must have a champion");
        assert!(
            !adapter_ref.is_challenger,
            "challenger must NOT be served when VOX_MENS_SERVE_CHALLENGER is not set"
        );
        assert!(
            adapter_ref.path.to_string_lossy().contains("vox-lang/"),
            "champion path must be returned, not challenger path; got {:?}",
            adapter_ref.path
        );
    }

    // ── Test 4: promote_makes_challenger_champion ────────────────────────────

    #[test]
    fn promote_makes_challenger_champion() {
        let mut router =
            make_router_with_champion("rust-expert", "/v1/rust/adapter_model.safetensors");

        let challenger_card = AdapterCard::for_test("qwen3_16g", "qlora");
        router
            .register_challenger(
                "rust-expert",
                "/v2/rust/adapter_model.safetensors",
                challenger_card,
            )
            .unwrap();

        router.promote_challenger("rust-expert").unwrap();

        // After promotion, route_by_signal must return the new champion (v2).
        let (adapter_ref, _) = router.route_by_signal("lib.rs");
        let adapter_ref = adapter_ref.expect("must route after promotion");
        assert!(
            adapter_ref.path.to_string_lossy().contains("v2"),
            "promoted challenger must be the new champion; got {:?}",
            adapter_ref.path
        );
        // The legacy route() should also reflect the new champion.
        let (path, _) = router.route("rust-expert").unwrap();
        assert!(path.to_string_lossy().contains("v2"));
    }

    // ── Test 5: rollback_restores_prior_champion ─────────────────────────────

    #[test]
    fn rollback_restores_prior_champion() {
        let mut router =
            make_router_with_champion("rust-expert", "/v1/rust/adapter_model.safetensors");

        let challenger_card = AdapterCard::for_test("qwen3_16g", "qlora");
        router
            .register_challenger(
                "rust-expert",
                "/v2/rust/adapter_model.safetensors",
                challenger_card,
            )
            .unwrap();

        router.promote_challenger("rust-expert").unwrap();

        // Verify v2 is now champion.
        let (path, _) = router.route("rust-expert").unwrap();
        assert!(path.to_string_lossy().contains("v2"));

        // Rollback.
        router.rollback("rust-expert").unwrap();

        let (path, _) = router.route("rust-expert").unwrap();
        assert!(
            path.to_string_lossy().contains("v1"),
            "rollback must restore the pre-promote champion; got {path:?}"
        );
    }

    // ── Extra: double rollback returns Err ────────────────────────────────────

    #[test]
    fn double_rollback_returns_err() {
        let mut router =
            make_router_with_champion("vox-lang", "/v1/vox-lang/adapter_model.safetensors");

        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        router
            .register_challenger("vox-lang", "/v2/vox-lang/adapter_model.safetensors", card)
            .unwrap();
        router.promote_challenger("vox-lang").unwrap();
        router.rollback("vox-lang").unwrap(); // first rollback — ok
        let r = router.rollback("vox-lang"); // second rollback — nothing to roll back to
        assert!(r.is_err(), "double rollback must return Err");
    }

    // ── Extra: register_challenger errors when no champion ───────────────────

    #[test]
    fn register_challenger_requires_champion() {
        let mut router = DomainRouter::new();
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        let r = router.register_challenger("no-champion-domain", "/some/path", card);
        assert!(
            r.is_err(),
            "register_challenger without prior champion must return Err"
        );
    }

    // ── Extra: vox-lang lane routing ─────────────────────────────────────────

    #[test]
    fn vox_lang_lane_routes_to_vox_lang() {
        let router =
            make_router_with_champion("vox-lang", "/models/vox-lang/adapter_model.safetensors");
        let (adapter_ref, _) = router.route_by_signal("lane:vox_lang some context");
        assert!(
            adapter_ref.is_some(),
            "lane:vox_lang must route to vox-lang domain"
        );
    }

    // ── Extra: tool-selection lane routing ────────────────────────────────────

    #[test]
    fn tool_selection_lane_routes_correctly() {
        let router = make_router_with_champion(
            "tool-selection",
            "/models/tool-selection/adapter_model.safetensors",
        );
        let (adapter_ref, _) = router.route_by_signal("lane:vox_tool_selection");
        assert!(
            adapter_ref.is_some(),
            "lane:vox_tool_selection must route to tool-selection"
        );
    }

    // ── Extra: telemetry fields on fallback ───────────────────────────────────

    #[test]
    fn telemetry_is_fallback_true_when_no_adapter() {
        // Domain registered but no adapters in router → is_fallback
        let router = DomainRouter::new();
        let (_, tel) = router.route_by_signal("lane:unknown_xyz");
        assert!(tel.is_fallback);
        assert!(tel.adapter_name.is_empty() || tel.adapter_name == "");
    }
}
