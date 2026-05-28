//! SkillRuntime extension point — runtime sandboxes for skill plugins.
//! Used by runtime-wasm and runtime-container.

use abi_stable::{sabi_trait, std_types::*};

pub const SKILL_RUNTIME_REVISION: u32 = 1;

#[sabi_trait]
pub trait SkillRuntime: Send + Sync {
    fn revision(&self) -> u32 {
        SKILL_RUNTIME_REVISION
    }
    fn invoke_skill(&self, skill_id: RStr<'_>, input_json: RStr<'_>) -> RResult<RString, RBoxError>;
}
