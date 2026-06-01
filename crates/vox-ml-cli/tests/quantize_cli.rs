#[test]
fn quantize_cli_produces_artifact() {
    use candle_core::{DType, Device, Tensor};
    use std::collections::HashMap;
    let indir = tempfile::tempdir().unwrap();
    let outdir = tempfile::tempdir().unwrap();
    let dev = Device::Cpu;
    let mut t: HashMap<String, Tensor> = HashMap::new();
    t.insert(
        "model.language_model.layers.0.mlp.gate_proj.weight".into(),
        Tensor::randn(0f32, 1f32, (256, 256), &dev).unwrap(),
    );
    t.insert(
        "model.language_model.norm.weight".into(),
        Tensor::ones((256,), DType::F32, &dev).unwrap(),
    );
    candle_core::safetensors::save(&t, indir.path().join("model.safetensors")).unwrap();
    std::fs::write(indir.path().join("config.json"),
        r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();

    let args = vox_ml_cli::commands::quantize::QuantizeArgs {
        input: indir.path().to_path_buf(),
        output: outdir.path().to_path_buf(),
        to: "q4_k_m".into(),
        no_verify: false,
        device: "cpu".into(),
        json: true,
    };
    vox_ml_cli::commands::quantize::run(args).unwrap();
    assert!(outdir.path().join("quant-metadata.json").exists());
}
