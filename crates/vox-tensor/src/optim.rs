use burn::lr_scheduler::linear::{LinearLrScheduler, LinearLrSchedulerConfig};
use burn::lr_scheduler::cosine::{CosineAnnealingLrScheduler, CosineAnnealingLrSchedulerConfig};
use burn::lr_scheduler::LrScheduler as BurnLrScheduler;
use burn::optim::{
    AdamW as AdamWLogic, Sgd as SgdLogic, Adam as AdamLogic,
    AdamWConfig, SgdConfig, AdamConfig,
    GradientsParams, adaptor::OptimizerAdaptor, Optimizer
};
use burn::module::AutodiffModule;
use burn::tensor::backend::AutodiffBackend;
use std::marker::PhantomData;

/// A real wrapper for the AdamW optimizer using Burn 0.19.
pub struct AdamW<M: AutodiffModule<B>, B: AutodiffBackend> {
    optim: OptimizerAdaptor<AdamWLogic, M, B>,
    _phantom: PhantomData<(M, B)>,
}

impl<M: AutodiffModule<B>, B: AutodiffBackend> AdamW<M, B> {
    pub fn new() -> Self {
        let config = AdamWConfig::new();
        Self {
            optim: config.init::<B, M>(),
            _phantom: PhantomData,
        }
    }

    /// Perform one optimization step on the module.
    pub fn step(&mut self, lr: f64, module: M, grads: B::Gradients) -> M {
        let grads = GradientsParams::from_grads(grads, &module);
        self.optim.step(lr, module, grads)
    }
}

/// A real wrapper for the SGD optimizer using Burn 0.19.
pub struct SGD<M: AutodiffModule<B>, B: AutodiffBackend> {
    optim: OptimizerAdaptor<SgdLogic<B::InnerBackend>, M, B>,
    _phantom: PhantomData<(M, B)>,
}

impl<M: AutodiffModule<B>, B: AutodiffBackend> SGD<M, B> {
    pub fn new() -> Self {
        let config = SgdConfig::new();
        Self {
            optim: config.init::<B, M>(),
            _phantom: PhantomData,
        }
    }

    pub fn step(&mut self, lr: f64, module: M, grads: B::Gradients) -> M {
        let grads = GradientsParams::from_grads(grads, &module);
        self.optim.step(lr, module, grads)
    }
}

/// A real wrapper for the Adam optimizer using Burn 0.19.
pub struct Adam<M: AutodiffModule<B>, B: AutodiffBackend> {
    optim: OptimizerAdaptor<AdamLogic, M, B>,
    _phantom: PhantomData<(M, B)>,
}

impl<M: AutodiffModule<B>, B: AutodiffBackend> Adam<M, B> {
    pub fn new() -> Self {
        let config = AdamConfig::new();
        Self {
            optim: config.init::<B, M>(),
            _phantom: PhantomData,
        }
    }

    pub fn step(&mut self, lr: f64, module: M, grads: B::Gradients) -> M {
        let grads = GradientsParams::from_grads(grads, &module);
        self.optim.step(lr, module, grads)
    }
}

// ─── LR Scheduler Enum ──────────────────────────────────────────────────
// Burn 0.19's LrScheduler trait requires Self: Sized, making it non-dyn-compatible.
// We use a concrete enum instead of Box<dyn LrScheduler> to avoid the error.

pub enum VoxScheduler {
    Linear(LinearLrScheduler),
    Cosine(CosineAnnealingLrScheduler),
}

impl VoxScheduler {
    pub fn step(&mut self) -> f64 {
        match self {
            VoxScheduler::Linear(s) => BurnLrScheduler::step(s),
            VoxScheduler::Cosine(s) => BurnLrScheduler::step(s),
        }
    }
}

/// A wrapper for the Linear learning rate scheduler.
pub struct LinearWarmupScheduler {
    inner: VoxScheduler,
}

impl LinearWarmupScheduler {
    pub fn new(initial_lr: f64, final_lr: f64, n_steps: usize) -> Self {
        let config = LinearLrSchedulerConfig::new(initial_lr, final_lr, n_steps);
        Self {
            inner: VoxScheduler::Linear(config.init().expect("Failed to initialize LinearLrScheduler")),
        }
    }

    pub fn step(&mut self) -> f64 {
        self.inner.step()
    }
}

/// A wrapper for the Cosine annealing learning rate scheduler.
pub struct CosineAnnealingScheduler {
    inner: VoxScheduler,
}

impl CosineAnnealingScheduler {
    pub fn new(initial_lr: f64, n_steps: usize) -> Self {
        let config = CosineAnnealingLrSchedulerConfig::new(initial_lr, n_steps);
        Self {
            inner: VoxScheduler::Cosine(config.init().expect("Failed to initialize CosineAnnealingLrScheduler")),
        }
    }

    pub fn step(&mut self) -> f64 {
        self.inner.step()
    }
}
