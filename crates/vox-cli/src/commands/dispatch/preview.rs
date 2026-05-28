//! `vox dispatch preview` — project the routing decision tree for a workflow without dispatching (P2-T6).

use clap::Args;
use serde::{Deserialize, Serialize};

#[derive(Debug, Args)]
pub struct PreviewArgs {
    /// Fully qualified workflow path, e.g. `my::workflow`.
    pub path: String,
    /// Workflow arguments, separated by `--`.
    #[arg(last = true)]
    pub args: Vec<String>,
}

/// Routing decision for one activity step in the preview projection.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoutingDecision {
    /// Activity has no `@remote` and no mesh policy match; would run in-proc.
    Local,
    /// Dispatcher would route to a specific peer (file affinity, label match, lease).
    Remote { peer_id: String, reason: String },
    /// Activity result cache (P2-T5) would short-circuit; no run at all.
    Cached {
        activity_id: String,
        arg_hash_hex: String,
    },
}

pub async fn run(args: PreviewArgs) -> anyhow::Result<()> {
    // Phase 3: wire to orchestrator admin client when daemon exposes preview_dispatch RPC.
    println!("[dispatch preview] path={} args={:?}", args.path, args.args);
    println!("(Phase 3: admin client dispatch preview not yet wired)");
    Ok(())
}
