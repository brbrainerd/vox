//! SafeTensors model source reader: single `model.safetensors` or sharded
//! via `model.safetensors.index.json` (HF `weight_map`).

use crate::error::QuantizeError;
use candle_core::{Device, Tensor};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A SafeTensors model source: single `model.safetensors` or sharded via
/// `model.safetensors.index.json` (HF `weight_map`).
pub struct SafeTensorsSource {
    map: HashMap<String, PathBuf>,
    names: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ShardIndex {
    weight_map: HashMap<String, String>,
}

impl SafeTensorsSource {
    pub fn open(dir: &Path) -> Result<Self, QuantizeError> {
        let index = dir.join("model.safetensors.index.json");
        let single = dir.join("model.safetensors");
        let mut map = HashMap::new();
        if index.exists() {
            let raw = std::fs::read_to_string(&index)?;
            let idx: ShardIndex =
                serde_json::from_str(&raw).map_err(|e| QuantizeError::ShardIndex(e.to_string()))?;
            for (name, file) in idx.weight_map {
                map.insert(name, dir.join(file));
            }
        } else if single.exists() {
            let st = candle_core::safetensors::load(&single, &Device::Cpu)?;
            for name in st.keys() {
                map.insert(name.clone(), single.clone());
            }
        } else {
            return Err(QuantizeError::ReadModel(format!(
                "no model.safetensors or model.safetensors.index.json in {}",
                dir.display()
            )));
        }
        let names: Vec<String> = map.keys().cloned().collect();
        Ok(Self { map, names })
    }

    pub fn tensor_names(&self) -> &[String] {
        &self.names
    }

    /// Load a tensor and cast to f32 on CPU.
    pub fn load_f32(&self, name: &str) -> Result<Tensor, QuantizeError> {
        let path = self
            .map
            .get(name)
            .ok_or_else(|| QuantizeError::ReadModel(format!("tensor `{name}` not found")))?;
        let st = candle_core::safetensors::load(path, &Device::Cpu)?;
        let t = st.get(name).ok_or_else(|| {
            QuantizeError::ReadModel(format!("tensor `{name}` missing from shard"))
        })?;
        Ok(t.to_dtype(candle_core::DType::F32)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};
    use std::collections::HashMap;

    fn write_st(dir: &std::path::Path, name: &str, tensors: &[(&str, Tensor)]) {
        let map: HashMap<String, Tensor> = tensors
            .iter()
            .map(|(k, t)| (k.to_string(), t.clone()))
            .collect();
        candle_core::safetensors::save(&map, dir.join(name)).unwrap();
    }

    #[test]
    fn reads_single_file_model() {
        let dir = tempfile::tempdir().unwrap();
        let t = Tensor::zeros((4, 256), candle_core::DType::F32, &Device::Cpu).unwrap();
        write_st(dir.path(), "model.safetensors", &[("w", t)]);
        let src = SafeTensorsSource::open(dir.path()).unwrap();
        let names: Vec<_> = src.tensor_names().to_vec();
        assert_eq!(names, vec!["w".to_string()]);
        let loaded = src.load_f32("w").unwrap();
        assert_eq!(loaded.dims(), &[4, 256]);
    }

    #[test]
    fn reads_sharded_model_via_index() {
        let dir = tempfile::tempdir().unwrap();
        let a = Tensor::zeros((2, 256), candle_core::DType::F32, &Device::Cpu).unwrap();
        let b = Tensor::zeros((2, 256), candle_core::DType::F32, &Device::Cpu).unwrap();
        write_st(dir.path(), "model-00001-of-00002.safetensors", &[("a", a)]);
        write_st(dir.path(), "model-00002-of-00002.safetensors", &[("b", b)]);
        std::fs::write(dir.path().join("model.safetensors.index.json"),
            r#"{"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}"#).unwrap();
        let src = SafeTensorsSource::open(dir.path()).unwrap();
        let mut names = src.tensor_names().to_vec();
        names.sort();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(src.load_f32("b").unwrap().dims(), &[2, 256]);
    }
}
