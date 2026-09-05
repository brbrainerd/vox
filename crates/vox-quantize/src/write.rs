//! Write a quantized SafeTensors-canonical artifact (ADR-043 on-disk format).
//!
//! Quantized tensors are serialized as 1-D `u8` SafeTensors tensors (the raw
//! GGML block bytes from candle), and a `quant-metadata.json` sidecar records
//! the per-tensor GGML dtype, original shape/dtype, and the mixture name.

use crate::error::QuantizeError;
use candle_core::Tensor;
use candle_core::quantized::QTensor;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct TensorMeta {
    pub ggml_dtype: String,
    pub orig_shape: Vec<usize>,
    pub orig_dtype: String,
    pub quantized: bool,
}

#[derive(Debug, Serialize)]
struct QuantMetadata {
    mixture: String,
    writer_version: String,
    tensors: HashMap<String, TensorMeta>,
}

#[derive(Default)]
pub struct ArtifactWriter {
    raw: HashMap<String, (Vec<u8>, candle_core::DType, Vec<usize>)>,
    meta: HashMap<String, TensorMeta>,
}

fn quantized_bytes(q: &QTensor) -> Result<Vec<u8>, QuantizeError> {
    Ok(q.data()?.into_owned())
}

impl ArtifactWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_quantized(
        &mut self,
        name: &str,
        q: &QTensor,
        orig_shape: &[usize],
    ) -> Result<(), QuantizeError> {
        let bytes = quantized_bytes(q)?;
        self.meta.insert(
            name.to_string(),
            TensorMeta {
                ggml_dtype: format!("{:?}", q.dtype()),
                orig_shape: orig_shape.to_vec(),
                orig_dtype: "F32".into(),
                quantized: true,
            },
        );
        let len = bytes.len();
        self.raw
            .insert(name.to_string(), (bytes, candle_core::DType::U8, vec![len]));
        Ok(())
    }

    pub fn add_f32(&mut self, name: &str, t: &Tensor) -> Result<(), QuantizeError> {
        let shape = t.dims().to_vec();
        let flat = t.flatten_all()?.to_vec1::<f32>()?;
        let mut bytes = Vec::with_capacity(flat.len() * 4);
        for v in &flat {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        self.meta.insert(
            name.to_string(),
            TensorMeta {
                ggml_dtype: "F32".into(),
                orig_shape: shape.clone(),
                orig_dtype: "F32".into(),
                quantized: false,
            },
        );
        self.raw
            .insert(name.to_string(), (bytes, candle_core::DType::F32, shape));
        Ok(())
    }

    pub fn finish(self, out_dir: &Path, mixture: &str) -> Result<(), QuantizeError> {
        std::fs::create_dir_all(out_dir)?;
        use candle_core::{Device, Tensor};
        let mut tensors: HashMap<String, Tensor> = HashMap::new();
        for (name, (bytes, dtype, shape)) in self.raw {
            let t = match dtype {
                candle_core::DType::U8 => Tensor::from_vec(bytes, shape.clone(), &Device::Cpu)?,
                candle_core::DType::F32 => {
                    let floats: Vec<f32> = bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|c| f32::from_le_bytes(*c))
                        .collect();
                    Tensor::from_vec(floats, shape.clone(), &Device::Cpu)?
                }
                _ => {
                    return Err(QuantizeError::Write(format!(
                        "unexpected dtype for `{name}`"
                    )));
                }
            };
            tensors.insert(name, t);
        }
        candle_core::safetensors::save(&tensors, out_dir.join("model.safetensors"))?;

        let meta = QuantMetadata {
            mixture: mixture.to_string(),
            writer_version: env!("CARGO_PKG_VERSION").to_string(),
            tensors: self.meta,
        };
        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| QuantizeError::Write(e.to_string()))?;
        std::fs::write(out_dir.join("quant-metadata.json"), json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::quantized::{GgmlDType, QTensor};
    use candle_core::{Device, Tensor};

    #[test]
    fn writes_blocks_and_metadata_that_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let dev = Device::Cpu;
        let t = Tensor::randn(0f32, 1f32, (8, 256), &dev).unwrap();
        let q = QTensor::quantize(&t, GgmlDType::Q4K).unwrap();
        let mut artifact = ArtifactWriter::new();
        artifact.add_quantized("w", &q, &[8, 256]).unwrap();
        artifact
            .add_f32(
                "norm",
                &Tensor::ones((8,), candle_core::DType::F32, &dev).unwrap(),
            )
            .unwrap();
        artifact.finish(dir.path(), "Q4_K_M").unwrap();

        // Parse-and-assert rather than substring matching: `to_string_pretty`
        // inserts spaces after colons, so `"ggml_dtype":"Q4K"` would never
        // appear literally. Asserting against the parsed value keeps the
        // intent (metadata records dtype + mixture) without coupling to
        // serializer whitespace.
        let meta_raw = std::fs::read_to_string(dir.path().join("quant-metadata.json")).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&meta_raw).unwrap();
        assert_eq!(meta["tensors"]["w"]["ggml_dtype"], "Q4K");
        assert_eq!(meta["tensors"]["w"]["quantized"], true);
        assert_eq!(meta["tensors"]["norm"]["ggml_dtype"], "F32");
        assert_eq!(meta["tensors"]["norm"]["quantized"], false);
        assert_eq!(meta["mixture"], "Q4_K_M");
        assert!(dir.path().join("model.safetensors").exists());
    }
}
