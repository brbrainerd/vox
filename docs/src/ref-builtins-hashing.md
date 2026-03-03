# Hashing & Identity Builtins

Vox provides **three native hashing primitives** backed directly by Rust crates.
These are exposed in Vox source as `std.*` calls and in Rust as
`vox_runtime::builtins::vox_*` functions. The compiler rewrites the Vox syntax to
direct Rust calls — there is no FFI overhead.

---

## Three-Tier Strategy

| Function | Algorithm | Output | Use Case |
|---|---|---|---|
| `std.hash_fast(x)` | XXH3-128 | 32-char hex | Caches, dedup, transient IDs |
| `std.crypto.hash_secure(x)` | BLAKE3-256 | 64-char hex | Provenance, content addressing, DB storage |
| `std.uuid()` | Timestamp + atomic counter | `vox-{ts}-{seq}` | Unique record IDs |
| `std.now_ms()` | `SystemTime` | `u64` ms | Timestamps |

---

## Vox Syntax

```vox
# Fast non-cryptographic hash (XXH3-128)
let cache_key = std.hash_fast(content)

# Cryptographic content-addressable hash (BLAKE3-256)
let input_hash = std.crypto.hash_secure(message)

# Unique monotonic ID (timestamp + counter, never repeats)
let request_id = std.uuid()

# Current UNIX timestamp in milliseconds
let ts = std.now_ms()
```

Also available via namespaced syntax:

```vox
let h1 = std.crypto.hash_fast(text)   # same as std.hash_fast
let h2 = std.crypto.uuid()            # same as std.uuid
let t  = std.time.now_ms()            # same as std.now_ms
```

---

## When to Use Which

### `std.hash_fast` — XXH3-128

- **Rate:** ~20–60 GB/s on modern hardware (SIMD-accelerated)
- **Output:** 32-character lowercase hex (128-bit)
- **Deterministic:** Yes — same input always produces same hash across machines
- **Collision resistance:** Excellent for non-adversarial data (~2⁻⁶⁴ probability for 128-bit)
- ✅ HashMap cache keys, training data deduplication, activity ID short-circuits
- ✅ `ast_hash` in training corpus (content fingerprint for incremental extraction)
- ✅ `payload_hash` in prompt canonicalization (debug logging)
- ❌ **Do not** store as permanent provenance in the database — not cryptographically secure

### `std.crypto.hash_secure` — BLAKE3-256

- **Rate:** ~6–14 GB/s on modern hardware (faster than SHA-256 and SHA-3)
- **Output:** 64-character lowercase hex (256-bit)
- **Deterministic:** Yes — identical output on all platforms
- **Security:** Cryptographically secure (collision resistance ≈ 2⁻¹²⁸, comparable to AES-128)
- ✅ `input_hash` in FTT `ProcessingRun` — permanent provenance stored in DB
- ✅ Content-addressable storage keys
- ✅ Cross-machine deduplication
- ✅ Integrity verification of LLM prompts and responses
- ❌ Slightly slower than `hash_fast` (~10× depending on workload)

### `std.uuid` — Monotonic ID

- **Format:** `vox-{16-char nanos hex}-{16-char counter hex}`
- **Uniqueness:** Guaranteed within a process (atomic counter prevents same-nanosecond collisions)
- **Rate:** Millions per second (atomic increment + SystemTime, no locks)
- ✅ `request_id`, `run_id`, companion IDs, battle IDs — any record needing a unique primary key
- ❌ Not a UUID v4 (not random) — do not use where RFC 4122 UUID is required

---

## Benchmark Estimates

Measured on a modern x86-64 CPU with 4 KB input. Numbers are throughput
estimates based on published benchmarks for the underlying crates.

| Operation | Crate | ~Throughput |
|---|---|---|
| `hash_fast` (XXH3-128, 4 KB) | `twox-hash 2.x` | ~60 GB/s |
| `hash_fast` (XXH3-128, 64 B) | `twox-hash 2.x` | ~15 GB/s |
| `hash_secure` (BLAKE3, 4 KB) | `blake3 1.x` | ~14 GB/s |
| `hash_secure` (BLAKE3, 64 B) | `blake3 1.x` | ~4 GB/s |
| `uuid` | std (atomic+clock) | >10 M/s |
| SHA-256 (reference) | `ring` | ~2 GB/s |
| SHA-3-256 (reference) | `sha3` | ~1 GB/s |

> **Key takeaway:** `hash_secure` (BLAKE3) is 5–7× faster than SHA-256 while being
> fully cryptographically secure. `hash_fast` (XXH3) is ~4× faster than BLAKE3
> for non-security use cases.

---

## Collision Avoidance Design

Two distinct risks are addressed by the three-tier design:

1. **Hash flooding / DoS**: An adversary who can craft collisions for a
   non-cryptographic hash could cause HashMap performance to degrade.
   Vox's `HashMap` uses Rust's default **SipHash-1-3** (already DoS-resistant)
   for internal data structures. `hash_fast` is used only where inputs are
   controlled (training data, internal content addressing).

2. **Cross-machine collision of permanent IDs**: `hash_secure` (BLAKE3) ensures
   two different input strings will never collide in a DB table with probability
   better than 2⁻¹²⁸. This is the appropriate hash for any ID stored permanently.

---

## Rust API

Accessible directly from Rust code (e.g. in `vox-cli`, `vox-runtime` internals):

```rust
use vox_runtime::builtins::{vox_hash_fast, vox_hash_secure, vox_uuid, vox_now_ms};

// Fast non-cryptographic (XXH3-128)
let key: String = vox_hash_fast("some cache key");  // 32-char hex

// Cryptographic (BLAKE3-256)
let id: String = vox_hash_secure("input to hash");  // 64-char hex

// Unique ID
let uid: String = vox_uuid();         // "vox-{ts_hex}-{counter_hex}"

// Current time
let ts: u64 = vox_now_ms();          // milliseconds since UNIX epoch
```

### Crate Dependencies

| Crate | Version | License |
|---|---|---|
| `twox-hash` | `2.x` | MIT |
| `blake3` | `1.x` | Apache-2.0/CC0 |

Both are added to the Vox workspace `Cargo.toml` and available in `vox-runtime`.

---

## Codegen Mapping

The Vox compiler (`vox-codegen-rust/src/emit_expr.rs`) rewrites these calls
at compile time:

| Vox Source | Generated Rust |
|---|---|
| `std.uuid()` | `vox_runtime::builtins::vox_uuid()` |
| `std.now_ms()` | `vox_runtime::builtins::vox_now_ms()` |
| `std.hash_fast(x)` | `vox_runtime::builtins::vox_hash_fast(&x)` |
| `std.hash_secure(x)` | `vox_runtime::builtins::vox_hash_secure(&x)` |
| `std.crypto.hash_fast(x)` | `vox_runtime::builtins::vox_hash_fast(&x)` |
| `std.crypto.hash_secure(x)` | `vox_runtime::builtins::vox_hash_secure(&x)` |
| `std.crypto.uuid()` | `vox_runtime::builtins::vox_uuid()` |
| `std.time.now_ms()` | `vox_runtime::builtins::vox_now_ms()` |

No heap allocation or FFI is involved — these are direct Rust function calls
that the compiler inlines into generated code.

---

## Related

- [Security Model](expl-security.md) — how Vox handles secrets and threat modeling
- [vox-runtime API](api/vox-runtime.md) — full runtime module reference
- [FTT Pipeline](api/example_greaterfool_reference.md) — live usage of `hash_secure` and `uuid` in production
