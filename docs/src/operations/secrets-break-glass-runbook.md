---
title: "Secrets Break-Glass Runbook"
description: "Emergency access workflow for Secrets Cloudless with JIT controls, immutable audit, and mandatory rotation."
category: "Operations"
last_updated: "2026-05-08"
training_eligible: true

schema_type: "TechArticle"
---

# Secrets Break-Glass Runbook

## Purpose

Define emergency access procedure that balances incident response speed with accountability and post-use containment.

## Preconditions

- Active incident ticket with severity.
- Named operator identity.
- Explicit reason code.
- Time-bound approval window.

## Break-glass workflow

1. Open incident and request emergency access.
2. Approver validates necessity and scope.
3. Issue short-lived privileged credential (JIT).
4. Record immutable audit event (grant time, operator, reason, scope).
5. Perform emergency actions.
6. Revoke credential immediately after use or TTL expiry.
7. Record immutable audit event (revoke and action summary).

## Mandatory controls

- No standing permanent break-glass credential.
- No shared unscoped root token for routine operations.
- All actions mapped to individual identity and ticket.
- Dual control required for high-impact classes.

## Post-incident mandatory tasks

1. Rotate all credentials touched during break-glass.
2. Validate systems return to strict policy mode.
3. Review audit trail completeness.
4. Capture corrective actions and close incident.

## Failure conditions

- Missing ticket/reason -> deny break-glass.
- Missing immutable audit sink -> deny break-glass.
- Inability to rotate touched credentials post-incident -> incident remains open.


