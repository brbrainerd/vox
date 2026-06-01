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
