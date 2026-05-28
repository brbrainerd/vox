//! Extension-point trait modules. SP2 ships placeholders only.
//! SP3 fills in MlBackend; SP7 fills in the rest.

pub mod audio_capture;
pub mod browser_automation;
pub mod cloud_sync;
pub mod hardware_probe;
pub mod mesh_driver;
pub mod ml_backend;
pub mod publication;
pub mod script_executor;
pub mod speech_to_text;
pub mod tensor_backend;
pub mod http_listener;
pub mod skill_runtime;
