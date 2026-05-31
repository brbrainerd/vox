//! Data-free k-quant post-training quantization engine.
//!
//! SafeTensors model in -> quantized SafeTensors-canonical artifact out.
//! Device-selectable: GPU when available (cuda/metal feature), CPU fallback.
pub mod error;
pub mod device;
pub mod policy;
pub mod read;
pub mod engine;
pub mod verify;
pub mod write;

// TODO(SP-1 Task 2+): re-export public API once each module defines its items.
pub use device::DevicePref;
// pub use engine::{quantize, QuantizeRequest};
pub use error::QuantizeError;
pub use policy::{QuantMixture, TensorRole};
pub use verify::{QuantReport, TensorQuantStat};
