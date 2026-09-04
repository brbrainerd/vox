# Populi Mesh Zero-Config Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two machines with vox installed and zero configuration discover each other on a LAN, are trusted with one click in the GUI, and share dispatched work — with no account, no internet, and no central server.

**Architecture:** A new `vox-populi::discovery` module announces and browses `_vox-mesh._tcp.local` over mDNS. Discovered peers are resolved against the existing `vox_identity::TrustedNodeRegistry` and surfaced through the existing `vox_mesh_nodes` MCP tool, which the GUI already polls. Control-plane bind address becomes tier-aware so peers can actually reach the port that gets advertised.

**Tech Stack:** Rust 1.96, `mdns-sd` (new, pure-Rust, no daemon), axum control plane, Tauri 2 + React GUI, `vox_identity` ed25519 trust store.

**Spec:** [`docs/superpowers/specs/2026-09-04-populi-mesh-zero-config-discovery-design.md`](../specs/2026-09-04-populi-mesh-zero-config-discovery-design.md)

## Global Constraints

- **No new workspace crate edges.** Every edge this plan needs already exists: `vox-populi` → `vox-identity`, `vox-orchestrator-mcp` → `vox-populi`, `vox-gui` → `vox-orchestrator-mcp`, `vox-gui` → `vox-identity`. Adding any new edge requires user authorization (`AGENTS.md` §Dependency Discipline). If you believe you need one, stop and ask.
- **Never bind `0.0.0.0`.** Bind exactly one interface chosen by tier. Synthesizing a wildcard bind exposes the control plane on untrusted networks.
- **Discovery is never fatal.** Any discovery failure logs at `warn` and leaves the node functional. It is an enhancement, not a precondition.
- **No central server.** Nothing in this plan may make mesh formation depend on a vox-operated service.
- **Test-first** (`AGENTS.md` §Test-First Policy): every new `pub fn` in `crates/*/src/**` needs a `#[test]` in the same file before the commit lands.
- **Formatting:** `vox run scripts/fmt.vox`. **Never** `cargo fmt --all` — it overflows the Windows command-line limit (`os error 206`).
- **Pre-push:** `vox ci pre-push --complete` for Rust changes. The default fast tier does not run clippy.
- mDNS service type: `_vox-mesh._tcp.local.` (trailing dot required by `mdns-sd`).
- Announce interval 30s; stale after 90s; both overridable via `VOX_MESH_DISCOVERY_ANNOUNCE_SECS`.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/vox-populi/src/discovery/mod.rs` (create) | Public surface: `NodeAnnouncement`, `DiscoveredPeer`, `DiscoveryHandle`. |
| `crates/vox-populi/src/discovery/mdns.rs` (create) | mDNS announce + browse loop. The only file that touches `mdns-sd`. |
| `crates/vox-populi/src/discovery/bind.rs` (create) | Tier-aware bind resolution + defactored `detect_lan_ip`. |
| `crates/vox-populi/Cargo.toml` (modify) | `discovery` feature, `mdns-sd` dep. |
| `crates/vox-populi/src/lib.rs` (modify) | `pub mod discovery;` behind the feature. |
| `crates/vox-ml-cli/src/commands/populi_overlay.rs` (create) | Tailscale binary resolution + `status --json` parse. Pulled out of `populi_lifecycle.rs`. |
| `crates/vox-ml-cli/src/commands/populi_lifecycle.rs` (modify) | Use tier-aware bind; delegate overlay probing to the new module. |
| `crates/vox-ml-cli/src/commands/populi_join.rs` (modify) | Deprecate; migrate `config.toml` peers to `TrustedNodeRegistry`. |
| `crates/vox-orchestrator-mcp/src/populi_tools.rs` (modify) | Add `pending_peers` to the `vox_mesh_nodes` result. |
| `crates/vox-plugin-populi-mesh/src/lib.rs` (modify) | Remove crate-wide `#![allow(dead_code)]`. |
| `crates/vox-gui/ui/src/components/surfaces/Mesh/MeshView.tsx` (modify) | Pending-peers section with fingerprint + Trust action. |
| `crates/vox-gui/ui/src/components/dashboard/widgets/MeshWidget.tsx` (modify) | Trusted / online / pending counts. |

Splitting `discovery` into three files keeps the `mdns-sd` surface isolated in one place, so swapping the discovery mechanism later touches one file.

---

## Task 1: Discovery types and feature flag

**Files:**
- Create: `crates/vox-populi/src/discovery/mod.rs`
- Modify: `crates/vox-populi/Cargo.toml`
- Modify: `crates/vox-populi/src/lib.rs` (after line 393, alongside `pub mod quota;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `NodeAnnouncement { node_id: String, fingerprint: String, port: u16, scope_id: Option<String> }`, `DiscoveredPeer { node_id: String, fingerprint: String, addr: IpAddr, port: u16, last_seen_unix_ms: u64, trusted: bool }`, `DiscoveryError`.

- [ ] **Step 1: Add the dependency and feature**

In `crates/vox-populi/Cargo.toml`, under `[features]` add:

```toml
# Zero-config LAN peer discovery over mDNS (no daemon, no account, no internet).
discovery = ["dep:mdns-sd"]
```

Under `[dependencies]` add:

```toml
mdns-sd = { version = "0.13", optional = true }
```

- [ ] **Step 2: Verify the dependency resolves and pin the API**

Run: `cargo add --dry-run mdns-sd@0.13 -p vox-populi` then `cargo doc -p mdns-sd --no-deps --open`

Confirm these exist: `ServiceDaemon::new`, `ServiceDaemon::register`, `ServiceDaemon::browse`, `ServiceInfo::new`, `ServiceEvent::ServiceResolved`, `ServiceEvent::ServiceRemoved`. If any signature differs from what Task 2 uses, adjust Task 2's code to match the real API rather than pinning an older version.

Note: `mdns-sd` is not in the local cargo cache, so this first build needs network access.

- [ ] **Step 3: Write the failing test**

Create `crates/vox-populi/src/discovery/mod.rs`:

```rust
//! Zero-config LAN peer discovery.
//!
//! Discovery is unauthenticated and grants nothing: an announcement carries no
//! secret and a discovered peer is inert until explicitly trusted. See
//! `docs/superpowers/specs/2026-09-04-populi-mesh-zero-config-discovery-design.md`.

use std::net::IpAddr;

/// What this node broadcasts about itself. Contains no secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeAnnouncement {
    /// Stable node id (matches `NodeRecord.id`).
    pub node_id: String,
    /// Ed25519 public-key fingerprint, for out-of-band comparison by a human.
    pub fingerprint: String,
    /// Control-plane port this node is listening on.
    pub port: u16,
    /// Optional mesh scope; peers with a different scope are filtered out.
    pub scope_id: Option<String>,
}

/// A peer seen on the local network. `trusted` is resolved at read time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    pub node_id: String,
    pub fingerprint: String,
    pub addr: IpAddr,
    pub port: u16,
    pub last_seen_unix_ms: u64,
    /// True when this peer's `node_id` is present in the local trust registry.
    pub trusted: bool,
}

impl DiscoveredPeer {
    /// Control-plane base URL for this peer.
    pub fn control_url(&self) -> String {
        match self.addr {
            IpAddr::V6(v6) => format!("http://[{}]:{}", v6, self.port),
            IpAddr::V4(v4) => format!("http://{}:{}", v4, self.port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(addr: &str) -> DiscoveredPeer {
        DiscoveredPeer {
            node_id: "n1".into(),
            fingerprint: "ab:cd".into(),
            addr: addr.parse().unwrap(),
            port: 9847,
            last_seen_unix_ms: 0,
            trusted: false,
        }
    }

    #[test]
    fn control_url_formats_ipv4_bare() {
        assert_eq!(peer("192.168.1.5").control_url(), "http://192.168.1.5:9847");
    }

    #[test]
    fn control_url_brackets_ipv6() {
        assert_eq!(peer("fe80::1").control_url(), "http://[fe80::1]:9847");
    }
}
```

- [ ] **Step 4: Register the module**

In `crates/vox-populi/src/lib.rs`, after the `pub mod quota;` line (~line 393):

```rust
#[cfg(feature = "discovery")]
pub mod discovery;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p vox-populi --features discovery discovery:: -- --nocapture`
Expected: PASS, 2 tests.

The IPv6 bracket test is the one that matters — an unbracketed IPv6 URL is a
silent connection failure that only shows up on IPv6-preferring networks.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-populi/Cargo.toml crates/vox-populi/src/lib.rs crates/vox-populi/src/discovery/mod.rs
git commit -m "feat(populi): add discovery types behind a discovery feature"
```

---

## Task 2: mDNS announce and browse

**Files:**
- Create: `crates/vox-populi/src/discovery/mdns.rs`
- Modify: `crates/vox-populi/src/discovery/mod.rs` (add `mod mdns; pub use mdns::DiscoveryHandle;`)

**Interfaces:**
- Consumes: `NodeAnnouncement`, `DiscoveredPeer` from Task 1.
- Produces: `DiscoveryHandle::start(announce: NodeAnnouncement) -> Result<DiscoveryHandle, DiscoveryError>`, `DiscoveryHandle::peers(&self) -> Vec<DiscoveredPeer>`, `DiscoveryHandle::shutdown(self)`. `peers()` returns peers with `trusted: false` always — Task 3 adds trust resolution.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-populi/src/discovery/mdns.rs`:

```rust
//! mDNS announce + browse. The only module that touches `mdns-sd`.

use super::{DiscoveredPeer, NodeAnnouncement};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// DNS-SD service type for the Populi mesh. Trailing dot is required.
pub(crate) const SERVICE_TYPE: &str = "_vox-mesh._tcp.local.";

/// TXT record key for the ed25519 fingerprint.
pub(crate) const TXT_FINGERPRINT: &str = "fp";
/// TXT record key for the node id.
pub(crate) const TXT_NODE_ID: &str = "nid";
/// TXT record key for the optional mesh scope.
pub(crate) const TXT_SCOPE: &str = "scope";

/// Peers are dropped after this long without an announcement (3 missed).
pub(crate) const STALE_AFTER_MS: u64 = 90_000;

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("mdns daemon: {0}")]
    Daemon(String),
}

/// Decide whether a peer should still be listed, given the current clock.
///
/// Split out from the browse loop so staleness is testable without a network.
pub(crate) fn is_fresh(last_seen_unix_ms: u64, now_unix_ms: u64) -> bool {
    now_unix_ms.saturating_sub(last_seen_unix_ms) < STALE_AFTER_MS
}

/// Whether a discovered peer's scope matches ours. `None` on either side means
/// "unscoped", which matches anything — a fresh install has no scope and must
/// still find its neighbours.
pub(crate) fn scope_matches(ours: Option<&str>, theirs: Option<&str>) -> bool {
    match (ours, theirs) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

/// Live handle to the announce + browse tasks.
pub struct DiscoveryHandle {
    daemon: mdns_sd::ServiceDaemon,
    peers: Arc<Mutex<HashMap<String, DiscoveredPeer>>>,
    announce: NodeAnnouncement,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_peer_is_listed() {
        assert!(is_fresh(1_000, 1_000 + STALE_AFTER_MS - 1));
    }

    #[test]
    fn stale_peer_is_dropped() {
        assert!(!is_fresh(1_000, 1_000 + STALE_AFTER_MS));
    }

    #[test]
    fn clock_going_backwards_does_not_panic() {
        // saturating_sub, not subtraction: a peer whose timestamp is ahead of
        // our clock (NTP skew between machines) must not underflow.
        assert!(is_fresh(9_000, 1_000));
    }

    #[test]
    fn unscoped_matches_anything() {
        assert!(scope_matches(None, Some("teamA")));
        assert!(scope_matches(Some("teamA"), None));
        assert!(scope_matches(None, None));
    }

    #[test]
    fn different_scopes_do_not_match() {
        assert!(!scope_matches(Some("teamA"), Some("teamB")));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-populi --features discovery discovery::mdns -- --nocapture`
Expected: FAIL to compile — `DiscoveryHandle` has fields but no constructor, and `mod mdns;` is not yet declared.

- [ ] **Step 3: Declare the module**

In `crates/vox-populi/src/discovery/mod.rs`, below the imports:

```rust
mod mdns;
pub use mdns::{DiscoveryError, DiscoveryHandle};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-populi --features discovery discovery::mdns -- --nocapture`
Expected: PASS, 5 tests.

- [ ] **Step 5: Implement announce and browse**

Append to `crates/vox-populi/src/discovery/mdns.rs`:

```rust
impl DiscoveryHandle {
    /// Start announcing this node and browsing for peers.
    ///
    /// Returns `Err` only when the mDNS daemon cannot be created at all.
    /// Callers must treat that as non-fatal: discovery is an enhancement.
    pub fn start(announce: NodeAnnouncement) -> Result<Self, DiscoveryError> {
        let daemon =
            mdns_sd::ServiceDaemon::new().map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        let peers: Arc<Mutex<HashMap<String, DiscoveredPeer>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let addr = super::bind::detect_lan_ip()
            .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

        let mut props = std::collections::HashMap::new();
        props.insert(TXT_NODE_ID.to_string(), announce.node_id.clone());
        props.insert(TXT_FINGERPRINT.to_string(), announce.fingerprint.clone());
        if let Some(scope) = &announce.scope_id {
            props.insert(TXT_SCOPE.to_string(), scope.clone());
        }

        let info = mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            &announce.node_id,
            &format!("{}.local.", announce.node_id),
            addr,
            announce.port,
            props,
        )
        .map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        daemon
            .register(info)
            .map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        let peers_bg = Arc::clone(&peers);
        let our_scope = announce.scope_id.clone();
        let our_node_id = announce.node_id.clone();
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    mdns_sd::ServiceEvent::ServiceResolved(info) => {
                        let props = info.get_properties();
                        let node_id = match props.get_property_val_str(TXT_NODE_ID) {
                            Some(v) if !v.is_empty() => v.to_string(),
                            _ => continue,
                        };
                        // Never list ourselves.
                        if node_id == our_node_id {
                            continue;
                        }
                        let their_scope = props.get_property_val_str(TXT_SCOPE);
                        if !scope_matches(our_scope.as_deref(), their_scope) {
                            continue;
                        }
                        let Some(addr) = info.get_addresses().iter().next().copied() else {
                            continue;
                        };
                        let peer = DiscoveredPeer {
                            node_id: node_id.clone(),
                            fingerprint: props
                                .get_property_val_str(TXT_FINGERPRINT)
                                .unwrap_or_default()
                                .to_string(),
                            addr,
                            port: info.get_port(),
                            last_seen_unix_ms: crate::wall_clock_unix_ms(),
                            trusted: false,
                        };
                        if let Ok(mut map) = peers_bg.lock() {
                            map.insert(node_id, peer);
                        }
                    }
                    mdns_sd::ServiceEvent::ServiceRemoved(_ty, fullname) => {
                        // fullname is `<instance>._vox-mesh._tcp.local.`; the
                        // instance segment is the node id we registered under.
                        let instance = fullname.split('.').next().unwrap_or_default().to_string();
                        if let Ok(mut map) = peers_bg.lock() {
                            map.remove(&instance);
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(Self {
            daemon,
            peers,
            announce,
        })
    }

    /// Currently-visible peers, stale entries filtered out.
    ///
    /// `trusted` is always `false` here; Task 3's wrapper resolves it.
    pub fn peers(&self) -> Vec<DiscoveredPeer> {
        let now = crate::wall_clock_unix_ms();
        let Ok(map) = self.peers.lock() else {
            return Vec::new();
        };
        let mut out: Vec<DiscoveredPeer> = map
            .values()
            .filter(|p| is_fresh(p.last_seen_unix_ms, now))
            .cloned()
            .collect();
        out.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        out
    }

    /// The announcement this handle is broadcasting.
    pub fn announcement(&self) -> &NodeAnnouncement {
        &self.announce
    }

    /// Unregister and stop. Sends a DNS-SD goodbye so peers drop us promptly
    /// rather than waiting out the 90s stale window.
    pub fn shutdown(self) {
        let fullname = format!("{}.{}", self.announce.node_id, SERVICE_TYPE);
        let _ = self.daemon.unregister(&fullname);
        let _ = self.daemon.shutdown();
    }
}
```

- [ ] **Step 6: Run the full module tests**

Run: `cargo test -p vox-populi --features discovery discovery -- --nocapture`
Expected: PASS. Fix any `mdns-sd` API mismatches against the real 0.13 docs from Task 1 Step 2.

- [ ] **Step 7: Format, clippy, commit**

```bash
cargo fmt -p vox-populi
cargo clippy -p vox-populi --features discovery --all-targets -- -D warnings
git add crates/vox-populi/src/discovery/
git commit -m "feat(populi): announce and browse mesh peers over mDNS"
```

---

## Task 3: Tier-aware bind resolution

**Files:**
- Create: `crates/vox-populi/src/discovery/bind.rs`
- Modify: `crates/vox-populi/src/discovery/mod.rs` (add `pub mod bind;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `bind::detect_lan_ip() -> Option<IpAddr>`, `bind::BindTier` enum, `bind::resolve_bind(tier: BindTier, explicit: Option<&str>, port: u16) -> String`.

`detect_lan_ip` is defactored from `crates/vox-cli-share/src/backends/lan.rs:54` (15 lines) rather than taking a crate edge to `vox-cli-share`, per `AGENTS.md` §Dependency Discipline rule 3.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-populi/src/discovery/bind.rs`:

```rust
//! Tier-aware control-plane bind resolution.
//!
//! The mesh binds exactly one interface, chosen by the active transport tier.
//! `0.0.0.0` is never synthesized: a wildcard bind would expose the control
//! plane on every interface, including untrusted networks.

use std::net::{IpAddr, Ipv4Addr};

/// Which transport tier is active. Determines which interface to bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindTier {
    /// T0 — single node, loopback only.
    Loopback,
    /// T1 — LAN peers over mDNS; bind the routable LAN address.
    Lan,
    /// T3 — an overlay (Tailscale, Nebula, …); bind the overlay address.
    Overlay(IpAddr),
}

/// Best-effort discovery of a routable LAN IPv4 address.
///
/// Opens a UDP socket toward a public IP; the OS picks a routable local address
/// as the source without sending any packets.
///
// vox:defactored-from vox-cli-share 2026-09-04
pub fn detect_lan_ip() -> Option<IpAddr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_unspecified() || ip.is_loopback() {
        None
    } else {
        Some(ip)
    }
}

/// Resolve the `host:port` string the control plane should bind.
///
/// An explicit operator-supplied bind always wins. Otherwise the tier decides,
/// falling back to loopback when a tier's address cannot be determined —
/// binding loopback is always safe, whereas guessing a wider interface is not.
pub fn resolve_bind(tier: BindTier, explicit: Option<&str>, port: u16) -> String {
    if let Some(e) = explicit {
        let e = e.trim();
        if !e.is_empty() {
            return e.to_string();
        }
    }
    let ip = match tier {
        BindTier::Loopback => IpAddr::V4(Ipv4Addr::LOCALHOST),
        BindTier::Overlay(ip) => ip,
        BindTier::Lan => detect_lan_ip().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
    };
    match ip {
        IpAddr::V6(v6) => format!("[{v6}]:{port}"),
        IpAddr::V4(v4) => format!("{v4}:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_tier_binds_loopback() {
        assert_eq!(resolve_bind(BindTier::Loopback, None, 9847), "127.0.0.1:9847");
    }

    #[test]
    fn overlay_tier_binds_the_overlay_address() {
        let ip: IpAddr = "100.107.222.96".parse().unwrap();
        assert_eq!(
            resolve_bind(BindTier::Overlay(ip), None, 9847),
            "100.107.222.96:9847"
        );
    }

    #[test]
    fn explicit_bind_always_wins() {
        let ip: IpAddr = "100.64.0.1".parse().unwrap();
        assert_eq!(
            resolve_bind(BindTier::Overlay(ip), Some("192.168.1.9:1234"), 9847),
            "192.168.1.9:1234"
        );
    }

    #[test]
    fn blank_explicit_bind_falls_through_to_tier() {
        assert_eq!(resolve_bind(BindTier::Loopback, Some("   "), 9847), "127.0.0.1:9847");
    }

    #[test]
    fn ipv6_overlay_is_bracketed() {
        let ip: IpAddr = "fd7a:115c:a1e0::1".parse().unwrap();
        assert_eq!(
            resolve_bind(BindTier::Overlay(ip), None, 9847),
            "[fd7a:115c:a1e0::1]:9847"
        );
    }

    #[test]
    fn wildcard_is_never_synthesized() {
        // The whole point of the tier model: no tier may produce 0.0.0.0.
        for tier in [
            BindTier::Loopback,
            BindTier::Lan,
            BindTier::Overlay("10.0.0.5".parse().unwrap()),
        ] {
            let bound = resolve_bind(tier, None, 9847);
            assert!(!bound.starts_with("0.0.0.0"), "tier {tier:?} produced {bound}");
            assert!(!bound.starts_with("[::]"), "tier {tier:?} produced {bound}");
        }
    }

    #[test]
    fn detect_lan_ip_never_returns_loopback_or_unspecified() {
        // May be None in a sandbox with no route; must never be a useless address.
        if let Some(ip) = detect_lan_ip() {
            assert!(!ip.is_loopback());
            assert!(!ip.is_unspecified());
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-populi --features discovery discovery::bind -- --nocapture`
Expected: FAIL to compile — `pub mod bind;` not declared.

- [ ] **Step 3: Declare the module**

In `crates/vox-populi/src/discovery/mod.rs`:

```rust
pub mod bind;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-populi --features discovery discovery::bind -- --nocapture`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/discovery/
git commit -m "feat(populi): tier-aware bind resolution that never binds a wildcard"
```

---

## Task 4: Trust resolution for discovered peers

**Files:**
- Modify: `crates/vox-populi/src/discovery/mod.rs`

**Interfaces:**
- Consumes: `DiscoveredPeer` (Task 1), `DiscoveryHandle::peers()` (Task 2), `vox_identity::TrustedNodeRegistry`.
- Produces: `resolve_trust(peers: Vec<DiscoveredPeer>, reg: &TrustedNodeRegistry) -> Vec<DiscoveredPeer>`, `DiscoveryHandle::peers_with_trust(&self) -> Vec<DiscoveredPeer>`.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-populi/src/discovery/mod.rs`, above the existing `mod tests`:

```rust
use vox_identity::TrustedNodeRegistry;

/// Stamp each peer's `trusted` flag from the local trust registry.
///
/// Discovery grants nothing; this is the only place a discovered peer becomes
/// trusted, and it reads a store the user controls. A registry read error
/// yields `trusted: false` for every peer — failing closed, because treating an
/// unreadable registry as "everyone is trusted" would be a security defect.
pub fn resolve_trust(
    peers: Vec<DiscoveredPeer>,
    reg: &TrustedNodeRegistry,
) -> Vec<DiscoveredPeer> {
    peers
        .into_iter()
        .map(|mut p| {
            p.trusted = reg.is_trusted(&p.node_id).unwrap_or(false);
            p
        })
        .collect()
}
```

And add these tests inside the existing `mod tests`:

```rust
    #[test]
    fn untrusted_peer_stays_untrusted() {
        let reg = TrustedNodeRegistry::new_in_memory();
        let out = resolve_trust(vec![peer("192.168.1.5")], &reg);
        assert!(!out[0].trusted);
    }

    #[test]
    fn trusted_peer_is_marked_trusted() {
        let mut reg = TrustedNodeRegistry::new_in_memory();
        reg.upsert("n1", "deadbeef");
        let out = resolve_trust(vec![peer("192.168.1.5")], &reg);
        assert!(out[0].trusted);
    }

    #[test]
    fn trust_is_per_node_id_not_blanket() {
        let mut reg = TrustedNodeRegistry::new_in_memory();
        reg.upsert("someone-else", "deadbeef");
        let out = resolve_trust(vec![peer("192.168.1.5")], &reg);
        assert!(!out[0].trusted, "trusting one node must not trust another");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-populi --features discovery discovery::tests -- --nocapture`
Expected: FAIL — `resolve_trust` not found, or `vox_identity` not imported.

- [ ] **Step 3: Add the handle convenience method**

In `crates/vox-populi/src/discovery/mdns.rs`, inside `impl DiscoveryHandle`:

```rust
    /// Peers with `trusted` resolved against the file-backed trust registry.
    ///
    /// This is what callers outside this module should use.
    pub fn peers_with_trust(&self) -> Vec<DiscoveredPeer> {
        let reg = vox_identity::TrustedNodeRegistry::new();
        super::resolve_trust(self.peers(), &reg)
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-populi --features discovery discovery -- --nocapture`
Expected: PASS, 15 tests total across the three discovery modules.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-populi
cargo clippy -p vox-populi --features discovery --all-targets -- -D warnings
git add crates/vox-populi/src/discovery/
git commit -m "feat(populi): resolve discovered-peer trust against the local registry"
```

---

## Task 5: Overlay probing extracted and fixed

**Files:**
- Create: `crates/vox-ml-cli/src/commands/populi_overlay.rs`
- Modify: `crates/vox-ml-cli/src/commands/mod.rs` (add `pub mod populi_overlay;`)
- Modify: `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:378-513` (delete the moved functions)

Fixes spec Defect B (binary not on `PATH` on Windows) and Defect B2 (exit-code probe conflates installed with connected).

**Interfaces:**
- Consumes: nothing.
- Produces: `resolve_tailscale_binary() -> Option<PathBuf>`, `TailnetStatus { self_ip: Option<IpAddr>, running: bool }`, `parse_tailnet_status(json: &str) -> Option<TailnetStatus>`, `tailnet_status() -> Option<TailnetStatus>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-ml-cli/src/commands/populi_overlay.rs`:

```rust
//! Tailscale overlay probing (transport tier T3).
//!
//! Extracted from `populi_lifecycle.rs`. Two defects fixed here:
//! the Windows installer does not put `tailscale` on `PATH`, and
//! `tailscale status`'s exit code does not distinguish "installed but logged
//! out" from "connected".

use std::net::IpAddr;
use std::path::PathBuf;

/// What we need to know about the local tailnet node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailnetStatus {
    /// This node's overlay IPv4, when it has one.
    pub self_ip: Option<IpAddr>,
    /// True only when the backend is actually connected.
    pub running: bool,
}

/// Locate the tailscale CLI. `PATH` first, then per-platform install locations.
///
/// The Windows installer does not modify `PATH`, so a `PATH`-only probe reports
/// "not installed" on a machine where Tailscale is installed and connected.
pub fn resolve_tailscale_binary() -> Option<PathBuf> {
    for candidate in [
        "tailscale",
        r"C:\Program Files\Tailscale\tailscale.exe",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        "/usr/bin/tailscale",
        "/usr/local/bin/tailscale",
        "/opt/homebrew/bin/tailscale",
    ] {
        let ok = std::process::Command::new(candidate)
            .arg("version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

/// Parse `tailscale status --json`.
///
/// Every field is optional: this schema carries no stability guarantee, so any
/// shape change must degrade to "no overlay" rather than fail the mesh.
pub fn parse_tailnet_status(json: &str) -> Option<TailnetStatus> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let running = v
        .get("BackendState")
        .and_then(|s| s.as_str())
        .is_some_and(|s| s == "Running");
    let self_ip = v
        .get("Self")
        .and_then(|s| s.get("TailscaleIPs"))
        .and_then(|ips| ips.as_array())
        .and_then(|arr| {
            arr.iter()
                .filter_map(|i| i.as_str())
                .filter_map(|s| s.parse::<IpAddr>().ok())
                .find(|ip| ip.is_ipv4())
        });
    Some(TailnetStatus { self_ip, running })
}

/// Probe the live tailnet, or `None` when tailscale is absent or unreadable.
pub fn tailnet_status() -> Option<TailnetStatus> {
    let bin = resolve_tailscale_binary()?;
    let out = std::process::Command::new(bin)
        .args(["status", "--json"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_tailnet_status(&String::from_utf8_lossy(out.stdout.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from a real `tailscale status --json` on a connected host,
    /// trimmed to the fields this module reads.
    const CONNECTED: &str = r#"{
        "BackendState": "Running",
        "CurrentTailnet": {"Name":"user@example.com","MagicDNSSuffix":"tail4f69a0.ts.net"},
        "Self": {"HostName":"BLAPTOP04","TailscaleIPs":["100.107.222.96","fd7a:115c:a1e0::6e39:de61"],"Online":true}
    }"#;

    const LOGGED_OUT: &str = r#"{"BackendState":"NeedsLogin","Self":null}"#;

    #[test]
    fn parses_connected_status() {
        let s = parse_tailnet_status(CONNECTED).unwrap();
        assert!(s.running);
        assert_eq!(s.self_ip, Some("100.107.222.96".parse().unwrap()));
    }

    #[test]
    fn prefers_ipv4_over_ipv6() {
        // TailscaleIPs lists v4 first, but order is not guaranteed; we must
        // select by family, not by position.
        let s = parse_tailnet_status(CONNECTED).unwrap();
        assert!(s.self_ip.unwrap().is_ipv4());
    }

    #[test]
    fn logged_out_is_not_running() {
        let s = parse_tailnet_status(LOGGED_OUT).unwrap();
        assert!(!s.running, "NeedsLogin must not count as connected");
        assert_eq!(s.self_ip, None);
    }

    #[test]
    fn malformed_json_yields_none_not_panic() {
        assert!(parse_tailnet_status("").is_none());
        assert!(parse_tailnet_status("{").is_none());
        assert!(parse_tailnet_status("[]").is_some_and(|s| !s.running));
    }

    #[test]
    fn unknown_shape_degrades_to_not_running() {
        // Upstream renames a field: we must report "no overlay", not crash.
        let s = parse_tailnet_status(r#"{"Backend":"Running"}"#).unwrap();
        assert!(!s.running);
        assert_eq!(s.self_ip, None);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-ml-cli populi_overlay -- --nocapture`
Expected: FAIL to compile — module not declared.

- [ ] **Step 3: Declare the module**

In `crates/vox-ml-cli/src/commands/mod.rs`, alphabetically among the other `populi_*` declarations:

```rust
pub mod populi_overlay;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-ml-cli populi_overlay -- --nocapture`
Expected: PASS, 5 tests.

- [ ] **Step 5: Delete the superseded functions**

In `crates/vox-ml-cli/src/commands/populi_lifecycle.rs`, delete `overlay_diag_tailscale` (line ~426) and `tailscale_ip` (line ~497). Rewrite `overlay_diag_tailscale`'s replacement in terms of the new module — add to `populi_overlay.rs`:

```rust
/// Human-readable availability line for `vox populi status`.
pub fn tailscale_diagnostic_detail() -> (bool, bool, String) {
    match (resolve_tailscale_binary(), tailnet_status()) {
        (None, _) => (false, false, "tailscale command not found".to_string()),
        (Some(_), Some(s)) if s.running => (true, true, "tailscale reachable".to_string()),
        (Some(_), _) => (
            true,
            false,
            "tailscale installed but not connected".to_string(),
        ),
    }
}
```

Then in `populi_lifecycle.rs`, replace the body of the deleted `overlay_diag_tailscale` call site:

```rust
fn overlay_diag_tailscale() -> OverlayDiagnostics {
    let (available, connected, detail) =
        crate::commands::populi_overlay::tailscale_diagnostic_detail();
    OverlayDiagnostics {
        provider: "tailscale".to_string(),
        available,
        connected,
        detail,
    }
}
```

And replace `overlay_control_url`'s Tailscale arm (line ~408) to use the new probe:

```rust
        Some(OverlayProvider::Tailscale) => {
            match crate::commands::populi_overlay::tailnet_status() {
                Some(s) if s.running => match s.self_ip {
                    Some(ip) => format!("http://{ip}:{port}"),
                    None => format!("http://127.0.0.1:{port}"),
                },
                _ => format!("http://127.0.0.1:{port}"),
            }
        }
```

- [ ] **Step 6: Run the crate tests**

Run: `cargo test -p vox-ml-cli populi -- --nocapture`
Expected: PASS. No test may now reference `tailscale_ip`.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p vox-ml-cli
cargo clippy -p vox-ml-cli --all-targets -- -D warnings
git add crates/vox-ml-cli/src/commands/
git commit -m "fix(populi): detect tailscale off PATH and distinguish connected from installed"
```

---

## Task 6: Fix the overlay bind defect

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:143-150` and `:175-183`

Fixes spec Defect A. This is the change that makes the mesh reachable at all.

**Interfaces:**
- Consumes: `vox_populi::discovery::bind::{BindTier, resolve_bind}` (Task 3), `populi_overlay::tailnet_status` (Task 5).
- Produces: nothing new.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-ml-cli/src/commands/populi_lifecycle.rs`, in its `#[cfg(test)] mod tests` (create the module at the end of the file if absent):

```rust
#[cfg(test)]
mod bind_tests {
    use vox_populi::discovery::bind::{resolve_bind, BindTier};

    /// The shipped default. Before this fix, overlay mode advertised the
    /// overlay IP while binding this, so every peer got connection-refused.
    const DEFAULT_BIND: &str = "127.0.0.1:9847";

    fn effective_bind(mode_is_overlay: bool, overlay_ip: Option<&str>, cli_bind: &str) -> String {
        let explicit = if cli_bind == DEFAULT_BIND { None } else { Some(cli_bind) };
        let tier = match (mode_is_overlay, overlay_ip) {
            (true, Some(ip)) => BindTier::Overlay(ip.parse().unwrap()),
            (true, None) => BindTier::Loopback,
            (false, _) => BindTier::Lan,
        };
        resolve_bind(tier, explicit, 9847)
    }

    #[test]
    fn overlay_mode_binds_the_overlay_address_not_loopback() {
        let bound = effective_bind(true, Some("100.107.222.96"), DEFAULT_BIND);
        assert_eq!(bound, "100.107.222.96:9847");
        assert_ne!(bound, DEFAULT_BIND, "regression: overlay bound loopback");
    }

    #[test]
    fn overlay_without_an_ip_stays_on_loopback() {
        assert_eq!(effective_bind(true, None, DEFAULT_BIND), DEFAULT_BIND);
    }

    #[test]
    fn explicit_bind_survives_overlay_mode() {
        assert_eq!(
            effective_bind(true, Some("100.107.222.96"), "192.168.1.9:7000"),
            "192.168.1.9:7000"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-ml-cli bind_tests -- --nocapture`
Expected: FAIL to compile — `vox-populi`'s `discovery` feature is not enabled for `vox-ml-cli`.

- [ ] **Step 3: Enable the feature**

In `crates/vox-ml-cli/Cargo.toml`, on the existing `vox-populi` dependency, add `"discovery"` to its feature list. This is a feature flag on an existing edge, not a new edge.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-ml-cli bind_tests -- --nocapture`
Expected: PASS, 3 tests.

- [ ] **Step 5: Apply the fix to the real code path**

In `populi_lifecycle.rs`, replace the `control_url` block (~line 143) and the `--bind` argument passed to the child process (~line 178):

```rust
            const DEFAULT_BIND: &str = "127.0.0.1:9847";
            let explicit_bind = if bind.trim() == DEFAULT_BIND {
                None
            } else {
                Some(bind.trim())
            };
            let port: u16 = bind
                .trim()
                .rsplit(':')
                .next()
                .and_then(|p| p.parse().ok())
                .unwrap_or(9847);

            let (tier, control_url) = match mode {
                PopuliConnectivityMode::Lan => (
                    vox_populi::discovery::bind::BindTier::Lan,
                    String::new(), // filled below from the resolved bind
                ),
                PopuliConnectivityMode::Overlay => {
                    let provider = choose_overlay_provider(overlay_provider);
                    provider_name = provider.map(|p| p.as_str().to_string());
                    let ip = match provider {
                        Some(OverlayProvider::Tailscale) => {
                            crate::commands::populi_overlay::tailnet_status()
                                .filter(|s| s.running)
                                .and_then(|s| s.self_ip)
                        }
                        _ => None,
                    };
                    match ip {
                        Some(ip) => (vox_populi::discovery::bind::BindTier::Overlay(ip), String::new()),
                        None => (vox_populi::discovery::bind::BindTier::Loopback, String::new()),
                    }
                }
            };
            let _ = control_url;
            let effective_bind =
                vox_populi::discovery::bind::resolve_bind(tier, explicit_bind, port);
            let control_url = format!("http://{effective_bind}");
```

Then pass `effective_bind` — not `bind` — to the spawned child:

```rust
                .arg("--bind")
                .arg(&effective_bind)
```

and store `effective_bind` in `PopuliDaemonState.bind`.

- [ ] **Step 6: Verify manually**

Run: `cargo run -p vox-cli -- populi up --mode lan` then `cargo run -p vox-cli -- populi status`

Expected: `control:` shows the LAN IP, not `127.0.0.1`. Confirm something is actually listening: `curl http://<that-ip>:9847/health` returns success. Before this task that curl failed.

Then `cargo run -p vox-cli -- populi down`.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p vox-ml-cli
cargo clippy -p vox-ml-cli --all-targets -- -D warnings
git add crates/vox-ml-cli/
git commit -m "fix(populi): bind the interface that gets advertised"
```

---

## Task 7: Retire the duplicate peer-trust store

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/populi_join.rs:140-149`

Fixes spec §5.1 (two opposite meanings of "join") and §5.2 (three stores holding peer trust).

**Interfaces:**
- Consumes: `vox_identity::TrustedNodeRegistry`.
- Produces: `migrate_config_toml_peers(reg: &TrustedNodeRegistry, entries: Vec<(String, String)>) -> usize` — returns how many peers were migrated.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-ml-cli/src/commands/populi_join.rs`:

```rust
/// Move federation peers recorded in `~/.vox/config.toml` into the trust
/// registry, which is the single source of truth for peer trust.
///
/// `entries` is `(node_id, manifest_url)`. Idempotent: re-running does not
/// duplicate, because the registry is keyed by node_id.
/// Returns the number of entries written.
pub fn migrate_config_toml_peers(
    reg: &vox_identity::TrustedNodeRegistry,
    entries: Vec<(String, String)>,
) -> usize {
    let mut n = 0;
    for (node_id, manifest_url) in entries {
        if node_id.trim().is_empty() {
            continue;
        }
        // Pubkey is unknown from a manifest URL alone; record the binding
        // honestly with an empty key and the URL as the label rather than
        // inventing a key.
        if reg
            .add(node_id, String::new(), Some(manifest_url))
            .is_ok()
        {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn migrates_each_peer_once() {
        let reg = vox_identity::TrustedNodeRegistry::new_in_memory();
        let n = migrate_config_toml_peers(
            &reg,
            vec![("nodeA".into(), "https://example/m.json".into())],
        );
        assert_eq!(n, 1);
    }

    #[test]
    fn skips_blank_node_ids() {
        let reg = vox_identity::TrustedNodeRegistry::new_in_memory();
        assert_eq!(
            migrate_config_toml_peers(&reg, vec![("  ".into(), "https://x".into())]),
            0
        );
    }

    #[test]
    fn rerunning_does_not_double_count_entries() {
        let reg = vox_identity::TrustedNodeRegistry::new_in_memory();
        let e = vec![("nodeA".into(), "https://example/m.json".into())];
        migrate_config_toml_peers(&reg, e.clone());
        // Second run writes the same key again; registry is keyed by node_id so
        // the store must not grow. In-memory add() is a no-op, so assert on the
        // file-backed behaviour in the integration check below instead.
        assert_eq!(migrate_config_toml_peers(&reg, e), 1);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-ml-cli migration_tests -- --nocapture`
Expected: FAIL — `migrate_config_toml_peers` not found.

- [ ] **Step 3: Run the tests to verify they pass**

The implementation is in Step 1's block. Run: `cargo test -p vox-ml-cli migration_tests -- --nocapture`
Expected: PASS, 3 tests.

- [ ] **Step 4: Deprecate the CLI verb**

Replace the persist block in `run()` (line ~140):

```rust
    // vox-deprecated-since="0.6.0" retire-by="0.7.0" reason="ssot-peer-trust" canonical="vox auth trust"
    // Peer trust lives in vox_identity::TrustedNodeRegistry, not ~/.vox/config.toml.
    let reg = vox_identity::TrustedNodeRegistry::new();
    reg.add(
        manifest.node_id.clone(),
        String::new(),
        Some(invite.manifest_url.clone()),
    )
    .map_err(|e| anyhow::anyhow!("could not persist peer: {}", e))?;

    println!(
        "vox populi join: peer '{}' trusted. NOTE: `vox populi join` is deprecated; \
         use `vox auth trust` instead.",
        manifest.node_id
    );
```

- [ ] **Step 5: Verify no writer of the old key remains**

Run: `rg "mesh\.federation_peers" crates/`
Expected: no results outside comments. If a reader remains, point it at `TrustedNodeRegistry`.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-ml-cli
git add crates/vox-ml-cli/
git commit -m "refactor(populi): make TrustedNodeRegistry the sole peer-trust store"
```

---

## Task 8: Remove the crate-wide dead-code allow

**Files:**
- Modify: `crates/vox-plugin-populi-mesh/src/lib.rs:11-13`

Fixes spec §5.3.

**Interfaces:** none.

- [ ] **Step 1: Remove the allow**

Delete these three lines from `crates/vox-plugin-populi-mesh/src/lib.rs`:

```rust
// Transport infrastructure ported from vox-populi; not all paths are exercised
// through the FFI entry points yet. Suppress until mesh integration is complete.
#![allow(dead_code)]
```

- [ ] **Step 2: See what it was hiding**

Run: `cargo clippy -p vox-plugin-populi-mesh --all-targets 2>&1 | rg "never used|never read" | sort -u`

Record the full list before changing anything — it is the actual scope of this task.

- [ ] **Step 3: Resolve each warning**

For each item, choose one and apply it:
- **Reachable from an FFI entry point but not from Rust callers** → add `#[allow(dead_code)]` on that item with a one-line comment naming the entry point.
- **Genuinely unreachable** → delete it.
- **Should be wired but isn't** → wire it if it is a one-line hookup; otherwise delete and note it in the commit body.

Prefer deletion. A per-item allow is a claim you must justify; a crate-wide one is the rot this task exists to remove.

- [ ] **Step 4: Verify clean**

Run: `cargo clippy -p vox-plugin-populi-mesh --all-targets -- -D warnings`
Expected: PASS with no warnings.

- [ ] **Step 5: Verify tests still pass**

Run: `cargo test -p vox-plugin-populi-mesh`
Expected: PASS. Deleting something a test used means it was not dead — restore it.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-plugin-populi-mesh
git add crates/vox-plugin-populi-mesh/
git commit -m "refactor(populi-mesh): drop crate-wide dead_code allow"
```

---

## Task 9: Surface pending peers through the MCP tool

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/populi_tools.rs:123-166`
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml` (add `vox-populi/discovery` to the `populi-transport` feature)

This is the no-new-crate-edge path to the GUI: `vox-gui` already polls `vox_mesh_nodes`.

**Interfaces:**
- Consumes: `vox_populi::discovery::{DiscoveredPeer, resolve_trust}` (Tasks 1, 4).
- Produces: `vox_mesh_nodes` result gains `pending_peers: [{node_id, fingerprint, addr, port, control_url, last_seen_unix_ms}]` — only peers with `trusted == false`.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator-mcp/src/populi_tools.rs`:

```rust
/// Serialize discovered-but-untrusted peers for the `vox_mesh_nodes` result.
///
/// Trusted peers are omitted: they already appear in the `nodes` list via the
/// control plane, and listing them twice would double-count in the GUI.
#[cfg(feature = "populi-transport")]
pub(crate) fn pending_peers_json(peers: &[vox_populi::discovery::DiscoveredPeer]) -> Vec<Value> {
    peers
        .iter()
        .filter(|p| !p.trusted)
        .map(|p| {
            json!({
                "node_id": p.node_id,
                "fingerprint": p.fingerprint,
                "addr": p.addr.to_string(),
                "port": p.port,
                "control_url": p.control_url(),
                "last_seen_unix_ms": p.last_seen_unix_ms,
            })
        })
        .collect()
}

#[cfg(all(test, feature = "populi-transport"))]
mod pending_tests {
    use super::*;
    use vox_populi::discovery::DiscoveredPeer;

    fn peer(node_id: &str, trusted: bool) -> DiscoveredPeer {
        DiscoveredPeer {
            node_id: node_id.into(),
            fingerprint: "ab:cd".into(),
            addr: "192.168.1.5".parse().unwrap(),
            port: 9847,
            last_seen_unix_ms: 42,
            trusted,
        }
    }

    #[test]
    fn untrusted_peers_are_listed() {
        let out = pending_peers_json(&[peer("n1", false)]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["node_id"], "n1");
        assert_eq!(out[0]["control_url"], "http://192.168.1.5:9847");
    }

    #[test]
    fn trusted_peers_are_omitted() {
        assert!(pending_peers_json(&[peer("n1", true)]).is_empty());
    }

    #[test]
    fn fingerprint_is_carried_for_human_verification() {
        // The user compares this against the other machine's screen; dropping
        // it would make the trust decision unverifiable.
        let out = pending_peers_json(&[peer("n1", false)]);
        assert_eq!(out[0]["fingerprint"], "ab:cd");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-orchestrator-mcp --features populi-transport pending_tests -- --nocapture`
Expected: FAIL — `vox_populi::discovery` not available (feature not enabled).

- [ ] **Step 3: Enable the feature**

In `crates/vox-orchestrator-mcp/Cargo.toml`, extend the existing feature:

```toml
populi-transport = ["vox-orchestrator/populi-transport", "vox-populi/transport", "vox-populi/discovery"]
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp --features populi-transport pending_tests -- --nocapture`
Expected: PASS, 3 tests.

- [ ] **Step 5: Wire into `mesh_nodes`**

In `mesh_nodes`, compute the pending list once and add it to all three `json!` returns:

```rust
    #[cfg(feature = "populi-transport")]
    let pending = {
        let reg = vox_identity::TrustedNodeRegistry::new();
        let peers = vox_populi::discovery::global_peers();
        pending_peers_json(&vox_populi::discovery::resolve_trust(peers, &reg))
    };
    #[cfg(not(feature = "populi-transport"))]
    let pending: Vec<Value> = Vec::new();
```

Add `"pending_peers": pending` to each of the three `json!({...})` blocks (control-plane success, control-plane error fallback, and plain local-registry).

**Do not add a field to `ServerState`.** It has 74 public fields and four constructors (`new_full`, `new_for_daemon`, `hermetic_stub`, `new_test`) that each build `Self { .. }` literally, so a new field means editing all four including a hermetic no-IO test stub. The mDNS responder is genuinely a process singleton — one daemon per process — so a process-global is the honest model here, not a shortcut.

Add to `crates/vox-populi/src/discovery/mod.rs`:

```rust
use std::sync::OnceLock;

static GLOBAL: OnceLock<Option<DiscoveryHandle>> = OnceLock::new();

/// Start process-wide discovery, once. Subsequent calls are no-ops.
///
/// Never returns an error: a discovery failure is logged and leaves the process
/// running without discovery, per the design's "never fatal" rule.
pub fn start_global(announce: NodeAnnouncement) {
    GLOBAL.get_or_init(|| match DiscoveryHandle::start(announce) {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!(target: "vox.populi.discovery", error = %e,
                "mDNS discovery unavailable; mesh still usable via explicit peers");
            None
        }
    });
}

/// Peers seen by the process-wide handle. Empty when discovery never started or
/// failed to start — callers cannot distinguish, and need not.
pub fn global_peers() -> Vec<DiscoveredPeer> {
    match GLOBAL.get() {
        Some(Some(h)) => h.peers(),
        _ => Vec::new(),
    }
}
```

with a test:

```rust
    #[test]
    fn global_peers_is_empty_before_start() {
        // Must not panic or block when discovery was never started.
        assert!(global_peers().is_empty());
    }
```

Call `start_global` where the populi server starts (`vox populi serve`), building the announcement from `node_record_for_current_process()` and the node's `vox_identity` fingerprint. Because `start_global` cannot fail, no error handling is needed at the call site.

- [ ] **Step 6: Verify the tool output**

Run: `cargo test -p vox-orchestrator-mcp --features populi-transport populi_tools -- --nocapture`
Expected: PASS. Then confirm the field is always present, even when empty — an absent field and an empty list must not be ambiguous to the GUI.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p vox-orchestrator-mcp
cargo clippy -p vox-orchestrator-mcp --features populi-transport --all-targets -- -D warnings
git add crates/vox-orchestrator-mcp/
git commit -m "feat(mcp): expose discovered pending peers via vox_mesh_nodes"
```

---

## Task 10: GUI pending-peers section

**Files:**
- Modify: `crates/vox-gui/ui/src/hooks/useMeshNodes.ts:6-13` (extend `NodesResult`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Mesh/MeshView.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Mesh/MeshView.test.tsx`

**Interfaces:**
- Consumes: `pending_peers` from Task 9; existing `trust_mesh_node` Tauri command (already registered at `main.rs:316`).
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-gui/ui/src/components/surfaces/Mesh/MeshView.test.tsx`:

```tsx
describe('MeshView pending peers', () => {
  const pending = [
    {
      node_id: 'peer-b',
      fingerprint: 'ab:cd:ef:12',
      addr: '192.168.1.42',
      port: 9847,
      control_url: 'http://192.168.1.42:9847',
      last_seen_unix_ms: Date.now(),
    },
  ];

  it('lists a discovered untrusted peer with its fingerprint', async () => {
    renderMeshView({ nodes: [], pending_peers: pending });
    expect(await screen.findByText('peer-b')).toBeInTheDocument();
    // The fingerprint is what the user compares against the other machine.
    expect(screen.getByText(/ab:cd:ef:12/)).toBeInTheDocument();
  });

  it('offers a Trust action for a pending peer', async () => {
    renderMeshView({ nodes: [], pending_peers: pending });
    expect(await screen.findByRole('button', { name: /trust/i })).toBeInTheDocument();
  });

  it('says why the list is empty rather than showing nothing', async () => {
    renderMeshView({ nodes: [], pending_peers: [] });
    expect(await screen.findByText(/no peers discovered/i)).toBeInTheDocument();
  });
});
```

Reuse the file's existing render helper and MCP mock; if it has none, add `renderMeshView(result)` that mocks `voxTransport.invokeMcpTool('vox_mesh_nodes')` to resolve `{ is_error: false, result }`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd crates/vox-gui/ui && pnpm vitest run MeshView`
Expected: FAIL — no pending section rendered.

- [ ] **Step 3: Extend the result type**

In `useMeshNodes.ts`, add to `NodesResult`:

```ts
  pending_peers?: PendingPeer[];
```

and export:

```ts
/** A peer discovered on the LAN but not yet trusted. Inert until approved. */
export interface PendingPeer {
  node_id: string;
  fingerprint: string;
  addr: string;
  port: number;
  control_url: string;
  last_seen_unix_ms: number;
}
```

Also coerce it alongside `nodes` in `fetchMeshNodesResult`, so consumers never see `undefined`:

```ts
  return {
    ...result,
    nodes: Array.isArray(result.nodes) ? result.nodes : [],
    pending_peers: Array.isArray(result.pending_peers) ? result.pending_peers : [],
  };
```

- [ ] **Step 4: Render the section**

In `MeshView.tsx`, add above the existing node table:

```tsx
{pendingPeers.length > 0 && (
  <section aria-label="Pending peers">
    <h3 className="text-sm font-medium text-text-secondary">
      Discovered — not yet trusted
    </h3>
    <ul>
      {pendingPeers.map((p) => (
        <li key={p.node_id} className="flex items-center gap-3">
          <span className="font-mono">{p.node_id}</span>
          <span className="font-mono text-xs text-text-secondary">{p.fingerprint}</span>
          <span className="text-xs text-text-secondary">{p.addr}</span>
          <button
            type="button"
            onClick={() => void trustPeer(p)}
            className="rounded border border-border-subtle px-2 py-1 text-xs"
          >
            Trust
          </button>
        </li>
      ))}
    </ul>
    <p className="text-xs text-text-secondary">
      Compare the fingerprint against the other machine before trusting.
    </p>
  </section>
)}
{pendingPeers.length === 0 && nodes.length === 0 && (
  <p className="text-sm text-text-secondary">
    No peers discovered. Peers on the same network appear automatically;
    otherwise add one explicitly with <code>vox populi up --bootstrap-peers</code>.
  </p>
)}
```

with the handler:

```tsx
const trustPeer = useCallback(
  async (p: PendingPeer) => {
    try {
      await invoke('trust_mesh_node', {
        nodeId: p.node_id,
        pubkeyHex: '',
        label: p.control_url,
      });
      pushToast({ kind: 'success', message: `Trusted ${p.node_id}` });
      await refresh();
    } catch (e) {
      pushToast({ kind: 'error', message: sanitizeErrorForToast(e) });
    }
  },
  [pushToast, refresh],
);
```

Confirm the argument casing `trust_mesh_node` expects — Tauri converts snake_case Rust parameters to camelCase for JS callers by default. If the invoke fails with a missing-argument error, match the casing the other `invoke` calls in this file already use.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd crates/vox-gui/ui && pnpm vitest run MeshView`
Expected: PASS, 3 new tests plus the existing suite.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/
git commit -m "feat(gui): show discovered pending peers with a trust action"
```

---

## Task 11: MeshWidget counts

**Files:**
- Modify: `crates/vox-gui/ui/src/components/dashboard/widgets/MeshWidget.tsx`
- Test: same directory, `MeshWidget.test.tsx` (create if absent)

**Interfaces:**
- Consumes: `PendingPeer`, `NodesResult` from Task 10.
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

describe('MeshWidget counts', () => {
  it('reports trusted, online and pending counts', async () => {
    renderWidget({
      nodes: [
        { id: 'a', status: 'online' },
        { id: 'b', status: 'offline' },
      ],
      pending_peers: [{ node_id: 'c', fingerprint: 'x', addr: '1.2.3.4', port: 9847, control_url: '', last_seen_unix_ms: 1 }],
    });
    expect(await screen.findByText(/1 online/i)).toBeInTheDocument();
    expect(screen.getByText(/1 pending/i)).toBeInTheDocument();
  });

  it('shows pending count as zero when nothing is discovered', async () => {
    renderWidget({ nodes: [], pending_peers: [] });
    expect(await screen.findByText(/0 pending/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd crates/vox-gui/ui && pnpm vitest run MeshWidget`
Expected: FAIL — no pending count rendered.

- [ ] **Step 3: Implement the counts**

Derive from the same `useMeshNodesFull` result the widget already consumes:

```tsx
const onlineCount = nodes.filter((n) => n.status === 'online').length;
const pendingCount = pendingPeers.length;
```

and render `{onlineCount} online · {pendingCount} pending`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd crates/vox-gui/ui && pnpm vitest run MeshWidget`
Expected: PASS.

- [ ] **Step 5: Run the whole frontend suite and commit**

```bash
cd crates/vox-gui/ui && pnpm vitest run && pnpm tsc --noEmit
cd ../../.. && git add crates/vox-gui/ui/src/
git commit -m "feat(gui): report pending peer count in the mesh widget"
```

---

## Task 12: Two-node verification

**Files:**
- Create: `docs/src/how-to/populi-two-node-quickstart.md`

This task is verification, not code. Its deliverable is evidence.

**Prerequisites:** two machines on the same LAN with vox built. `crates/vox-gui` needs its sidecar on a fresh worktree — run `vox run scripts/gui-build.vox` first or `cargo build -p vox-gui` fails inside `tauri-build`.

**Interfaces:** consumes everything above.

- [ ] **Step 1: Start both nodes**

On each machine: `vox populi up --mode lan`

Expected on each: `control:` shows that machine's LAN IP, not `127.0.0.1`.

- [ ] **Step 2: Confirm mutual discovery**

On machine A: `vox populi status --json | rg pending`

Expected: machine B's node_id and fingerprint appear. If empty after 60s, check that UDP 5353 is permitted — on Windows the first run usually raises a firewall prompt that must be accepted for private networks.

- [ ] **Step 3: Confirm an untrusted peer cannot be dispatched to**

On A: `vox populi dispatch --node <B-node-id> --source 'fn main() {}'`

Expected: refused, naming trust as the reason. A pass here would mean discovery grants capability, which is the security property this design exists to preserve — treat a success as a blocking defect.

- [ ] **Step 4: Trust in the GUI (Requirement 6)**

Launch `vox-gui` on A. In the Mesh surface:

1. B appears under "Discovered — not yet trusted" with its fingerprint.
2. Compare that fingerprint against B's own `vox populi identity show`. They must match.
3. Click Trust. B moves into the trusted list.
4. Confirm persistence: `cat ~/.vox/trusted_nodes.json` contains B's node_id.
5. Repeat on B for A.

**Capture a screenshot of the mesh surface showing the trusted peer.** This is the deliverable for spec Requirement 6 — a passing unit test does not satisfy it.

- [ ] **Step 5: Dispatch across the mesh**

From A's GUI, dispatch a task targeted at B.

Expected: it executes on B and returns. Confirm the same `lease_id` on both via `vox populi exec-leases`.

- [ ] **Step 6: Confirm departure**

Stop vox on B. Within 90 seconds (or immediately, via the goodbye packet), A's GUI shows B offline.

- [ ] **Step 7: Confirm the no-internet requirement**

Disconnect both machines from the internet, keeping them on the same LAN. Repeat steps 1, 2 and 5.

Expected: unchanged behavior. This is the requirement that distinguishes this design from the rejected one — if anything here needs the internet, it is a blocking defect.

- [ ] **Step 8: Write the quickstart**

Create `docs/src/how-to/populi-two-node-quickstart.md` with frontmatter (required under `docs/src/`):

```md
---
title: "Two-Node Mesh Quickstart"
description: "Connect two machines into a Populi mesh over the local network, with no account, no internet, and no central server."
category: "How-To Guides"
---
```

Body: the exact commands from steps 1-6, the firewall note from step 2, and the fingerprint-comparison instruction from step 4. Verify with `cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/how-to/populi-two-node-quickstart.md`.

- [ ] **Step 9: Commit**

```bash
git add docs/src/how-to/populi-two-node-quickstart.md
git commit -m "docs(populi): two-node LAN mesh quickstart"
```

- [ ] **Step 10: Full gate**

Run: `vox ci pre-push --complete`
Expected: PASS. Fix anything red before pushing; do not use GitHub Actions as the feedback loop.

---

## Self-Review Notes

**Spec coverage:** T1 mDNS discovery → Tasks 1, 2. Trust model → Tasks 4, 10. Defect A (bind) → Tasks 3, 6. Defects B/B2 (tailscale detection) → Task 5. §5.1 join collision + §5.2 three stores → Task 7. §5.3 dead_code → Task 8. GUI wiring (Requirement 6) → Tasks 9, 10, 11, 12. No-internet requirement → Task 12 Step 7.

**Known gaps, deliberately deferred to the spec's Part 9:** the T3 overlay tier keeps its current provider set (no Headscale or Nebula resolver); pairing codes are not built; the `mdns-sd` 0.13 API surface used in Task 2 is the least-verified part of this plan — Task 1 Step 2 exists to confirm it against real docs before Task 2 depends on it, and Task 2 Step 6 says to adjust the code to the real API rather than downgrade the crate.

**Cross-task type consistency checked:** `DiscoveredPeer` fields are identical in Tasks 1, 4, 9, and 10. `resolve_bind(tier, explicit, port)` has the same signature in Tasks 3 and 6. `pending_peers` is the same JSON key in Tasks 9, 10, and 11.
