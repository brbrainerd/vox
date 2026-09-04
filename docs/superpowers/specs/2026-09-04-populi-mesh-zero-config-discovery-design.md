---
title: "Populi Mesh: Zero-Config Discovery, Trust, and Transport Tiers"
description: "Design for making the Populi mesh work out of the box for every user with no account, no central server, and no internet, via LAN discovery and an explicit trust store, with optional overlay transport for cross-site meshes."
category: "Architecture SSOTs"
status: "current"
---

# Populi Mesh: Zero-Config Discovery, Trust, and Transport Tiers

**Status.** Design approved 2026-09-04. Supersedes the enrollment portion of
[`populi-mesh-north-star-2026.md`](../../src/architecture/populi-mesh-north-star-2026.md)
section W7; the remaining workstreams there are unchanged.

**Revision note.** An earlier draft of this spec recommended deriving mesh
identity from a Tailscale tailnet. That was rejected: it requires an account, an
internet round-trip to a third-party coordination server, and a third-party
install, none of which are acceptable for a default path. Tailscale survives as
one optional transport tier, never as the enrollment mechanism.

---

## Part 1 — Requirements

These are binding and they order every decision below.

1. **Works out of the box.** Two machines with vox installed and no
   configuration must find each other and share work.
2. **No central server.** The mesh must be fully functional with Vox project
   infrastructure switched off permanently. No vox-operated service may become
   load-bearing for mesh formation.
3. **Works without internet.** A LAN with no route to the internet, or a fully
   air-gapped network, is a supported deployment.
4. **No account required.** No sign-up, no third-party identity provider, no
   email address.
5. **Secure by default.** The safe configuration must be the one a user reaches
   without reading documentation.
6. **Confirmed in the GUI.** The capability is not done until it is visible and
   operable in `vox-gui`.

Requirement 2 is the one that reshapes the design. It forbids any architecture
whose happy path traverses a coordination server, including the operator's
planned Hetzner instance, which is scoped to telemetry and feedback only.

---

## Part 2 — What is already shipped

Establishing this first, because it determines how small the change is.

| Capability | Location | State |
|---|---|---|
| Device identity: ed25519, `node_id`, `pubkey_hex`, `fingerprint()`, challenge/response | `crates/vox-identity/src/identity.rs` | Shipped |
| Trust store: `~/.vox/trusted_nodes.json` (node_id + pubkey + label) | `crates/vox-identity/src/trust.rs` | Shipped, **already GUI-wired** |
| X25519 pairing keys for secret sealing | `crates/vox-identity/src/pairing_x25519.rs` | Shipped |
| HTTP control plane: join/heartbeat/leave, exec leases, A2A, dispatch, federation, admin | `crates/vox-plugin-populi-mesh/src/transport/router.rs:64` | Shipped |
| Lease authority with fail-closed local-fallback gate (ADR-017) | `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs:26` | Shipped |
| `NodeRecord` with probe-backed GPU truth, `host_triple`, ed25519 pubkey | `crates/vox-populi-types/src/node_record.rs` | Shipped |
| Mesh peers as model-selector candidates with reputation blacklisting | `crates/vox-orchestrator/src/models/registry.rs:378` | Shipped |
| GUI mesh surface: `MeshView`, `MeshWidget`, `useMeshNodes` polling `vox_mesh_nodes` | `crates/vox-gui/ui/src/hooks/useMeshNodes.ts` | Shipped |
| GUI trust mutation: list / trust / untrust against `TrustedNodeRegistry` | `crates/vox-gui/src/commands/mesh.rs` | Shipped |
| LAN IP detection | `crates/vox-cli-share/src/backends/lan.rs:54` | Shipped |
| **LAN peer discovery** | — | **Missing. This is the gap.** |

The trust store already implements the correct model: identity is a public key,
trust is an explicit local decision, and no server participates. The model
selector requires no changes; it consumes `federation_directory()`, so feeding
discovered peers into the existing federation path is sufficient.

---

## Part 3 — Transport tiers

The mesh does not choose *a* transport. It layers five tiers, each independently
sufficient, tried in order of decreasing locality.

| Tier | Mechanism | Requires | Default |
|---|---|---|---|
| **T0** Loopback | Single node, `127.0.0.1` | Nothing | Always |
| **T1** LAN | mDNS/DNS-SD on `_vox-mesh._tcp.local` | Same broadcast domain | **Yes — out of the box** |
| **T2** Explicit peers | Operator-supplied addresses | Known addresses | On configuration |
| **T3** Overlay | Tailscale, Headscale, Nebula, ZeroTier, WireGuard | User-supplied overlay | On detection |
| **T4** Relay | Rendezvous through a third party | Internet | **Never. Out of scope.** |

**T1 is the out-of-the-box answer.** Two machines on the same Wi-Fi discover each
other on launch with no configuration, no account, and no internet.

**T3 is where the existing `OverlayProvider` enum lives**, demoted from "the
enrollment mechanism" to one optional tier. When a user already runs an overlay,
vox detects it and uses its address; when they do not, nothing degrades.

**T4 is a standing architectural prohibition, not a backlog item.** Should a
rendezvous service ever ship, tiers T0–T3 must remain fully functional with it
switched off. Any change that makes mesh formation depend on a vox-operated
service violates Requirement 2 and must be rejected in review.

### Why mDNS for T1

mDNS/DNS-SD is the platform-native mechanism for LAN service discovery. It is
what AirPlay, Chromecast, and Syncthing use, so port 5353 is already permitted by
firewall rules users have in place.

The alternative considered was hand-rolled UDP multicast: roughly eighty lines,
zero dependencies. Rejected. It would reimplement name-conflict resolution, TTL
handling, and goodbye packets, and it would arrive on an arbitrary port that
Windows Firewall blocks by default — trading one small dependency for a support
burden on every Windows install.

**One new dependency: `mdns-sd`.** Pure Rust, no system daemon, no Bonjour or
Avahi requirement, cross-platform. Announcing and browsing are each a few lines.

### Overlay alternatives (T3), for the record

Retained from the prior analysis, since the tier-3 slot still needs candidates.
The deciding constraint is that the control plane is HTTP over TCP bound to a
`SocketAddr`: any transport supplying a routable IP works unchanged.

| Option | Model | Trade-off |
|---|---|---|
| Tailscale | WireGuard + hosted coordination, DERP relay fallback | Easiest for users already on it. Account required, so unusable as a default. |
| Headscale | Self-hosted reimplementation of the coordination server | Same clients, no SaaS. User runs a server. |
| Nebula | Own CA, lighthouse nodes for NAT hole-punching | Fully self-hosted. User runs a CA and a lighthouse. |
| NetBird | WireGuard + self-hostable control plane, SSO | Younger; more moving parts than Headscale. |
| ZeroTier | L2 virtual ethernet, proprietary protocol | Mature; L2 exceeds requirements. |
| Plain WireGuard | Manual peer configuration | No NAT traversal, no dynamic membership. |

`iroh` and `libp2p` were considered and rejected: both supply authenticated
streams between opaque NodeIds rather than IP addresses, so adopting either means
porting the axum router and every `http://host:port` reference onto their stream
abstraction — a rewrite of a working transport to solve a problem the overlay
tier already solves. Revisit only if mesh operation without any system-level
network daemon becomes a hard requirement.

Cloudflare Tunnel and ngrok remain available as the existing `tunnel` variant for
exposing a single node publicly. They are hub-and-spoke through a third-party TLS
terminator and are not a mesh transport.

---

## Part 4 — Trust model

### The constraint, stated plainly

Automatic, secure, and serverless cannot all hold simultaneously. Discovering a
peer does not prove it is yours — everyone on a cafe network is "on the LAN."
Establishing that a peer belongs to a user requires something shared out of band
exactly once. No architecture removes this; designs that appear to have removed
it have instead hidden a bearer secret or a trusted third party.

**Therefore: discovery is unauthenticated and free; trust is explicit.** This is
the Syncthing model, and `TrustedNodeRegistry` already implements its storage.

### Flow

1. Every node announces `node_id`, `fingerprint`, address, and port over mDNS.
   The announcement carries no secret and grants nothing.
2. Discovered peers absent from `TrustedNodeRegistry` are **pending**: visible,
   inert, unable to dispatch or receive work.
3. The user approves a pending peer in the GUI, seeing its fingerprint. Approval
   on both machines establishes mutual trust.
4. Trusted peers join the federation directory and become routable, at which
   point the existing model selector picks them up unchanged.

### Security properties

- A hostile peer on the same LAN can be discovered and can do nothing further.
- Fingerprints are displayed for out-of-band comparison, defending against an
  attacker who announces a chosen `node_id`.
- Trust survives IP and hostname changes; it binds to the public key.
- Revocation is local and immediate: remove from the registry.
- No secret is transmitted during discovery, so no discovery traffic is
  sensitive, and mDNS announcements may be freely observed.

### Explicitly rejected: pairing codes as the default

A short pairing code would remove the approval click, and was rejected as a
default because it is a bearer secret: anyone who observes it joins silently,
which fails Requirement 5. It may later ship as an opt-in for users enrolling
many machines at once, with a TTL and rotation. Not in this pass.

---

## Part 5 — SSOT rework

Three collisions found during design. All are fixed in this pass; leaving them
compounds the maintenance cost of everything built on top.

### 5.1 Two incompatible meanings of "join"

- `POST /v1/populi/join` — a node registers **itself** with a control plane.
- `vox populi join <invite-url>` — registers a **remote peer** locally.

These run in opposite directions under one verb. The HTTP endpoint keeps `join`;
it is the ADR-008 wire surface and the meaning matches the protocol.

The CLI verb is retired in favor of the existing trust vocabulary. `vox auth
trust` / `vox auth untrust` already write `TrustedNodeRegistry` and are the
established verbs for this operation. `vox populi join` is deprecated with an
inline retirement marker per `AGENTS.md`, printing a pointer to `vox auth trust`.

### 5.2 Three stores holding overlapping peer state

| Store | Holds | Verdict |
|---|---|---|
| `~/.vox/trusted_nodes.json` (`TrustedNodeRegistry`) | node_id, pubkey, label | **SSOT for trust.** GUI-wired. Keep. |
| `~/.vox/cache/populi/local-registry.json` (`NodeRecord`) | capabilities, liveness, GPU truth | Keep — a cache of *observed* mesh state, correctly separate from trust. |
| `~/.vox/config.toml` (written by `vox populi join:142`) | peer manifest URLs | **Remove.** Third store encoding "I trust this peer." |

The first two answer different questions and both stay. The third duplicates the
first. Federation peer manifest URLs move onto `TrustedNodeRegistry` as an
optional field, with a one-time migration reading any existing `config.toml`
entries on first run.

### 5.3 Crate-wide `#![allow(dead_code)]`

`crates/vox-plugin-populi-mesh/src/lib.rs:13` suppresses dead-code warnings for
the entire crate, with a comment deferring until "mesh integration is complete."
This pass completes that integration, so the blanket allow is removed and any
genuinely unreachable code is either wired or deleted. A crate-wide allow hides
exactly the rot this design must not accumulate.

---

## Part 6 — Component design

### New module: `crates/vox-populi/src/discovery.rs`

Placed in `vox-populi` rather than the CLI crate because discovery is a runtime
concern consumed by the GUI, the daemon, and the CLI alike. Placing it in
`vox-ml-cli` would force a crate edge from `vox-gui`.

**No new workspace crate edges are introduced by this design.** `vox-gui` does
*not* depend on `vox-populi` and must not gain that edge (`AGENTS.md` section
Dependency Discipline: exceptions are user-authorized only). The GUI reaches
discovery through crates it already depends on:

- `vox-orchestrator-mcp` already depends on `vox-populi` and already implements
  the `vox_mesh_nodes` tool (`crates/vox-orchestrator-mcp/src/populi_tools.rs`).
  Discovered peers are added to that tool's existing result.
- `vox-gui` already depends on `vox-orchestrator-mcp`, and `useMeshNodes` already
  polls `vox_mesh_nodes`. The pending-peer list arrives through the transport the
  GUI already uses.
- `vox-gui` already depends on `vox-identity`, so trust mutations remain direct
  Tauri commands against `TrustedNodeRegistry`, unchanged.

```text
DiscoveryHandle::start(announce: NodeAnnouncement) -> Result<DiscoveryHandle>
DiscoveryHandle::peers() -> Vec<DiscoveredPeer>
DiscoveryHandle::shutdown()

NodeAnnouncement { node_id, fingerprint, port, scope_id: Option<String> }
DiscoveredPeer   { node_id, fingerprint, addr, port, last_seen_unix_ms, trusted: bool }
```

`trusted` is resolved against `TrustedNodeRegistry` at read time, so the GUI
renders pending and trusted peers from one call without a second lookup.

Announcements are best-effort. Failure to bind the multicast socket is logged at
warn and leaves the node functional at T0/T2/T3 — discovery is an enhancement,
never a precondition.

**Timings.** Announce every 30 seconds; treat a peer as stale after 90 seconds
(three missed announcements) and emit a DNS-SD goodbye packet on clean shutdown
so departure is usually immediate rather than waiting out the stale window. Both
values are overridable via `VOX_MESH_DISCOVERY_ANNOUNCE_SECS`. The 30-second
figure matches Syncthing's local-discovery cadence and keeps a device
appearing or disappearing visible within roughly a minute at negligible cost.

### Bind address correctness

Discovery is useless if the control plane listens only on loopback. Today
`--bind` defaults to `127.0.0.1:9847` and overlay mode rewrites only the
*advertised* URL, so peers reach a port nothing is listening on.

Binding follows the active tier: T0 loopback; T1 the detected LAN IP via the
existing `detect_lan_ip()`; T3 the overlay IP. An explicit `--bind` is always
honored verbatim. **`0.0.0.0` is never synthesized** — that would expose the
control plane on every interface including untrusted networks, and the tier
model exists precisely to bind the one correct interface.

### GUI wiring

The deliverable is a working surface, not a working library.

- The `vox_mesh_nodes` tool result gains a `pending_peers` field carrying
  `DiscoveredPeer` entries. No new Tauri command and no new crate edge; the
  existing `list_trusted_nodes` / trust / untrust commands are unchanged.
- `MeshView` gains a **Pending** section listing discovered-untrusted peers with
  fingerprint and a Trust action, and marks trusted peers online or offline.
- `MeshWidget` reports counts: trusted, online, pending.
- An empty mesh states why — discovery disabled, no peers found, or firewall
  blocked — rather than rendering an unexplained empty list.

---

## Part 7 — Failure modes

| Condition | Behavior |
|---|---|
| Multicast blocked by firewall or network | No peers found; GUI says so explicitly and points at T2 explicit peers. Node stays functional. |
| mDNS socket bind fails | Warn, continue at T0/T2/T3. Never fatal. |
| Two nodes announce the same `node_id` | Both listed; fingerprints differ and are displayed. Trust is per-pubkey, so the collision cannot escalate. |
| Peer discovered, vox not actually serving | Health check with short timeout; absent from the federation directory. |
| Peer trusted, then IP changes | Unaffected. Trust binds to pubkey; the address refreshes on the next announcement. |
| Overlay present and LAN present | Both tiers active; the LAN address is preferred for same-subnet peers as the lower-latency path. |
| No internet at all | Fully supported. T1 and T2 involve no external traffic. |

---

## Part 8 — Test plan

### Unit

- Announcement round-trips through a loopback mDNS responder.
- A discovered peer absent from the trust registry reports `trusted: false`.
- Trusting a peer flips `trusted` without re-discovery.
- Socket bind failure yields an empty peer list, not an error.
- Bind resolution: T0 loopback, T1 LAN IP, T3 overlay IP, explicit `--bind`
  preserved, `0.0.0.0` never produced.
- `config.toml` federation entries migrate into `TrustedNodeRegistry` once and
  are not double-applied on a second run.

### Two-node integration

Two nodes on one LAN, no configuration beyond installation:

1. Both discover each other within one announcement interval.
2. Both list the other as pending, not trusted.
3. Dispatch to an untrusted peer is refused.
4. After mutual trust, a task submitted on A executes on B and returns.
5. The same `lease_id` is visible on both nodes.
6. B goes offline; A reflects it within the 90-second stale window, or immediately when B shuts down cleanly and sends a goodbye packet.

### GUI verification (Requirement 6)

Not complete until, with a second node running and `vox-gui` launched:

1. `MeshView` lists the second node as pending with its fingerprint.
2. Clicking Trust moves it to trusted and persists to `trusted_nodes.json`.
3. `MeshWidget` counts update.
4. A task dispatched from the GUI runs on the peer and returns.
5. Stopping the peer flips it to offline in the UI.

Evidence is a screenshot of the mesh surface with a real second node, not a
passing unit test.

### Cross-platform

The two-node integration runs Windows-to-macOS over the LAN. mDNS behavior
differs across platforms and this is the case most likely to break.

---

## Part 9 — Out of scope

- **T4 relay / rendezvous.** Prohibited as a dependency, deferred as a feature.
- Pairing codes for zero-click enrollment — deferred; approval is the default.
- Headscale and Nebula resolvers — the T3 slot accepts them; not implemented.
- Cross-node secret pairing (north-star W3), trace propagation (W5), and mesh
  model inventory (W4) — unchanged.
- Any vox-operated coordination service. Not deferred: rejected.
