//! Archive engine: chunking, compression, dedup pipeline, and codec-aware reads (design §3-§5).
pub mod cache;
pub mod chunking;
pub mod compression;
pub mod dictionary;
pub mod members;
pub mod membership;
pub mod pipeline;
