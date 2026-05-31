//! Integration test for the `merge-qlora --quantize` recombine+quantize path.
//!
//! Drives `vox_quantize::recombine::recombine` + `vox_quantize::quantize`
//! directly — the same calls the CLI post-merge branch makes — without needing
//! a real trained adapter or GPU.

#[test]
fn merge_then_quantize_produces_quantized_artifact() {
    use candle_core::{DType, Device, Tensor};
    use std::collections::HashMap;
    let dev = Device::Cpu;
    let base = tempfile::tempdir().unwrap();
    let merged_dir = tempfile::tempdir().unwrap();
    let q_out = tempfile::tempdir().unwrap();

    let mut b: HashMap<String, Tensor> = HashMap::new();
    b.insert(
        "model.language_model.layers.0.mlp.gate_proj.weight".into(),
        Tensor::randn(0f32, 1f32, (256, 256), &dev).unwrap(),
    );
    b.insert(
        "model.language_model.norm.weight".into(),
        Tensor::ones((256,), DType::F32, &dev).unwrap(),
    );
    candle_core::safetensors::save(&b, base.path().join("model.safetensors")).unwrap();
    std::fs::write(
        base.path().join("config.json"),
        r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#,
    )
    .unwrap();

    let mut m: HashMap<String, Tensor> = HashMap::new();
    m.insert(
        "model.language_model.layers.0.mlp.gate_proj.weight".into(),
        Tensor::full(0.5f32, (256, 256), &dev).unwrap(),
    );
    let merged_file = merged_dir.path().join("merged.safetensors");
    candle_core::safetensors::save(&m, &merged_file).unwrap();

    let recombined = merged_dir.path().join("recombined_full");
    vox_quantize::recombine::recombine(base.path(), &merged_file, &recombined).unwrap();
    let report = vox_quantize::quantize(&vox_quantize::QuantizeRequest {
        input_dir: recombined.clone(),
        output_dir: q_out.path().to_path_buf(),
        mixture: vox_quantize::QuantMixture::Q4KM,
        verify: true,
        device: vox_quantize::DevicePref::Cpu,
    })
    .unwrap();

    assert!(q_out.path().join("quant-metadata.json").exists());
    assert!(report.compression_ratio > 1.5);
}
