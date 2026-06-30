//! Agent lifecycle: spawn, retire, session mapping, handoff, pause/resume, heartbeat.
//!
//! All methods here operate on the `agents` / `agent_handles` maps and the supporting
//! subsystems (lock manager, affinity map, scope guard, heartbeat monitor).

mod doubt;
mod fallback;
mod propose;
mod handoff;
mod lifecycle_ops;
/// PAV phase-boundary interventions (approve_plan / skip_verify / force_verify).
mod pav_interventions;
mod registration;
mod spawn;
