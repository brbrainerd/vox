---
title: "Internal Web IR Implementation Blueprint"
description: "Implementation plan, acceptance gates, and parity requirements for the Vox internal Web IR layer."
category: "Architecture SSOTs"
status: current
---

# Internal Web IR Implementation Blueprint

This document describes the implementation plan and acceptance gates for the Vox internal Web IR layer.

## Overview

Web IR is an intermediate representation that sits between the HIR (High-level IR) and the final TypeScript/TSX output. It enables validation, optimization, and parity testing across different codegen paths.

See also: [ADR 012 — Internal Web IR Strategy](../adr/012-internal-web-ir-strategy.md)

## Architecture

The Web IR pipeline consists of three stages:

1. **Lowering** (`lower_hir_to_web_ir`) — translates HIR into Web IR nodes
2. **Validation** (`validate_web_ir`) — checks structural invariants  
3. **Emission** (`emit_tsx`) — generates TypeScript/TSX from Web IR nodes

## Acceptance gates

The following gates must pass before the Web IR layer is considered stable:

| Gate | Description | Status |
|------|-------------|--------|
| G1 Lower Gate | HIR lowers to Web IR without panics for all fixtures | ✅ |
| G2 Validate Gate | `validate_web_ir` returns empty diagnostics for clean fixtures | ✅ |
| G3 Emit Gate | Web IR emitter produces valid TSX matching legacy codegen | ✅ |
| G4 Parity Gate | Legacy TSX output matches Web IR TSX output for all golden fixtures | ✅ |

## Parity Gate (G4)

The G4 Parity Gate verifies that the Web IR emitter produces output identical to the legacy codegen path for all golden `.vox` fixtures. Differences trigger a parity contract failure.

Components tested under G4 Parity Gate:
- `component` declarations with `state` and `view`
- `@island` declarations with prop types
- `routes { }` blocks
- `@endpoint(kind: query/mutation/server)` functions
