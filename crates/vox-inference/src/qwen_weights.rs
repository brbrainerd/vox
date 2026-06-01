//! Load an SP-1 quantized artifact (u8 GGML blocks + quant-metadata.json) into
//! candle QMatMul / Tensor weights for the quantized Qwen forward (Option B).
use candle_core::quantized::{GgmlDType, QMatMul};
use candle_core::{Device, Tensor};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum WeightsError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("candle: {0}")]
    Candle(#[from] candle_core::Error),
    #[error("missing tensor `{0}` in safetensors")]
    MissingTensor(String),
    #[error("unsupported ggml dtype `{0}`")]
    BadDtype(String),
}

#[derive(serde::Deserialize)]
struct TensorMeta {
    ggml_dtype: String,
    orig_shape: Vec<usize>,
    quantized: bool,
}
#[derive(serde::Deserialize)]
struct QuantMetadata {
    tensors: HashMap<String, TensorMeta>,
}

fn parse_ggml(s: &str) -> Result<GgmlDType, WeightsError> {
    Ok(match s {
        "Q4K" => GgmlDType::Q4K,
        "Q5K" => GgmlDType::Q5K,
        "Q6K" => GgmlDType::Q6K,
        "Q8_0" => GgmlDType::Q8_0,
        "F32" => GgmlDType::F32,
        other => return Err(WeightsError::BadDtype(other.to_string())),
    })
}

/// Reconstructed Qwen weights: quantized 2-D tensors as [`QMatMul`], KEEP-F32
/// tensors as [`Tensor`].
pub struct QwenWeights {
    qmm: HashMap<String, QMatMul>,
    f32: HashMap<String, Tensor>,
}

impl QwenWeights {
    /// Load an SP-1 artifact directory into candle weights.
    pub fn load(artifact_dir: &Path, dev: &Device) -> Result<Self, WeightsError> {
        let meta_raw = std::fs::read_to_string(artifact_dir.join("quant-metadata.json"))?;
        let meta: QuantMetadata = serde_json::from_str(&meta_raw)?;
        let st = candle_core::safetensors::load(artifact_dir.join("model.safetensors"), dev)?;
        let mut qmm = HashMap::new();
        let mut f32 = HashMap::new();
        for (name, tm) in &meta.tensors {
            let raw = st
                .get(name)
                .ok_or_else(|| WeightsError::MissingTensor(name.clone()))?;
            if !tm.quantized {
                f32.insert(name.clone(), raw.to_dtype(candle_core::DType::F32)?);
                continue;
            }
            let dtype = parse_ggml(&tm.ggml_dtype)?;
            let bytes = raw
                .to_dtype(candle_core::DType::U8)?
                .flatten_all()?
                .to_vec1::<u8>()?;
            let qt = candle_core::quantized::ggml_file::qtensor_from_ggml(
                dtype,
                &bytes,
                tm.orig_shape.clone(),
                dev,
            )?;
            qmm.insert(name.clone(), QMatMul::from_qtensor(qt)?);
        }
        Ok(Self { qmm, f32 })
    }

    /// Quantized weight as a [`QMatMul`], if present.
    pub fn qmatmul(&self, name: &str) -> Option<&QMatMul> {
        self.qmm.get(name)
    }
    /// KEEP-F32 weight as a [`Tensor`], if present.
    pub fn tensor(&self, name: &str) -> Option<&Tensor> {
        self.f32.get(name)
    }
    /// Whether any reconstructed weight exists under `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.qmm.contains_key(name) || self.f32.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device, Tensor};

    #[test]
    fn loads_sp1_artifact_into_qmatmul_and_tensor() {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        let dev = Device::Cpu;
        let mut m = std::collections::HashMap::new();
        m.insert(
            "model.language_model.layers.0.mlp.gate_proj.weight".to_string(),
            Tensor::randn(0f32, 1f32, (256, 256), &dev).unwrap(),
        );
        m.insert(
            "model.language_model.norm.weight".to_string(),
            Tensor::ones((256,), DType::F32, &dev).unwrap(),
        );
        candle_core::safetensors::save(&m, indir.path().join("model.safetensors")).unwrap();
        std::fs::write(
            indir.path().join("config.json"),
            r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#,
        )
        .unwrap();
        vox_quantize::quantize(&vox_quantize::QuantizeRequest {
            input_dir: indir.path().to_path_buf(),
            output_dir: outdir.path().to_path_buf(),
            mixture: vox_quantize::QuantMixture::Q4KM,
            verify: false,
            device: vox_quantize::DevicePref::Cpu,
        })
        .unwrap();

        let w = QwenWeights::load(outdir.path(), &dev).unwrap();
        // gate_proj is a Matrix -> Q4K -> QMatMul
        let qmm = w
            .qmatmul("model.language_model.layers.0.mlp.gate_proj.weight")
            .expect("qmatmul");
        let x = Tensor::zeros((1, 256), DType::F32, &dev).unwrap();
        let y = candle_core::Module::forward(qmm, &x).unwrap();
        assert_eq!(y.dims(), &[1, 256]);
        // norm stays F32 -> Tensor
        assert!(w.tensor("model.language_model.norm.weight").is_some());
        assert!(w.qmatmul("model.language_model.norm.weight").is_none());
    }
}
