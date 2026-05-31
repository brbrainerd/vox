//! Typed error for the quantization engine.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuantizeError {
    #[error("failed to read model: {0}")]
    ReadModel(String),
    #[error("unsupported source dtype for tensor `{0}`")]
    UnsupportedDtype(String),
    #[error("shard index error: {0}")]
    ShardIndex(String),
    #[error("candle quantize error: {0}")]
    Quantize(#[from] candle_core::Error),
    #[error("write error: {0}")]
    Write(String),
    #[error("verification failed for tensor `{tensor}`: non-finite error (mse={mse})")]
    VerifyFailed { tensor: String, mse: f64 },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_is_human_readable() {
        let e = QuantizeError::ShardIndex("model.safetensors.index.json missing".into());
        assert!(format!("{e}").contains("shard index"));
    }
}
