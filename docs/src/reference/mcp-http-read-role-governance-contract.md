---
title: MCP HTTP read-role governance contract
description: Canonical governance profile and schema for MCP HTTP read-token tool eligibility.
category: "Language Reference"

schema_type: "TechArticle"
---

# MCP HTTP read-role governance contract

Machine-readable governance profile for MCP HTTP read-token tool scope lives at:

**[`contracts/mcp/http-read-role-governance.yaml`](../../../contracts/mcp/http-read-role-governance.yaml)** (from repo root)

Schema:
**[`contracts/mcp/http-read-role-governance.schema.json`](../../../contracts/mcp/http-read-role-governance.schema.json)**

This contract defines the canonical set of tool names expected to carry
`http_read_role_eligible: true` in the MCP tool registry.

## Enforcement

- `vox ci command-compliance` validates the governance profile against schema.
- `vox ci command-compliance` enforces parity between:
  - governance profile `read_role_tools`
  - MCP tool registry entries with `http_read_role_eligible: true`

## Related

- [MCP HTTP gateway contract](mcp-http-gateway-contract.md)
- [MCP tool registry (contract SSOT)](mcp-tool-registry-contract.md)
- [`contracts/README.md`](../../../contracts/README.md)
