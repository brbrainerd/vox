# vox-populi — Semantic Behavior Map

Synthesized from 236 extracted Behavior claims (deduped to ~70 distinct symbols). vox-populi is the mesh/control-plane + MENS-training crate, so its surfaces split into four families: (1) cryptographic envelope/attestation, (2) HTTP control-plane lease/maintenance/quarantine lifecycle, (3) quota/reputation rate-limiting, and (4) the MENS model-training pipeline (HF config parsing, memory budgeting, keymaps, manifests, preflight). Security and lifecycle surfaces are generally proven with error+invariant coverage; the MENS pipeline carries most of the happy-path-only debt.

## Cryptographic envelope & attestation

- **SignedA2AEnvelope::verify_self_signed()** — error+ : rejects swapped signature, tampered payload, and pubkey-not-matching-signer (all `SignatureMismatch`). Strong negative coverage.
- **verify_against_trust()** — happy+error : admits known pubkey (returns node_id), rejects unknown pubkey (`UnknownPubkey`).
- **AuthScheme::from_env()** — happy : defaults to `Ed25519Envelope` when env unset. (No malformed-value edge.)
- **AttestationManifest::verify() / ::new_signed()** — happy+error : roundtrip verifies; rejects swapped pubkey (`SignatureMismatch`) and expired manifest (`Expired`). Good edge coverage.
- **fetch_and_verify()** — happy only : fetches+verifies remote manifest, extracts github_login. No network-failure / malformed-remote path.
- **DeviceFlow::start() / ::poll_until_token()** — happy only : returns user_code / access_token from mock. No denied / slow_down / expired-token poll error.

## HTTP control-plane (auth, leases, maintenance, quarantine, inbox)

- **deliver_bearer_string()** — happy : mesh > submitter > admin precedence proven across three cases.
- **mesh_worker_plane_bearer_string()** — happy+error : returns trimmed mesh token; returns None for non-mesh (submitter-only) token.
- **normalize_http_control_base()** — happy+error : adds scheme/strips slash, accepts loopback; rejects `0.0.0.0` and `[::]` bind-all. Good security edge.
- **exec_lease grant/renew/release/list/revoke** — happy+error+invariant : grant yields non-empty lease_id + positive expiry; renew-after-release → 404; revoke-twice → 404; second holder → 409; renew by non-holder → CONFLICT; grant idempotent for same holder (identical lease_id, non-decreasing expiry); list reflects grant+sweep; release succeeds under maintenance for drain. Excellent lifecycle coverage.
- **relay_a2a_lease_renew** — error : non-holder renew → CONFLICT; maintenance blocks renew → FORBIDDEN.
- **relay_a2a_inbox / _limited / _all_paged / A2AInboxPager** — happy+invariant : honors max_messages and before-cursor; paged collection returns descending-ID order; pager terminates on empty pages respecting page size.
- **admin_quarantine** — happy : quarantined node's inbox is empty, releases after clear. (Negative is implicit in the happy assertion.)
- **admin_maintenance / maintenance_for_ms** — happy+error : claims resume after deadline; renew blocked during maintenance.
- **bootstrap_exchange** — happy+error+invariant : valid token returns mesh_token; one-time-use (second → GONE); wrong token → UNAUTHORIZED; unconfigured → NOT_FOUND. Strong.
- **PopuliMeshAuthRuntime (JWT HS256)** — happy : accepts first valid token, rejects jti replay (401 after 200).
- **relay_a2a() (job-result attestation)** — happy+error : valid ed25519 signature succeeds; incomplete attestation (hash without signature) errors.
- **dispatch_result_poll() / PopuliTransportState::new_for_serve()** — happy : persisted results survive restart (success/output/is_truncated). No corrupt-store or missing-key error path.

## Quota / reputation

- **PeerBucket::try_consume()** — happy+edge+invariant : starts full, drains, rejects overflow when empty, refills over time, refill clamps to capacity. Well-covered.
- **ReputationEma::update()** — happy : drops below 0.1 after failures, recovers after successes.
- **QuotaRegistry::reputation() / record_outcome()** — happy : unknown key → 1.0 default; repeated failures drive below threshold.

## Store layer

- **InMemoryMeshStore (a2a put/list/ack/paginate, exec_lease put/list/revoke, dispatch get/put, load_all)** — happy : roundtrip, receiver filtering, ack-exclusion (with include_acked override), since_id + limit pagination.
- **integrity_check()** — error : detects duplicate idempotency_dedupe_key, report carries `dedupe_violation` finding. Good integrity negative.

## MENS — HF config & architecture detection

- **detect_hf_architecture() / config_dims_for_architecture()** — happy : GPT2 from minimal config; qwen2 / qwen2.5 → Qwen35 with correct dims.
- **parse_transformer_layout() / HfTransformerLayout::from_config_path()** — happy+edge+invariant : GPT2/Llama/Mistral fixtures; Phi falls through to Gpt2 (edge); Llama/Mistral classified Qwen35 (invariant); Qwen3.5 linear-attention geometry fields parsed.
- **HfArchitecture** — invariant : Llama/Mistral map to Qwen35; Phi → Gpt2 fallthrough.

## MENS — hardware probe pipeline

- **ProbePipeline::run() / reorder() / default_for_platform()** — happy+edge+invariant : returns first Found; skips NoDevice/NotApplicable and continues; failure does not abort; all-fail → "Host CPU" fallback; empty pipeline → CPU fallback zero attempts; multi-run no panic; every attempt has probe_name; NotApplicable duration=0; default platform has ≥1 probe.
- **validate_probe_names()** — happy+error : accepts known, rejects unknown (error names the offender). Proper validator negative.
- **MockProbe / probe()** — happy : returns configured name/applicable/result and propagates configured ProbeError.

## MENS — memory budgeting

- **params_b_from_model_hint()** — happy : parses 4B/0.8b/70B patterns, unknown → None.
- **plan_qwen35() / plan_qwen25coder()** — edge+invariant : 4B→2B and 3B→1.5B VRAM retreats; not over_budget after retreat; reasonable seq-len preserved. Good edge.
- **is_qwen35() / is_qwen25coder()** — happy+invariant : case-insensitive positive detection, negative for Llama / non-coder.
- **auto_preset() / get_system_vram_gb()** — happy : preset mapping across VRAM tiers (None when ineligible); env override respected.

## MENS — keymaps, contracts, manifests, training text

- **hf_keymap (middle/full-block/strict-preflight key generators)** — happy+edge+invariant : GPT2/Qwen2/Qwen3.5 HF-standard keys; MLP+attn inclusion; strict preflight omits synthesizable rope inv_freq (edge) and is exactly 2 keys shorter (invariant).
- **finetune_contract_digest()** — invariant : digest changes with each of 6 flags (proxy-stack, lm_head_only, proxy_max_layers, ce_last_k, deployment target, provenance). Strong determinism coverage.
- **chatml_supervised_text()** — happy+invariant : contains markers/role/assistant text; starts with open-assistant prefix, ends with im_end.
- **PopuliTrainBackend (FromStr)** — happy : lora/burn-lora → BurnLora, qlora → CandleQlora. No invalid-string rejection.
- **AdapterMethodRegistry::resolve** — happy : Lora/Qlora → Some. No None/unknown path.
- **initial_training_manifest / InitialManifestRun::from_lora_config / write+load_manifest** — happy : kernel/objective/proxy fields set; grad_accum=0 clamped to 1; MobileEdge sets deployment fields; grad_accum=7 roundtrips.
- **TrainingPreflightRecord** — happy : serializes schema_version/contract_digest/kernel/notes.
- **format_loss_for_log** — happy : 0.0 → "0".
- **normalize_gpu_name() / ProviderKind / CloudTarget** — happy+error : strips GeForce + lowercases; provider display strings; parses auto/vast/runpod, rejects unknown ("gcp" → error).
- **edit_distance()** — happy : 0 for identical, 1 for one insert.
- **TimeEstimator::estimate()** — happy : Conservative with no data, Measured on exact match.

## MENS — checkpoint & data loading

- **CheckpointState::save/load/delete** — happy+edge+error : roundtrip persists/restores/deletes; load missing → None (edge); load corrupt JSON → None (error); load wrong schema → None.
- **load_all (debug_loader)** — error : panics with informative message when data file missing.
- **ExecutionPlanner::plan** — happy+error+edge : force BurnLora honored; rejects Burn+HF-without-config (architecture validation error); BurnLora leaves candle_proxy fields None (edge).
- **CheckpointBundle::to_operation_kind** — happy : JSON roundtrip to TrainingCheckpoint.

## MENS — distributed training (mostly happy)

- **DataParallelSession::step/checkpoint/resume + GradientShard::verify** — happy : step increments; shard/checkpoint signatures verify; resume matches step. All positive; no tamper/forgery negatives.
- **observe_telemetry / run_trust_snapshot_cycle** — happy : finding/snapshot built with correct fields and tier filter.
- **InferenceDispatcher::predict_auto** — happy : CPU stub returns "stub".

## Contracts / schema / OpenAPI

- **training-preflight.schema.json (Validator)** — happy+error : accepts valid record; rejects record missing contract_digest. Validator with a real rejection.
- **training-presets.v1.yaml** — invariant : default_base_model matches DEFAULT_MODEL_ID; KNOWN_PRESETS all present as ids/aliases.
- **OpenAPI spec** — happy+invariant : parses as 3.x with non-empty paths; paths match transport router exactly; version starts "3."; info.title present; each path has ≥1 operation; exec-lease + GPU-truth-layering schemas declared; A2AInboxRequest snake_case keys.
- **MERGE_QLORA_REJECTS_BURN_BIN** — invariant : message contains required keywords (Candle / mens-training-ssot / safetensors).
- **NodeRecord / capabilities / scope_id** — happy : sample JSON roundtrips id/gpu_vulkan/scope_id.

## TLS

- **TlsOptions::build_acceptor / handshake** — happy : builds acceptor at V1_3, handshake succeeds with self-signed cert. No bad-cert / version-mismatch rejection.

## Semantic gaps

Symbols proven **only** on the happy path whose contracts clearly carry a failure/empty/conflict mode. Ordered by actionability.

### Validators & parsers missing a rejection test
- **preflight_train_jsonl()** — the empty-line rejection exists, but there is no proof for a missing file or a malformed/non-JSON line — the two most likely real failures of a JSONL preflight. (`train_jsonl_preflight.rs`)
- **PopuliTrainBackend::from_str** — every proven case is a valid alias; no invalid-string → error proof, so the FromStr rejection arm is unverified. (`train_backend.rs`)
- **AdapterMethodRegistry::resolve** — only `Some` for builtins; the None/unknown-method contract is untested. (`finetune_registry.rs`)
- **TrainingPreflightRecord serialization** — field mapping is proven, but no invalid-kernel or schema-version-drift rejection. (`preflight_train.rs`)

### Mutators / lifecycle missing a failure path
- **InitialManifestRun::from_lora_config** — grad_accum=0→1 clamp is the only edge; other invalid configs (negative/oversized fields, conflicting deployment flags) have no rejection. (`manifest/tests.rs`)
- **sync_node_registry** — fresher-wins merge is happy-only; no tie-on-equal-timestamp resolution and no control-plane-fetch-failure path despite being a network merge. (`sync_node_registry.rs`)
- **normalize_hf_token_env** — both copy directions proven independently; the both-set (conflict/precedence) and neither-set (no-op) edges are unproven. (`mens/hub.rs`)
- **DomainRouter::route** — has the unregistered→None edge but no duplicate/conflicting-registration behavior. (`domain_router.rs`)

### Integrity / security surfaces proven only positive
- **GradientShard::verify()** and **DataParallelSession::resume()/checkpoint()** — all signed-artifact checks are proven only on *valid* signatures. There is no forged-shard, tampered-checkpoint, or signature-mismatch rejection — a security surface (signed gradients/checkpoints) verified happy-path-only. By contrast `SignedA2AEnvelope` and `AttestationManifest` both have tamper-rejection tests; the distributed-training signing surface should match that bar. (`distributed_training/strategy/data_parallel.rs`)
- **CheckpointBundle::to_operation_kind** — JSON roundtrip only; no corrupt or incompatible-kind deserialization path. (`distributed_training/checkpoint.rs`)
- **TlsOptions::build_acceptor** — handshake success only; no bad-cert / unsupported-version rejection, so the negative side of the TLS gate is unproven. (`tls_smoke.rs`)

### Remote/IO surfaces with no failure proof
- **fetch_and_verify()** and **DeviceFlow::poll_until_token()** — both are network round-trips proven only on success; no network-failure, malformed-remote-manifest, or denied/slow_down/expired-token poll error. (`github_attestation.rs`)
- **InferenceDispatcher::predict_auto** — only the CPU-stub backend path; no no-eligible-backend / backend-unavailable failure. (`inference/dispatcher.rs`)

### Estimators / metrics with thin edge coverage
- **edit_distance()** — only identical and single-insert; no substitution, deletion, or empty-string cases, so the DP core is under-exercised. (`mens/cloud/estimator.rs`)
- **TimeEstimator::estimate()** — Conservative and Measured sources proven; any interpolated/partial-profile or stale-data branch is unverified. (`mens/cloud/estimator.rs`)
- **observe_telemetry / run_trust_snapshot_cycle** — finding/snapshot built happy; no opted-out / zero-telemetry / empty-peer-graph edge. (`mens/discovery_publish.rs`)
