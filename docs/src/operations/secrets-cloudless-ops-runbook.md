---
title: "Secrets Cloudless Ops Runbook"
description: "Operator procedures for Cloudless secret custody, backup/restore, rotation, and incident handling."
category: "Operations"
last_updated: "2026-05-08"
training_eligible: true

schema_type: "TechArticle"
---

# Secrets Cloudless Ops Runbook

## Purpose

Define operator-grade procedures for running Cloudless secret persistence safely across local, canonical, and replicated VoxDB modes.

## Operational invariants

- No plaintext secrets in persisted database rows.
- Secret values never logged.
- All privileged actions produce auditable events.
- Rotation is mandatory after incident-driven privileged access.

## Identity & UX Warnings

- **Default Account Warning**: If `vox secrets doctor` flags that `VOX_ACCOUNT_ID` is set to `default-account`, you **MUST** configure a unique identifier. Running the cloudless vault on `default-account` can cause catastrophic multi-device database drift and conflicting secret IDs when syncing state.
- Always run `vox secrets status` after provisioning to verify that Clavis identifies your local KEK and node identity properly.

## Key custody model & KEK Rotation

- Account-level secrets are encrypted with DEK-per-record using AES-256-GCM.
- KEK references are managed by the approved custody path (local keyring bootstrap via OS secure enclave/credential manager).
- **KEK Rotation**: 
  - To rotate the Key Encryption Key (KEK), use `vox secrets rotate-kek`.
  - The vault will temporarily decrypt all secrets using the active KEK, generate a new OS keyring entry, re-wrap all DEKs, and permanently shred the old KEK reference.
  - Doing this while offline is supported, but you must ensure any remote replicas are synced immediately after coming back online to prevent split-brain decryption failures.

## Multi-Device Vault (Synchronization)

When using Vox across multiple environments, there are two primary patterns for syncing your Clavis credentials:
1. **LibSQL Replica (Recommended)**: Run the cloudless vault using `vox secrets vault serve --libsql-sync`. This sets up a shadow local SQLite file synced securely via an embedded replica. Your KEK remains device-local, meaning the synced vault file is useless without the enclave KEK. You must securely exchange your KEK to the new device once (via `vox secrets export-kek`).
2. **Manual Export**: Run `vox secrets export-env --encrypted` to dump a ciphertext payload that can be transferred via secure channels or committed to a private repository.

## VoxDb Schema Hardening

- **CRITICAL INVARIANT**: Never store plaintext secrets, API keys, or OAuth tokens in the standard `VoxDb` schema or user-facing tables. 
- All external API secrets MUST route through the separate Clavis vault plane.
- The Product DB / Codex plane must ONLY store `SecretId` references or cryptographic checksums.

## Backup procedure (encrypted data only)

1. Verify cluster/store health via `vox secrets doctor`.
2. Snapshot encrypted secret rows and key-reference metadata via `vox secrets snapshot`.
3. Verify snapshot integrity hash and store in approved backup location.
4. Record audit event with operator identity and reason.

## Restore procedure

1. Restore encrypted rows and key-reference metadata.
2. Validate key-reference availability before enabling reads.
3. Run integrity checks for ciphertext parse/decryptability.
4. Enable read path in staged mode; then full mode after verification.

## Incident handling

1. Trigger incident record and severity.
2. Restrict access boundaries (least privilege).
3. Execute break-glass only if approved and required.
4. Rotate all affected credentials strictly through `vox secrets reset --force` immediately after containment.
5. Publish post-incident findings and closure criteria.

## Replication and consistency notes

- Treat stale replica reads as non-authoritative for secret mutation checks.
- Use strict consistency for write-critical operations.
- For replica-latest modes, enforce deterministic stale-data error handling.

## Health checks

- Backend availability via `vox secrets backend-status`.
- Encryption/decryption roundtrip checks.
- Local keyring integrity.
- Audit log append health.



