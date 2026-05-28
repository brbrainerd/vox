//! Orchestrator service layer: routing, scaling, messaging gateway, and policy.
//!
//! These modules provide separation of concerns while the main `Orchestrator`
//! retains API-compatible wrappers that delegate to these services.
//!
//! ## Service boundaries
//!
//! - **RoutingService** (`routing` submodule): File-affinity and group-based task routing.
//!   Inputs: file manifest, affinity map, group registry, agent queues, config.
//!   Output: `RouteResult::Existing(AgentId)` or `RouteResult::SpawnAgent(name)`.
//!   Orchestrator calls `resolve_route()` which uses this and performs spawn when needed.
//!
//! - **ScalingService** (`scaling` submodule): Scale-up/down decisions from load and policy.
//!   Inputs: status, config, load history, idle dynamic agents (id, last_active).
//!   Output: `ScalingAction::NoOp | ScaleUp { name } | ScaleDown { agent_ids }`.
//!   Runtime/orchestrator applies the action (spawn_dynamic_agent / retire_agent).
//!
//! - **MessageGateway** (`gateway` submodule): Unified fan-out to bulletin, A2A bus, event bus.
//!   Functions take mutable refs to the buses and publish task completed/failed,
//!   agent spawned/retired so dashboard and monitors stay in sync.
//!
//! - **PolicyEngine** (`policy` submodule): Pre-queue validation (locks and optional scope).
//!   Inputs: lock manager, optional scope guard, event bus, manifest, agent id.
//!   Output: `PolicyCheckResult::Allowed | LockConflict(...) | ScopeDenied(...)`.
//!   Call before enqueueing to fail fast and emit scope violation events.

pub mod discovery_gate;
pub mod gateway;
pub mod persistence_obs;
// `routes` moved to vox-orchestrator-mcp in 2026-05-08 reorg Phase 4.
// Compatibility stub below keeps old `vox_orchestrator::services::routes` call sites
// compiling until they are updated to import from vox-orchestrator-mcp.
#[cfg(feature = "news-publish")]
pub mod news;
pub mod policy;
pub mod routes;
pub mod routing;
pub mod scaling;

pub mod campaign_scheduler;
pub mod embeddings;
pub mod flywheel;
pub mod topology_ingest;

pub use campaign_scheduler::{CampaignSchedulePlan, CampaignScheduler, CampaignSchedulingMode};
pub use discovery_gate::DiscoveryGate;
pub use gateway::MessageGateway;
pub use policy::{PolicyCheckResult, PolicyEngine, PolicyTrustRelax};
pub use routing::{RouteResult, RoutingService};
pub use scaling::{ScalingAction, ScalingService};
