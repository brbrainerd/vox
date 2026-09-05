//! iroh QUIC mesh transport (ADR-047).
//!
//! Replaces the hand-rolled HTTP control plane, ed25519 envelope, and JWT auth
//! matrix that `vox populi` used to carry. iroh supplies transport, identity,
//! and NAT traversal; vox keeps capability scheduling.
//!
//! Three invariants this crate exists to hold, all of them learned the hard way
//! and recorded in [ADR-047](../../../docs/src/adr/047-iroh-transport.md):
//!
//! 1. **`presets::Minimal` only.** `presets::N0` adds pkarr publishing, DNS
//!    address lookup, and n0's relay servers. The mesh must contact no third
//!    party and must work with both machines off the internet.
//! 2. **Never `into_0rtt()`.** In the 0-RTT state `remote_id()` becomes
//!    fallible, and the `?` someone adds to satisfy the compiler quietly turns
//!    every trust check in this crate advisory.
//! 3. **Pairing grants reachability, never native execution.** A trusted peer
//!    gets a sandbox by default.

pub mod directory;
pub mod endpoint;
pub mod identity;
pub mod mailbox;
pub mod protocol;
pub mod trust;

pub use directory::{PeerEntry, directory};
pub use endpoint::{JobExecutor, ReceivedJob, bind, serve};
pub use identity::load_or_create;
pub use mailbox::{Inbox, MailboxLimits, Outbox};
pub use protocol::{ALPN, Hello, Isolation, JobLimits, JobRequest, PROTO, check_hello};
pub use trust::{MeshTrust, TrustLevel, TrustedEndpoint};
