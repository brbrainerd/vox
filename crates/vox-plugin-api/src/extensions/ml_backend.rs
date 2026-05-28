//! MlBackend extension-point trait — the first real code-plugin extension.
//!
//! Implementations live in plugins like `vox-plugin-mens-candle-cuda`.
//! The host obtains an instance via `VoxPlugin::as_ml_backend()` and dispatches
//! training / eval / checkpoint operations through it.
//!
//! # Wire format
//!
//! For `train_step` and `eval_step`, batch payloads are serialized as JSON
//! strings rather than typed `StableAbi` payloads. This keeps the trait
//! shape stable while the schema for batches and stats can evolve via
//! plain-Rust serde types defined in the calling crate. If JSON ser/de
//! shows up as a hot-path cost in profiles, switch to a bincode-encoded
//! `RVec<u8>` in a future trait revision.

use abi_stable::{StableAbi, sabi_trait, std_types::*};

pub const ML_BACKEND_REVISION: u32 = 4;

/// Opaque handle to a backend-owned model. The host never inspects the
/// contents — it only passes it back to the same backend across calls.
/// Held inside `RBox<MlModelHandle>` for stable-ABI ownership.
#[repr(C)]
#[derive(StableAbi)]
pub struct MlModelHandle {
    /// Implementation-defined opaque pointer. Plugins are free to store
    /// `Box::into_raw(Box::new(...))` here and reconstitute on access.
    pub opaque: usize,
}

#[sabi_trait]
pub trait MlBackend: Send + Sync {
    fn revision(&self) -> u32 {
        ML_BACKEND_REVISION
    }

    /// Load a pretrained model from `model_path`. Returns an opaque handle
    /// that subsequent calls reference.
    fn load_model(&self, model_path: RStr<'_>) -> RResult<RBox<MlModelHandle>, RBoxError>;

    /// Release resources for a handle previously returned by [`Self::load_model`].
    /// The host must not use `model` after a successful unload.
    fn unload_model(&self, model: &MlModelHandle) -> RResult<(), RBoxError> {
        let _ = model;
        RResult::ROk(())
    }

    /// Run one training step. Batch is JSON-encoded; stats response is
    /// JSON-encoded. Schema is owned by the caller.
    fn train_step(
        &self,
        model: &MlModelHandle,
        batch_json: RStr<'_>,
    ) -> RResult<RString, RBoxError>;

    /// Run one evaluation step. Same wire format as train_step.
    fn eval_step(&self, model: &MlModelHandle, batch_json: RStr<'_>)
    -> RResult<RString, RBoxError>;

    /// Persist a checkpoint of `model` to `dest` (filesystem path).
    fn save_checkpoint(&self, model: &MlModelHandle, dest: RStr<'_>) -> RResult<(), RBoxError>;

    /// Run a complete training session described by `config_json`. The
    /// plugin owns the entire loop (data loading, epochs, optimizer,
    /// checkpointing, telemetry). Returns a JSON-encoded summary of
    /// the run on success. This is the coarse-grained entry point used
    /// by orchestrators that want fire-and-result semantics; the
    /// per-step methods (`train_step`, `eval_step`) remain for callers
    /// that want finer control.
    fn run_full_training(&self, config_json: RStr<'_>) -> RResult<RString, RBoxError>;

    /// Run inference on a loaded model. Returns generated text + metadata as JSON.
    fn run_inference(
        &self,
        model: &MlModelHandle,
        prompt_json: RStr<'_>,
    ) -> RResult<RString, RBoxError>;

    /// Merge a QLoRA adapter into base weights and write to dest_path.
    /// Used by `vox mens merge-qlora` workflow.
    fn merge_adapter(
        &self,
        base_path: RStr<'_>,
        adapter_path: RStr<'_>,
        dest_path: RStr<'_>,
    ) -> RResult<(), RBoxError>;
}
