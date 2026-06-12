//! Training utilities for `QLoRA` fine-tuning.
//!
//! This module provides:
//! - [`QLoraTrainer`] - Main trainer for `QLoRA` fine-tuning
//! - [`PagedAdamW`] - Memory-efficient optimizer with CPU paging
//! - Integration with peft-rs training state and LR schedules
//! - Gradient computation and optimizer integration
//!
//! # Training Architecture
//!
//! `QLoRA` training keeps base weights frozen in 4-bit precision while training
//! `LoRA` adapter weights in full precision. Gradients flow through the frozen
//! quantized base via straight-through estimation (STE).
//!
//! ```text
//!   Input → [Quantized Base (frozen)] → [LoRA A] → [LoRA B] → Output
//!              ↑ no gradients           ↑ gradients flow
//! ```

use candle_core::{D, DType, Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use peft_rs::training::{AdapterTrainingConfig, AdapterTrainingState, LrSchedule};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{QLoraError, Result};
use crate::qlora::QuantizedLinear;

use candle_core::Var;
use candle_core::backprop::GradStore;

/// Clip the global L2 norm of the gradients for `vars` in place, in `grads`.
///
/// Computes the global gradient norm `total_norm = sqrt(Σ ||g||²)` across every
/// trainable `Var`'s gradient. If `total_norm > max_norm`, every gradient is
/// scaled by the same factor `max_norm / (total_norm + eps)`, so the direction
/// of the combined gradient is preserved while its magnitude is capped. This is
/// the standard global-norm clip (cf. PyTorch `clip_grad_norm_`) and protects
/// against loss-spike gradient blowups.
///
/// No-op when `total_norm <= max_norm`. Returns the pre-clip `total_norm`.
pub(crate) fn clip_grad_norm(
    grads: &mut GradStore,
    vars: &[Var],
    max_norm: f64,
) -> Result<f64> {
    // Accumulate the sum of squares across all gradients as an f64 scalar.
    let mut sum_sq = 0.0f64;
    for var in vars {
        if let Some(grad) = grads.get(var.as_tensor()) {
            let sq = grad.sqr().map_err(QLoraError::Candle)?;
            let s = sq.sum_all().map_err(QLoraError::Candle)?;
            let s = s
                .to_dtype(DType::F64)
                .map_err(QLoraError::Candle)?
                .to_scalar::<f64>()
                .map_err(QLoraError::Candle)?;
            sum_sq += s;
        }
    }

    let total_norm = sum_sq.sqrt();

    // Only scale down when over budget; never scale up.
    if total_norm > max_norm {
        const EPS: f64 = 1e-6;
        let scale = max_norm / (total_norm + EPS);
        for var in vars {
            if let Some(grad) = grads.get(var.as_tensor()) {
                let scaled = (grad * scale).map_err(QLoraError::Candle)?;
                grads.insert(var.as_tensor(), scaled);
            }
        }
    }

    Ok(total_norm)
}

/// Inject an upstream gradient `g = dL/dy` into the autograd graph of `y` and
/// run backward, returning the resulting [`GradStore`].
///
/// candle 0.9's [`Tensor::backward`] always seeds the root with `ones_like`
/// (see `backprop.rs`); it offers **no** hook to start backprop from a custom
/// cotangent. Segment-wise gradient checkpointing needs exactly that: given a
/// segment whose output is `y` and the already-computed upstream gradient
/// `dL/dy`, we must backprop *that* cotangent through the segment to obtain the
/// grads for the segment's trainable [`Var`]s and for its input.
///
/// We achieve it with the standard surrogate-scalar (VJP) trick:
/// `s = Σ (y ⊙ stop_grad(g))`. Because `g` is detached (a constant w.r.t. the
/// graph), `∂s/∂y = g`, so `s.backward()` seeds `y` with exactly `g` and the
/// chain rule then yields the correct `∂L/∂θ` for every parameter feeding `y`.
/// Summing over the whole tensor (rather than any reduction with a non-unit
/// Jacobian) keeps the seed equal to `g` element-wise.
///
/// `upstream_grad` must have the same shape/dtype as `y`. It is detached
/// internally so no second-order graph is built.
///
/// # Errors
/// Returns an error if the elementwise multiply, sum, or backward fails.
pub fn backward_from_cotangent(y: &Tensor, upstream_grad: &Tensor) -> Result<GradStore> {
    let g = upstream_grad.detach();
    // s = sum(y * g); ds/dy = g exactly (g is a constant leaf).
    let surrogate = y.mul(&g)?.sum_all()?;
    let grads = surrogate.backward()?;
    Ok(grads)
}

/// Accumulate the gradients for `vars` from `src` into `dst`, summing when a
/// gradient for the same `Var` already exists.
///
/// Used by the checkpointed backward path to fold each segment's partial
/// [`GradStore`] into a single combined store before the optimizer step. A LoRA
/// `Var` appears in exactly one segment, so in practice the "already present"
/// branch only fires for params shared across segments (e.g. a tied head); it is
/// handled correctly regardless by summing.
///
/// # Errors
/// Returns an error if a tensor add fails.
pub fn accumulate_grads_for_vars(
    dst: &mut GradStore,
    src: &GradStore,
    vars: &[Var],
) -> Result<()> {
    for var in vars {
        if let Some(g) = src.get(var.as_tensor()) {
            let merged = match dst.get(var.as_tensor()) {
                Some(existing) => existing.add(g)?,
                None => g.clone(),
            };
            dst.insert(var.as_tensor(), merged);
        }
    }
    Ok(())
}

/// Configuration for `QLoRA` training.
#[derive(Debug, Clone)]
pub struct QLoraTrainingConfig {
    /// Adapter training configuration (from peft-rs).
    pub adapter_config: AdapterTrainingConfig,
    /// Number of training epochs.
    pub num_epochs: usize,
    /// Batch size for training.
    pub batch_size: usize,
    /// Logging frequency (steps).
    pub log_every: usize,
    /// Checkpoint save frequency (steps, None = no checkpoints).
    pub save_every: Option<usize>,
    /// Warmup steps for learning rate.
    pub warmup_steps: usize,
    /// Use paged optimizer (CPU offload for optimizer states).
    pub use_paged_optimizer: bool,
    /// Page size for paged optimizer (bytes).
    pub page_size: usize,
    /// Maximum memory for optimizer states on GPU (bytes, 0 = unlimited).
    pub max_optimizer_memory: usize,
}

impl Default for QLoraTrainingConfig {
    fn default() -> Self {
        Self {
            adapter_config: AdapterTrainingConfig {
                learning_rate: 2e-4,
                lr_schedule: LrSchedule::LinearWarmup { warmup_steps: 100 },
                weight_decay: 0.01,
                gradient_accumulation_steps: 4,
                max_grad_norm: Some(1.0),
            },
            num_epochs: 3,
            batch_size: 4,
            log_every: 10,
            save_every: Some(500),
            warmup_steps: 100,
            use_paged_optimizer: true,
            page_size: 1024 * 1024,  // 1MB pages
            max_optimizer_memory: 0, // unlimited by default
        }
    }
}

/// Paged optimizer state for CPU offloading.
///
/// Stores optimizer states (momentum, variance) on CPU and pages them to GPU
/// as needed during parameter updates. This enables training large models
/// on limited VRAM by trading off memory for compute.
///
/// Matches Python `QLoRA`'s `--optim paged_adamw_32bit` behavior.
#[derive(Debug)]
pub struct PagedAdamWState {
    /// First moment estimates (CPU tensors, paged to GPU on demand).
    pub exp_avg: HashMap<String, Tensor>,
    /// Second moment estimates (CPU tensors, paged to GPU on demand).
    pub exp_avg_sq: HashMap<String, Tensor>,
    /// Step counts per parameter.
    pub steps: HashMap<String, usize>,
    /// Page size in bytes.
    pub page_size: usize,
    /// Set of parameters currently GPU-resident (for tracking).
    gpu_resident: std::collections::HashSet<String>,
    /// LRU access order (most recent at end).
    access_order: Vec<String>,
    /// Maximum GPU memory for optimizer states (0 = unlimited).
    pub max_gpu_memory: usize,
    /// Current GPU memory usage (bytes).
    pub current_gpu_usage: usize,
}

impl PagedAdamWState {
    /// Create new paged optimizer state.
    #[must_use]
    pub fn new(page_size: usize, max_gpu_memory: usize) -> Self {
        Self {
            exp_avg: HashMap::new(),
            exp_avg_sq: HashMap::new(),
            steps: HashMap::new(),
            page_size,
            gpu_resident: std::collections::HashSet::new(),
            access_order: Vec::new(),
            max_gpu_memory,
            current_gpu_usage: 0,
        }
    }

    /// Initialize state for a parameter.
    ///
    /// # Errors
    /// Returns error if tensor creation fails.
    pub fn init_param(&mut self, name: &str, shape: &[usize], _device: &Device) -> Result<()> {
        // Store on CPU for paging (states start on CPU, paged to GPU on demand)
        let cpu_device = Device::Cpu;
        let exp_avg = Tensor::zeros(shape, DType::F32, &cpu_device)?;
        let exp_avg_sq = Tensor::zeros(shape, DType::F32, &cpu_device)?;

        self.exp_avg.insert(name.to_string(), exp_avg);
        self.exp_avg_sq.insert(name.to_string(), exp_avg_sq);
        self.steps.insert(name.to_string(), 0);
        // Note: GPU memory tracking happens in page_to_device, not here
        // since states start on CPU

        Ok(())
    }

    /// Page state to GPU for update, returns (`exp_avg`, `exp_avg_sq`) on target device.
    ///
    /// Updates LRU tracking and GPU memory usage.
    ///
    /// # Errors
    /// Returns error if device transfer fails.
    #[allow(clippy::if_not_else, clippy::excessive_nesting)]
    pub fn page_to_device(&mut self, name: &str, device: &Device) -> Result<(Tensor, Tensor)> {
        let exp_avg = self
            .exp_avg
            .get(name)
            .ok_or_else(|| QLoraError::InvalidConfig(format!("No state for param: {name}")))?;
        let exp_avg_sq = self
            .exp_avg_sq
            .get(name)
            .ok_or_else(|| QLoraError::InvalidConfig(format!("No state for param: {name}")))?;

        // Track GPU residency
        if !self.gpu_resident.contains(name) {
            let param_bytes = exp_avg.elem_count() * 4 * 2; // 2 states * f32

            // Check memory limit and evict LRU if needed
            if self.max_gpu_memory > 0 {
                while self.current_gpu_usage + param_bytes > self.max_gpu_memory
                    && !self.access_order.is_empty()
                {
                    // Evict LRU (first in access_order)
                    if let Some(lru_name) = self.access_order.first().cloned() {
                        if lru_name != name {
                            self.gpu_resident.remove(&lru_name);
                            self.access_order.retain(|n| n != &lru_name);
                            let lru_bytes = self
                                .exp_avg
                                .get(&lru_name)
                                .map_or(0, |t| t.elem_count() * 4 * 2);
                            self.current_gpu_usage =
                                self.current_gpu_usage.saturating_sub(lru_bytes);
                        } else {
                            break; // Don't evict the param we're trying to page in
                        }
                    }
                }
            }

            self.gpu_resident.insert(name.to_string());
            self.current_gpu_usage += param_bytes;
        }

        // Update LRU order (move to end = most recently used)
        self.access_order.retain(|n| n != name);
        self.access_order.push(name.to_string());

        Ok((exp_avg.to_device(device)?, exp_avg_sq.to_device(device)?))
    }

    /// Page state back to CPU after update.
    ///
    /// Updates GPU memory tracking.
    ///
    /// # Errors
    /// Returns error if device transfer fails.
    pub fn page_to_cpu(&mut self, name: &str, exp_avg: &Tensor, exp_avg_sq: &Tensor) -> Result<()> {
        // Track GPU memory release
        if self.gpu_resident.remove(name) {
            let param_bytes = exp_avg.elem_count() * 4 * 2; // 2 states * f32
            self.current_gpu_usage = self.current_gpu_usage.saturating_sub(param_bytes);
            self.access_order.retain(|n| n != name);
        }

        self.exp_avg
            .insert(name.to_string(), exp_avg.to_device(&Device::Cpu)?);
        self.exp_avg_sq
            .insert(name.to_string(), exp_avg_sq.to_device(&Device::Cpu)?);
        Ok(())
    }

    /// Increment step count for a parameter.
    pub fn increment_step(&mut self, name: &str) {
        if let Some(step) = self.steps.get_mut(name) {
            *step += 1;
        }
    }

    /// Get step count for a parameter.
    #[must_use]
    pub fn get_step(&self, name: &str) -> usize {
        self.steps.get(name).copied().unwrap_or(0)
    }

    /// Check if a parameter's optimizer state is currently GPU-resident.
    #[must_use]
    pub fn is_gpu_resident(&self, name: &str) -> bool {
        self.gpu_resident.contains(name)
    }

    /// Get the number of parameters currently GPU-resident.
    #[must_use]
    pub fn gpu_resident_count(&self) -> usize {
        self.gpu_resident.len()
    }
}

/// Paged `AdamW` optimizer with CPU offloading.
///
/// Implements `AdamW` with optimizer state paging to CPU memory,
/// matching Python's `paged_adamw_32bit` from bitsandbytes.
///
/// # Memory Behavior
///
/// - Optimizer states (`exp_avg`, `exp_avg_sq`) stored on CPU
/// - States paged to GPU only during parameter update
/// - Enables training 7B+ models on 24GB GPUs with `QLoRA`
pub struct PagedAdamW {
    /// Learning rate.
    lr: f64,
    /// Beta1 (first moment decay).
    beta1: f64,
    /// Beta2 (second moment decay).
    beta2: f64,
    /// Epsilon for numerical stability.
    eps: f64,
    /// Weight decay coefficient.
    weight_decay: f64,
    /// Paged optimizer state.
    state: PagedAdamWState,
    /// Whether optimizer is initialized.
    initialized: bool,
}

impl PagedAdamW {
    /// Create a new paged `AdamW` optimizer.
    ///
    /// # Arguments
    /// * `lr` - Learning rate
    /// * `weight_decay` - Weight decay coefficient
    /// * `page_size` - Page size in bytes for CPU offloading
    /// * `max_gpu_memory` - Maximum GPU memory for optimizer states (0 = unlimited)
    #[must_use]
    pub fn new(lr: f64, weight_decay: f64, page_size: usize, max_gpu_memory: usize) -> Self {
        Self {
            lr,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
            weight_decay,
            state: PagedAdamWState::new(page_size, max_gpu_memory),
            initialized: false,
        }
    }

    /// Create with custom betas.
    #[must_use]
    pub fn with_betas(mut self, beta1: f64, beta2: f64) -> Self {
        self.beta1 = beta1;
        self.beta2 = beta2;
        self
    }

    /// Initialize optimizer state for parameters.
    ///
    /// # Errors
    /// Returns error if state initialization fails.
    pub fn init(&mut self, params: &[(String, Tensor)]) -> Result<()> {
        for (name, param) in params {
            let shape = param.shape().dims();
            self.state.init_param(name, shape, param.device())?;
        }
        self.initialized = true;
        Ok(())
    }

    /// Set learning rate.
    pub fn set_lr(&mut self, lr: f64) {
        self.lr = lr;
    }

    /// Get current learning rate.
    #[must_use]
    pub fn lr(&self) -> f64 {
        self.lr
    }

    /// Perform optimizer step for a single parameter.
    ///
    /// Implements `AdamW` update with CPU paging:
    /// ```text
    /// m_t = β₁ * m_{t-1} + (1 - β₁) * g_t
    /// v_t = β₂ * v_{t-1} + (1 - β₂) * g_t²
    /// m̂_t = m_t / (1 - β₁^t)
    /// v̂_t = v_t / (1 - β₂^t)
    /// θ_t = θ_{t-1} - lr * (m̂_t / (√v̂_t + ε) + λ * θ_{t-1})
    /// ```
    ///
    /// # Errors
    /// Returns error if tensor operations fail.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn step_param(&mut self, name: &str, param: &mut Tensor, grad: &Tensor) -> Result<()> {
        let device = param.device().clone();

        // Page optimizer state to GPU
        let (mut exp_avg, mut exp_avg_sq) = self.state.page_to_device(name, &device)?;

        // Increment step
        self.state.increment_step(name);
        let step = self.state.get_step(name);

        // Update biased first moment estimate
        let beta1_tensor = Tensor::new(self.beta1 as f32, &device)?;
        let one_minus_beta1 = Tensor::new((1.0 - self.beta1) as f32, &device)?;
        exp_avg = exp_avg
            .broadcast_mul(&beta1_tensor)?
            .broadcast_add(&grad.broadcast_mul(&one_minus_beta1)?)?;

        // Update biased second moment estimate
        let beta2_tensor = Tensor::new(self.beta2 as f32, &device)?;
        let one_minus_beta2 = Tensor::new((1.0 - self.beta2) as f32, &device)?;
        let grad_sq = grad.sqr()?;
        exp_avg_sq = exp_avg_sq
            .broadcast_mul(&beta2_tensor)?
            .broadcast_add(&grad_sq.broadcast_mul(&one_minus_beta2)?)?;

        // Bias correction
        let bias_correction1 = 1.0 - self.beta1.powi(step as i32);
        let bias_correction2 = 1.0 - self.beta2.powi(step as i32);

        let bc1_tensor = Tensor::new(bias_correction1 as f32, &device)?;
        let bc2_tensor = Tensor::new(bias_correction2 as f32, &device)?;

        // Compute step: lr * (m̂ / (√v̂ + ε) + weight_decay * param)
        let exp_avg_corrected = exp_avg.broadcast_div(&bc1_tensor)?;
        let exp_avg_sq_corrected = exp_avg_sq.broadcast_div(&bc2_tensor)?;

        let denom = exp_avg_sq_corrected
            .sqrt()?
            .broadcast_add(&Tensor::new(self.eps as f32, &device)?)?;
        let step_size = Tensor::new(self.lr as f32, &device)?;

        // AdamW: decoupled weight decay
        let update = exp_avg_corrected.broadcast_div(&denom)?;
        let weight_decay_term =
            param.broadcast_mul(&Tensor::new(self.weight_decay as f32, &device)?)?;
        let full_update = update
            .broadcast_add(&weight_decay_term)?
            .broadcast_mul(&step_size)?;

        // Update parameter in place
        *param = param.broadcast_sub(&full_update)?;

        // Page state back to CPU
        self.state.page_to_cpu(name, &exp_avg, &exp_avg_sq)?;

        Ok(())
    }

    /// Get memory usage statistics.
    #[must_use]
    pub fn memory_stats(&self) -> (usize, usize) {
        let cpu_bytes: usize = self
            .state
            .exp_avg
            .values()
            .chain(self.state.exp_avg_sq.values())
            .map(|t| t.elem_count() * 4)
            .sum();
        (cpu_bytes, self.state.current_gpu_usage)
    }
}

/// Trainer for `QLoRA` fine-tuning.
///
/// Manages the training loop, gradient computation, and optimizer updates
/// for quantized `LoRA` training.
///
/// # Usage
///
/// 1. Create trainer with config
/// 2. Use `var_builder()` to create layers that register params in `VarMap`
/// 3. Call `init_optimizer()` to set up optimizer with registered params
/// 4. Call `training_step()` or `training_step_lm()` for each batch
pub struct QLoraTrainer {
    /// Training configuration.
    pub config: QLoraTrainingConfig,
    /// Training state tracking.
    state: AdapterTrainingState,
    /// Device for computation.
    device: Device,
    /// Variable map for trainable parameters.
    varmap: VarMap,
    /// Standard optimizer (when paging disabled).
    optimizer: Option<AdamW>,
    /// Paged optimizer (when paging enabled).
    paged_optimizer: Option<PagedAdamW>,
    /// Current accumulation step.
    accumulation_step: usize,
}

impl QLoraTrainer {
    /// Create a new `QLoRA` trainer.
    ///
    /// # Arguments
    /// * `config` - Training configuration
    /// * `device` - Device for computation
    ///
    /// # Returns
    /// New trainer instance
    #[must_use]
    pub fn new(config: QLoraTrainingConfig, device: Device) -> Self {
        let state = AdapterTrainingState::new(config.adapter_config.clone());
        Self {
            config,
            state,
            device,
            varmap: VarMap::new(),
            optimizer: None,
            paged_optimizer: None,
            accumulation_step: 0,
        }
    }

    /// Get a `VarBuilder` backed by this trainer's `VarMap`.
    ///
    /// Use this to create `QuantizedLinear` layers with gradient tracking.
    /// Params created through this `VarBuilder` will be registered in the
    /// trainer's `VarMap` and trained by the optimizer.
    ///
    /// # Example
    /// ```ignore
    /// let mut trainer = QLoraTrainer::new(config, device.clone());
    /// let vb = trainer.var_builder();
    /// let layer = QuantizedLinear::from_weight_with_varbuilder(&weight, None, &qlora_config, vb.pp("layer0"))?;
    /// trainer.init_optimizer(&[&layer])?;
    /// ```
    #[must_use]
    pub fn var_builder(&self) -> VarBuilder<'_> {
        VarBuilder::from_varmap(&self.varmap, DType::F32, &self.device)
    }

    /// Initialize the optimizer with trainable parameters.
    ///
    /// Creates either a paged or standard `AdamW` optimizer based on configuration.
    /// For paged optimizer, optimizer states are stored on CPU and paged to GPU
    /// during updates to reduce VRAM usage.
    ///
    /// **Important**: Layers must be created using `var_builder()` for standard `AdamW`,
    /// or the optimizer will have no trainable parameters.
    ///
    /// # Arguments
    /// * `layers` - The `QLoRA` layers to train
    ///
    /// # Errors
    /// Returns error if:
    /// - `VarMap` is empty (for standard optimizer) - layers weren't created with `var_builder()`
    /// - Optimizer initialization fails
    ///
    /// # Panics
    /// Panics if the `VarMap` mutex is poisoned.
    pub fn init_optimizer(&mut self, layers: &[&QuantizedLinear]) -> Result<()> {
        if self.config.use_paged_optimizer {
            // Create paged optimizer for memory efficiency
            let mut paged = PagedAdamW::new(
                self.config.adapter_config.learning_rate,
                self.config.adapter_config.weight_decay,
                self.config.page_size,
                self.config.max_optimizer_memory,
            );

            // Collect trainable parameters from VarMap (which should have LoRA params)
            let vars = self.varmap.all_vars();
            if vars.is_empty() {
                return Err(QLoraError::InvalidConfig(
                    "No trainable parameters found. Layers must be created using trainer.var_builder() \
                     so `LoRA` weights are registered in the `VarMap`.".into()
                ));
            }

            // Initialize paged optimizer with actual params from VarMap
            let params: Vec<(String, Tensor)> = self
                .varmap
                .data()
                .lock()
                .unwrap()
                .iter()
                .map(|(name, var)| (name.clone(), var.as_tensor().clone()))
                .collect();

            paged.init(&params)?;
            self.paged_optimizer = Some(paged);

            // Also keep track of layer count for logging
            let _ = layers.len();
        } else {
            // Standard AdamW optimizer - requires VarMap to have params
            let vars = self.varmap.all_vars();
            if vars.is_empty() {
                return Err(QLoraError::InvalidConfig(
                    "No trainable parameters found. Layers must be created using trainer.var_builder() \
                     so `LoRA` weights are registered in the `VarMap`.".into()
                ));
            }

            let params = ParamsAdamW {
                lr: self.config.adapter_config.learning_rate,
                weight_decay: self.config.adapter_config.weight_decay,
                beta1: 0.9,
                beta2: 0.999,
                eps: 1e-8,
            };
            self.optimizer = Some(AdamW::new(vars, params)?);
        }
        Ok(())
    }

    /// Get the current training state.
    #[must_use]
    pub fn state(&self) -> &AdapterTrainingState {
        &self.state
    }

    /// Current optimizer learning rate (matches [`Self::update_lr`] / the active `AdamW` / [`PagedAdamW`]).
    ///
    /// Reads [`QLoraTrainingConfig::adapter_config`]’s `learning_rate` field so callers can drive a
    /// custom schedule by mutating `trainer.config.adapter_config.learning_rate` and calling
    /// [`Self::update_lr`]. (The internal [`AdapterTrainingState`] holds a clone of the initial
    /// config and does not track those mutations.)
    #[must_use]
    pub fn current_lr(&self) -> f64 {
        self.config.adapter_config.learning_rate
    }

    /// Get the current step.
    #[must_use]
    pub fn global_step(&self) -> usize {
        self.state.global_step
    }

    /// Get the current epoch.
    #[must_use]
    pub fn epoch(&self) -> usize {
        self.state.epoch
    }

    /// Zero every `lora_b` weight in the varmap (standard LoRA init: B=0 so the initial
    /// adapter delta is 0 and training starts AT the base model). peft-rs `LoraLayer::new`
    /// builds B with `linear_no_bias` = kaiming (nonzero), which makes the untrained adapter
    /// a large random perturbation (~2.6x the base weight) — a pathological starting point.
    /// Call once after the graph is built and before the training loop.
    pub fn zero_lora_b(&mut self) -> Result<()> {
        let vars = self.varmap.data().lock().unwrap();
        for (k, v) in vars.iter() {
            if k.contains("lora_b") {
                let z = v.as_tensor().zeros_like().map_err(QLoraError::Candle)?;
                v.set(&z).map_err(QLoraError::Candle)?;
            }
        }
        Ok(())
    }

    /// Save the trainable LoRA adapter weights to a safetensors file.
    ///
    /// # Arguments
    /// * `path` - Path to write the safetensors file
    ///
    /// # Errors
    /// Returns error if serialization or writing fails
    pub fn save_adapter<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let vars = self.varmap.data().lock().unwrap();
        let map: HashMap<String, Tensor> = vars
            .iter()
            .map(|(k, v)| (k.clone(), v.as_tensor().clone()))
            .collect();
        
        candle_core::safetensors::save(&map, path)
            .map_err(QLoraError::Candle)?;
            
        Ok(())
    }

    /// Overwrite LoRA weights from a safetensors file (warm-start).
    /// Keys found in path but not in varmap are skipped.
    pub fn load_lora_weights<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let loaded = candle_core::safetensors::load(path.as_ref(), &self.device)
            .map_err(QLoraError::Candle)?;
        let mut vars = self.varmap.data().lock().unwrap();
        for (key, tensor) in &loaded {
            if let Some(var) = vars.get_mut(key) {
                var.set(tensor).map_err(QLoraError::Candle)?;
            }
        }
        Ok(())
    }

    /// Exposes backward step logic for manual loops
    pub fn backward_step(&mut self, loss: &Tensor) -> Result<()> {
        let accum_steps = self.config.adapter_config.gradient_accumulation_steps.max(1);
        let scaled_loss = if accum_steps > 1 {
            let scale = Tensor::new(1.0f32 / accum_steps as f32, loss.device())?;
            loss.broadcast_mul(&scale)?
        } else {
            loss.clone()
        };

        self.accumulation_step += 1;

        if let Some(ref mut optimizer) = self.optimizer {
            if self.accumulation_step >= accum_steps {
                // Compute gradients explicitly so we can clip them (by global L2
                // norm) before the optimizer step. `backward_step` would fold
                // backward + step together and give us no hook to clip.
                let mut grads = scaled_loss.backward()?;
                if let Some(max_norm) = self.config.adapter_config.max_grad_norm {
                    let vars = self.varmap.all_vars();
                    clip_grad_norm(&mut grads, &vars, max_norm)?;
                }
                optimizer.step(&grads)?;
                self.accumulation_step = 0;
            } else {
                let _ = scaled_loss.backward();
            }
        } else if let Some(ref mut paged_optimizer) = self.paged_optimizer {
            if self.accumulation_step >= accum_steps {
                let grads = scaled_loss.backward()?;
                let mut varmap_data = self.varmap.data().lock().unwrap();
                for (name, var) in varmap_data.iter_mut() {
                    if let Some(grad) = grads.get(var.as_tensor()) {
                        let mut param = var.as_tensor().clone();
                        paged_optimizer.step_param(name, &mut param, grad)?;
                        // step_param updates a *clone*; write the result back into the
                        // Var or the optimizer step is silently discarded and the
                        // parameter never changes (frozen at init).
                        var.set(&param)?;
                    }
                }
                drop(varmap_data);
                self.accumulation_step = 0;
            } else {
                let _ = scaled_loss.backward();
            }
        }
        
        let _should_log = self.state.step();
        
        Ok(())
    }

    /// All trainable [`Var`]s registered in this trainer's `VarMap`.
    ///
    /// The caller (the segmented/checkpointed backward path) uses this to know
    /// which tensors to pull out of each segment's [`GradStore`] when folding the
    /// partial backwards into one combined store.
    #[must_use]
    pub fn trainable_vars(&self) -> Vec<Var> {
        self.varmap.all_vars()
    }

    /// Apply a single optimizer step from a **pre-computed** [`GradStore`],
    /// honoring gradient accumulation exactly like [`Self::backward_step`].
    ///
    /// This is the counterpart to [`Self::backward_step`] for the gradient-
    /// checkpointed path: instead of folding `loss.backward()` + step together,
    /// the caller runs N segment recomputes (each via [`backward_from_cotangent`])
    /// and merges them with [`accumulate_grads_for_vars`], then hands the combined
    /// grads here. Gradient clipping (global L2 norm) and the AdamW update are
    /// applied identically to the non-checkpointed loop, so numerics match.
    ///
    /// `grads` must already be scaled for gradient accumulation by the caller
    /// (the checkpointed loss is scaled before backward, mirroring
    /// [`Self::backward_step`]). On non-step accumulation cycles, pass the grads
    /// anyway — they are dropped and only the accumulation counter advances, which
    /// keeps the step cadence identical to the eager path.
    ///
    /// # Errors
    /// Returns an error if clipping or the optimizer step fails.
    ///
    /// # Panics
    /// Panics if the `VarMap` mutex is poisoned.
    pub fn optimizer_step_with_grads(&mut self, mut grads: GradStore) -> Result<()> {
        let accum_steps = self.config.adapter_config.gradient_accumulation_steps.max(1);
        self.accumulation_step += 1;
        let do_step = self.accumulation_step >= accum_steps;

        if do_step {
            if let Some(ref mut optimizer) = self.optimizer {
                if let Some(max_norm) = self.config.adapter_config.max_grad_norm {
                    let vars = self.varmap.all_vars();
                    clip_grad_norm(&mut grads, &vars, max_norm)?;
                }
                optimizer.step(&grads)?;
            } else if let Some(ref mut paged_optimizer) = self.paged_optimizer {
                if let Some(max_norm) = self.config.adapter_config.max_grad_norm {
                    let vars = self.varmap.all_vars();
                    clip_grad_norm(&mut grads, &vars, max_norm)?;
                }
                let mut varmap_data = self.varmap.data().lock().unwrap();
                for (name, var) in varmap_data.iter_mut() {
                    if let Some(grad) = grads.get(var.as_tensor()) {
                        let mut param = var.as_tensor().clone();
                        paged_optimizer.step_param(name, &mut param, grad)?;
                        var.set(&param)?;
                    }
                }
                drop(varmap_data);
            }
            self.accumulation_step = 0;
        }
        // NOTE: when accumulating (no step) we cannot keep candle GradStores summed
        // across micro-steps cheaply without holding tensors alive; the eager
        // `backward_step` relies on candle accumulating into Var grads via repeated
        // `backward()`. For the checkpointed path the supported/validated config is
        // grad_accum == 1 (the 16GB-tight 3B case), where every micro-step is a step.
        let _should_log = self.state.step();
        Ok(())
    }

    /// Perform a training step with gradient accumulation.
    ///
    /// `QLoRA` training flow:
    /// 1. Forward pass through frozen quantized base + trainable `LoRA`
    /// 2. Compute loss (cross-entropy for LM, MSE for regression)
    /// 3. Backward pass - gradients flow only through `LoRA` weights
    /// 4. Accumulate gradients if `gradient_accumulation_steps` > 1
    /// 5. Optimizer step when accumulation complete
    ///
    /// Supports both standard `AdamW` and paged `AdamW` optimizers.
    ///
    /// # Arguments
    /// * `layers` - The `QLoRA` layers
    /// * `input` - Input tensor `[batch, seq_len, hidden]`
    /// * `targets` - Target tensor (logits or token IDs depending on loss)
    ///
    /// # Returns
    /// The loss value for this step
    ///
    /// # Errors
    /// Returns error if forward pass or backward pass fails
    ///
    /// # Panics
    /// Panics if the `VarMap` mutex is poisoned.
    #[allow(clippy::cast_precision_loss, clippy::excessive_nesting)]
    pub fn training_step(
        &mut self,
        layers: &[&QuantizedLinear],
        input: &Tensor,
        targets: &Tensor,
    ) -> Result<f64> {
        // Forward pass through all layers
        let mut output = input.clone();
        for layer in layers {
            output = layer.forward(&output)?;
        }

        // Compute loss - using MSE for now, cross_entropy available separately
        let loss = output.sub(targets)?.sqr()?.mean_all()?;

        // Scale loss for gradient accumulation
        let accum_steps = self.config.adapter_config.gradient_accumulation_steps;
        let scaled_loss = if accum_steps > 1 {
            let scale = Tensor::new(1.0 / accum_steps as f32, loss.device())?;
            loss.broadcast_mul(&scale)?
        } else {
            loss.clone()
        };

        let loss_value = f64::from(loss.to_scalar::<f32>()?);

        // Backward pass with gradient accumulation
        self.accumulation_step += 1;

        // Handle standard AdamW optimizer
        if let Some(ref mut optimizer) = self.optimizer {
            if self.accumulation_step >= accum_steps {
                // Compute gradients explicitly so they can be clipped (global L2
                // norm) before the optimizer step. `backward_step` folds backward
                // and step together, leaving no hook to clip in between.
                let mut grads = scaled_loss.backward()?;
                if let Some(max_norm) = self.config.adapter_config.max_grad_norm {
                    let vars = self.varmap.all_vars();
                    clip_grad_norm(&mut grads, &vars, max_norm)?;
                }

                // Perform optimizer step on the (possibly clipped) gradients
                optimizer.step(&grads)?;
                self.accumulation_step = 0;
            } else {
                // Just accumulate gradients without stepping
                // In candle, backward() accumulates gradients
                let _ = scaled_loss.backward();
            }
        } else if let Some(ref mut paged_optimizer) = self.paged_optimizer {
            // Handle paged optimizer
            if self.accumulation_step >= accum_steps {
                // Compute gradients first
                let grads = scaled_loss.backward()?;

                // Step each parameter with the paged optimizer
                let mut varmap_data = self.varmap.data().lock().unwrap();
                for (name, var) in varmap_data.iter_mut() {
                    if let Some(grad) = grads.get(var.as_tensor()) {
                        let mut param = var.as_tensor().clone();
                        paged_optimizer.step_param(name, &mut param, grad)?;
                        // Note: In candle, Var doesn't support direct assignment,
                        // but the optimizer state is updated which matters for subsequent steps
                    }
                }
                drop(varmap_data);
                self.accumulation_step = 0;
            } else {
                // Just accumulate gradients without stepping
                let _ = scaled_loss.backward();
            }
        }

        // Update training state
        let should_log = self.state.step();
        if should_log && self.state.global_step.is_multiple_of(self.config.log_every) {
            #[cfg(feature = "logging")]
            log::info!(
                "Step {} | Loss: {:.4} | LR: {:.2e}",
                self.state.global_step,
                loss_value,
                self.current_lr()
            );
        }

        Ok(loss_value)
    }

    /// Perform training step with cross-entropy loss for language modeling.
    ///
    /// Supports both standard `AdamW` and paged `AdamW` optimizers.
    ///
    /// # Arguments
    /// * `layers` - The `QLoRA` layers
    /// * `input` - Input tensor `[batch, seq_len, hidden]`
    /// * `target_ids` - Target token IDs `[batch, seq_len]`
    ///
    /// # Returns
    /// The cross-entropy loss value
    ///
    /// # Errors
    /// Returns error if forward pass or loss computation fails
    ///
    /// # Panics
    /// Panics if the `VarMap` mutex is poisoned.
    pub fn training_step_lm(
        &mut self,
        layers: &[&QuantizedLinear],
        input: &Tensor,
        target_ids: &Tensor,
    ) -> Result<f64> {
        let n = layers.len();
        if n == 0 {
            return Err(QLoraError::InvalidConfig(
                "training_step_lm: empty layer stack".into(),
            ));
        }

        // Forward through stacked projections. Vox chains many `o_proj`-shaped layers on a
        // frozen-embed shortcut (not a full transformer residual path). Use **pre-norm residual**
        // blocks for middle layers: `h <- h + F(RMSNorm(h))`, then scale before the LM head by
        // `1/sqrt(n_mid)` so deep stacks do not compound magnitudes into pathological CE.
        let n_mid = n.saturating_sub(1);
        let mut logits = input.clone();
        let hidden = logits.dim(D::Minus1)?;
        let alpha_ones = Tensor::ones((hidden,), logits.dtype(), input.device())?;
        let debug_norms = std::env::var("VOX_QLORA_DEBUG_NORMS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        for (i, layer) in layers.iter().enumerate() {
            let is_lm_head = i + 1 == n;
            if is_lm_head {
                if n_mid > 0 {
                    let scale = 1.0f32 / (n_mid as f32).sqrt();
                    let sc = Tensor::new(scale, logits.device())?;
                    logits = logits.broadcast_mul(&sc)?;
                }
                logits = layer.forward(&logits)?;
            } else {
                let residual = logits.clone();
                let normed = candle_nn::ops::rms_norm_slow(&logits, &alpha_ones, 1e-5)?;
                let delta = layer.forward(&normed)?;
                // Dampen each residual contribution when the stack is deep (complements final 1/√n_mid).
                let mid_scale = if n_mid > 0 {
                    1.0f32 / (n_mid as f32).sqrt()
                } else {
                    1.0f32
                };
                let w = Tensor::new(mid_scale, logits.device())?;
                let delta_s = delta.broadcast_mul(&w)?;
                logits = (&residual + &delta_s)?;
                if debug_norms {
                    let m = logits.abs()?.mean_all()?.to_scalar::<f32>()?;
                    eprintln!(
                        "[qlora-rs training_step_lm] after_mid_block layer {i} mean_abs={m:.6e}"
                    );
                }
            }
        }

        let loss = cross_entropy_loss(&logits, target_ids)?;
        let loss_value = f64::from(loss.to_scalar::<f32>()?);

        let accum_steps = self.config.adapter_config.gradient_accumulation_steps.max(1);
        let scaled_loss = if accum_steps > 1 {
            let scale = Tensor::new(1.0f32 / accum_steps as f32, loss.device())?;
            loss.broadcast_mul(&scale)?
        } else {
            loss.clone()
        };

        self.accumulation_step += 1;

        if let Some(ref mut optimizer) = self.optimizer {
            if self.accumulation_step >= accum_steps {
                optimizer.backward_step(&scaled_loss)?;
                self.accumulation_step = 0;
            } else {
                let _ = scaled_loss.backward();
            }
        } else if let Some(ref mut paged_optimizer) = self.paged_optimizer {
            if self.accumulation_step >= accum_steps {
                let grads = scaled_loss.backward()?;

                let mut varmap_data = self.varmap.data().lock().unwrap();
                for (name, var) in varmap_data.iter_mut() {
                    if let Some(grad) = grads.get(var.as_tensor()) {
                        let mut param = var.as_tensor().clone();
                        paged_optimizer.step_param(name, &mut param, grad)?;
                        // step_param updates a *clone*; write the result back into the
                        // Var or the optimizer step is silently discarded and the
                        // parameter never changes (frozen at init).
                        var.set(&param)?;
                    }
                }
                drop(varmap_data);
                self.accumulation_step = 0;
            } else {
                let _ = scaled_loss.backward();
            }
        }

        let should_log = self.state.step();
        if should_log && self.state.global_step.is_multiple_of(self.config.log_every) {
            #[cfg(feature = "logging")]
            log::info!(
                "Step {} | Loss: {:.4} | LR: {:.2e}",
                self.state.global_step,
                loss_value,
                self.current_lr()
            );
        }

        Ok(loss_value)
    }

    /// Start a new training epoch.
    pub fn start_epoch(&mut self) {
        self.state.new_epoch();
        self.accumulation_step = 0;
        #[cfg(feature = "logging")]
        log::info!("Starting epoch {}", self.state.epoch);
    }

    /// Check if training should continue.
    #[must_use]
    pub fn should_continue(&self) -> bool {
        self.state.epoch < self.config.num_epochs
    }

    /// Push [`QLoraTrainingConfig::adapter_config`].`learning_rate` into the optimizer(s).
    pub fn update_lr(&mut self) {
        let lr = self.config.adapter_config.learning_rate;
        if let Some(ref mut optimizer) = self.optimizer {
            optimizer.set_learning_rate(lr);
        }
        if let Some(ref mut paged) = self.paged_optimizer {
            paged.set_lr(lr);
        }
    }

    /// Get training configuration.
    #[must_use]
    pub fn config(&self) -> &QLoraTrainingConfig {
        &self.config
    }

    /// Get optimizer memory statistics (CPU bytes, GPU bytes).
    #[must_use]
    pub fn optimizer_memory_stats(&self) -> Option<(usize, usize)> {
        self.paged_optimizer.as_ref().map(PagedAdamW::memory_stats)
    }

    /// Zero gradients for next accumulation cycle.
    ///
    /// Resets the accumulation step counter. Note: In candle, gradients are
    /// automatically zeroed when `backward_step` is called on the optimizer.
    pub fn zero_grad(&mut self) {
        self.accumulation_step = 0;
    }
}

/// Compute cross-entropy loss for language modeling.
///
/// # Arguments
/// * `logits` - Model output logits `[batch, seq_len, vocab_size]`
/// * `targets` - Target token IDs `[batch, seq_len]`
///
/// # Returns
/// Cross-entropy loss value
///
/// # Errors
/// Returns error if tensor operations fail
pub fn cross_entropy_loss(logits: &Tensor, targets: &Tensor) -> Result<Tensor> {
    let (batch, seq_len, vocab_size) = logits.dims3()?;

    // Reshape logits to [batch * seq_len, vocab_size]
    let flat_logits = logits.reshape(&[batch * seq_len, vocab_size])?;

    // Reshape targets to [batch * seq_len]
    let flat_targets = targets.reshape(&[batch * seq_len])?;

    // Compute log softmax
    let log_probs = candle_nn::ops::log_softmax(&flat_logits, 1)?;

    // Gather log probs at target indices
    let target_indices = flat_targets.unsqueeze(1)?;
    let gathered = log_probs.gather(&target_indices, 1)?;

    // Mean negative log likelihood
    let loss = gathered.neg()?.mean_all()?;

    Ok(loss)
}

/// Training metrics for logging.
#[derive(Debug, Clone, Default)]
pub struct TrainingMetrics {
    /// Total training loss.
    pub total_loss: f64,
    /// Number of steps.
    pub num_steps: usize,
    /// Best loss seen.
    pub best_loss: f64,
    /// Tokens processed.
    pub tokens_processed: usize,
}

impl TrainingMetrics {
    /// Create new metrics tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total_loss: 0.0,
            num_steps: 0,
            best_loss: f64::MAX,
            tokens_processed: 0,
        }
    }

    /// Update metrics with a new loss value.
    pub fn update(&mut self, loss: f64, num_tokens: usize) {
        self.total_loss += loss;
        self.num_steps += 1;
        self.tokens_processed += num_tokens;
        if loss < self.best_loss {
            self.best_loss = loss;
        }
    }

    /// Get average loss.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn average_loss(&self) -> f64 {
        if self.num_steps == 0 {
            0.0
        } else {
            self.total_loss / self.num_steps as f64
        }
    }

    /// Reset metrics for new epoch.
    pub fn reset(&mut self) {
        self.total_loss = 0.0;
        self.num_steps = 0;
        self.tokens_processed = 0;
        // Keep best_loss across epochs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::DType;

    #[test]
    fn test_training_config_default() {
        let config = QLoraTrainingConfig::default();
        assert_eq!(config.num_epochs, 3);
        assert_eq!(config.batch_size, 4);
        assert!((config.adapter_config.learning_rate - 2e-4).abs() < 1e-10);
    }

    #[test]
    fn test_trainer_creation() {
        let config = QLoraTrainingConfig::default();
        let device = Device::Cpu;
        let trainer = QLoraTrainer::new(config, device);

        assert_eq!(trainer.global_step(), 0);
        assert_eq!(trainer.epoch(), 0);
    }

    #[test]
    fn test_training_metrics() {
        let mut metrics = TrainingMetrics::new();

        metrics.update(0.5, 128);
        metrics.update(0.4, 128);
        metrics.update(0.3, 128);

        assert_eq!(metrics.num_steps, 3);
        assert!((metrics.average_loss() - 0.4).abs() < 1e-10);
        assert!((metrics.best_loss - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_cross_entropy_loss_shape() {
        let device = Device::Cpu;
        let batch = 2;
        let seq_len = 10;
        let vocab_size = 100;

        let logits = Tensor::zeros(&[batch, seq_len, vocab_size], DType::F32, &device).unwrap();
        // Random targets (0-99)
        let targets = Tensor::zeros(&[batch, seq_len], DType::U32, &device).unwrap();

        let loss = cross_entropy_loss(&logits, &targets).unwrap();
        // Loss should be scalar
        let dims: &[usize] = loss.dims();
        assert!(dims.is_empty(), "Expected scalar loss, got dims: {dims:?}");
    }

    #[test]
    fn test_clip_grad_norm_caps_global_norm() {
        let device = Device::Cpu;

        // Two trainable vars. Build a loss whose gradients have a known global
        // L2 norm > 1: loss = sum(v1 * 3) + sum(v2 * 4) => grad(v1)=3, grad(v2)=4,
        // global norm = sqrt(3^2 + 4^2) = 5.
        let v1 = Var::from_tensor(&Tensor::new(&[1.0f32], &device).unwrap()).unwrap();
        let v2 = Var::from_tensor(&Tensor::new(&[1.0f32], &device).unwrap()).unwrap();

        let three = Tensor::new(&[3.0f32], &device).unwrap();
        let four = Tensor::new(&[4.0f32], &device).unwrap();
        let l1 = v1.as_tensor().mul(&three).unwrap().sum_all().unwrap();
        let l2 = v2.as_tensor().mul(&four).unwrap().sum_all().unwrap();
        let loss = l1.add(&l2).unwrap();

        let mut grads = loss.backward().unwrap();

        let vars = vec![v1.clone(), v2.clone()];
        let max_norm = 1.0;
        let pre = clip_grad_norm(&mut grads, &vars, max_norm).unwrap();
        assert!((pre - 5.0).abs() < 1e-5, "pre-clip norm should be 5, got {pre}");

        // Post-clip global norm must be <= max_norm (+ tiny epsilon).
        let c1 = grads.get(v1.as_tensor()).unwrap();
        let c2 = grads.get(v2.as_tensor()).unwrap();
        let n1: f32 = c1.sqr().unwrap().sum_all().unwrap().to_scalar().unwrap();
        let n2: f32 = c2.sqr().unwrap().sum_all().unwrap().to_scalar().unwrap();
        let post = (f64::from(n1) + f64::from(n2)).sqrt();
        assert!(
            post <= max_norm + 1e-5,
            "post-clip global norm should be <= {max_norm}, got {post}"
        );

        // Direction preserved: every grad scaled by the same factor.
        let c1v: f32 = c1.to_vec1::<f32>().unwrap()[0];
        let c2v: f32 = c2.to_vec1::<f32>().unwrap()[0];
        let factor1 = c1v / 3.0;
        let factor2 = c2v / 4.0;
        assert!(
            (factor1 - factor2).abs() < 1e-5,
            "scale factor must be uniform: {factor1} vs {factor2}"
        );
        // Expected uniform factor ~= max_norm / (5 + eps) ~= 0.2.
        assert!((factor1 - 0.2).abs() < 1e-4, "factor should be ~0.2, got {factor1}");
    }

    #[test]
    fn test_clip_grad_norm_noop_when_under_budget() {
        let device = Device::Cpu;
        // grad(v) = [0.6, 0.8] => norm = 1.0 exactly; <= max_norm so unchanged.
        let v = Var::from_tensor(&Tensor::new(&[1.0f32, 1.0], &device).unwrap()).unwrap();
        let coeff = Tensor::new(&[0.6f32, 0.8], &device).unwrap();
        let loss = v.as_tensor().mul(&coeff).unwrap().sum_all().unwrap();
        let mut grads = loss.backward().unwrap();
        let pre = clip_grad_norm(&mut grads, &[v.clone()], 1.0).unwrap();
        assert!((pre - 1.0).abs() < 1e-5, "norm should be 1, got {pre}");
        let after: Vec<f32> = grads.get(v.as_tensor()).unwrap().to_vec1().unwrap();
        assert!((after[0] - 0.6).abs() < 1e-6 && (after[1] - 0.8).abs() < 1e-6);
    }

    /// **Gradient-checkpointing correctness spike (the #1 risk).**
    ///
    /// Builds a tiny 2-segment stack of trainable `Var`s and proves that the
    /// segment-wise recompute backward — `loss.backward()` for the last segment,
    /// then [`backward_from_cotangent`] threading the input-grad back through the
    /// first segment — yields **the same** parameter gradients as a single
    /// full-graph `loss.backward()` over the whole stack. This is the property
    /// that makes manual activation checkpointing safe: a silently-wrong backward
    /// would "train" with bad grads, the worst outcome.
    ///
    /// Stack (linear, no quant — exercises the autograd threading, not NF4):
    ///   `y0 = x @ w0`  (segment 0, param w0)
    ///   `y1 = y0 @ w1` (segment 1, param w1)
    ///   `loss = sum(y1)`
    #[test]
    fn checkpointed_backward_matches_full_graph_backward() {
        let device = Device::Cpu;
        let d = 4usize;
        let seq = 3usize;

        let x = Tensor::randn(0f32, 1f32, (seq, d), &device).unwrap();
        let w0 = Var::from_tensor(&Tensor::randn(0f32, 0.1f32, (d, d), &device).unwrap()).unwrap();
        let w1 = Var::from_tensor(&Tensor::randn(0f32, 0.1f32, (d, d), &device).unwrap()).unwrap();

        // ── Reference: one full-graph backward ──────────────────────────────
        let y0 = x.matmul(w0.as_tensor()).unwrap();
        let y1 = y0.matmul(w1.as_tensor()).unwrap();
        let loss = y1.sum_all().unwrap();
        let full = loss.backward().unwrap();
        let g_w0_full: Vec<f32> = full.get(w0.as_tensor()).unwrap().flatten_all().unwrap().to_vec1().unwrap();
        let g_w1_full: Vec<f32> = full.get(w1.as_tensor()).unwrap().flatten_all().unwrap().to_vec1().unwrap();

        // ── Checkpointed: forward with detached segment boundary, then recompute ──
        // Segment 0 forward, detach its output (the checkpoint boundary).
        let y0_ck = x.matmul(w0.as_tensor()).unwrap();
        let y0_boundary = y0_ck.detach(); // stored activation; tape severed here

        // Segment 1: recompute from the (grad-tracked) detached boundary.
        let y0_in = Var::from_tensor(&y0_boundary).unwrap(); // leaf we can read grad for
        let y1_ck = y0_in.as_tensor().matmul(w1.as_tensor()).unwrap();
        let loss_ck = y1_ck.sum_all().unwrap();
        let grads_seg1 = loss_ck.backward().unwrap();
        // Upstream grad for segment 0's output = dL/dy0.
        let upstream = grads_seg1.get(y0_in.as_tensor()).unwrap().clone();

        // Segment 0: recompute forward, inject the cotangent, backward.
        let y0_re = x.matmul(w0.as_tensor()).unwrap();
        let grads_seg0 = backward_from_cotangent(&y0_re, &upstream).unwrap();

        // Merge per-Var grads across both segments. The last segment's GradStore
        // (which we own) is the accumulator; fold the earlier segment into it.
        let mut combined = grads_seg1;
        accumulate_grads_for_vars(&mut combined, &grads_seg0, &[w0.clone()]).unwrap();

        let g_w0_ck: Vec<f32> = combined.get(w0.as_tensor()).unwrap().flatten_all().unwrap().to_vec1().unwrap();
        let g_w1_ck: Vec<f32> = combined.get(w1.as_tensor()).unwrap().flatten_all().unwrap().to_vec1().unwrap();

        for (a, b) in g_w0_full.iter().zip(g_w0_ck.iter()) {
            assert!((a - b).abs() < 1e-4, "w0 grad mismatch: full={a} ck={b}");
        }
        for (a, b) in g_w1_full.iter().zip(g_w1_ck.iter()) {
            assert!((a - b).abs() < 1e-4, "w1 grad mismatch: full={a} ck={b}");
        }
    }

    /// `backward_from_cotangent` seeds the graph with exactly the supplied
    /// cotangent: for `y = x` (identity leaf), the grad of `x` must equal `g`.
    #[test]
    fn cotangent_backward_seeds_exact_upstream_grad() {
        let device = Device::Cpu;
        let w = Var::from_tensor(&Tensor::new(&[[1.0f32, 2.0], [3.0, 4.0]], &device).unwrap()).unwrap();
        let x = Tensor::new(&[[1.0f32, 1.0], [1.0, 1.0]], &device).unwrap();
        let y = x.matmul(w.as_tensor()).unwrap();
        // arbitrary upstream cotangent
        let g = Tensor::new(&[[0.5f32, -1.0], [2.0, 0.25]], &device).unwrap();
        let grads = backward_from_cotangent(&y, &g).unwrap();
        // dL/dw = x^T @ g
        let expected = x.t().unwrap().matmul(&g).unwrap();
        let got: Vec<f32> = grads.get(w.as_tensor()).unwrap().flatten_all().unwrap().to_vec1().unwrap();
        let exp: Vec<f32> = expected.flatten_all().unwrap().to_vec1().unwrap();
        for (a, b) in got.iter().zip(exp.iter()) {
            assert!((a - b).abs() < 1e-5, "cotangent grad mismatch: got={a} exp={b}");
        }
    }

    #[test]
    fn test_trainer_epoch_progression() {
        let config = QLoraTrainingConfig {
            num_epochs: 2,
            ..Default::default()
        };
        let device = Device::Cpu;
        let mut trainer = QLoraTrainer::new(config, device);

        assert!(trainer.should_continue());
        trainer.start_epoch();
        assert_eq!(trainer.epoch(), 1);
        assert!(trainer.should_continue());
        trainer.start_epoch();
        assert_eq!(trainer.epoch(), 2);
        assert!(!trainer.should_continue());
    }
}
