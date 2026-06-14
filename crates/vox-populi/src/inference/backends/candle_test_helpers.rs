//! Shared test helpers for Candle inference backend tests.
//!
//! These functions are compiled only under `#[cfg(test)]` via the module
//! declaration in `mod.rs`, so they do not appear in production builds.

use candle_core::{DType, Device, Tensor};

fn rand2(out: usize, inp: usize, dev: &Device) -> Tensor {
    (Tensor::randn(0f32, 1f32, (out, inp), dev).unwrap() * 0.02).unwrap()
}

fn ones1(n: usize, dev: &Device) -> Tensor {
    Tensor::ones((n,), DType::F32, dev).unwrap()
}

/// Build a tiny quantized artifact directory suitable for all Candle backend tests.
///
/// Creates a minimal Qwen3.5 config + random weights, runs them through
/// `vox_quantize` (Q4KM), and writes a hand-rolled WordLevel `tokenizer.json`.
/// Returns the `TempDir` so the caller keeps it alive for the duration of the test.
pub(crate) fn candle_model_build_dir() -> tempfile::TempDir {
    let dev = Device::Cpu;
    let hidden = 256usize;
    let heads = 8usize;
    let head_dim = hidden / heads;
    let inter = 256usize;
    let vocab = 512usize;
    let p = "model.language_model.layers";

    let cfg = format!(
        r#"{{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],
            "text_config":{{"hidden_size":{hidden},"num_attention_heads":{heads},
            "num_key_value_heads":{heads},"num_hidden_layers":1,"vocab_size":{vocab},
            "intermediate_size":{inter},"head_dim":{head_dim},
            "rope_parameters":{{"rope_theta":10000,"partial_rotary_factor":1.0}},
            "layer_types":["full_attention"]}}}}"#
    );

    let mut t = std::collections::HashMap::new();
    t.insert(
        "model.language_model.embed_tokens.weight".into(),
        rand2(vocab, hidden, &dev),
    );
    t.insert(
        format!("{p}.0.self_attn.q_proj.weight"),
        rand2(hidden, hidden, &dev),
    );
    t.insert(
        format!("{p}.0.self_attn.k_proj.weight"),
        rand2(hidden, hidden, &dev),
    );
    t.insert(
        format!("{p}.0.self_attn.v_proj.weight"),
        rand2(hidden, hidden, &dev),
    );
    t.insert(
        format!("{p}.0.self_attn.o_proj.weight"),
        rand2(hidden, hidden, &dev),
    );
    t.insert(
        format!("{p}.0.mlp.gate_proj.weight"),
        rand2(inter, hidden, &dev),
    );
    t.insert(
        format!("{p}.0.mlp.up_proj.weight"),
        rand2(inter, hidden, &dev),
    );
    t.insert(
        format!("{p}.0.mlp.down_proj.weight"),
        rand2(hidden, inter, &dev),
    );
    t.insert(format!("{p}.0.input_layernorm.weight"), ones1(hidden, &dev));
    t.insert(
        format!("{p}.0.post_attention_layernorm.weight"),
        ones1(hidden, &dev),
    );
    t.insert(
        "model.language_model.norm.weight".into(),
        ones1(hidden, &dev),
    );
    t.insert("lm_head.weight".into(), rand2(vocab, hidden, &dev));

    let indir = tempfile::tempdir().unwrap();
    let outdir = tempfile::tempdir().unwrap();
    candle_core::safetensors::save(&t, indir.path().join("model.safetensors")).unwrap();
    std::fs::write(indir.path().join("config.json"), &cfg).unwrap();
    vox_quantize::quantize(&vox_quantize::QuantizeRequest {
        input_dir: indir.path().to_path_buf(),
        output_dir: outdir.path().to_path_buf(),
        mixture: vox_quantize::QuantMixture::Q4KM,
        verify: false,
        device: vox_quantize::DevicePref::Cpu,
    })
    .unwrap();

    let tok = r#"{
      "version": "1.0",
      "truncation": null,
      "padding": null,
      "added_tokens": [],
      "normalizer": null,
      "pre_tokenizer": { "type": "Whitespace" },
      "post_processor": null,
      "decoder": null,
      "model": {
        "type": "WordLevel",
        "vocab": {
          "<|endoftext|>": 0,
          "[UNK]": 1,
          "hello": 2,
          "world": 3,
          "foo": 4,
          "bar": 5
        },
        "unk_token": "[UNK]"
      }
    }"#;
    std::fs::write(outdir.path().join("tokenizer.json"), tok).unwrap();
    outdir
}
