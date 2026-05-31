//! Full pipeline: SafeTensors model in -> quantized artifact out.
//!
//! Ties together [`read`](crate::read), [`policy`](crate::policy),
//! [`verify`](crate::verify), and [`write`](crate::write) behind a single
//! [`quantize`] entry point driven by a [`QuantizeRequest`].

use crate::device::{select, DevicePref};
use crate::error::QuantizeError;
use crate::policy::{resolve_dtype, QuantMixture, TensorRole};
use crate::read::SafeTensorsSource;
use crate::verify::{round_trip_max_abs, round_trip_mse, QuantReport, TensorQuantStat};
use crate::write::ArtifactWriter;
use candle_core::quantized::{GgmlDType, QTensor};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct QuantizeRequest {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub mixture: QuantMixture,
    pub verify: bool,
    pub device: DevicePref,
}

pub fn quantize(req: &QuantizeRequest) -> Result<QuantReport, QuantizeError> {
    let dev = select(req.device)?;
    let src = SafeTensorsSource::open(&req.input_dir)?;
    let mut writer = ArtifactWriter::new();
    let mut stats = Vec::new();
    let mut total_src: u64 = 0;
    let mut total_quant: u64 = 0;

    for name in src.tensor_names() {
        let t = src.load_f32(name)?;
        let shape = t.dims().to_vec();
        let params: usize = shape.iter().product();
        let src_bytes = (params * 4) as u64;
        total_src += src_bytes;

        // A 0-D / scalar tensor has no last dimension to align against; a
        // last_dim of 0 spuriously satisfies the 256-divisibility check and
        // would route to a k-quant. Keep such tensors in F32.
        if shape.is_empty() {
            writer.add_f32(name, &t)?;
            total_quant += src_bytes;
            stats.push(TensorQuantStat {
                name: name.clone(),
                src_dtype: "F32".into(),
                target_dtype: "F32".into(),
                params,
                mse: 0.0,
                max_abs: 0.0,
                fallback: false,
            });
            continue;
        }

        let role = TensorRole::from_key(name);
        let desired = req.mixture.target_for(role);

        match desired {
            None => {
                writer.add_f32(name, &t)?;
                total_quant += src_bytes;
                stats.push(TensorQuantStat {
                    name: name.clone(),
                    src_dtype: "F32".into(),
                    target_dtype: "F32".into(),
                    params,
                    mse: 0.0,
                    max_abs: 0.0,
                    fallback: false,
                });
            }
            Some(target) => {
                let last_dim = *shape.last().unwrap_or(&0);
                let resolved = resolve_dtype(target, last_dim);
                let fallback = resolved != target;
                if matches!(resolved, GgmlDType::F32) {
                    writer.add_f32(name, &t)?;
                    total_quant += src_bytes;
                    stats.push(TensorQuantStat {
                        name: name.clone(),
                        src_dtype: "F32".into(),
                        target_dtype: "F32".into(),
                        params,
                        mse: 0.0,
                        max_abs: 0.0,
                        fallback,
                    });
                } else {
                    let q = QTensor::quantize_onto(&t, resolved, &dev)?;
                    let (mse, max_abs) = if req.verify {
                        let mse = round_trip_mse(&t, &q)?;
                        if !mse.is_finite() {
                            return Err(QuantizeError::VerifyFailed {
                                tensor: name.clone(),
                                mse,
                            });
                        }
                        (mse, round_trip_max_abs(&t, &q)?)
                    } else {
                        (0.0, 0.0)
                    };
                    total_quant += q.storage_size_in_bytes() as u64;
                    let dtype_str = format!("{resolved:?}");
                    writer.add_quantized(name, &q, &shape)?;
                    stats.push(TensorQuantStat {
                        name: name.clone(),
                        src_dtype: "F32".into(),
                        target_dtype: dtype_str,
                        params,
                        mse,
                        max_abs,
                        fallback,
                    });
                }
            }
        }
    }

    let cfg = req.input_dir.join("config.json");
    if cfg.exists() {
        std::fs::create_dir_all(&req.output_dir)?;
        std::fs::copy(&cfg, req.output_dir.join("config.json"))?;
    }

    let mixture_name = match &req.mixture {
        QuantMixture::Q4KM => "Q4_K_M",
        QuantMixture::Q5KM => "Q5_K_M",
        QuantMixture::Q6K => "Q6_K",
        QuantMixture::Q8_0 => "Q8_0",
        QuantMixture::Manual(_) => "manual",
    };
    writer.finish(&req.output_dir, mixture_name)?;

    let worst_mse = stats.iter().map(|s| s.mse).fold(0.0_f64, f64::max);
    let compression_ratio = if total_quant == 0 {
        0.0
    } else {
        total_src as f64 / total_quant as f64
    };
    Ok(QuantReport {
        tensors: stats,
        total_src_bytes: total_src,
        total_quant_bytes: total_quant,
        compression_ratio,
        worst_mse,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DevicePref;
    use crate::policy::QuantMixture;
    use candle_core::{Device, DType, Tensor};
    use std::collections::HashMap;

    fn tiny_model(dir: &std::path::Path) {
        let dev = Device::Cpu;
        let d = 256usize;
        let mut t: HashMap<String, Tensor> = HashMap::new();
        let w = |r, c| Tensor::randn(0f32, 1f32, (r, c), &dev).unwrap();
        let v = |n| Tensor::ones((n,), DType::F32, &dev).unwrap();
        t.insert("model.language_model.embed_tokens.weight".into(), w(512, d));
        t.insert("model.language_model.layers.0.self_attn.q_proj.weight".into(), w(d, d));
        t.insert("model.language_model.layers.0.self_attn.v_proj.weight".into(), w(d, d));
        t.insert("model.language_model.layers.0.mlp.down_proj.weight".into(), w(d, d));
        t.insert("model.language_model.layers.0.input_layernorm.weight".into(), v(d));
        t.insert("model.language_model.layers.0.linear_attn.A_log".into(), v(d));
        t.insert("model.language_model.norm.weight".into(), v(d));
        candle_core::safetensors::save(&t, dir.join("model.safetensors")).unwrap();
        std::fs::write(dir.join("config.json"),
            r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();
    }

    #[test]
    fn quantizes_end_to_end_q4km() {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        tiny_model(indir.path());
        let req = QuantizeRequest {
            input_dir: indir.path().to_path_buf(),
            output_dir: outdir.path().to_path_buf(),
            mixture: QuantMixture::Q4KM,
            verify: true,
            device: DevicePref::Cpu,
        };
        let report = quantize(&req).unwrap();
        let by_name: std::collections::HashMap<_, _> =
            report.tensors.iter().map(|s| (s.name.as_str(), s)).collect();
        assert_eq!(by_name["model.language_model.layers.0.input_layernorm.weight"].target_dtype, "F32");
        assert_eq!(by_name["model.language_model.layers.0.linear_attn.A_log"].target_dtype, "F32");
        assert_eq!(by_name["model.language_model.layers.0.self_attn.q_proj.weight"].target_dtype, "Q4K");
        assert_eq!(by_name["model.language_model.layers.0.mlp.down_proj.weight"].target_dtype, "Q6K");
        assert_eq!(by_name["model.language_model.layers.0.self_attn.v_proj.weight"].target_dtype, "Q6K");
        assert!(report.compression_ratio > 1.5, "ratio {}", report.compression_ratio);
        assert!(report.worst_mse.is_finite());
        assert!(outdir.path().join("quant-metadata.json").exists());
        assert!(outdir.path().join("config.json").exists());
    }

    #[test]
    fn quantizes_sharded_model() {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        let dev = candle_core::Device::Cpu;
        let w = |r, c| candle_core::Tensor::randn(0f32, 1f32, (r, c), &dev).unwrap();
        let mut s1 = std::collections::HashMap::new();
        s1.insert("model.language_model.layers.0.mlp.gate_proj.weight".to_string(), w(256, 256));
        let mut s2 = std::collections::HashMap::new();
        s2.insert("model.language_model.layers.0.mlp.up_proj.weight".to_string(), w(256, 256));
        candle_core::safetensors::save(&s1, indir.path().join("model-00001-of-00002.safetensors")).unwrap();
        candle_core::safetensors::save(&s2, indir.path().join("model-00002-of-00002.safetensors")).unwrap();
        std::fs::write(indir.path().join("model.safetensors.index.json"),
            r#"{"weight_map":{"model.language_model.layers.0.mlp.gate_proj.weight":"model-00001-of-00002.safetensors","model.language_model.layers.0.mlp.up_proj.weight":"model-00002-of-00002.safetensors"}}"#).unwrap();
        let req = QuantizeRequest {
            input_dir: indir.path().to_path_buf(), output_dir: outdir.path().to_path_buf(),
            mixture: QuantMixture::Q4KM, verify: true, device: DevicePref::Cpu,
        };
        let report = quantize(&req).unwrap();
        assert_eq!(report.tensors.len(), 2);
        assert!(report.tensors.iter().all(|s| s.target_dtype == "Q4K"));
    }

    #[test]
    fn quantizes_with_alignment_fallback() {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        let dev = Device::Cpu;
        let mut t: HashMap<String, Tensor> = HashMap::new();
        // last_dim 96: 96 % 256 != 0 but 96 % 32 == 0 -> Q8_0 fallback.
        t.insert(
            "model.language_model.layers.0.mlp.gate_proj.weight".into(),
            Tensor::randn(0f32, 1f32, (256, 96), &dev).unwrap(),
        );
        // last_dim 100: divisible by neither 256 nor 32 -> F32 fallback.
        t.insert(
            "model.language_model.layers.0.mlp.up_proj.weight".into(),
            Tensor::randn(0f32, 1f32, (256, 100), &dev).unwrap(),
        );
        // A norm weight: KEEP-F32 by role.
        t.insert(
            "model.language_model.norm.weight".into(),
            Tensor::ones((256,), DType::F32, &dev).unwrap(),
        );
        candle_core::safetensors::save(&t, indir.path().join("model.safetensors")).unwrap();
        std::fs::write(
            indir.path().join("config.json"),
            r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#,
        )
        .unwrap();

        let req = QuantizeRequest {
            input_dir: indir.path().to_path_buf(),
            output_dir: outdir.path().to_path_buf(),
            mixture: QuantMixture::Q4KM,
            verify: true,
            device: DevicePref::Cpu,
        };
        let report = quantize(&req).unwrap();
        let by_name: std::collections::HashMap<_, _> =
            report.tensors.iter().map(|s| (s.name.as_str(), s)).collect();

        let gate = by_name["model.language_model.layers.0.mlp.gate_proj.weight"];
        assert_eq!(gate.target_dtype, "Q8_0");
        assert!(gate.fallback);

        let up = by_name["model.language_model.layers.0.mlp.up_proj.weight"];
        assert_eq!(up.target_dtype, "F32");
        assert!(up.fallback);

        let norm = by_name["model.language_model.norm.weight"];
        assert_eq!(norm.target_dtype, "F32");
        assert!(!norm.fallback);
    }
}
