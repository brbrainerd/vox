---
title: "Populi Mesh: Overlay Transport and Per-Account Enrollment"
description: "Design for bringing the Populi mesh to working cross-device status by fixing overlay bind and detection defects and deriving per-account mesh identity from the operator's tailnet, with a provider-agnostic escape hatch."
category: "Architecture SSOTs"
status: "current"
---

# Populi Mesh: Overlay Transport and Per-Account Enrollment

**Status.** Design approved 2026-09-04. Supersedes the enrollment portion of
[`populi-mesh-north-star-2026.md`](../../src/architecture/populi-mesh-north-star-2026.md)
section W7; the remaining workstreams there are unchanged.

**Problem.** The Populi mesh subsystem is substantially complete — authoritative
exec leases, A2A inbox, federation directory, probe-backed `NodeRecord`, and a
model selector that already synthesizes `mesh/<scope>/<kind>` candidates. It is
nevertheless unreachable across devices, and adding a machine requires manual
token and peer configuration.

---

## Part 1 — What is already shipped

Establishing this first, because it determines how small the change is.

| Capability | Location |
|---|---|
| HTTP control plane (join/heartbeat/leave, exec leases, A2A, dispatch, federation, admin) | `crates/vox-plugin-populi-mesh/src/transport/router.rs:64` |
| Lease authority with fail-closed local-fallback gate (ADR-017, W1) | `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs:26` |
| `NodeRecord` with probe-backed GPU truth, `owner_vox_user_id`, ed25519 pubkey, `host_triple` | `crates/vox-populi-types/src/node_record.rs` |
| Mesh peers as model candidates, with reputation blacklisting | `crates/vox-orchestrator/src/models/registry.rs:378` |
| `VOX_ROUTING_PREFER_MESH` scoring bonus | `crates/vox-orchestrator/src/models/scoring.rs:408` |
| `OverlayProvider` enum (Tailscale, WireGuard, Tunnel) with auto-detection | `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:378` |

The model selector requires **no changes** under this design. It consumes
`federation_directory()`, so seeding federation bootstrap peers is sufficient to
make new machines routable.

---

## Part 2 — Overlay transport: alternatives considered

### The deciding constraint

The Populi control plane is HTTP over TCP bound to a `SocketAddr`, and peers are
addressed as `http://host:port`. Any transport that supplies a stable routable IP
works with no changes to vox. Any transport that does not requires rewriting a
working HTTP layer. This splits the candidate space cleanly and decides it.

### Family A — IP overlays (zero transport change)

| Option | Model | Trade-off |
|---|---|---|
| **Tailscale** | WireGuard plus hosted coordination server, with DERP relay fallback | Already installed and authenticated on all nine of the operator's devices. Clients BSD-3; coordination server proprietary. Free tier covers 100 devices and 3 users. |
| **Headscale** | Open-source reimplementation of the coordination server only | Same Tailscale clients. Self-hosted. The de-SaaS path is a client config change, not a migration. Lags upstream feature releases. |
| **Nebula** (Slack) | Own CA, lighthouse nodes for discovery and NAT hole-punching | Fully self-hosted, no SaaS. The operator's `financier-vps` has a public IP suitable as a lighthouse. Certificate identity with per-group firewall rules. Requires running a CA. |
| **NetBird** | WireGuard plus self-hostable control plane and SSO | Younger; more moving parts than Headscale for the same outcome. |
| **ZeroTier** | L2 virtual ethernet, proprietary protocol | Mature, but L2 exceeds requirements and the hosted controller free tier caps at 25 devices. |
| **Plain WireGuard** | Manual peer configuration | No NAT traversal, no key distribution, no dynamic membership. Adding a device means editing configuration on every peer — the exact capability this design must provide. |

### Family B — p2p libraries (require transport rewrite)

`iroh` (Rust, QUIC with hole punching and relays) and `libp2p` are attractive as
true crate dependencies with no system daemon and no administrator privileges.
Both provide authenticated streams between opaque NodeIds rather than IP
addresses. Adopting either means porting the axum router, `PopuliHttpClient`, and
every `http://host:port` reference onto their stream abstraction — trading a
working transport for a rewrite, to solve a problem the overlay already solves.

**Rejected.** Revisit only if a hard requirement emerges for mesh operation
without any system-level network daemon.

### Family C — tunnels (wrong topology)

Cloudflare Tunnel and ngrok are hub-and-spoke through a third-party edge that
terminates TLS. Correct for exposing a single service publicly; incorrect for a
symmetric mesh. The existing `tunnel` variant is retained for the
expose-one-node case and is not promoted to a mesh default.

### Decision

**Tailscale is the default provider. Headscale is the documented de-SaaS path.
Nebula is the documented fully-independent alternative.**

This is not a lock-in bet. `OverlayProvider` is already an enum with a resolver
shape; each provider is a default value plus approximately sixty lines of
resolver. The decision is reversible at any time without touching the mesh
transport, the lease layer, or the model selector.

### On whether Tailscale is a dependency

No, in three distinct senses, and each is load-bearing:

1. **No crate dependency.** Integration is a subprocess invocation and a JSON
   parse. No linking, no license entanglement, no build-time impact, and no
   interaction with the cryptography policy in `AGENTS.md`.
2. **No runtime requirement.** When Tailscale is absent or logged out the
   resolver returns `None`, `--overlay-provider auto` falls through to LAN mode,
   and the existing bootstrap-token join path is unaffected.
3. **No lock-in.** Headscale substitutes the coordination server with clients and
   vox both unchanged. Nebula substitutes the entire stack for one added resolver.

---

## Part 3 — Identity: the tailnet is the account

`tailscale status --json` supplies the account boundary directly, so no
enrollment infrastructure is built.

### Field mapping

| Vox concept | Source | Rationale |
|---|---|---|
| `owner_vox_user_id` (default only) | `Self.UserID` (numeric, e.g. `5008244628085777`) | Stable across renames. Deliberately **not** the login email — that would place PII in every `NodeRecord` crossing the wire. See the authority note below. |
| `scope_id` (default) | `CurrentTailnet.MagicDNSSuffix` (e.g. `tail4f69a0.ts.net`) | Unique per tailnet, stable, non-secret. Overridable by explicit `--scope`. |
| Peer control URL | `Peer[].DNSName` | **Not** `TailscaleIPs`; see below. |
| Bind address | `Self.TailscaleIPs[0]` | An interface cannot be bound by name. |
| Membership filter | `peer.UserID == self.UserID` | Devices shared into the tailnet by other users are excluded from the per-account mesh by default. |

### Authority: the tailnet supplies defaults, never assertions

`NodeRecord.owner_vox_user_id` is documented as assigned from the join token, and
that remains authoritative. The tailnet-derived value is used only as the local
**default** for a node's own configuration before it joins. The server continues
to assign `owner_vox_user_id` from the presented join token and never accepts a
self-reported owner from the joining node.

This matters because tailnet membership is evidence of network reachability, not
of vox account ownership. Treating a self-reported tailnet `UserID` as an
identity assertion would let any node that reaches the control plane claim
membership in any account. Presence on the tailnet decides *who can connect*;
the join token decides *whose mesh they are in*.

### Why DNSName rather than TailscaleIPs for peers

Tailnet IPs are stable per device registration, not per physical machine. The
operator's MacBook demonstrates the failure: it moved from `100.94.149.82` to
`100.120.21.37` on re-registration under a new node name, while
`bertrands-mbp.tail4f69a0.ts.net` would have remained correct. Peer URLs use
MagicDNS names; only the local bind uses a literal IP.

### Resulting enrollment flow

Installing vox on a machine already joined to the tailnet places it in the mesh
with no enrollment step and no token to transport. Removing the device from the
tailnet removes it from routing on the next poll. Device add and remove are
Tailscale operations, not vox operations.

The existing bootstrap-token flow (`vox populi pair`) is retained unchanged for
peers with no overlay.

---

## Part 4 — Defects and fixes

### Defect A — overlay advertises a reachable address but binds loopback

`overlay_control_url` (`populi_lifecycle.rs:401`) rewrites the advertised control
URL to the tailnet IP, while `--bind` retains its `127.0.0.1:9847` default and is
passed unmodified to `populi serve` (`populi_lifecycle.rs:178`). The node
advertises `http://100.x.y.z:9847` while listening only on loopback, so every
peer receives connection-refused.

**Fix.** In overlay mode, when `--bind` is at its default, bind the resolved
overlay IP. An explicitly supplied `--bind` is honored unchanged. Never
substitute `0.0.0.0` — that would silently expose the control plane on every
interface, including untrusted LANs.

### Defect B — provider detection fails on Windows

`command_ok("tailscale", ...)` (`populi_lifecycle.rs:427`) requires the binary on
`PATH`. The Windows installer places it at `C:\Program Files\Tailscale\` and does
not modify `PATH`, so `--overlay-provider auto` never selects Tailscale on
Windows despite it being installed and connected.

**Fix.** Resolve the binary in order: `PATH`, then
`C:\Program Files\Tailscale\tailscale.exe`, then
`/Applications/Tailscale.app/Contents/MacOS/Tailscale`, then `/usr/bin/tailscale`.
Cache the resolved path for the process lifetime.

### Defect B2 — exit-code probe conflates installed with connected

`command_ok("tailscale", ["status"])` tests an exit code that does not reliably
distinguish "installed but logged out" from "connected".

**Fix.** Parse `status --json` and require `BackendState == "Running"`. This is
one call that answers availability, connectivity, identity, and peer list
together, replacing three separate subprocess invocations.

### Defect C — peer discovery is manual

Peers arrive only through the operator-supplied `--bootstrap-peers` flag. Nothing
consults the tailnet device list, so the mesh does not adapt as machines appear
and disappear.

**Fix.** Seed `VOX_MESH_FEDERATION_BOOTSTRAP_PEERS` from online, same-owner
tailnet peers at `vox populi up`, and refresh every 30 seconds thereafter. The
existing federation announce and directory path propagates from there. No change
to `vox-orchestrator::models::registry`.

The 30-second interval is chosen so a device joining or leaving the tailnet is
reflected in routing within roughly a minute, while costing one subprocess call
per interval. It is configurable via `VOX_MESH_OVERLAY_POLL_SECS` for operators
with larger tailnets, where the `status --json` call is more expensive.

---

## Part 5 — Component design

### New module: `crates/vox-ml-cli/src/commands/populi_overlay.rs`

Same crate as the existing overlay code, so no new crate edge is introduced
(`AGENTS.md` section Dependency Discipline). Extracting the overlay logic also
removes roughly ninety lines from `populi_lifecycle.rs`, currently 522 lines.

Surface:

```text
resolve_tailscale_binary() -> Option<PathBuf>
tailnet_status() -> Option<TailnetStatus>      // one `status --json` call

TailnetStatus {
    user_id: u64,
    magic_dns_suffix: String,
    self_ip: IpAddr,
    peers: Vec<TailnetPeer>,                   // same-owner, online only
}

TailnetPeer { dns_name: String, os: String, ip: IpAddr }
```

Every field of the deserialization struct is `Option`, and any parse failure
yields `None` rather than an error. The `tailscale status --json` schema carries
no stability guarantee; the design treats it as untrusted input that may change
shape without notice.

### Failure modes

| Condition | Behavior |
|---|---|
| Tailscale not installed | Resolver returns `None`; `auto` falls to LAN; warning, not failure. |
| Installed, logged out | `BackendState != "Running"`; same as above. |
| JSON shape changed upstream | Permissive parse yields `None`; treated as no overlay. |
| Peer online, vox not running | Absent from federation directory; per-peer health check with short timeout. |
| Peer IP changed | Unaffected — peer URLs use MagicDNS names. |
| Explicit `--bind` supplied | Honored verbatim; overlay never overrides an operator's explicit choice. |

---

## Part 6 — Test plan

### Unit

- `resolve_tailscale_binary` finds the Windows path when absent from `PATH`.
- `tailnet_status` parses a captured fixture of real `status --json` output.
- Peers with a differing `UserID` are excluded.
- Offline peers are excluded.
- Truncated, empty, and shape-changed JSON each yield `None`, not a panic.
- Default `--bind` in overlay mode resolves to the overlay IP; explicit `--bind`
  is preserved; `0.0.0.0` is never synthesized.

Fixtures are captured from live `tailscale status --json` output with device
names and user IDs preserved, per the project's real-data fixture policy.

### Two-node integration (Windows `blaptop04`, macOS `bertrands-mbp`)

1. `vox populi up --mode overlay` on Windows; assert it binds
   `100.107.222.96:9847` and advertises the MagicDNS name.
2. Request the Windows `/health` endpoint from the Mac — proves Defect A is fixed.
3. Start the Mac node; assert it appears in `GET /v1/populi/nodes` with
   `owner_vox_user_id` matching the tailnet `UserID`.
4. Dispatch a task from Windows; assert it executes on the Mac and returns.
5. Assert the same `lease_id` is visible on both nodes.
6. Take the Mac offline; assert it leaves routing within two poll intervals
   (60 seconds at the default).

### Definition of done

A task submitted on either machine executes on the other and returns its result,
with the lease visible on both nodes, and with no vox configuration performed on
either machine beyond installation.

---

## Part 7 — Out of scope

- Cross-node secret pairing (north-star W3) — unchanged, still deferred.
- Cross-node trace propagation (W5) — unchanged.
- Mesh model inventory (W4) — unchanged.
- Headscale and Nebula resolvers — documented as escape hatches, not implemented.
  Implement when the operator chooses to leave the hosted coordination server.
- Tailscale API key custody for provisioning devices onto the tailnet — rejected;
  it would make vox custodian of a credential able to add devices to the
  operator's network, for no benefit to machines already enrolled.
