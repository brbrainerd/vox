---
title: "Populi Mesh: Ticket Pairing, LAN Discovery, and Fail-Closed Trust"
description: "Design for making the Populi mesh usable by every user with no account, no central server, and no internet, via a universal pasteable ticket, opportunistic LAN discovery, and an enforced pubkey trust gate on the control plane."
category: "Architecture SSOTs"
status: "current"
---

# Populi Mesh: Ticket Pairing, LAN Discovery, and Fail-Closed Trust

**Status.** Revision 2 of 2026-09-04, following an eight-track adversarial audit
against the codebase. Revision 1's architecture survived; four of its stated
rationales did not, and one security prerequisite it omitted is now blocking.

## Revision history and what changed

**Revision 0** derived mesh identity from a Tailscale tailnet. Rejected: it
requires an account, an internet round-trip to a third-party coordination
server, and a third-party install.

**Revision 1** made mDNS LAN discovery the out-of-the-box default and demoted
overlays to one optional tier. The audit upheld the tiering and the trust model
but found four false rationales and one unflagged security regression:

| Revision 1 claim | Finding |
|---|---|
| "Port 5353 is already permitted by firewall rules users have in place" | **False.** Windows Firewall rules are per-application. A new `vox.exe` inherits nothing from Chrome or Teams; both mDNS and a bespoke UDP port get the identical prompt. The rejection of the simpler 80-line option rested on this. |
| "Adopting iroh means porting the axum router onto its stream abstraction" | **False.** `iroh::endpoint::SendStream` implements `tokio::io::AsyncWrite`; serving the unchanged router is ~20–40 lines via `hyper_util::TokioIo`, or zero via a local TCP forward. |
| "This is the Syncthing model" | **Half true.** Syncthing's *trust* model is what this design follows. Its *discovery* is a bespoke protobuf on UDP 21027, explicitly not mDNS. |
| LAN discovery satisfies "works out of the box" for all users | **False for a large class.** See §3.1. |
| (omitted) | Binding off-loopback while no auth material is configured grants `FullAccess` to the LAN, including remote execute. See §5. |

**Revision 2** (this document) inverts the tier ordering so the universal
mechanism ships first, makes the auth gate a blocking prerequisite, reconciles
with the ratified mesh SSOT, and corrects the four rationales.

---

## Part 1 — Requirements

Binding, and they order every decision below. Unchanged from revision 1.

1. **Works out of the box.** Two machines with vox installed and no
   configuration must be able to find each other and share work.
2. **No central server.** Fully functional with Vox project infrastructure
   switched off permanently.
3. **Works without internet.** A LAN with no route out, or an air-gapped
   network, is a supported deployment.
4. **No account required.** No sign-up, no third-party identity provider, no
   email address.
5. **Secure by default.** The safe configuration is the one a user reaches
   without reading documentation.
6. **Confirmed in the GUI.** Not done until visible and operable in `vox-gui`.

---

## Part 2 — Reconciliation with the ratified mesh SSOT

[`mesh-and-language-distribution-ssot-2026.md`](../../src/architecture/mesh-and-language-distribution-ssot-2026.md)
is ratified (council decision D16, 2026-05-15). Its line 101 states a binding
non-goal: *"Paired peers + GitHub attestation are the binary gates."* Phase 6
task P6-T7 commits to a `vox populi join <invite>` flow built on
GitHub-attested manifests.

**Requirement 4 of this document is incompatible with that gate.** GitHub
attestation is a third-party identity provider requiring an internet round-trip
to gist.github.com. Revision 1 did not name this conflict. This revision does.

### What is actually shipped

The conflict is smaller than it appears, because the attestation gate was never
wired. Verified by call-graph search:

| Symbol | Non-test callers |
|---|---|
| `pairing::github_attestation::fetch_and_verify` | **zero** |
| `pairing::device_flow` (GitHub OAuth) | **zero** |
| `transport::auth_ed25519::verify_against_trust` — the only function that consults `TrustedNodeRegistry` from the wire path | **zero** |
| `PublicAttestationManifest` | two CLI commands and tests only |
| `mesh.federation_peers.<id>` config key written by `vox populi join` | **never read by anything** |
| `crates/vox-populi/tests/pairing_e2e.rs` | a one-line placeholder comment |

`populi_join.rs::run()` parses an `invite_sig_b64` and offers
`--insecure_skip_verify`, then verifies nothing — it fetches the manifest over
HTTP and persists it. Roughly 2,500 lines of pairing, attestation, quota, and
federation source exist with no path from the wire to any of it.

### Resolution

- **Local peers** (same LAN or same overlay) are gated by ticket-carried pubkey
  and explicit approval. No attestation, no internet.
- **Internet peers**, if that tier is ever built, may additionally require
  attestation. This design neither implements nor forbids that.
- **P6-T7's `vox populi join <invite>` is superseded, not silently dropped.**
  Its invite-URL format carries a manifest URL for a remote peer; the ticket
  format in §4 carries a pubkey for a reachable peer. They are different tiers.
  The deprecation is deliberate and recorded in §7.1.

An amendment noting this belongs in the SSOT, filed with the first
implementation commit. This document does not unilaterally amend a ratified SSOT.

---

## Part 3 — Transport and discovery tiers

Revision 1 ordered these by locality. That was wrong: it made the mechanism that
*sometimes* works the headline and the mechanism that *always* works an
unspecified footnote. **Ordered by coverage instead.**

| Tier | Mechanism | Coverage | Status |
|---|---|---|---|
| **T0** Loopback | `127.0.0.1` | single machine | always |
| **T1 Ticket** | pasteable `vox-mesh://` URI | **100% of networks** | **default; ships first** |
| **T2 LAN auto** | mDNS on `_vox-mesh._tcp.local.` | most home/office LANs | convenience layer over T1 |
| **T3 Overlay** | Tailscale, Headscale, Nebula, ZeroTier, iroh | cross-site | optional, detected |
| **T4 Relay** | rendezvous via a third party | — | **prohibited. Not deferred.** |

### 3.1 Why the ticket is the default, not the fallback

No broadcast or multicast mechanism can satisfy Requirement 1 on these networks,
and the blocker is the firewall and the access point, not the wire format:

- **Managed Windows, non-administrator user.** Per Microsoft's firewall-rules
  documentation, when the inbound-bind Security Alert is declined *"block rules
  are created. It doesn't matter what option is selected"* — and a standard user
  cannot create an allow rule at all. With notifications disabled (normal on
  managed fleets) the block is silent, with no prompt.
- **Guest Wi-Fi with client isolation.** A vendor-recommended default at both
  Meraki and Ubiquiti; it also breaks AirPlay, Chromecast, and network printers.
- **Across VLANs or subnets.** `224.0.0.251` is TTL-1 link-local and does not
  cross a router without an opt-in reflector.
- **Busy 802.11.** RFC 9119: multicast frames are unacknowledged, not
  retransmitted, and sent at the lowest basic rate.

No credible published figure exists for what fraction of users this is, and one
is not invented here. The qualitative shape is sufficient: it is not a rare edge.

**The ticket costs ~40 lines and works everywhere** — managed Windows, isolated
guest Wi-Fi, across VLANs, over an overlay, air-gapped. Critically, it adds *no
user friction that the design did not already require*: the trust model in §4
mandates an out-of-band comparison step regardless. The ticket collapses
discovery and that comparison into one pasteable string, exactly as Syncthing
does with its device ID.

**Format:**

```text
vox-mesh://<node_id>@<host>:<port>#<pubkey_hex>
```

`node_id` is derived from the pubkey (`hex(secure_hash(pubkey)[0..16])`), so the
ticket is self-verifying: the recipient recomputes the id from the key and
rejects a mismatch. The fragment carries the full 64-hex key, not a fingerprint,
so trust binds to key material rather than to a display string.

### 3.2 Why mDNS is still worth building (T2)

For the majority case — two machines on a home or office LAN — the ticket's
paste step is friction the user does not need. mDNS removes it. It is a
convenience layer over T1, not a replacement, and it uses the identical trust
path: a discovered peer is exactly a ticket the user did not have to paste, and
is equally inert until approved.

`mdns-sd` (pure Rust, no daemon, no Bonjour or Avahi requirement) is the right
crate; no better-maintained pure-Rust alternative exists. `libmdns` is
responder-only; `zeroconf` and `astro-dnssd` are FFI to Avahi/Bonjour and would
require users to install Bonjour on Windows, violating Requirement 1.

**Pin `mdns-sd >= 0.20` exactly.** Multi-NIC, VPN, and Hyper-V vSwitch handling
was the historic bug source and was reworked in 0.19–0.20. The crate has shipped
20 breaking releases in 78; budget a breaking bump every 6–12 months and keep the
`mdns-sd` surface confined to one file.

### 3.3 Overlays (T3)

The deciding constraint is that the control plane is HTTP over TCP bound to a
`SocketAddr`: any transport supplying a routable address works unchanged.

| Option | Model | Trade-off |
|---|---|---|
| **iroh** | QUIC, hole punching, relays — a Rust crate, no install, no account | **Best on Requirement 4 of any row here.** `SendStream: AsyncWrite`, so the existing router is served over it in ~20–40 lines. Defaults traverse n0's relays and `dns.iroh.link`, so it must be configured `RelayMode::Disabled` or self-hosted to satisfy Requirement 2. |
| Tailscale | WireGuard + hosted coordination | Easiest where already deployed. Account required, so unusable as a default. |
| Headscale | Self-hosted coordination, same clients | No SaaS. User runs a server. |
| Nebula | Own CA + lighthouses | Fully self-hosted. User runs a CA and a lighthouse. |
| NetBird | WireGuard + self-hostable, SSO | Younger; more moving parts than Headscale. |
| ZeroTier | L2 overlay, proprietary protocol | Mature; L2 exceeds requirements. |
| Plain WireGuard | Manual peer config | No NAT traversal, no dynamic membership. |

**iroh is not used for T1/T2 discovery**, for two reasons revision 1 failed to
give: its own local discovery *is* mDNS (the opt-in
`iroh-mdns-address-lookup` crate), so it cannot beat mDNS at LAN discovery
because it is mDNS; and its default endpoint configuration publishes to n0
infrastructure, which Requirement 2 forbids.

`libp2p` is rejected: `libp2p-mdns` is a `NetworkBehaviour` requiring a `Swarm`,
with no discovery-only mode, and its `PeerId`s are meaningful only inside libp2p.

Cloudflare Tunnel and ngrok remain the existing `tunnel` variant for exposing a
single node publicly. Hub-and-spoke through a third-party TLS terminator is not
a mesh transport.

### 3.4 T4 is a prohibition with a gate

Should a rendezvous service ever ship, T0–T3 must remain fully functional with it
switched off. **This needs a detector, not just prose** — a `vox-code-audit` rule
that fails on a vox-operated hostname in mesh-formation code. Absent enforcement,
a prohibition is a comment.

---

## Part 4 — Trust model

### The constraint, stated plainly

Automatic, secure, and serverless cannot all hold at once. Discovering or
receiving an address does not prove the peer is yours. Something must be
exchanged out of band exactly once. Designs that appear to avoid this have
hidden a bearer secret or a trusted third party.

**Discovery grants nothing; trust is explicit and binds to a public key.**

### Flow

1. Peer B prints a ticket (`vox mesh ticket`, or the GUI shows it). It carries
   B's pubkey.
2. A consumes the ticket: paste into the GUI, or `vox auth trust <ticket>`.
   A recomputes `node_id` from the pubkey and rejects a mismatch.
3. A stores `{node_id, pubkey_hex}` in `TrustedNodeRegistry`. B does the same
   for A. Trust is mutual and explicit.
4. Every subsequent request between them is an ed25519 signature verified
   against the stored pubkey. **Unsigned or unknown-key requests are rejected.**

T2 (mDNS) inserts one optional step: a discovered peer is displayed with its
pubkey-derived fingerprint, and approving it performs step 3 without a paste.
The user compares the fingerprint against the peer's own display. The peer's
pubkey arrives in the announcement and is verified by challenge before storage —
never trusted from the announcement alone.

### Security properties

- A hostile peer on the LAN can be discovered and can do nothing else.
- Trust binds to the pubkey, so it survives IP and hostname changes and cannot
  be claimed by announcing someone else's `node_id`.
- Revocation is local and immediate.
- Discovery traffic carries no secret; announcements may be freely observed.

### Rejected: pairing codes as the default

A short code removes a comparison step but is a bearer secret — anyone who
observes it joins silently, failing Requirement 5. The ticket is not a bearer
secret: it carries a *public* key, and pasting it grants the pasting side
nothing until the far side reciprocates.

---

## Part 5 — Fail-closed auth is a blocking prerequisite

**This is the single most important change in revision 2.**

Revision 1 mandated binding the LAN address by default. The audit found what
that unlocks:

- [`router.rs:124`](../../../crates/vox-plugin-populi-mesh/src/transport/router.rs) —
  when `requires_bearer()` is false, **every route** receives
  `PopuliAuthContext::FullAccess`.
- [`auth.rs:148`](../../../crates/vox-plugin-populi-mesh/src/transport/auth.rs) —
  `requires_bearer()` is false when no mesh, worker, submitter, admin, or JWT
  secret is configured. **That is exactly the zero-config case this design
  targets.**
- [`auth.rs:34`](../../../crates/vox-plugin-populi-mesh/src/transport/auth.rs) —
  `FullAccess` satisfies `auth_allows_worker_plane`, `auth_allows_deliver`, and
  `auth_allows_admin_route`.
- [`dispatch.rs:230`](../../../crates/vox-plugin-populi-mesh/src/transport/handlers/dispatch.rs) —
  that is the only gate on `POST /v1/populi/worker/execute`, whose default
  policy is `permissive`, and which writes the posted bytes to the temp
  directory and executes them.

Loopback binding is the only thing containing this today. **Changing the bind
without fixing the auth default converts a latent hole into an unauthenticated
remote-code-execution endpoint, and discovery advertises it.**

### Required before any bind change

1. When the resolved bind is **not loopback** and no auth material is
   configured, absent credentials must mean **deny**, not `FullAccess`.
2. Zero-config auth is the ed25519 node-signature path:
   `auth_ed25519::verify_against_trust` wired into the router middleware,
   checking the presented pubkey against `TrustedNodeRegistry`.
3. `--insecure-local` must hard-fail when the bind is non-loopback.
4. The signed payload currently covers `path.ts.nonce` but **not the request
   body**, so a valid signature does not authenticate the payload. Extend it.
5. `TrustedNodeRegistry::lookup_by_pubkey_hex` searches only the in-memory cache
   and never loads from disk, so on the production file-backed registry it
   always returns `None`. Fix before anything depends on it.

Only after these does the spec's own acceptance criterion — *dispatch to an
untrusted peer is refused* — describe reality rather than aspiration.

### Related live defects, fixed in the same pass

- `vox populi up` spawns `populi serve` **without `--enable`**, which bails
  immediately; both pipes are `Stdio::null()`, so it fails silently while the
  parent records a pid and writes state as though it succeeded. **`vox populi
  up` has never started a server.**
- `/v1/populi/bootstrap/exchange` swaps its used-flag *before* comparing the
  token, so one unauthenticated malformed POST permanently burns the bootstrap
  window.
- `tls.rs` has no caller in `serve()`. A non-loopback bind therefore carries
  bearer tokens in plaintext. Either wire the acceptor or state the limitation.

---

## Part 6 — Component design

### Process topology — the constraint that shapes the wiring

There are **three** processes, not one:

1. `vox-gui` — no in-process MCP host; it was deliberately removed.
2. `vox-orchestrator-d` — a spawned child binary reached over TCP `127.0.0.1:9745`.
   **`vox_mesh_nodes` executes here.**
3. `vox populi serve` — a separate child process spawned by `vox populi up`.

A process-global cannot span these. Discovery state must therefore be either
(a) started independently in whichever process reads it, or (b) passed over an
existing channel. This design uses (a): announce-and-browse in `populi serve`,
and lazy **browse-only** in the reading process. Two mDNS sockets on one host is
normal and supported.

`vox-orchestrator-d` depends on neither `vox-populi` nor `vox-identity`, so the
lazy browse must live behind a call the MCP crate can already make.

### Crate edges

**No new workspace crate edges.** Two that revision 1 assumed exist do not:
`vox-orchestrator-mcp → vox-identity` and `vox-ml-cli → vox-identity`. Both are
avoided by routing through `vox-populi`, which already depends on `vox-identity`,
and exposing `global_peers_with_trust()` from there.

### Feature reachability

Revision 1 hung discovery on `populi-transport`. That feature gates
`reqwest`/`axum`/`turso` HTTP machinery and is **off in every shipped binary**;
`vox-ml-cli`'s `populi` feature is likewise not in its default set, and the
release builder passes no features. The feature must be independent, and it must
be enabled by `vox-gui`, `vox-orchestrator-d`, and the release build, or the
work ships dead.

### Discovery module

Confined to `crates/vox-populi/src/discovery/`, with the `mdns-sd` surface in
one file.

```text
ticket::{Ticket, parse, render}          // T1 — no mdns dependency
mdns::{DiscoveryHandle, ...}             // T2
bind::{BindTier, resolve_bind, is_advertisable}

start_global(announce)          // announce + browse (populi serve)
global_peers_browsing()         // lazy browse-only (any reading process)
global_peers_with_trust()       // resolves trust inside vox-populi
```

**Liveness is the crate's, not ours.** `ServiceResolved` fires **once per
instance**, not per announcement, so a self-managed staleness clock would empty
the peer list permanently after its first window. `mdns-sd` re-queries before TTL
expiry and emits `ServiceRemoved` on expiry and on goodbye. Online-versus-offline
comes from the control-plane health check, not from mDNS.

Peers are keyed by DNS-SD fullname, not by the announced `node_id`, so a peer
claiming another's id cannot silently replace it and both are listed. The map is
capped at 256 entries with oldest-eviction.

An announcement is made only when a routable address exists; otherwise the node
browses without announcing. Announcing `127.0.0.1` would publish an address that
health-checks green against every peer's own loopback.

### Bind resolution

Bind follows the tier; explicit `--bind` is honored verbatim; `0.0.0.0` is never
synthesized. Two corrections from the audit:

- `detect_lan_ip` is a **default-route** probe, not a LAN probe — on a VPS it
  returns the public address. The `Lan` tier must additionally require a private,
  link-local, or CGNAT range and fall back to loopback otherwise.
- Detecting "explicit" by string-comparing against the default value silently
  overrides a user who deliberately types the default. `--bind` becomes
  `Option<String>`.

### GUI

- `vox_mesh_nodes` gains `pending_peers` and a `discovery_state` discriminator
  (`disabled` / `failed` / `running`) so "firewall blocked" is distinguishable
  from "nobody home" — the spec's own requirement that an empty mesh states why.
- `MeshView` gains a Pending section with fingerprint and a Trust action, a
  paste box for tickets, the local node's own ticket for copying, and an Untrust
  action. Trusted peers not yet reported by a control plane must still render;
  otherwise approving a peer makes it vanish from the UI entirely.
- `MeshWidget` takes `data: DashboardData` and calls no hook today. Adding a
  pending count is a real change to that component, not a derivation.
- The local fingerprint currently lives only in Settings and reads
  `(locked — provide master password to view)` on a fresh install, which makes
  the comparison ceremony unperformable. The Mesh surface must show it.

---

## Part 7 — SSOT rework

### 7.1 Two meanings of "join"

`POST /v1/populi/join` (a node registers itself) keeps the verb — it is the
ADR-008 wire surface. The CLI `vox populi join <invite>` (registers a remote
peer) is deprecated in favor of `vox auth trust`, which already writes
`TrustedNodeRegistry`. Per §2 this supersedes P6-T7 deliberately.

`vox auth trust` currently takes a 64-hex pubkey. It gains ticket parsing so the
deprecation notice points somewhere a user can actually go.

### 7.2 Three stores holding peer state

| Store | Holds | Verdict |
|---|---|---|
| `~/.vox/trusted_nodes.json` | node_id, pubkey | **SSOT for trust.** |
| `~/.vox/cache/populi/local-registry.json` | capabilities, liveness | Keep — observed state, correctly separate. |
| `~/.vox/config.toml` `mesh.federation_peers.*` | manifest URLs | **Delete.** Written by `vox populi join`, read by nothing. |

Revision 1 specified a migration of the third into the first. **Dropped.** A
manifest URL is not evidence of a key, and migrating one into the trust registry
would create rows with empty pubkeys — precisely the spoofable state §4 forbids.
The key is unread and is removed outright.

### 7.3 Crate-wide `#![allow(dead_code)]`

Removed from `vox-plugin-populi-mesh`. Expect this to surface far more than
revision 1 anticipated, given §2's ~2,500 unreferenced lines.

---

## Part 8 — Failure modes

| Condition | Behavior |
|---|---|
| Multicast blocked (managed Windows, guest Wi-Fi, cross-VLAN) | T2 yields nothing; GUI reports `discovery_state: failed` or empty and directs the user to the ticket. **T1 is unaffected — this is why it is the default.** |
| mDNS bind fails | Warn, continue at T0/T1/T3. Never fatal. |
| Two nodes announce one `node_id` | Both listed (keyed by fullname); differing pubkeys make the impostor untrustable. |
| Peer discovered, vox not serving | Absent from the health-checked list. |
| Peer trusted, then IP changes | Unaffected — trust binds to the pubkey. |
| Overlay and LAN both present | Address *preference* (same-subnet LAN first) is separate from bind tier. |
| No internet | Fully supported: T0, T1, T2 involve no external traffic. |
| Ticket pasted with mismatched id and key | Rejected — the id is recomputed from the key. |

---

## Part 9 — Out of scope

- Internet-peer attestation (see §2) — neither implemented nor forbidden.
- Headscale, Nebula, and iroh T3 resolvers — documented, not implemented.
- Pairing codes — rejected as a default; possible opt-in later.
- Cross-node secret pairing (W3), trace propagation (W5), model inventory (W4).
- Any vox-operated coordination service. Rejected, and §3.4 asks for a detector.
