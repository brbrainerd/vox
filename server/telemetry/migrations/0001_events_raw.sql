-- Migration 0001: events_raw + materialized views
-- Generated template — run `cargo run --bin vox-server -- --gen-ddl` to regenerate
-- from the vendored taxonomy contract.
--
-- Apply: docker exec vox_clickhouse clickhouse-client --user X --password Y --multiquery < migrations/0001_events_raw.sql

CREATE DATABASE IF NOT EXISTS vox_telemetry;

CREATE TABLE IF NOT EXISTS vox_telemetry.events_raw
(
    install_id                               String,
    event_name                               LowCardinality(String),
    ts                                       DateTime64(3, 'UTC'),
    -- command_usage
    verb                                     LowCardinality(Nullable(String)),
    exit_class                               LowCardinality(Nullable(String)),
    duration_bucket                          LowCardinality(Nullable(String)),
    -- skill_activation
    skill_id_hash                            Nullable(String),
    trigger_source                           LowCardinality(Nullable(String)),
    accepted                                 Nullable(UInt8),
    surface                                  LowCardinality(Nullable(String)),
    -- edit_pattern
    op_type                                  LowCardinality(Nullable(String)),
    file_kind                                LowCardinality(Nullable(String)),
    size_bucket                              LowCardinality(Nullable(String)),
    -- harness_usage
    tool_call_kind                           LowCardinality(Nullable(String)),
    mode                                     LowCardinality(Nullable(String)),
    -- error_surface
    error_class                              LowCardinality(Nullable(String)),
    subsystem                                LowCardinality(Nullable(String)),
    recoverable                              Nullable(UInt8),
    -- default_decision
    decision_id                              LowCardinality(Nullable(String)),
    chosen                                   LowCardinality(Nullable(String)),
    outcome                                  LowCardinality(Nullable(String)),
    magnitude_bucket                         Nullable(Int64),
    -- internal
    _schema_version                          UInt8 DEFAULT 1
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(ts)
ORDER BY (event_name, install_id, ts)
TTL toDateTime(ts) + INTERVAL 180 DAY;

-- ── Materialized views (daily rollups per category) ───────────────────────────

CREATE MATERIALIZED VIEW IF NOT EXISTS vox_telemetry.mv_command_usage
ENGINE = SummingMergeTree()
ORDER BY (event_name, day)
POPULATE AS
SELECT
    event_name,
    toDate(ts)        AS day,
    verb,
    exit_class,
    duration_bucket,
    count()           AS cnt
FROM vox_telemetry.events_raw
WHERE event_name = 'vox.command'
GROUP BY ALL;

CREATE MATERIALIZED VIEW IF NOT EXISTS vox_telemetry.mv_skill_activation
ENGINE = SummingMergeTree()
ORDER BY (event_name, day)
POPULATE AS
SELECT
    event_name,
    toDate(ts)        AS day,
    trigger_source,
    surface,
    accepted,
    count()           AS cnt
FROM vox_telemetry.events_raw
WHERE event_name = 'vox.skill'
GROUP BY ALL;

CREATE MATERIALIZED VIEW IF NOT EXISTS vox_telemetry.mv_edit_pattern
ENGINE = SummingMergeTree()
ORDER BY (event_name, day)
POPULATE AS
SELECT
    event_name,
    toDate(ts)        AS day,
    op_type,
    file_kind,
    size_bucket,
    count()           AS cnt
FROM vox_telemetry.events_raw
WHERE event_name = 'vox.edit'
GROUP BY ALL;

CREATE MATERIALIZED VIEW IF NOT EXISTS vox_telemetry.mv_harness_usage
ENGINE = SummingMergeTree()
ORDER BY (event_name, day)
POPULATE AS
SELECT
    event_name,
    toDate(ts)        AS day,
    tool_call_kind,
    mode,
    count()           AS cnt
FROM vox_telemetry.events_raw
WHERE event_name = 'vox.harness'
GROUP BY ALL;

CREATE MATERIALIZED VIEW IF NOT EXISTS vox_telemetry.mv_error_surface
ENGINE = SummingMergeTree()
ORDER BY (event_name, day)
POPULATE AS
SELECT
    event_name,
    toDate(ts)        AS day,
    error_class,
    subsystem,
    recoverable,
    count()           AS cnt
FROM vox_telemetry.events_raw
WHERE event_name = 'vox.error'
GROUP BY ALL;

CREATE MATERIALIZED VIEW IF NOT EXISTS vox_telemetry.mv_default_decision
ENGINE = SummingMergeTree()
ORDER BY (event_name, day)
POPULATE AS
SELECT
    event_name,
    toDate(ts)        AS day,
    decision_id,
    chosen,
    outcome,
    count()           AS cnt
FROM vox_telemetry.events_raw
WHERE event_name = 'vox.default_decision'
GROUP BY ALL;
