use crate::error::QuantizeError;
use crate::read::SafeTensorsSource;
use candle_core::Device;
use std::collections::HashMap;
use std::path::Path;

/// Build a complete f32 model in `out_dir` by taking every base tensor and
/// overwriting the keys present in the merged subset. Errors if the subset
/// contains a key absent from the base (a sign of an adapter/base mismatch).
pub fn recombine(base_dir: &Path, merged_subset: &Path, out_dir: &Path) -> Result<(), QuantizeError> {
    let base = SafeTensorsSource::open(base_dir)?;
    let merged = candle_core::safetensors::load(merged_subset, &Device::Cpu)?;

    let base_names: std::collections::HashSet<&str> =
        base.tensor_names().iter().map(|s| s.as_str()).collect();
    for k in merged.keys() {
        if !base_names.contains(k.as_str()) {
            return Err(QuantizeError::ReadModel(format!(
                "merged key `{k}` not present in base model — adapter/base mismatch"
            )));
        }
    }

    let mut complete: HashMap<String, candle_core::Tensor> = HashMap::new();
    for name in base.tensor_names() {
        let t = match merged.get(name) {
            Some(m) => {
                let base_tensor = base.load_f32(name)?;
                let merged_dims = m.dims().to_vec();
                let base_dims = base_tensor.dims().to_vec();
                if merged_dims != base_dims {
                    return Err(QuantizeError::ReadModel(format!(
                        "merged key `{name}` shape {merged_dims:?} does not match base shape {base_dims:?} — adapter/base mismatch"
                    )));
                }
                m.to_dtype(candle_core::DType::F32)?
            }
            None => base.load_f32(name)?,
        };
        complete.insert(name.clone(), t);
    }

    std::fs::create_dir_all(out_dir)?;
    candle_core::safetensors::save(&complete, out_dir.join("model.safetensors"))?;
    let cfg = base_dir.join("config.json");
    if cfg.exists() {
        std::fs::copy(&cfg, out_dir.join("config.json"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};
    use std::collections::HashMap;

    #[test]
    fn merged_subset_overrides_base_keys() {
        let dev = Device::Cpu;
        let base = tempfile::tempdir().unwrap();
        let merged = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();

        let mut b: HashMap<String, Tensor> = HashMap::new();
        b.insert("w_adapted".into(), Tensor::zeros((256, 256), candle_core::DType::F32, &dev).unwrap());
        b.insert("w_frozen".into(), Tensor::ones((256, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&b, base.path().join("model.safetensors")).unwrap();
        std::fs::write(base.path().join("config.json"), r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();

        let mut m: HashMap<String, Tensor> = HashMap::new();
        m.insert("w_adapted".into(), Tensor::full(2.0f32, (256, 256), &dev).unwrap());
        candle_core::safetensors::save(&m, merged.path().join("merged.safetensors")).unwrap();

        recombine(base.path(), &merged.path().join("merged.safetensors"), out.path()).unwrap();

        let result = candle_core::safetensors::load(out.path().join("model.safetensors"), &dev).unwrap();
        assert_eq!(result["w_adapted"].mean_all().unwrap().to_scalar::<f32>().unwrap(), 2.0);
        assert_eq!(result["w_frozen"].mean_all().unwrap().to_scalar::<f32>().unwrap(), 1.0);
        assert!(out.path().join("config.json").exists());
    }

    #[test]
    fn merged_key_absent_from_base_errors() {
        let dev = Device::Cpu;
        let base = tempfile::tempdir().unwrap();
        let merged = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let mut b: HashMap<String, Tensor> = HashMap::new();
        b.insert("w_frozen".into(), Tensor::ones((256, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&b, base.path().join("model.safetensors")).unwrap();
        let mut m: HashMap<String, Tensor> = HashMap::new();
        m.insert("not_in_base".into(), Tensor::zeros((256, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&m, merged.path().join("merged.safetensors")).unwrap();
        assert!(recombine(base.path(), &merged.path().join("merged.safetensors"), out.path()).is_err());
    }

    #[test]
    fn merged_key_shape_mismatch_errors() {
        let dev = candle_core::Device::Cpu;
        let base = tempfile::tempdir().unwrap();
        let merged = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let mut b = std::collections::HashMap::new();
        b.insert("w".to_string(), candle_core::Tensor::ones((256, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&b, base.path().join("model.safetensors")).unwrap();
        let mut m = std::collections::HashMap::new();
        m.insert("w".to_string(), candle_core::Tensor::ones((128, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&m, merged.path().join("merged.safetensors")).unwrap();
        let err = recombine(base.path(), &merged.path().join("merged.safetensors"), out.path());
        assert!(err.is_err(), "shape mismatch must error");
    }
}
