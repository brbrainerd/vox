//! HttpListener extension point — HTTP listener plugins.
//! Used by Webhook and similar plugins.

use abi_stable::{sabi_trait, std_types::*};

pub const HTTP_LISTENER_REVISION: u32 = 1;

#[sabi_trait]
pub trait HttpListener: Send + Sync {
    fn revision(&self) -> u32 {
        HTTP_LISTENER_REVISION
    }
    fn start_listening(&self, config_json: RStr<'_>) -> RResult<(), RBoxError>;
    fn stop_listening(&self) -> RResult<(), RBoxError>;
}
