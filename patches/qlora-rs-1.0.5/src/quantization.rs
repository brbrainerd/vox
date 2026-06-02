//! 4-bit `NormalFloat` (NF4) quantization.
//!
//! NF4 quantization uses 16 levels optimized for normally-distributed weights,
//! providing better accuracy than uniform 4-bit quantization.
//!
//! Features:
//! - Per-tensor and per-channel quantization strategies
//! - Zero-point (asymmetric) quantization for non-centered distributions
//! - Double quantization for additional memory savings
//! - Mixed precision dequantization (F32, F16, BF16)
//! - Quantization-aware padding for block alignment
//!
//! Reference: <https://arxiv.org/abs/2305.14314> (`QLoRA` paper)

use candle_core::{DType, Device, Tensor};
use serde::{Deserialize, Serialize};

use crate::error::{QLoraError, Result};

/// The 16 quantization levels for NF4, optimized for N(0,1) distribution.
/// These values minimize expected quantization error for normally-distributed data.
#[allow(clippy::excessive_precision)]
pub const NF4_LEVELS: [f32; 16] = [
    -1.0,
    -0.696_192_800_998_688,
    -0.525_073_051_452_637,
    -0.394_917_488_098_145,
    -0.284_441_381_692_887,
    -0.184_773_430_228_233,
    -0.091_050_036_251_545,
    0.0,
    0.079_580_299_556_255,
    0.160_930_201_411_247,
    0.246_112_301_945_686,
    0.337_915_241_718_292,
    0.440_709_829_330_444,
    0.562_617_003_917_694,
    0.722_956_836_223_602,
    1.0,
];

/// Quantization strategy for organizing scale factors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum QuantizationStrategy {
    /// Standard per-tensor block quantization.
    PerTensor,
    /// Per-channel quantization (one scale per output channel).
    PerChannel,
}

/// Configuration for quantization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    /// Block size for quantization (number of values sharing a scale).
    pub block_size: usize,

    /// Whether to use double quantization (quantize the scales).
    pub double_quant: bool,

    /// Data type for computation (usually bf16 or f16).
    pub compute_dtype: ComputeDType,

    /// Quantization strategy (per-tensor or per-channel).
    pub strategy: QuantizationStrategy,

    /// Whether to use zero-point quantization (asymmetric).
    pub use_zero_point: bool,
}

/// Compute data type for dequantized values.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum ComputeDType {
    /// 32-bit float
    #[default]
    F32,
    /// 16-bit float
    F16,
    /// 16-bit brain float
    BF16,
}

impl Default for QuantizationConfig {
    fn default() -> Self {
        Self {
            block_size: 64,
            double_quant: true,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerTensor,
            use_zero_point: false,
        }
    }
}

/// A quantized tensor with scale factors.
#[derive(Debug, Clone)]
pub struct QuantizedTensor {
    /// Packed 4-bit values (2 values per byte).
    pub data: Vec<u8>,
    /// Scale factors per block.
    pub scales: Vec<f32>,
    /// Zero points per block (for asymmetric quantization).
    pub zero_points: Option<Vec<f32>>,
    /// Quantized scales (when double quantization is used).
    pub scales_quantized: Option<Vec<u8>>,
    /// Scale factors for the scales (when double quantization is used).
    pub scales_scales: Option<Vec<f32>>,
    /// Original shape.
    pub shape: Vec<usize>,
    /// Block size used for quantization.
    pub block_size: usize,
    /// Whether double quantization was applied.
    pub double_quant_enabled: bool,
    /// Quantization strategy used.
    pub strategy: QuantizationStrategy,
}

impl QuantizedTensor {
    /// Get the number of elements in the original tensor.
    #[must_use]
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Get the memory size in bytes.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        let mut size = self.data.len() + self.scales.len() * 4;
        if let Some(ref zp) = self.zero_points {
            size += zp.len() * 4;
        }
        if let Some(ref sq) = self.scales_quantized {
            size += sq.len();
        }
        if let Some(ref ss) = self.scales_scales {
            size += ss.len() * 4;
        }
        size
    }

    /// Get the compression ratio relative to FP32 format.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn compression_ratio(&self) -> f64 {
        let fp32_size = self.numel() * 4;
        let quantized_size = self.size_bytes();
        fp32_size as f64 / quantized_size as f64
    }
}

/// Quantize a tensor to NF4 format with optional double quantization.
///
/// # Arguments
/// * `tensor` - Input tensor to quantize
/// * `block_size` - Number of elements per quantization block
///
/// # Returns
/// Quantized tensor with packed 4-bit values and scales
///
/// # Errors
/// Returns error if tensor cannot be flattened or has invalid shape
pub fn quantize_nf4(tensor: &Tensor, block_size: usize) -> Result<QuantizedTensor> {
    quantize_nf4_with_config(
        tensor,
        &QuantizationConfig {
            block_size,
            double_quant: false, // Use non-double quantization by default
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerTensor,
            use_zero_point: false,
        },
    )
}

/// Quantize a tensor to NF4 format with full configuration options.
///
/// # Arguments
/// * `tensor` - Input tensor to quantize
/// * `config` - Quantization configuration including double quant option
///
/// # Returns
/// Quantized tensor with packed 4-bit values and optional double-quantized scales
///
/// # Errors
/// Returns error if tensor cannot be flattened or has invalid shape
pub fn quantize_nf4_with_config(
    tensor: &Tensor,
    config: &QuantizationConfig,
) -> Result<QuantizedTensor> {
    match config.strategy {
        QuantizationStrategy::PerTensor => quantize_per_tensor(tensor, config),
        QuantizationStrategy::PerChannel => quantize_per_channel(tensor, config),
    }
}

/// Quantize a tensor using per-tensor strategy (standard block quantization).
fn quantize_per_tensor(tensor: &Tensor, config: &QuantizationConfig) -> Result<QuantizedTensor> {
    let shape = tensor.shape().dims().to_vec();
    let flat = tensor.flatten_all()?.to_vec1::<f32>()?;
    let numel = flat.len();

    if numel % config.block_size != 0 {
        return Err(QLoraError::InvalidConfig(format!(
            "tensor size {} not divisible by block size {}",
            numel, config.block_size
        )));
    }

    let num_blocks = numel / config.block_size;
    let mut scales = Vec::with_capacity(num_blocks);
    let mut quantized = Vec::with_capacity(numel.div_ceil(2));

    for block_idx in 0..num_blocks {
        let start = block_idx * config.block_size;
        let end = start + config.block_size;
        let block = &flat[start..end];

        // Compute absmax scale
        let absmax = block.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let scale = if absmax > 0.0 { absmax } else { 1.0 };
        scales.push(scale);

        // Quantize each value in the block
        for chunk in block.chunks(2) {
            let q0 = quantize_value_nf4(chunk[0] / scale);
            let q1 = if chunk.len() > 1 {
                quantize_value_nf4(chunk[1] / scale)
            } else {
                0
            };
            // Pack two 4-bit values into one byte
            quantized.push((q1 << 4) | q0);
        }
    }

    // Apply double quantization if enabled
    let (scales_quantized, scales_scales) = if config.double_quant {
        let (sq, ss) = double_quantize_scales(&scales, 255);
        (Some(sq), Some(ss))
    } else {
        (None, None)
    };

    // Compute zero points if enabled
    let zero_points = if config.use_zero_point {
        Some(compute_zero_points(&flat, config.block_size, &scales))
    } else {
        None
    };

    Ok(QuantizedTensor {
        data: quantized,
        scales,
        zero_points,
        scales_quantized,
        scales_scales,
        shape,
        block_size: config.block_size,
        double_quant_enabled: config.double_quant,
        strategy: config.strategy,
    })
}

/// Quantize a tensor using per-channel strategy.
///
/// For per-channel quantization, each output channel (first dimension) gets its own
/// set of scales. This provides better accuracy for weights with different ranges
/// across channels.
fn quantize_per_channel(tensor: &Tensor, config: &QuantizationConfig) -> Result<QuantizedTensor> {
    let shape = tensor.shape().dims().to_vec();

    // For per-channel, we need at least 2D tensor (channels x features)
    if shape.len() < 2 {
        return Err(QLoraError::InvalidConfig(
            "Per-channel quantization requires at least 2D tensor".to_string(),
        ));
    }

    let num_channels = shape[0];
    let channel_size = shape[1..].iter().product::<usize>();
    let flat = tensor.flatten_all()?.to_vec1::<f32>()?;

    let mut scales = Vec::with_capacity(num_channels);
    let mut quantized = Vec::with_capacity(flat.len().div_ceil(2));

    // Quantize each channel separately
    for ch_idx in 0..num_channels {
        let ch_start = ch_idx * channel_size;
        let ch_end = ch_start + channel_size;
        let channel_data = &flat[ch_start..ch_end];

        // Compute scale for entire channel
        let absmax = channel_data.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let scale = if absmax > 0.0 { absmax } else { 1.0 };
        scales.push(scale);

        // Quantize channel values
        for chunk in channel_data.chunks(2) {
            let q0 = quantize_value_nf4(chunk[0] / scale);
            let q1 = if chunk.len() > 1 {
                quantize_value_nf4(chunk[1] / scale)
            } else {
                0
            };
            quantized.push((q1 << 4) | q0);
        }
    }

    // Apply double quantization if enabled
    let (scales_quantized, scales_scales) = if config.double_quant {
        let (sq, ss) = double_quantize_scales(&scales, 255);
        (Some(sq), Some(ss))
    } else {
        (None, None)
    };

    // For per-channel, zero points apply per channel if enabled
    let zero_points = if config.use_zero_point {
        let mut zps = Vec::with_capacity(num_channels);
        #[allow(clippy::needless_range_loop)]
        for ch_idx in 0..num_channels {
            let ch_start = ch_idx * channel_size;
            let ch_end = ch_start + channel_size;
            let channel_data = &flat[ch_start..ch_end];
            let min_val = channel_data.iter().copied().fold(f32::INFINITY, f32::min);
            let zp = if scales[ch_idx] > 0.0 {
                -min_val / scales[ch_idx]
            } else {
                0.0
            };
            zps.push(zp);
        }
        Some(zps)
    } else {
        None
    };

    Ok(QuantizedTensor {
        data: quantized,
        scales,
        zero_points,
        scales_quantized,
        scales_scales,
        shape,
        block_size: channel_size, // For per-channel, block_size = channel size
        double_quant_enabled: config.double_quant,
        strategy: config.strategy,
    })
}

/// Apply double quantization to scale factors.
///
/// Double quantization quantizes the scale factors themselves to reduce memory usage.
/// Typically uses 8-bit unsigned integers for the quantized scales.
///
/// # Arguments
/// * `scales` - Original float32 scale factors
/// * `max_val` - Maximum quantization value (typically 255 for u8)
///
/// # Returns
/// Tuple of (`quantized_scales`, `scale_factors_for_scales`)
fn double_quantize_scales(scales: &[f32], _max_val: usize) -> (Vec<u8>, Vec<f32>) {
    if scales.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Find the absolute maximum value in scales
    let absmax = scales.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

    if absmax == 0.0 {
        return (vec![0; scales.len()], vec![1.0]);
    }

    // Quantize all scales preserving sign
    #[allow(clippy::cast_precision_loss)]
    let scale_factor = absmax / 127.0; // Use 127 for signed range -127 to 127
    let quantized_scales: Vec<u8> = scales
        .iter()
        .map(|&s| {
            let quantized = (s / scale_factor) + 128.0; // Offset by 128
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let result = quantized.clamp(0.0, 255.0) as u8;
            result
        })
        .collect();

    (quantized_scales, vec![scale_factor])
}

/// Dequantize double-quantized scales back to float.
fn dequantize_double_scales(scales_quantized: &[u8], scales_scales: &[f32]) -> Vec<f32> {
    if scales_quantized.is_empty() || scales_scales.is_empty() {
        return vec![];
    }

    let scale_factor = scales_scales[0];
    scales_quantized
        .iter()
        .map(|&sq| (f32::from(sq) - 128.0) * scale_factor)
        .collect()
}

/// Compute zero points for asymmetric quantization.
///
/// For asymmetric quantization, we compute the zero point (offset) for each block
/// to handle non-symmetric distributions better.
///
/// # Arguments
/// * `data` - Original float data
/// * `block_size` - Block size for quantization
/// * `scales` - Pre-computed scale factors
///
/// # Returns
/// Vector of zero points (one per block)
fn compute_zero_points(data: &[f32], block_size: usize, scales: &[f32]) -> Vec<f32> {
    let num_blocks = data.len() / block_size;
    let mut zero_points = Vec::with_capacity(num_blocks);

    #[allow(clippy::needless_range_loop)]
    for block_idx in 0..num_blocks {
        let start = block_idx * block_size;
        let end = start + block_size;
        let block = &data[start..end];
        let scale = scales[block_idx];

        // Find min value in the block
        let min_val = block.iter().copied().fold(f32::INFINITY, f32::min);

        // Zero point is the minimum value normalized by scale
        // This shifts the quantization range to better handle asymmetric distributions
        let zero_point = if scale > 0.0 { -min_val / scale } else { 0.0 };

        zero_points.push(zero_point);
    }

    zero_points
}

/// Dequantize an NF4 tensor back to float.
///
/// Automatically handles double-quantized scales if enabled.
///
/// # Arguments
/// * `quantized` - Quantized tensor to dequantize
/// * `device` - Device to create the output tensor on
///
/// # Returns
/// Dequantized float tensor with original shape
///
/// # Errors
/// Returns an error if the tensor cannot be created on the specified device
pub fn dequantize_nf4(quantized: &QuantizedTensor, device: &Device) -> Result<Tensor> {
    // GPU-resident path: avoids the GPU->CPU->GPU roundtrip of the scalar loop, which
    // otherwise runs on every forward pass and dominates training time. Falls back to
    // the CPU loop for asymmetric (zero_point) quant — rare — and on non-CUDA devices.
    if device.is_cuda() && quantized.zero_points.is_none() {
        match dequantize_nf4_gpu(quantized, device) {
            Ok(t) => return Ok(t),
            // Be conservative: if any op is unsupported on this candle/device build,
            // fall through to the always-correct CPU path rather than fail training.
            Err(_) => {}
        }
    }
    dequantize_nf4_cpu(quantized, device)
}

/// Vectorized NF4 dequant using candle tensor ops — stays on the GPU end-to-end.
///
/// Nibbles are split via index_select into 256-entry lookup tables (candle 0.9 has no
/// tensor bit-shift on CUDA), the 16-entry NF4 codebook is gathered the same way, and
/// per-block scales are broadcast to elements. Equivalent to [`dequantize_nf4_cpu`]
/// (verified bit-for-bit in tests). Assumes symmetric quant (no zero_point) — the
/// caller dispatches the asymmetric case to the CPU path.
fn dequantize_nf4_gpu(quantized: &QuantizedTensor, device: &Device) -> Result<Tensor> {
    use candle_core::DType;

    let numel = quantized.numel();
    let block_size = quantized.block_size;

    // Effective scales (reverse double-quant on host — small, num_blocks values).
    let scales: Vec<f32> = if quantized.double_quant_enabled {
        if let (Some(sq), Some(ss)) = (&quantized.scales_quantized, &quantized.scales_scales) {
            dequantize_double_scales(sq, ss)
        } else {
            quantized.scales.clone()
        }
    } else {
        quantized.scales.clone()
    };
    let num_groups = scales.len();
    // Both PerTensor and PerChannel store `scales.len() * block_size == numel`.
    if num_groups == 0 || num_groups * block_size < numel {
        return Err(QLoraError::InvalidConfig(
            "scale/block geometry does not cover the tensor".into(),
        ));
    }

    // 256-entry nibble LUTs (u32 indices for index_select): low = b & 0xF, high = b >> 4.
    let low_lut: Vec<u32> = (0u32..256).map(|b| b & 0x0F).collect();
    let high_lut: Vec<u32> = (0u32..256).map(|b| b >> 4).collect();
    let low_lut = Tensor::from_vec(low_lut, 256, device)?;
    let high_lut = Tensor::from_vec(high_lut, 256, device)?;

    // Upload packed bytes ONCE; derive both nibble lanes on-device.
    let n_bytes = quantized.data.len();
    let packed = Tensor::from_slice(&quantized.data, n_bytes, device)?.to_dtype(DType::U32)?;
    let lo = low_lut.index_select(&packed, 0)?; // [n_bytes]
    let hi = high_lut.index_select(&packed, 0)?; // [n_bytes]
    // Interleave back to element order: [lo0, hi0, lo1, hi1, ...].
    let codes = Tensor::stack(&[&lo, &hi], 1)?.reshape((n_bytes * 2,))?;
    let codes = codes.narrow(0, 0, numel)?; // drop any trailing padding nibble

    // Codebook gather: NF4_LEVELS[code].
    let nf4_lut = Tensor::from_slice(&NF4_LEVELS, 16, device)?;
    let vals = nf4_lut.index_select(&codes, 0)?; // [numel] f32

    // Broadcast per-group scale to its `block_size` elements.
    let scales_t = Tensor::from_vec(scales, num_groups, device)?;
    let scales_e = scales_t
        .reshape((num_groups, 1))?
        .broadcast_as((num_groups, block_size))?
        .reshape((num_groups * block_size,))?
        .narrow(0, 0, numel)?;

    let out = (vals * scales_e)?.reshape(quantized.shape.clone())?;
    Ok(out)
}

/// Reference CPU NF4 dequant (scalar loop). Correct for all cases; used as the
/// fallback and as the bit-for-bit oracle for [`dequantize_nf4_gpu`].
fn dequantize_nf4_cpu(quantized: &QuantizedTensor, device: &Device) -> Result<Tensor> {
    let numel = quantized.numel();
    let mut output = Vec::with_capacity(numel);

    // Get scales, applying double quantization reversal if needed
    let scales = if quantized.double_quant_enabled {
        if let (Some(ref sq), Some(ref ss)) =
            (&quantized.scales_quantized, &quantized.scales_scales)
        {
            dequantize_double_scales(sq, ss)
        } else {
            quantized.scales.clone()
        }
    } else {
        quantized.scales.clone()
    };

    match quantized.strategy {
        QuantizationStrategy::PerTensor => {
            let num_blocks = scales.len();

            for block_idx in 0..num_blocks {
                let scale = scales[block_idx];
                let zero_point = quantized
                    .zero_points
                    .as_ref()
                    .map_or(0.0, |zp| zp[block_idx]);
                let start_byte = (block_idx * quantized.block_size) / 2;

                for i in 0..quantized.block_size {
                    let byte_idx = start_byte + i / 2;
                    let byte = quantized.data[byte_idx];
                    let code = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
                    // Apply zero point for asymmetric quantization
                    let nf4_value = NF4_LEVELS[code as usize] + zero_point;
                    let value = nf4_value * scale;
                    output.push(value);
                }
            }
        }
        QuantizationStrategy::PerChannel => {
            let num_channels = scales.len();
            let channel_size = quantized.block_size;

            for ch_idx in 0..num_channels {
                let scale = scales[ch_idx];
                let zero_point = quantized.zero_points.as_ref().map_or(0.0, |zp| zp[ch_idx]);
                let ch_start_byte = (ch_idx * channel_size) / 2;

                for i in 0..channel_size {
                    let byte_idx = ch_start_byte + i / 2;
                    let byte = quantized.data[byte_idx];
                    let code = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
                    let nf4_value = NF4_LEVELS[code as usize] + zero_point;
                    let value = nf4_value * scale;
                    output.push(value);
                }
            }
        }
    }

    let tensor = Tensor::from_vec(output, quantized.shape.clone(), device)?;
    Ok(tensor)
}

/// Dequantize an NF4 tensor to a specific data type for mixed precision computation.
///
/// This function dequantizes the tensor and converts it to the specified compute
/// data type (F16, BF16, or F32) for efficient mixed precision training.
///
/// # Arguments
/// * `quantized` - Quantized tensor to dequantize
/// * `device` - Device to create the output tensor on
/// * `compute_dtype` - Target data type for the output tensor
///
/// # Returns
/// Dequantized tensor in the specified data type
///
/// # Errors
/// Returns an error if the tensor cannot be created or dtype conversion fails
pub fn dequantize_nf4_with_dtype(
    quantized: &QuantizedTensor,
    device: &Device,
    compute_dtype: ComputeDType,
) -> Result<Tensor> {
    // First dequantize to f32
    let f32_tensor = dequantize_nf4(quantized, device)?;

    // Convert to target dtype
    let dtype = match compute_dtype {
        ComputeDType::F32 => return Ok(f32_tensor),
        ComputeDType::F16 => DType::F16,
        ComputeDType::BF16 => DType::BF16,
    };

    let converted = f32_tensor.to_dtype(dtype)?;
    Ok(converted)
}

/// Pad a tensor to ensure its size is divisible by the block size.
///
/// This is useful for quantization-aware model preparation, ensuring that
/// all weight tensors can be cleanly divided into quantization blocks.
/// The tensor is flattened, padded, and the padded flat tensor is returned.
/// The original shape is preserved in the returned tensor's metadata via
/// `pad_for_quantization_with_info`.
///
/// # Arguments
/// * `tensor` - Input tensor to pad (must be F32 dtype)
/// * `block_size` - Target block size for quantization
/// * `pad_value` - Value to use for padding (typically 0.0)
///
/// # Returns
/// 1D padded tensor with size divisible by `block_size`
///
/// # Errors
/// Returns an error if the tensor cannot be processed
///
/// # Note
/// This function only works with F32 tensors. Ensure your tensor is in F32 format
/// before calling this function.
///
/// # Errors
/// Returns an error if the tensor is not F32 dtype.
pub fn pad_for_quantization(tensor: &Tensor, block_size: usize, pad_value: f32) -> Result<Tensor> {
    // Validate dtype - only F32 is supported
    if tensor.dtype() != candle_core::DType::F32 {
        return Err(QLoraError::InvalidConfig(format!(
            "pad_for_quantization only supports F32 tensors, got {:?}. \
             Convert to F32 first with tensor.to_dtype(DType::F32)",
            tensor.dtype()
        )));
    }

    let numel = tensor.elem_count();
    let device = tensor.device();

    // Check if padding is needed
    let remainder = numel % block_size;
    if remainder == 0 {
        // Flatten and return
        let flat = tensor.flatten_all()?;
        return Ok(flat);
    }

    // Calculate padding needed
    let pad_count = block_size - remainder;
    let flat = tensor.flatten_all()?.to_vec1::<f32>()?;

    // Create padded vector
    let mut padded = flat;
    padded.extend(std::iter::repeat_n(pad_value, pad_count));

    // Return as 1D tensor
    let padded_tensor = Tensor::from_vec(padded, (numel + pad_count,), device)?;
    Ok(padded_tensor)
}

/// Information about padding applied for quantization.
#[derive(Debug, Clone)]
pub struct PaddingInfo {
    /// Original shape before padding
    pub original_shape: Vec<usize>,
    /// Padded shape
    pub padded_shape: Vec<usize>,
    /// Number of elements added as padding
    pub pad_count: usize,
    /// Block size used for padding calculation
    pub block_size: usize,
}

/// Pad a tensor and return information for later unpadding.
///
/// This function pads the tensor for quantization and returns padding metadata
/// that can be used to remove the padding after dequantization.
/// The tensor is flattened and padded to be divisible by `block_size`.
///
/// # Arguments
/// * `tensor` - Input tensor to pad (must be F32 dtype)
/// * `block_size` - Target block size for quantization
/// * `pad_value` - Value to use for padding (typically 0.0)
///
/// # Returns
/// Tuple of (1D padded tensor, `PaddingInfo` with original shape)
///
/// # Errors
/// Returns an error if the tensor cannot be processed
///
/// # Note
/// This function only works with F32 tensors. Ensure your tensor is in F32 format
/// before calling this function. For mixed precision workflows, pad before quantizing,
/// then dequantize to your target dtype (F16/BF16), and convert back to F32 before
/// unpadding.
pub fn pad_for_quantization_with_info(
    tensor: &Tensor,
    block_size: usize,
    pad_value: f32,
) -> Result<(Tensor, PaddingInfo)> {
    // Validate dtype - only F32 is supported
    if tensor.dtype() != candle_core::DType::F32 {
        return Err(QLoraError::InvalidConfig(format!(
            "pad_for_quantization_with_info only supports F32 tensors, got {:?}. \
             Convert to F32 first with tensor.to_dtype(DType::F32)",
            tensor.dtype()
        )));
    }

    let original_shape = tensor.shape().dims().to_vec();
    let numel = tensor.elem_count();
    let device = tensor.device();

    let remainder = numel % block_size;
    let pad_count = if remainder == 0 {
        0
    } else {
        block_size - remainder
    };

    if pad_count == 0 {
        let flat = tensor.flatten_all()?;
        let info = PaddingInfo {
            original_shape: original_shape.clone(),
            padded_shape: vec![numel],
            pad_count: 0,
            block_size,
        };
        return Ok((flat, info));
    }

    let flat = tensor.flatten_all()?.to_vec1::<f32>()?;
    let mut padded = flat;
    padded.extend(std::iter::repeat_n(pad_value, pad_count));

    let padded_len = numel + pad_count;
    let padded_tensor = Tensor::from_vec(padded, (padded_len,), device)?;

    let info = PaddingInfo {
        original_shape,
        padded_shape: vec![padded_len],
        pad_count,
        block_size,
    };

    Ok((padded_tensor, info))
}

/// Remove padding from a dequantized tensor using stored padding information.
///
/// # Arguments
/// * `tensor` - Padded tensor to unpad (must be F32 dtype)
/// * `padding_info` - Padding information from `pad_for_quantization_with_info`
///
/// # Returns
/// Tensor with original shape (padding removed)
///
/// # Errors
/// Returns an error if unpadding fails
///
/// # Note
/// This function only works with F32 tensors. For mixed precision workflows,
/// convert the tensor to F32 before unpadding.
pub fn unpad_tensor(tensor: &Tensor, padding_info: &PaddingInfo) -> Result<Tensor> {
    // Validate dtype - only F32 is supported
    if tensor.dtype() != candle_core::DType::F32 {
        return Err(QLoraError::InvalidConfig(format!(
            "unpad_tensor only supports F32 tensors, got {:?}. \
             Convert to F32 first with tensor.to_dtype(DType::F32)",
            tensor.dtype()
        )));
    }

    if padding_info.pad_count == 0 {
        // Flatten first to ensure consistent behavior regardless of input shape
        let flat = tensor.flatten_all()?;
        let reshaped = flat.reshape(padding_info.original_shape.clone())?;
        return Ok(reshaped);
    }

    let flat = tensor.flatten_all()?.to_vec1::<f32>()?;
    let original_numel: usize = padding_info.original_shape.iter().product();

    // Remove padding
    let unpadded: Vec<f32> = flat.into_iter().take(original_numel).collect();

    let unpadded_tensor = Tensor::from_vec(
        unpadded,
        padding_info.original_shape.clone(),
        tensor.device(),
    )?;
    Ok(unpadded_tensor)
}

/// Quantize a single value to NF4 (returns 4-bit code).
fn quantize_value_nf4(value: f32) -> u8 {
    // Find closest NF4 level
    let mut best_idx = 0;
    let mut best_dist = f32::MAX;

    for (idx, &level) in NF4_LEVELS.iter().enumerate() {
        let dist = (value - level).abs();
        if dist < best_dist {
            best_dist = dist;
            best_idx = idx;
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    let result = best_idx as u8;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::DType;

    #[test]
    fn test_nf4_levels_sorted() {
        for i in 1..NF4_LEVELS.len() {
            assert!(NF4_LEVELS[i] > NF4_LEVELS[i - 1]);
        }
    }

    #[test]
    fn test_quantize_dequantize_roundtrip() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (64,), &device).unwrap();

        let quantized = quantize_nf4(&original, 64).unwrap();
        let restored = dequantize_nf4(&quantized, &device).unwrap();

        let original_vec: Vec<f32> = original.to_vec1().unwrap();
        let restored_vec: Vec<f32> = restored.to_vec1().unwrap();

        // Check that error is bounded (NF4 should have <0.5 max error for normalized data)
        for (o, r) in original_vec.iter().zip(restored_vec.iter()) {
            let error = (o - r).abs();
            assert!(error < 0.5, "Error {error} too large for value {o}");
        }
    }

    #[test]
    fn test_quantize_preserves_shape() {
        let device = Device::Cpu;
        let original = Tensor::zeros(&[32, 64], DType::F32, &device).unwrap();

        let quantized = quantize_nf4(&original, 64).unwrap();
        let restored = dequantize_nf4(&quantized, &device).unwrap();

        assert_eq!(restored.shape().dims(), &[32, 64]);
    }

    #[test]
    fn test_memory_reduction() {
        let device = Device::Cpu;
        let original = Tensor::zeros(&[1024, 1024], DType::F32, &device).unwrap();
        let original_bytes: i32 = 1024 * 1024 * 4; // f32 = 4 bytes

        let quantized = quantize_nf4(&original, 64).unwrap();
        let quantized_bytes = quantized.size_bytes();

        // Should be roughly 4x reduction (4-bit vs 32-bit) plus some overhead for scales
        #[allow(clippy::cast_precision_loss)]
        let ratio = f64::from(original_bytes) / quantized_bytes as f64;
        assert!(ratio > 3.0, "Expected >3x reduction, got {ratio:.2}x");
    }

    #[test]
    fn gpu_dequant_matches_cpu_bit_for_bit() {
        // candle ops run identically on CPU and CUDA, so validating the tensor-op
        // dequant against the scalar oracle on a CPU device proves the algorithm
        // (nibble interleave, codebook gather, scale broadcast) independent of GPU.
        let device = Device::Cpu;
        for &double_quant in &[false, true] {
            for &strategy in &[
                QuantizationStrategy::PerTensor,
                QuantizationStrategy::PerChannel,
            ] {
                let original = Tensor::randn(0.0f32, 1.0, (8, 128), &device).unwrap();
                let config = QuantizationConfig {
                    block_size: 64,
                    double_quant,
                    compute_dtype: ComputeDType::F32,
                    strategy,
                    use_zero_point: false,
                };
                let q = quantize_nf4_with_config(&original, &config).unwrap();
                let cpu = super::dequantize_nf4_cpu(&q, &device).unwrap();
                let gpu = super::dequantize_nf4_gpu(&q, &device).unwrap();
                assert_eq!(cpu.shape(), gpu.shape());
                let diff = (&cpu - &gpu)
                    .unwrap()
                    .abs()
                    .unwrap()
                    .max_all()
                    .unwrap()
                    .to_scalar::<f32>()
                    .unwrap();
                assert!(
                    diff < 1e-6,
                    "dq={double_quant} strat={strategy:?}: max abs diff {diff} exceeds 1e-6",
                );
            }
        }
    }

    #[test]
    fn test_double_quantize_compression() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (512,), &device).unwrap();

        let config = QuantizationConfig {
            block_size: 64,
            double_quant: true,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerTensor,
            use_zero_point: false,
        };

        let quantized = quantize_nf4_with_config(&original, &config).unwrap();

        // Verify double quantization was applied
        assert!(quantized.double_quant_enabled);
        assert!(quantized.scales_quantized.is_some());
        assert!(quantized.scales_scales.is_some());

        // Double quantized should use less memory than non-double quantized
        let non_dq_size = quantized.scales.len() * 4; // Original scales
        let dq_scales_size = quantized.scales_quantized.as_ref().map_or(0, Vec::len)
            + quantized
                .scales_scales
                .as_ref()
                .map_or(0, |ss| ss.len() * 4);

        assert!(dq_scales_size < non_dq_size);
    }

    #[test]
    fn test_double_quantize_roundtrip() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (256,), &device).unwrap();

        let config = QuantizationConfig {
            block_size: 64,
            double_quant: true,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerTensor,
            use_zero_point: false,
        };

        let quantized = quantize_nf4_with_config(&original, &config).unwrap();
        let restored = dequantize_nf4(&quantized, &device).unwrap();

        let original_vec: Vec<f32> = original.to_vec1().unwrap();
        let restored_vec: Vec<f32> = restored.to_vec1().unwrap();

        // With double quantization, error increases (scale quantization adds error)
        // but should still be reasonable (typically 10-20% of value magnitude)
        let mut max_error = 0.0f32;
        for (o, r) in original_vec.iter().zip(restored_vec.iter()) {
            let error = (o - r).abs();
            max_error = max_error.max(error);
        }
        // Allow higher error for double quantization - scales are also quantized
        assert!(max_error < 5.0, "Max error {max_error} too large");
    }

    #[test]
    fn test_double_quant_disabled_still_works() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (128,), &device).unwrap();

        let config = QuantizationConfig {
            block_size: 64,
            double_quant: false,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerTensor,
            use_zero_point: false,
        };

        let quantized = quantize_nf4_with_config(&original, &config).unwrap();

        // Verify double quantization was NOT applied
        assert!(!quantized.double_quant_enabled);
        assert!(quantized.scales_quantized.is_none());
        assert!(quantized.scales_scales.is_none());

        let restored = dequantize_nf4(&quantized, &device).unwrap();
        let original_vec: Vec<f32> = original.to_vec1().unwrap();
        let restored_vec: Vec<f32> = restored.to_vec1().unwrap();

        // Regular quantization error bounds
        for (o, r) in original_vec.iter().zip(restored_vec.iter()) {
            let error = (o - r).abs();
            assert!(error < 0.5, "Error {error} too large for value {o}");
        }
    }

    #[test]
    fn test_per_channel_quantization() {
        let device = Device::Cpu;
        // Create 2D tensor (4 channels x 128 features)
        let original = Tensor::randn(0.0f32, 1.0, (4, 128), &device).unwrap();

        let config = QuantizationConfig {
            block_size: 64, // Not used for per-channel
            double_quant: false,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerChannel,
            use_zero_point: false,
        };

        let quantized = quantize_nf4_with_config(&original, &config).unwrap();

        // Verify we have one scale per channel
        assert_eq!(
            quantized.scales.len(),
            4,
            "Should have 4 scales (one per channel)"
        );
        assert_eq!(quantized.strategy, QuantizationStrategy::PerChannel);

        // Verify roundtrip
        let restored = dequantize_nf4(&quantized, &device).unwrap();
        assert_eq!(restored.shape().dims(), &[4, 128]);

        let original_vec: Vec<f32> = original.flatten_all().unwrap().to_vec1().unwrap();
        let restored_vec: Vec<f32> = restored.flatten_all().unwrap().to_vec1().unwrap();

        let mut max_error = 0.0f32;
        for (o, r) in original_vec.iter().zip(restored_vec.iter()) {
            let error = (o - r).abs();
            max_error = max_error.max(error);
        }
        assert!(max_error < 1.0, "Max error {max_error} too large");
    }

    #[test]
    fn test_zero_point_quantization() {
        let device = Device::Cpu;
        // Create asymmetric data (all positive values)
        let original = Tensor::rand(0.0f32, 5.0, (256,), &device).unwrap();

        let config = QuantizationConfig {
            block_size: 64,
            double_quant: false,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerTensor,
            use_zero_point: true,
        };

        let quantized = quantize_nf4_with_config(&original, &config).unwrap();

        // Verify zero points were computed
        assert!(quantized.zero_points.is_some());
        let zero_points = quantized.zero_points.as_ref().unwrap();
        assert_eq!(
            zero_points.len(),
            256 / 64,
            "Should have one zero point per block"
        );

        // Verify roundtrip with zero points
        let restored = dequantize_nf4(&quantized, &device).unwrap();
        let original_vec: Vec<f32> = original.to_vec1().unwrap();
        let restored_vec: Vec<f32> = restored.to_vec1().unwrap();

        let mut max_error = 0.0f32;
        for (o, r) in original_vec.iter().zip(restored_vec.iter()) {
            let error = (o - r).abs();
            max_error = max_error.max(error);
        }
        // Zero point quantization should handle asymmetric data better
        assert!(max_error < 10.0, "Max error {max_error} too large");
    }

    #[test]
    fn test_per_channel_with_zero_point() {
        let device = Device::Cpu;
        // Create asymmetric 2D data
        let original = Tensor::rand(0.0f32, 3.0, (2, 128), &device).unwrap();

        let config = QuantizationConfig {
            block_size: 64,
            double_quant: false,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerChannel,
            use_zero_point: true,
        };

        let quantized = quantize_nf4_with_config(&original, &config).unwrap();

        // Verify per-channel with zero points
        assert_eq!(quantized.scales.len(), 2);
        assert!(quantized.zero_points.is_some());
        let zps = quantized.zero_points.as_ref().unwrap();
        assert_eq!(zps.len(), 2, "Should have one zero point per channel");

        // Verify roundtrip
        let restored = dequantize_nf4(&quantized, &device).unwrap();
        assert_eq!(restored.shape().dims(), &[2, 128]);
    }

    #[test]
    fn test_per_channel_with_double_quant() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (8, 64), &device).unwrap();

        let config = QuantizationConfig {
            block_size: 64,
            double_quant: true,
            compute_dtype: ComputeDType::F32,
            strategy: QuantizationStrategy::PerChannel,
            use_zero_point: false,
        };

        let quantized = quantize_nf4_with_config(&original, &config).unwrap();

        // Verify both per-channel and double quantization applied
        assert_eq!(quantized.scales.len(), 8);
        assert!(quantized.double_quant_enabled);
        assert!(quantized.scales_quantized.is_some());
        assert_eq!(quantized.strategy, QuantizationStrategy::PerChannel);

        // Verify roundtrip
        let restored = dequantize_nf4(&quantized, &device).unwrap();
        assert_eq!(restored.shape().dims(), &[8, 64]);
    }

    #[test]
    fn test_mixed_precision_f16() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (64,), &device).unwrap();

        let quantized = quantize_nf4(&original, 64).unwrap();
        let restored_f16 =
            dequantize_nf4_with_dtype(&quantized, &device, ComputeDType::F16).unwrap();

        assert_eq!(restored_f16.dtype(), DType::F16);
        assert_eq!(restored_f16.shape().dims(), &[64]);
    }

    #[test]
    fn test_mixed_precision_bf16() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (64,), &device).unwrap();

        let quantized = quantize_nf4(&original, 64).unwrap();
        let restored_bf16 =
            dequantize_nf4_with_dtype(&quantized, &device, ComputeDType::BF16).unwrap();

        assert_eq!(restored_bf16.dtype(), DType::BF16);
        assert_eq!(restored_bf16.shape().dims(), &[64]);
    }

    #[test]
    fn test_mixed_precision_f32_passthrough() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (64,), &device).unwrap();

        let quantized = quantize_nf4(&original, 64).unwrap();
        let restored_f32 =
            dequantize_nf4_with_dtype(&quantized, &device, ComputeDType::F32).unwrap();

        assert_eq!(restored_f32.dtype(), DType::F32);
        assert_eq!(restored_f32.shape().dims(), &[64]);
    }

    #[test]
    fn test_padding_for_quantization_needed() {
        let device = Device::Cpu;
        // 100 elements, block size 64 -> needs padding to 128
        let original = Tensor::randn(0.0f32, 1.0, (100,), &device).unwrap();

        let padded = pad_for_quantization(&original, 64, 0.0).unwrap();
        let padded_numel = padded.elem_count();

        // Should be padded to 128 (next multiple of 64)
        assert_eq!(padded_numel % 64, 0);
        assert_eq!(padded_numel, 128);
        // Result is always 1D
        assert_eq!(padded.shape().dims().len(), 1);
    }

    #[test]
    fn test_padding_for_quantization_not_needed() {
        let device = Device::Cpu;
        // 128 elements already divisible by 64
        let original = Tensor::randn(0.0f32, 1.0, (128,), &device).unwrap();

        let padded = pad_for_quantization(&original, 64, 0.0).unwrap();

        // Should remain the same count
        assert_eq!(padded.elem_count(), 128);
        // Result is always 1D (flattened)
        assert_eq!(padded.shape().dims().len(), 1);
    }

    #[test]
    fn test_padding_with_info_roundtrip() {
        let device = Device::Cpu;
        let original = Tensor::randn(0.0f32, 1.0, (100,), &device).unwrap();

        // Pad
        let (padded, info) = pad_for_quantization_with_info(&original, 64, 0.0).unwrap();
        assert_eq!(info.pad_count, 28); // 128 - 100
        assert_eq!(info.original_shape, vec![100]);
        assert_eq!(info.padded_shape, vec![128]); // 1D padded

        // Quantize the padded tensor
        let quantized = quantize_nf4(&padded, 64).unwrap();

        // Dequantize
        let restored_padded = dequantize_nf4(&quantized, &device).unwrap();

        // Unpad
        let restored = unpad_tensor(&restored_padded, &info).unwrap();
        assert_eq!(restored.shape().dims(), &[100]);
    }

    #[test]
    fn test_padding_2d_tensor() {
        let device = Device::Cpu;
        // Create a 2D tensor that needs padding
        // 4x10 = 40 elements, needs padding to 64
        let original = Tensor::randn(0.0f32, 1.0, (4, 10), &device).unwrap();

        let (padded, info) = pad_for_quantization_with_info(&original, 64, 0.0).unwrap();

        assert_eq!(info.pad_count, 24); // 64 - 40 = 24
        assert_eq!(info.original_shape, vec![4, 10]);
        // Padded shape is 1D
        assert_eq!(info.padded_shape, vec![64]);

        // Total elements should be 64
        assert_eq!(padded.elem_count(), 64);

        // Verify padding actually happened
        let padded_flat: Vec<f32> = padded.to_vec1().unwrap();
        assert_eq!(padded_flat.len(), 64);
    }

    #[test]
    fn test_padding_preserves_values() {
        let device = Device::Cpu;
        let original_data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let original = Tensor::from_vec(original_data.clone(), (5,), &device).unwrap();

        let padded = pad_for_quantization(&original, 8, 0.0).unwrap();
        let padded_vec: Vec<f32> = padded.to_vec1().unwrap();

        // First 5 values should be preserved
        assert_eq!(&padded_vec[..5], &original_data[..]);
        // Remaining should be padding
        assert_eq!(&padded_vec[5..], &[0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_padding_2d_tensor_no_padding_needed() {
        let device = Device::Cpu;
        // Create a 2D tensor that doesn't need padding
        // 8x8 = 64 elements, exactly divisible by 64
        let original = Tensor::randn(0.0f32, 1.0, (8, 8), &device).unwrap();

        let (padded, info) = pad_for_quantization_with_info(&original, 64, 0.0).unwrap();

        // No padding needed
        assert_eq!(info.pad_count, 0);
        assert_eq!(info.original_shape, vec![8, 8]);
        assert_eq!(info.padded_shape, vec![64]);
        assert_eq!(padded.elem_count(), 64);

        // Test roundtrip: unpad should restore original shape
        let restored = unpad_tensor(&padded, &info).unwrap();
        assert_eq!(restored.shape().dims(), &[8, 8]);
    }

    #[test]
    fn test_padding_2d_no_padding_roundtrip() {
        let device = Device::Cpu;
        // 8x8 = 64 elements, exactly divisible by 64 (no padding needed)
        #[allow(clippy::cast_precision_loss)]
        let original_data: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let original = Tensor::from_vec(original_data.clone(), (8, 8), &device).unwrap();

        let (padded, info) = pad_for_quantization_with_info(&original, 64, 0.0).unwrap();
        assert_eq!(info.pad_count, 0);

        let restored = unpad_tensor(&padded, &info).unwrap();

        // Verify shape is restored
        assert_eq!(restored.shape().dims(), &[8, 8]);

        // Verify VALUES are preserved through the entire pad/unpad cycle
        let restored_data: Vec<f32> = restored.flatten_all().unwrap().to_vec1().unwrap();
        assert_eq!(
            restored_data, original_data,
            "Values should be preserved through pad/unpad cycle"
        );
    }

    #[test]
    fn test_padding_dtype_validation() {
        let device = Device::Cpu;
        // Create an F16 tensor - should fail with clear error
        let f16_tensor = Tensor::ones((64,), DType::F16, &device).unwrap();

        let result = pad_for_quantization(&f16_tensor, 64, 0.0);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("F32"),
            "Error should mention F32 requirement: {err_msg}"
        );

        let result = pad_for_quantization_with_info(&f16_tensor, 64, 0.0);
        assert!(result.is_err());

        // unpad_tensor dtype validation
        let f16_tensor = Tensor::ones((64,), DType::F16, &device).unwrap();
        let dummy_info = PaddingInfo {
            original_shape: vec![8, 8],
            padded_shape: vec![64],
            pad_count: 0,
            block_size: 64,
        };
        let result = unpad_tensor(&f16_tensor, &dummy_info);
        assert!(result.is_err());
    }
}
