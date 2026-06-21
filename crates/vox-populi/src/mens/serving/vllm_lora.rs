//! vLLM multi-LoRA client with provenance enforcement and LRU eviction.
//!
//! This module provides [`VllmLoraClient`]: a lightweight client that tracks
//! which adapters are currently loaded in a vLLM instance (LRU-bounded), enforces
//! provenance compatibility before loading, and builds well-formed vLLM chat
//! request JSON with guided-decoding attached.
//!
//! No actual HTTP calls are made in unit tests — the "load" bookkeeping is pure
//! in-memory. A real HTTP dispatch layer is left for integration tests (B6.0).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::mens::tensor::adapter_card::AdapterCard;

/// A single adapter slot tracked by the client.
#[derive(Debug, Clone)]
pub struct AdapterEntry {
    pub name: String,
    pub path: PathBuf,
    pub card: AdapterCard,
}

/// In-memory LRU tracker for adapters loaded into a vLLM instance.
///
/// The client does NOT make HTTP calls directly — callers are responsible for
/// issuing the actual `/v1/load_lora_adapter` and `/v1/unload_lora_adapter`
/// requests. This struct tracks which adapters the instance currently holds and
/// enforces provenance + capacity invariants.
#[derive(Debug)]
pub struct VllmLoraClient {
    /// Base URL of the vLLM server (e.g., "http://localhost:8000").
    pub base_url: String,
    /// Names of currently-loaded adapters in LRU order (front = oldest).
    lru_order: VecDeque<String>,
    /// The actual entries, keyed by adapter name.
    loaded: HashMap<String, AdapterEntry>,
    /// Maximum number of adapters the vLLM instance can hold simultaneously.
    max_loaded: usize,
}

impl VllmLoraClient {
    /// Create a new client pointed at `base_url` with the given adapter capacity.
    ///
    /// `max_loaded` must be >= 1; panics in debug mode if 0.
    pub fn new(base_url: String, max_loaded: usize) -> Self {
        debug_assert!(max_loaded >= 1, "max_loaded must be >= 1");
        Self {
            base_url,
            lru_order: VecDeque::new(),
            loaded: HashMap::new(),
            max_loaded,
        }
    }

    /// Returns the name of the adapter that would be evicted next (the LRU entry), if any.
    pub fn lru_eviction_candidate(&self) -> Option<&str> {
        self.lru_order.front().map(String::as_str)
    }

    /// Returns `true` if the named adapter is currently tracked as loaded.
    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded.contains_key(name)
    }

    /// How many adapters are currently tracked as loaded.
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    /// Idempotent load: if the adapter is already tracked, refresh its LRU position
    /// and return `Ok(())`. Otherwise verify provenance compatibility, evict the LRU
    /// entry if at capacity, and insert the new entry.
    ///
    /// Returns `Err` if `card.is_compatible_with(serve_rung, serve_quant)` is false.
    ///
    /// # Note on HTTP
    /// Callers are expected to issue the actual HTTP load/unload calls to vLLM
    /// *around* this method. The evicted name (if any) can be retrieved via
    /// [`lru_eviction_candidate`] *before* calling `ensure_adapter_loaded` when
    /// `loaded_count() == max_loaded` and the adapter is not yet tracked.
    pub fn ensure_adapter_loaded(
        &mut self,
        name: &str,
        path: &Path,
        card: &AdapterCard,
        serve_rung: &str,
        serve_quant: &str,
    ) -> Result<()> {
        // Provenance check first — even for idempotent re-loads.
        if !card.is_compatible_with(serve_rung, serve_quant) {
            anyhow::bail!(
                "adapter '{}' provenance mismatch: card rung={} quant={}, serve expects rung={} quant={}",
                name,
                card.base_rung,
                card.quantization,
                serve_rung,
                serve_quant,
            );
        }

        if self.loaded.contains_key(name) {
            // Already loaded: refresh LRU position (move to back = most recently used).
            self.lru_order.retain(|n| n != name);
            self.lru_order.push_back(name.to_string());
            return Ok(());
        }

        // Evict LRU entry if at capacity.
        if self.loaded.len() >= self.max_loaded {
            if let Some(evict_name) = self.lru_order.pop_front() {
                self.loaded.remove(&evict_name);
            }
        }

        // Insert new entry.
        self.lru_order.push_back(name.to_string());
        self.loaded.insert(
            name.to_string(),
            AdapterEntry {
                name: name.to_string(),
                path: path.to_path_buf(),
                card: card.clone(),
            },
        );

        Ok(())
    }

    /// Build a vLLM `/v1/chat/completions` request JSON for the given task and adapter.
    ///
    /// Sets `model` to `adapter_name` (how vLLM selects the LoRA) and merges
    /// `guided_json` from `tool_schema` for constrained decoding.
    ///
    /// The guided-decoding field is embedded inline (vox-orchestrator-mcp is not a
    /// dependency of vox-populi; the schema is merged directly per the vLLM API).
    pub fn build_chat(&self, task: &str, adapter_name: &str, tool_schema: &Value) -> Value {
        serde_json::json!({
            "model": adapter_name,
            "messages": [
                { "role": "user", "content": task }
            ],
            "guided_json": tool_schema,
            "temperature": 0.0,
            "max_tokens": 512,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card(rung: &str, quant: &str) -> AdapterCard {
        AdapterCard::for_test(rung, quant)
    }

    fn fake_path() -> &'static Path {
        Path::new("/fake/adapter_model.safetensors")
    }

    // ── B6.1 Test 1: idempotent load ────────────────────────────────────────

    #[test]
    fn ensure_adapter_loaded_is_idempotent() {
        let mut client = VllmLoraClient::new("http://localhost:8000".into(), 4);
        let card = make_card("qwen3_16g", "qlora");

        client
            .ensure_adapter_loaded("vox-lang", fake_path(), &card, "qwen3_16g", "qlora")
            .expect("first load must succeed");
        client
            .ensure_adapter_loaded("vox-lang", fake_path(), &card, "qwen3_16g", "qlora")
            .expect("second load must succeed (idempotent)");

        assert_eq!(
            client.loaded_count(),
            1,
            "loading the same adapter twice must not create two entries"
        );
    }

    // ── B6.1 Test 2: rung mismatch ───────────────────────────────────────────

    #[test]
    fn load_rejects_rung_mismatch() {
        let mut client = VllmLoraClient::new("http://localhost:8000".into(), 4);
        let card = make_card("qwen3_16g", "qlora"); // card says 16g

        let result = client.ensure_adapter_loaded(
            "vox-lang",
            fake_path(),
            &card,
            "qwen3_24g", // serve expects 24g → mismatch
            "qlora",
        );

        assert!(
            result.is_err(),
            "loading with serve_rung != card.base_rung must return Err"
        );
        assert_eq!(
            client.loaded_count(),
            0,
            "failed load must not insert an entry"
        );
    }

    // ── B6.1 Test 3: quant mismatch ─────────────────────────────────────────

    #[test]
    fn load_rejects_quant_mismatch() {
        let mut client = VllmLoraClient::new("http://localhost:8000".into(), 4);
        let card = make_card("qwen3_16g", "qlora"); // card says qlora

        let result = client.ensure_adapter_loaded(
            "vox-lang",
            fake_path(),
            &card,
            "qwen3_16g",
            "lora", // serve expects lora → mismatch
        );

        assert!(
            result.is_err(),
            "loading with serve_quant != card.quantization must return Err"
        );
        assert_eq!(
            client.loaded_count(),
            0,
            "failed load must not insert an entry"
        );
    }

    // ── B6.1 Test 4: build_chat sets model and guided_json ──────────────────

    #[test]
    fn build_chat_sets_model_and_guided_json() {
        let client = VllmLoraClient::new("http://localhost:8000".into(), 4);
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "result": { "type": "string" } },
            "required": ["result"]
        });

        let req = client.build_chat("Write a hello world in Vox", "vox-lang-adapter", &schema);

        assert_eq!(
            req["model"].as_str(),
            Some("vox-lang-adapter"),
            "model field must equal the adapter name"
        );
        assert!(
            req.get("guided_json").is_some(),
            "guided_json key must be present"
        );
        assert_eq!(
            req["guided_json"]["type"].as_str(),
            Some("object"),
            "guided_json must embed the provided schema"
        );
    }

    // ── B6.1 Test 5: LRU eviction ───────────────────────────────────────────

    #[test]
    fn lru_evicts_oldest_on_overflow() {
        let mut client = VllmLoraClient::new("http://localhost:8000".into(), 1);
        let card = make_card("qwen3_16g", "qlora");

        // Load adapter A — fills capacity.
        client
            .ensure_adapter_loaded(
                "adapter-a",
                Path::new("/a/adapter_model.safetensors"),
                &card,
                "qwen3_16g",
                "qlora",
            )
            .expect("load A");

        assert!(client.is_loaded("adapter-a"), "A should be loaded");
        assert_eq!(client.loaded_count(), 1);

        // Load adapter B — A is LRU and must be evicted.
        client
            .ensure_adapter_loaded(
                "adapter-b",
                Path::new("/b/adapter_model.safetensors"),
                &card,
                "qwen3_16g",
                "qlora",
            )
            .expect("load B");

        assert!(
            !client.is_loaded("adapter-a"),
            "adapter-a (LRU) must have been evicted when adapter-b was loaded at max_loaded=1"
        );
        assert!(client.is_loaded("adapter-b"), "adapter-b must be loaded");
        assert_eq!(client.loaded_count(), 1);
    }

    // ── Extra: LRU order after refresh ──────────────────────────────────────

    #[test]
    fn lru_refresh_on_second_access_updates_order() {
        let mut client = VllmLoraClient::new("http://localhost:8000".into(), 2);
        let card = make_card("qwen3_16g", "qlora");

        // Load A then B.
        client
            .ensure_adapter_loaded(
                "adapter-a",
                Path::new("/a/adapter_model.safetensors"),
                &card,
                "qwen3_16g",
                "qlora",
            )
            .unwrap();
        client
            .ensure_adapter_loaded(
                "adapter-b",
                Path::new("/b/adapter_model.safetensors"),
                &card,
                "qwen3_16g",
                "qlora",
            )
            .unwrap();

        // Re-access A → A becomes MRU.
        client
            .ensure_adapter_loaded(
                "adapter-a",
                Path::new("/a/adapter_model.safetensors"),
                &card,
                "qwen3_16g",
                "qlora",
            )
            .unwrap();

        // Now load C at max_loaded=2 → B should be evicted (it is LRU now).
        let mut client2 = VllmLoraClient::new("http://localhost:8000".into(), 2);
        let card2 = make_card("qwen3_16g", "qlora");
        client2
            .ensure_adapter_loaded(
                "adapter-a",
                Path::new("/a/adapter_model.safetensors"),
                &card2,
                "qwen3_16g",
                "qlora",
            )
            .unwrap();
        client2
            .ensure_adapter_loaded(
                "adapter-b",
                Path::new("/b/adapter_model.safetensors"),
                &card2,
                "qwen3_16g",
                "qlora",
            )
            .unwrap();
        // Re-access A so B becomes LRU.
        client2
            .ensure_adapter_loaded(
                "adapter-a",
                Path::new("/a/adapter_model.safetensors"),
                &card2,
                "qwen3_16g",
                "qlora",
            )
            .unwrap();
        client2
            .ensure_adapter_loaded(
                "adapter-c",
                Path::new("/c/adapter_model.safetensors"),
                &card2,
                "qwen3_16g",
                "qlora",
            )
            .unwrap();

        assert!(
            !client2.is_loaded("adapter-b"),
            "B must be evicted because A was refreshed (B is LRU)"
        );
        assert!(client2.is_loaded("adapter-a"), "A must still be loaded");
        assert!(client2.is_loaded("adapter-c"), "C must be loaded");
    }
}
