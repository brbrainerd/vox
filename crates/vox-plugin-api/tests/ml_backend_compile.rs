//! Compile-only test that the MlBackend trait shape is sabi-stable.
//! Runtime behavior is exercised in vox-plugin-mens-candle-cuda's tests.

use abi_stable::{erased_types::TD_Opaque, std_types::*};
use vox_plugin_api::extensions::ml_backend::{
    ML_BACKEND_REVISION, MlBackend, MlBackend_TO, MlModelHandle,
};

#[test]
fn revision_constant_is_four() {
    assert_eq!(ML_BACKEND_REVISION, 4);
}

struct DummyBackend;

impl MlBackend for DummyBackend {
    fn load_model(&self, _path: RStr<'_>) -> RResult<RBox<MlModelHandle>, RBoxError> {
        RResult::ROk(RBox::new(MlModelHandle { opaque: 0 }))
    }
    fn train_step(&self, _model: &MlModelHandle, _batch: RStr<'_>) -> RResult<RString, RBoxError> {
        RResult::ROk(RString::from("{}"))
    }
    fn eval_step(&self, _model: &MlModelHandle, _batch: RStr<'_>) -> RResult<RString, RBoxError> {
        RResult::ROk(RString::from("{}"))
    }
    fn save_checkpoint(&self, _model: &MlModelHandle, _dest: RStr<'_>) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }
    fn run_full_training(&self, _config_json: RStr<'_>) -> RResult<RString, RBoxError> {
        RResult::ROk(RString::from("{}"))
    }
    fn run_inference(
        &self,
        _model: &MlModelHandle,
        _prompt_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        RResult::ROk(RString::from("{}"))
    }
    fn merge_adapter(
        &self,
        _base_path: RStr<'_>,
        _adapter_path: RStr<'_>,
        _dest_path: RStr<'_>,
    ) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }
}

#[test]
fn dummy_backend_constructs_as_trait_object() {
    let _: MlBackend_TO<'static, RBox<()>> = MlBackend_TO::from_value(DummyBackend, TD_Opaque);
}
