//! Serving layer for Mens fine-tuned adapters.
//!
//! Provides a vLLM multi-LoRA client ([`vllm_lora`]) with provenance enforcement,
//! LRU slot management, and schema-guided-decoding request construction.

pub mod vllm_lora;

pub use vllm_lora::{AdapterEntry, VllmLoraClient};
