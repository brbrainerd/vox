use std::time::Duration;

#[cfg(feature = "runtime")]
pub(super) const AGENT_NOTIFY_TIMEOUT: Duration = vox_config::timeouts::D_30S;

mod attention_fields;
mod batch;
pub(crate) mod dei_plan_materialize;
mod goal;
mod task_submit;
