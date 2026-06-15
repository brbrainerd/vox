//! Static path lists and feature matrix lanes for CI guards.

/// Canonical GitHub `owner/name` slug for this project's repository. Single
/// source of truth for the CI commands that hit the GitHub API.
pub(crate) const REPO_SLUG: &str = "vox-foundation/vox";

pub(crate) const DOCS_SSOT_FILES: &[&str] = &[
    "docs/src/how-to/how-to-train-mens-4080.md",
    "docs/src/how-to/how-to-voxdb-canonical-store.md",
    "docs/src/ci/runner-contract.md",
    "docs/src/ci/command-surface-duals.md",
    "docs/src/ci/documentation-pointers.md",
    "docs/src/ci/crate-hardening-matrix.md",
    "docs/src/ci/github-hosted-exceptions.md",
    "docs/src/ci/workflow-enumeration.md",
    "docs/src/ci/binary-release-contract.md",
    "docs/src/ci/cli-baseline-metrics.md",
];

pub(crate) const CODEX_SSOT_FILES: &[&str] = &[
    "contracts/index.yaml",
    "contracts/index.schema.json",
    "contracts/db/baseline-version-policy.yaml",
    "contracts/reports/evidence-snapshot-rev-c.json",
    "contracts/codex-api.openapi.yaml",
    "docs/src/adr/004-codex-arca-turso-ssot.md",
    "infra/coolify/docker-compose.yml",
];

pub(crate) const OPENAPI_SUBSTRINGS: &[&str] = &[
    "openapi:",
    "/api/codex/research-session",
    "/api/codex/conversations/{conv_id}/versions",
    "/api/codex/conversation-edges",
    "/api/codex/topics/{topic_id}/evolution-events",
];

pub(crate) const MANIFEST_SNIPPETS: &[&str] = &[
    "BASELINE_VERSION",
    "SCHEMA_FRAGMENTS",
    "schema_baseline_digest_hex",
    "super::spec",
];

pub(crate) const FEATURE_SETS: &[&str] = &[
    "",
    "codex",
    "stub-check",
    "codex,stub-check",
    "live",
    "dei",
    "ars",
    "extras-ludus",
    "ludus-hud",
    "script-execution",
    "script-execution,stub-check",
    "mcp-server",
];
