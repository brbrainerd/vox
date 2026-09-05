//! `vox mesh …` — pairing and trust for the iroh transport (ADR-047).
//!
//! One verb does pairing in both directions. `vox mesh join` with no argument
//! prints this node's ticket and waits; `vox mesh join <ticket>` consumes one.
//! The user never learns the word `vox-orchestrator-d`.
//!
//! **Consuming a ticket is the highest-privilege action in the system** — it is
//! what admits a peer — so it decodes the ticket, shows the `EndpointId` it is
//! about to trust, and requires a yes. `--yes` exists for scripts and CI, not as
//! the normal path.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::Subcommand;
use iroh::EndpointId;
use iroh_tickets::endpoint::EndpointTicket;
use vox_mesh_transport::{MeshTrust, TrustLevel, identity};

#[derive(Subcommand)]
pub enum MeshCli {
    /// Pair with a peer. With no ticket: print ours and wait. With a ticket: trust that peer.
    Join {
        /// A `vox mesh join` ticket printed by the other machine. Omit to print ours.
        ticket: Option<String>,
        /// Skip the confirmation prompt. For scripts; consuming a ticket admits a peer.
        #[arg(long)]
        yes: bool,
        /// Human-readable label recorded beside the peer, e.g. the machine's name.
        #[arg(long)]
        label: Option<String>,
    },
    /// Print this node's `EndpointId` and ticket without waiting.
    Id,
    /// List trusted peers and the level each is trusted at.
    List,
    /// Remove a peer from the allowlist and close any live connection to it.
    Untrust {
        /// The peer's `EndpointId`, as shown by `vox mesh list`.
        endpoint_id: String,
    },
    /// Replace this node's identity. Prints the new ticket and who must re-pair.
    Rotate {
        /// Rotating orphans every peer that trusted the old key. Required.
        #[arg(long)]
        yes: bool,
    },
}

/// `~/.vox/mesh.key` — the persisted mesh identity.
fn key_path() -> PathBuf {
    vox_config::paths::dot_vox_user_dir().join("mesh.key")
}

/// `~/.vox/mesh_trust.json` — the `EndpointId` allowlist.
///
/// Deliberately NOT `trusted_nodes.json`, which is keyed by `node_id` from a
/// different keyspace; see `vox_mesh_transport::trust`.
fn trust_path() -> PathBuf {
    vox_config::paths::dot_vox_user_dir().join("mesh_trust.json")
}

/// Decode a ticket to the `EndpointId` it would admit, so the user is shown the
/// identity **before** being asked to approve it.
fn peer_of(ticket: &str) -> Result<(EndpointTicket, EndpointId)> {
    let t: EndpointTicket = ticket
        .trim()
        .parse()
        .context("that does not look like a `vox mesh join` ticket")?;
    let id = t.endpoint_addr().id;
    Ok((t, id))
}

pub async fn run(cmd: MeshCli, json: bool) -> Result<()> {
    match cmd {
        MeshCli::Id => {
            let sk = identity::load_or_create(&key_path())?;
            let ep = vox_mesh_transport::endpoint::bind(sk).await?;
            let ticket = EndpointTicket::new(ep.addr());
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "endpoint_id": ep.id().to_string(), "ticket": ticket.to_string() })
                );
            } else {
                println!("endpoint-id: {}", ep.id());
                println!("ticket:      {ticket}");
            }
            // Short-lived commands must close the endpoint. Dropping it logs
            // "Endpoint dropped without calling `Endpoint::close`. Aborting
            // ungracefully." and tears the socket down mid-flight.
            ep.close().await;
            Ok(())
        }

        MeshCli::List => {
            let trust = MeshTrust::at(&trust_path());
            let rows = trust.rows();
            if json {
                println!("{}", serde_json::to_string(&rows)?);
            } else if rows.is_empty() {
                println!("No trusted peers. Run `vox mesh join` on both machines to pair.");
            } else {
                for r in &rows {
                    let level = match r.level {
                        TrustLevel::Sandboxed => "sandboxed",
                        TrustLevel::Native => "NATIVE",
                    };
                    println!(
                        "{}  {:<10}  {}",
                        r.endpoint_id,
                        level,
                        r.label.as_deref().unwrap_or("")
                    );
                }
            }
            Ok(())
        }

        MeshCli::Untrust { endpoint_id } => {
            let id: EndpointId = endpoint_id
                .trim()
                .parse()
                .context("not a valid endpoint id — copy one from `vox mesh list`")?;
            let trust = MeshTrust::at(&trust_path());
            trust.untrust(&id)?;
            println!("Removed {id} and closed any live connection to it.");
            Ok(())
        }

        MeshCli::Rotate { yes } => {
            let trust = MeshTrust::at(&trust_path());
            let peers = trust.rows();
            if !yes {
                println!("Rotating this node's identity orphans every peer that trusted it.");
                if peers.is_empty() {
                    println!("No peers currently trust this node.");
                } else {
                    println!("These {} peer(s) would have to pair again:", peers.len());
                    for r in &peers {
                        println!("  {}  {}", r.endpoint_id, r.label.as_deref().unwrap_or(""));
                    }
                }
                anyhow::bail!("re-run with --yes to rotate");
            }
            let path = key_path();
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("removing {}", path.display()))?;
            }
            let sk = identity::load_or_create(&path)?;
            let ep = vox_mesh_transport::endpoint::bind(sk).await?;
            println!("New identity: {}", ep.id());
            println!("New ticket:   {}", EndpointTicket::new(ep.addr()));
            if !peers.is_empty() {
                println!("\nRe-pair with these peers — they still trust the old key:");
                for r in &peers {
                    println!("  {}  {}", r.endpoint_id, r.label.as_deref().unwrap_or(""));
                }
            }
            ep.close().await;
            Ok(())
        }

        MeshCli::Join { ticket, yes, label } => match ticket {
            None => {
                let sk = identity::load_or_create(&key_path())?;
                let ep = vox_mesh_transport::endpoint::bind(sk).await?;
                println!("Your ticket — paste it on the other machine:\n");
                println!("  vox mesh join {}\n", EndpointTicket::new(ep.addr()));
                println!("endpoint-id: {}", ep.id());
                println!("\nWaiting for the peer to pair. Ctrl-C to stop.");
                let trust = std::sync::Arc::new(MeshTrust::at(&trust_path()));
                let exec = std::sync::Arc::new(vox_mesh_transport::endpoint::ProbeOnlyExecutor);
                vox_mesh_transport::endpoint::serve(ep, trust, exec).await;
                Ok(())
            }
            Some(t) => {
                let (ticket, peer) = peer_of(&t)?;
                if !yes {
                    println!("This ticket admits:\n");
                    println!("  {peer}\n");
                    println!(
                        "Pairing lets that peer send this machine work. Received work runs\n\
                         SANDBOXED — pairing never grants native execution."
                    );
                    let ok = dialoguer::Confirm::new()
                        .with_prompt("Trust this peer?")
                        .default(false)
                        .interact()
                        .unwrap_or(false);
                    if !ok {
                        anyhow::bail!("not paired");
                    }
                }
                let trust = MeshTrust::at(&trust_path());
                trust.trust(&peer, label.as_deref())?;
                println!("Trusted {peer} (sandboxed).");

                let sk = identity::load_or_create(&key_path())?;
                let ep = vox_mesh_transport::endpoint::bind(sk).await?;
                let conn = ep
                    .connect(ticket.endpoint_addr().clone(), vox_mesh_transport::ALPN)
                    .await
                    .context("paired, but could not reach the peer — is it still running `vox mesh join`?")?;
                println!("Reachable: connected to {}.", conn.remote_id());
                println!("\nReciprocate on the other machine so it trusts you too:");
                println!("  vox mesh join {}", EndpointTicket::new(ep.addr()));
                conn.close(0u32.into(), b"paired");
                ep.close().await;
                Ok(())
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_lives_under_the_user_dot_vox_dir_not_the_repo() {
        // A repo-relative path would mint mesh identities per checkout and make
        // pairing depend on which directory `vox` was run from.
        let k = key_path();
        let t = trust_path();
        assert!(k.ends_with("mesh.key"), "{}", k.display());
        assert!(t.ends_with("mesh_trust.json"), "{}", t.display());
        assert_eq!(k.parent(), t.parent(), "both live in the same ~/.vox dir");
    }

    #[test]
    fn the_trust_store_is_not_trusted_nodes_json() {
        // Different keyspace; overloading that file makes `untrust` ambiguous.
        assert!(!trust_path().ends_with("trusted_nodes.json"));
    }

    #[test]
    fn a_ticket_is_decoded_to_the_peer_it_would_admit_before_any_prompt() {
        // The confirmation is worth nothing if it cannot name who is being let in.
        let sk = iroh::SecretKey::from_bytes(&[3u8; 32]);
        let addr = iroh::EndpointAddr::from(sk.public());
        let ticket = EndpointTicket::new(addr);
        let (_t, peer) = peer_of(&ticket.to_string()).expect("round-trips");
        assert_eq!(peer, sk.public());
    }

    #[test]
    fn garbage_is_refused_with_a_message_naming_what_was_expected() {
        let e = peer_of("not-a-ticket").unwrap_err().to_string();
        assert!(e.contains("ticket"), "{e}");
    }
}
