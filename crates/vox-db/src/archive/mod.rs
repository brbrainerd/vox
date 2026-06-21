//! Archive engine: chunking, compression, dedup pipeline, and codec-aware reads (design §3-§5).
pub mod chunking;
pub mod compression;
pub mod membership;
