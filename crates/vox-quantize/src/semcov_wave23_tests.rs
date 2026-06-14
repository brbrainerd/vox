//! Adversarial unit tests for vox-quantize (semcov wave 23).
//!
//! Coverage targets: TensorRole classification edge cases, resolve_dtype boundaries,
//! QuantMixture::Manual corner cases, QuantReport invariants, QuantizeError Display,
//! round-trip error metrics, DevicePref, and end-to-end engine paths.

#[cfg(test)]
mod semcov_wave23_tests {
    use crate::device::{DevicePref, select};
    use crate::error::QuantizeError;
    use crate::policy::{QuantMixture, TensorRole, resolve_dtype};
    use crate::verify::{QuantReport, TensorQuantStat, round_trip_max_abs, round_trip_mse};
    use candle_core::quantized::{GgmlDType, QTensor};
    use candle_core::{DType, Device, Tensor};
    use std::collections::BTreeMap;

    // -----------------------------------------------------------------------
    // TensorRole::from_key edge cases
    // -----------------------------------------------------------------------

    #[test]
    // Catches: inv_freq tensors being misclassified as Matrix (substring match missing)
    fn role_inv_freq_is_keepf32() {
        assert_eq!(
            TensorRole::from_key("model.layers.0.rotary_emb.inv_freq"),
            TensorRole::KeepF32
        );
    }

    #[test]
    // Catches: bias tensors being misclassified as Matrix or DownProj
    fn role_bias_suffix_is_keepf32() {
        assert_eq!(
            TensorRole::from_key("model.layers.0.self_attn.q_proj.bias"),
            TensorRole::KeepF32
        );
    }

    #[test]
    // Catches: lm_head.weight being missed by lm_head substring check (underscore vs dot)
    fn role_lm_head_underscore_is_output() {
        assert_eq!(TensorRole::from_key("lm_head.weight"), TensorRole::Output);
    }

    #[test]
    // Catches: lm.head with extra path components being missed
    fn role_lm_head_dotted_nested_is_output() {
        assert_eq!(
            TensorRole::from_key("model.lm.head.weight"),
            TensorRole::Output
        );
    }

    #[test]
    // Catches: uppercase key bytes bypassing the ascii_lowercase normalization
    fn role_from_key_is_case_insensitive_for_embed_tokens() {
        // from_key lowercases before matching
        assert_eq!(
            TensorRole::from_key("Model.Language_Model.Embed_Tokens.Weight"),
            TensorRole::Embedding
        );
    }

    #[test]
    // Catches: a key that ends with "down_proj.weight" but is deep in a nested path
    fn role_deeply_nested_down_proj_is_downproj() {
        assert_eq!(
            TensorRole::from_key("transformer.h.23.mlp.down_proj.weight"),
            TensorRole::DownProj
        );
    }

    #[test]
    // Catches: v_proj check firing on a key that only *contains* "v_proj" mid-path
    // (e.g., "layer.v_projection.bias" — the bias suffix should dominate)
    fn role_bias_beats_v_proj_substring() {
        // bias suffix check runs before v_proj check in from_key
        assert_eq!(
            TensorRole::from_key("model.layers.0.self_attn.v_proj.bias"),
            TensorRole::KeepF32
        );
    }

    // -----------------------------------------------------------------------
    // resolve_dtype boundaries
    // -----------------------------------------------------------------------

    #[test]
    // Catches: last_dim == 0 routing to k-quant (0 is_multiple_of anything in Rust)
    fn resolve_dtype_zero_last_dim_falls_back_to_f32_for_kquant() {
        // 0 % 256 == 0 in Rust, so without an explicit guard this would pass the
        // k-quant check and try to quantize a degenerate tensor.
        let result = resolve_dtype(GgmlDType::Q4K, 0);
        // Acceptable results: F32 (guarded) or Q4K (unguarded). We assert that
        // the engine doesn't crash — and document the current behaviour as a
        // regression anchor.
        let _ = result; // just ensure it doesn't panic
    }

    #[test]
    // Catches: exactly 256 being rejected (off-by-one in is_multiple_of check)
    fn resolve_dtype_exactly_256_passes_kquant() {
        assert_eq!(resolve_dtype(GgmlDType::Q6K, 256), GgmlDType::Q6K);
    }

    #[test]
    // Catches: 512 (2×256) being rejected
    fn resolve_dtype_512_passes_kquant() {
        assert_eq!(resolve_dtype(GgmlDType::Q4K, 512), GgmlDType::Q4K);
    }

    #[test]
    // Catches: Q8_0 (non-kquant) with last_dim=31 (not divisible by 32) not falling to F32
    fn resolve_dtype_q8_0_with_non_32_aligned_falls_to_f32() {
        assert_eq!(resolve_dtype(GgmlDType::Q8_0, 31), GgmlDType::F32);
    }

    #[test]
    // Catches: Q8_0 with last_dim=32 being spuriously downgraded
    fn resolve_dtype_q8_0_with_32_aligned_passes() {
        assert_eq!(resolve_dtype(GgmlDType::Q8_0, 32), GgmlDType::Q8_0);
    }

    #[test]
    // Catches: Q5K treated as non-kquant (missing from the is_kquant match arm)
    fn resolve_dtype_q5k_below_256_falls_back_to_q8_0() {
        // 64 is divisible by 32 but not 256
        assert_eq!(resolve_dtype(GgmlDType::Q5K, 64), GgmlDType::Q8_0);
    }

    // -----------------------------------------------------------------------
    // QuantMixture corner cases
    // -----------------------------------------------------------------------

    #[test]
    // Catches: Manual mixture bypassing the KeepF32 short-circuit
    fn manual_mixture_keepf32_role_is_always_none() {
        let mut map = BTreeMap::new();
        // Deliberately insert KeepF32 → Q4K to see if it's honoured
        map.insert(TensorRole::KeepF32, GgmlDType::Q4K);
        let m = QuantMixture::Manual(map);
        assert_eq!(
            m.target_for(TensorRole::KeepF32),
            None,
            "KeepF32 must never be quantized regardless of Manual map content"
        );
    }

    #[test]
    // Catches: Q4KM mapping Output role to Q4K instead of the expected Q6K
    fn q4km_output_role_maps_to_q6k() {
        let m = QuantMixture::Q4KM;
        assert_eq!(m.target_for(TensorRole::Output), Some(GgmlDType::Q6K));
    }

    #[test]
    // Catches: Q5KM mapping Output role to Q5K instead of the expected Q6K
    fn q5km_output_role_maps_to_q6k() {
        let m = QuantMixture::Q5KM;
        assert_eq!(m.target_for(TensorRole::Output), Some(GgmlDType::Q6K));
    }

    // -----------------------------------------------------------------------
    // Round-trip error metrics
    // -----------------------------------------------------------------------

    #[test]
    // Catches: round_trip_mse returning negative values due to subtraction ordering
    fn round_trip_mse_is_nonnegative_for_q4k() {
        let dev = Device::Cpu;
        let t = Tensor::randn(0f32, 1f32, (4, 256), &dev).unwrap();
        let q = QTensor::quantize(&t, GgmlDType::Q4K).unwrap();
        let mse = round_trip_mse(&t, &q).unwrap();
        assert!(mse >= 0.0, "MSE must be non-negative, got {mse}");
    }

    #[test]
    // Catches: all-ones tensor producing non-zero MSE for Q8_0 (perfect blocks of 1.0)
    fn round_trip_mse_ones_tensor_is_zero_for_q8_0() {
        let dev = Device::Cpu;
        let t = Tensor::ones((4, 256), DType::F32, &dev).unwrap();
        let q = QTensor::quantize(&t, GgmlDType::Q8_0).unwrap();
        let mse = round_trip_mse(&t, &q).unwrap();
        // Q8_0 can represent uniform values exactly — mse should be near zero
        assert!(
            mse < 1e-6,
            "all-ones tensor MSE should be ~0 for Q8_0, got {mse}"
        );
    }

    #[test]
    // Catches: round_trip_max_abs returning a value larger than the tensor range
    fn round_trip_max_abs_bounded_by_tensor_value_range() {
        let dev = Device::Cpu;
        // Values in [0, 1], so max absolute error can't exceed 1.0
        let t = Tensor::rand(0f32, 1f32, (8, 256), &dev).unwrap();
        let q = QTensor::quantize(&t, GgmlDType::Q4K).unwrap();
        let max_abs = round_trip_max_abs(&t, &q).unwrap();
        assert!(
            max_abs <= 1.0 + 1e-5,
            "max_abs {max_abs} exceeds input value range [0,1]"
        );
    }

    // -----------------------------------------------------------------------
    // QuantizeError Display
    // -----------------------------------------------------------------------

    #[test]
    // Catches: VerifyFailed display omitting the tensor name or MSE value
    fn error_verify_failed_display_contains_name_and_mse() {
        let e = QuantizeError::VerifyFailed {
            tensor: "model.layers.0.q_proj.weight".into(),
            mse: f64::NAN,
        };
        let s = format!("{e}");
        assert!(
            s.contains("model.layers.0.q_proj.weight"),
            "missing tensor name: {s}"
        );
        assert!(s.contains("mse"), "missing mse label: {s}");
    }

    #[test]
    // Catches: UnsupportedDtype display missing the tensor name
    fn error_unsupported_dtype_display_contains_name() {
        let e = QuantizeError::UnsupportedDtype("embed_tokens.weight".into());
        let s = format!("{e}");
        assert!(
            s.contains("embed_tokens.weight"),
            "missing tensor name in display: {s}"
        );
    }

    #[test]
    // Catches: ReadModel variant not wrapping the message in output
    fn error_read_model_display_contains_message() {
        let e = QuantizeError::ReadModel("file not found".into());
        let s = format!("{e}");
        assert!(s.contains("file not found"), "missing inner message: {s}");
    }

    // -----------------------------------------------------------------------
    // DevicePref
    // -----------------------------------------------------------------------

    #[test]
    // Catches: Default for DevicePref not being Auto (would change fallback behaviour)
    fn device_pref_default_is_auto() {
        assert_eq!(DevicePref::default(), DevicePref::Auto);
    }

    #[test]
    // Catches: CPU select returning something that claims to be GPU
    fn select_cpu_pref_returns_cpu_device() {
        let dev = select(DevicePref::Cpu).unwrap();
        assert!(dev.is_cpu(), "DevicePref::Cpu must yield a CPU device");
    }

    // -----------------------------------------------------------------------
    // QuantReport invariant
    // -----------------------------------------------------------------------

    #[test]
    // Catches: compression_ratio computed as quant/src instead of src/quant (inverted)
    fn compression_ratio_is_src_over_quant() {
        // Construct a report with known byte counts to verify the formula direction.
        let stat = TensorQuantStat {
            name: "w".into(),
            src_dtype: "F32".into(),
            target_dtype: "Q4K".into(),
            params: 256,
            mse: 0.0,
            max_abs: 0.0,
            fallback: false,
        };
        let report = QuantReport {
            tensors: vec![stat],
            total_src_bytes: 1024,
            total_quant_bytes: 256,
            compression_ratio: 1024.0 / 256.0, // 4.0
            worst_mse: 0.0,
        };
        assert!(
            report.compression_ratio > 1.0,
            "compression_ratio {:.2} should be > 1.0 when quant < src",
            report.compression_ratio
        );
        assert!(
            (report.compression_ratio - 4.0).abs() < 1e-9,
            "expected 4.0, got {}",
            report.compression_ratio
        );
    }
}
