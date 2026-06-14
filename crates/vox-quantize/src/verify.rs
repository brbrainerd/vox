//! Round-trip error measurement and the report structs the engine returns.

use crate::error::QuantizeError;
use candle_core::Tensor;
use candle_core::quantized::QTensor;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TensorQuantStat {
    pub name: String,
    pub src_dtype: String,
    pub target_dtype: String,
    pub params: usize,
    pub mse: f64,
    pub max_abs: f64,
    pub fallback: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuantReport {
    pub tensors: Vec<TensorQuantStat>,
    pub total_src_bytes: u64,
    pub total_quant_bytes: u64,
    pub compression_ratio: f64,
    pub worst_mse: f64,
}

/// Dequantize `q` and compute mean-squared error against the f32 source `src`.
pub fn round_trip_mse(src: &Tensor, q: &QTensor) -> Result<f64, QuantizeError> {
    let deq = q.dequantize(src.device())?;
    let diff = (src - &deq)?;
    let sq = diff.sqr()?;
    let mse = sq.mean_all()?.to_scalar::<f32>()? as f64;
    Ok(mse)
}

/// Max absolute error against the f32 source.
pub fn round_trip_max_abs(src: &Tensor, q: &QTensor) -> Result<f64, QuantizeError> {
    let deq = q.dequantize(src.device())?;
    let diff = (src - &deq)?.abs()?;
    Ok(diff.max_all()?.to_scalar::<f32>()? as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::quantized::{GgmlDType, QTensor};
    use candle_core::{Device, Tensor};

    #[test]
    fn q8_0_error_smaller_than_q4k() {
        let dev = Device::Cpu;
        let t = Tensor::randn(0f32, 1f32, (16, 256), &dev).unwrap();
        let q8 = QTensor::quantize(&t, GgmlDType::Q8_0).unwrap();
        let q4 = QTensor::quantize(&t, GgmlDType::Q4K).unwrap();
        let e8 = round_trip_mse(&t, &q8).unwrap();
        let e4 = round_trip_mse(&t, &q4).unwrap();
        assert!(e8 < e4, "Q8_0 mse {e8} should be < Q4K mse {e4}");
        assert!(e8.is_finite() && e4.is_finite());
    }
}

#[cfg(test)]
mod semcov_wave5_tests {
    use super::*;
    use candle_core::quantized::{GgmlDType, QTensor};
    use candle_core::{Device, Tensor};

    #[test]
    fn round_trip_max_abs_is_nonnegative_and_finite_for_q8_0() {
        let dev = Device::Cpu;
        let t = Tensor::randn(0f32, 1f32, (8, 256), &dev).unwrap();
        let q = QTensor::quantize(&t, GgmlDType::Q8_0).unwrap();
        let max_abs = round_trip_max_abs(&t, &q).unwrap();
        assert!(
            max_abs >= 0.0,
            "max_abs must be non-negative, got {max_abs}"
        );
        assert!(max_abs.is_finite(), "max_abs must be finite, got {max_abs}");
    }

    #[test]
    fn round_trip_max_abs_is_greater_for_q4k_than_q8_0() {
        // Q4K has coarser quantization than Q8_0, so its max absolute error
        // should be >= Q8_0 max absolute error with high probability on random data.
        let dev = Device::Cpu;
        let t = Tensor::randn(0f32, 1f32, (32, 256), &dev).unwrap();
        let q4 = QTensor::quantize(&t, GgmlDType::Q4K).unwrap();
        let q8 = QTensor::quantize(&t, GgmlDType::Q8_0).unwrap();
        let max4 = round_trip_max_abs(&t, &q4).unwrap();
        let max8 = round_trip_max_abs(&t, &q8).unwrap();
        assert!(
            max4 >= max8,
            "Q4K max_abs {max4} should be >= Q8_0 max_abs {max8}"
        );
        assert!(max4.is_finite() && max8.is_finite());
    }

    #[test]
    fn round_trip_max_abs_of_zero_tensor_is_zero() {
        let dev = Device::Cpu;
        let t = Tensor::zeros((4, 256), candle_core::DType::F32, &dev).unwrap();
        let q = QTensor::quantize(&t, GgmlDType::Q8_0).unwrap();
        let max_abs = round_trip_max_abs(&t, &q).unwrap();
        assert_eq!(max_abs, 0.0);
    }
}
