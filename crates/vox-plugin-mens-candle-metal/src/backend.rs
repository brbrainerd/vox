//! CandleMetalPlugin — implements VoxPlugin + MlBackend.

use abi_stable::{erased_types::TD_Opaque, std_types::*};
use vox_plugin_api::abi::VoxPlugin;
use vox_plugin_api::extensions::ml_backend::{MlBackend, MlBackend_TO, MlModelHandle};

/// Convert an `anyhow::Error` to an `RBoxError`.
/// `anyhow::Error` does not implement `std::error::Error`, so we wrap it via
/// `std::io::Error` (which does).
fn anyhow_to_rbox(e: anyhow::Error) -> RBoxError {
    RBoxError::new(std::io::Error::other(e.to_string()))
}

#[derive(Clone)]
pub struct CandleMetalPlugin;

impl CandleMetalPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl VoxPlugin for CandleMetalPlugin {
    fn id(&self) -> RString {
        RString::from("mens-candle-metal")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }

    fn as_ml_backend(&self) -> ROption<MlBackend_TO<'static, RBox<()>>> {
        ROption::RSome(MlBackend_TO::from_value(self.clone(), TD_Opaque))
    }
}

impl MlBackend for CandleMetalPlugin {
    fn load_model(&self, model_path: RStr<'_>) -> RResult<RBox<MlModelHandle>, RBoxError> {
        match crate::model::CandleModel::load_from_path(model_path.as_str()) {
            Ok(model) => {
                // SAFETY: usize holds `Box::into_raw` from `CandleModel`; freed by `unload_model`.
                let raw = Box::into_raw(Box::new(model)) as usize;
                RResult::ROk(RBox::new(MlModelHandle { opaque: raw }))
            }
            Err(e) => RResult::RErr(anyhow_to_rbox(e)),
        }
    }

    fn unload_model(&self, model: &MlModelHandle) -> RResult<(), RBoxError> {
        if model.opaque == 0 {
            return RResult::ROk(());
        }
        #[allow(unsafe_code)]
        // SAFETY: pointer came from `Box::into_raw` in `load_model` for this plugin.
        unsafe {
            drop(Box::from_raw(model.opaque as *mut crate::model::CandleModel));
        }
        RResult::ROk(())
    }

    fn train_step(
        &self,
        model: &MlModelHandle,
        batch_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        // SAFETY: opaque was set by load_model from Box<CandleModel> — valid as long as the
        // host passes back a handle that came from this plugin instance's load_model call.
        #[allow(unsafe_code)]
        let candle_model = unsafe { &mut *(model.opaque as *mut crate::model::CandleModel) };
        match crate::training::run_train_step(candle_model, batch_json.as_str()) {
            Ok(stats_json) => RResult::ROk(RString::from(stats_json)),
            Err(e) => RResult::RErr(anyhow_to_rbox(e)),
        }
    }

    fn eval_step(
        &self,
        model: &MlModelHandle,
        batch_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        #[allow(unsafe_code)]
        let candle_model = unsafe { &*(model.opaque as *const crate::model::CandleModel) };
        match crate::training::run_eval_step(candle_model, batch_json.as_str()) {
            Ok(stats_json) => RResult::ROk(RString::from(stats_json)),
            Err(e) => RResult::RErr(anyhow_to_rbox(e)),
        }
    }

    fn save_checkpoint(&self, model: &MlModelHandle, dest: RStr<'_>) -> RResult<(), RBoxError> {
        #[allow(unsafe_code)]
        let candle_model = unsafe { &*(model.opaque as *const crate::model::CandleModel) };
        match crate::checkpoint::save(candle_model, dest.as_str()) {
            Ok(()) => RResult::ROk(()),
            Err(e) => RResult::RErr(anyhow_to_rbox(e)),
        }
    }

    fn run_full_training(&self, config_json: RStr<'_>) -> RResult<RString, RBoxError> {
        match crate::training::run_full_training(config_json.as_str()) {
            Ok(summary_json) => RResult::ROk(RString::from(summary_json)),
            Err(e) => RResult::RErr(RBoxError::new(e)),
        }
    }

    fn run_inference(
        &self,
        model: &MlModelHandle,
        prompt_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        // The model handle's opaque pointer is the directory path encoded as a usize pointer
        // to a Box<String> set in load_model. For inference we re-load via the model_path stored
        // in the CandleModel wrapper.
        #[allow(unsafe_code)]
        let candle_model = unsafe { &*(model.opaque as *const crate::model::CandleModel) };
        match crate::inference::run(&candle_model.model_path, prompt_json.as_str()) {
            Ok(s) => RResult::ROk(RString::from(s)),
            Err(e) => RResult::RErr(anyhow_to_rbox(e)),
        }
    }

    fn merge_adapter(
        &self,
        base_path: RStr<'_>,
        adapter_path: RStr<'_>,
        dest_path: RStr<'_>,
    ) -> RResult<(), RBoxError> {
        match crate::merge::merge_qlora_adapter(
            base_path.as_str(),
            adapter_path.as_str(),
            dest_path.as_str(),
        ) {
            Ok(()) => RResult::ROk(()),
            Err(e) => RResult::RErr(anyhow_to_rbox(e)),
        }
    }
}
