//! Arca SQL: Populi cloud + news + publication (adjacent manifest fragments merged; DDL byte-compatible).
pub const SCHEMA_PUBLISH_CLOUD: &str = r#"
CREATE TABLE IF NOT EXISTS training_throughput_profiles (
    gpu_name     TEXT    NOT NULL,
    seq_len      INTEGER NOT NULL,
    batch_size   INTEGER NOT NULL,
    ms_per_step  REAL    NOT NULL,
    sample_count INTEGER NOT NULL DEFAULT 0,
    last_updated TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    PRIMARY KEY (gpu_name, seq_len, batch_size)
);

CREATE TABLE IF NOT EXISTS cloud_dispatch_log (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id              TEXT    NOT NULL UNIQUE,
    provider            TEXT    NOT NULL,
    offer_id            TEXT    NOT NULL DEFAULT '',
    gpu_name            TEXT    NOT NULL DEFAULT '',
    vram_mb             INTEGER NOT NULL DEFAULT 0,
    price_per_hr_usd    REAL    NOT NULL DEFAULT 0.0,
    estimated_cost      REAL,
    actual_cost         REAL,
    job_kind            TEXT    NOT NULL DEFAULT 'train',
    status              TEXT    NOT NULL DEFAULT 'running',
    -- Phase timing (seconds) — filled in by vox mens status --cloud updates
    setup_secs          REAL,
    download_secs       REAL,
    train_secs          REAL,
    upload_secs         REAL,
    -- Efficiency tracking
    total_steps         INTEGER,
    total_tokens        INTEGER,
    tokens_per_dollar   REAL,
    -- Termination audit
    termination_reason  TEXT,
    created_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    completed_at        TEXT,
    notes               TEXT
);

CREATE INDEX IF NOT EXISTS idx_cloud_dispatch_status
    ON cloud_dispatch_log(status, created_at);
CREATE INDEX IF NOT EXISTS idx_cloud_dispatch_provider
    ON cloud_dispatch_log(provider, job_kind);
CREATE INDEX IF NOT EXISTS idx_throughput_gpu
    ON training_throughput_profiles(gpu_name);

-- Local training log — tracks 4080 Super (and other local GPUs) for cost and
-- efficiency parity with cloud training. All fields mirror cloud_dispatch_log.
CREATE TABLE IF NOT EXISTS local_train_log (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    gpu_name      TEXT    NOT NULL,
    model_id      TEXT    NOT NULL,
    preset        TEXT    NOT NULL DEFAULT 'auto',
    wall_secs     REAL    NOT NULL,
    total_steps   INTEGER NOT NULL DEFAULT 0,
    total_tokens  INTEGER NOT NULL DEFAULT 0,
    ms_per_step   REAL,
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_local_train_gpu
    ON local_train_log(gpu_name, created_at);

-- Table of published news items to prevent duplicate syndication.
CREATE TABLE IF NOT EXISTS published_news (
    news_id TEXT PRIMARY KEY,
    published_at_ms INTEGER NOT NULL,
    github_release_id TEXT,
    twitter_tweet_id TEXT,
    opencollective_update_id TEXT,
    content_sha3_256 TEXT
);

-- Two-person approval: distinct approver identities per news id (filename stem).
CREATE TABLE IF NOT EXISTS news_publish_approvals (
    news_id TEXT NOT NULL,
    approver TEXT NOT NULL,
    approved_at_ms INTEGER NOT NULL,
    PRIMARY KEY (news_id, approver)
);

-- Digest-bound approvals (v2): approvals are tied to immutable content hash.
CREATE TABLE IF NOT EXISTS news_publish_approvals_v2 (
    news_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    approver TEXT NOT NULL,
    approved_at_ms INTEGER NOT NULL,
    PRIMARY KEY (news_id, content_sha3_256, approver)
);

CREATE TABLE IF NOT EXISTS news_publish_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    news_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    attempted_at_ms INTEGER NOT NULL,
    result_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_news_publish_attempts_news ON news_publish_attempts(news_id);

CREATE TABLE IF NOT EXISTS publication_manifests (
    publication_id TEXT PRIMARY KEY,
    content_type TEXT NOT NULL,
    source_ref TEXT,
    title TEXT NOT NULL,
    author TEXT NOT NULL,
    abstract_text TEXT,
    body_markdown TEXT NOT NULL,
    citations_json TEXT,
    metadata_json TEXT,
    revision_history_json TEXT,
    content_sha3_256 TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    state TEXT NOT NULL DEFAULT 'draft',
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_publication_manifests_type_state
    ON publication_manifests(content_type, state);
CREATE INDEX IF NOT EXISTS idx_publication_manifests_digest
    ON publication_manifests(content_sha3_256);

CREATE TABLE IF NOT EXISTS publication_approvals (
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    approver TEXT NOT NULL,
    approved_at_ms INTEGER NOT NULL,
    -- Phase D solo-critic gate columns. `approver_role` is one of
    --   'human' | 'audited_llm_critic'
    -- enforced at the Rust layer (Turso does not support CHECK).
    -- `critic_fingerprint_json` and `critic_report_uri` are populated only
    -- when approver_role = 'audited_llm_critic'.
    approver_role TEXT NOT NULL DEFAULT 'human',
    critic_fingerprint_json TEXT,
    critic_report_uri TEXT,
    PRIMARY KEY (publication_id, content_sha3_256, approver)
);

CREATE TABLE IF NOT EXISTS publication_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    channel TEXT NOT NULL,
    attempted_at_ms INTEGER NOT NULL,
    outcome_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_publication_attempts_pub_attempted
    ON publication_attempts(publication_id, attempted_at_ms);

-- scholarly_submissions.status: remote/venue-specific strings (normalized by adapter). Future optional CHECK
-- should validate non-empty; avoid constraining to a closed set without adapter-aware migration.
CREATE TABLE IF NOT EXISTS scholarly_submissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    adapter TEXT NOT NULL,
    external_submission_id TEXT NOT NULL,
    status TEXT NOT NULL,
    submitted_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    response_fingerprint TEXT,
    metadata_json TEXT,
    UNIQUE(adapter, external_submission_id)
);

CREATE INDEX IF NOT EXISTS idx_scholarly_submissions_pub_status
    ON scholarly_submissions(publication_id, status);

CREATE TABLE IF NOT EXISTS publication_media_assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    asset_ref TEXT NOT NULL,
    media_type TEXT NOT NULL,
    storage_uri TEXT,
    status TEXT NOT NULL,
    metadata_json TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE(publication_id, asset_ref)
);

CREATE INDEX IF NOT EXISTS idx_publication_media_assets_pub_status
    ON publication_media_assets(publication_id, status);

CREATE TABLE IF NOT EXISTS publication_status_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    status TEXT NOT NULL,
    detail_json TEXT,
    recorded_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_publication_status_events_pub_id
    ON publication_status_events(publication_id, id);

-- external_submission_jobs.status (operational queue; migration-safe vocabulary):
--   queued | running | retryable_failed | failed | succeeded
-- Future: add CHECK(status IN (...)) once all readers/writers enforce the same set.
CREATE TABLE IF NOT EXISTS external_submission_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    adapter TEXT NOT NULL,
    operation TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    lock_owner TEXT,
    lock_expires_at_ms INTEGER,
    next_retry_at_ms INTEGER,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error_class TEXT,
    last_error_message TEXT,
    metadata_json TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_external_jobs_pub_digest_adapter
    ON external_submission_jobs(publication_id, content_sha3_256, adapter);
CREATE INDEX IF NOT EXISTS idx_external_jobs_status_retry
    ON external_submission_jobs(status, next_retry_at_ms);

-- error_class: adapter values from ScholarlyError (disabled, config, auth, rate_limit, transient, fatal)
-- plus job-layer preflight; http_status filled when the underlying failure maps to an HTTP code
CREATE TABLE IF NOT EXISTS external_submission_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    attempted_at_ms INTEGER NOT NULL,
    http_status INTEGER,
    error_class TEXT,
    retryable INTEGER NOT NULL DEFAULT 0,
    request_fingerprint TEXT,
    response_fingerprint TEXT,
    detail_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_external_attempts_job
    ON external_submission_attempts(job_id, attempted_at_ms);

CREATE TABLE IF NOT EXISTS external_status_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    adapter TEXT NOT NULL,
    external_submission_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    fetched_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_external_snapshots_adapter_ext
    ON external_status_snapshots(adapter, external_submission_id, fetched_at_ms);

CREATE TABLE IF NOT EXISTS publication_external_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    adapter TEXT NOT NULL,
    link_kind TEXT NOT NULL,
    link_value TEXT NOT NULL,
    metadata_json TEXT,
    created_at_ms INTEGER NOT NULL,
    UNIQUE(publication_id, content_sha3_256, adapter, link_kind)
);

CREATE INDEX IF NOT EXISTS idx_publication_external_links_pub
    ON publication_external_links(publication_id, content_sha3_256);

-- Maps an immutable local content digest to the adapter's current revision/version identifier
-- (e.g. Zenodo deposition version, OpenReview revision tag) for idempotent updates.
CREATE TABLE IF NOT EXISTS publication_external_revisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id TEXT NOT NULL,
    content_sha3_256 TEXT NOT NULL,
    adapter TEXT NOT NULL,
    external_revision TEXT NOT NULL,
    metadata_json TEXT,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE(publication_id, content_sha3_256, adapter)
);

CREATE INDEX IF NOT EXISTS idx_publication_external_revisions_pub_digest
    ON publication_external_revisions(publication_id, content_sha3_256);

CREATE TABLE IF NOT EXISTS scientia_external_intelligence (
    id TEXT PRIMARY KEY,
    source_url TEXT NOT NULL,
    source_kind TEXT NOT NULL,  
    title TEXT NOT NULL,
    abstract_text TEXT,
    embedding_id TEXT,
    provenance_json TEXT DEFAULT '[]',
    ingest_status TEXT NOT NULL DEFAULT 'pending',
    preflight_score REAL,
    ingested_at_ms INTEGER NOT NULL,
    reviewed_at_ms INTEGER,
    -- Wave 1: Socrates + Worthiness enrichment
    socrates_risk_band TEXT,
    socrates_confidence REAL,
    worthiness_score REAL,
    claim_evidence_coverage REAL
);

CREATE TABLE IF NOT EXISTS scientia_feed_sources (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    crawl_interval_ms INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_crawled_at_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS syndication_events (
    id               TEXT    PRIMARY KEY,
    publication_id   TEXT    NOT NULL,
    channel          TEXT    NOT NULL,
    outcome          TEXT    NOT NULL,
    external_id      TEXT,
    attempt_number   INTEGER NOT NULL DEFAULT 1,
    retryable        INTEGER NOT NULL DEFAULT 0,
    attempted_at     TEXT    NOT NULL,
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_syndication_events_pub
    ON syndication_events (publication_id);
CREATE INDEX IF NOT EXISTS idx_syndication_events_channel
    ON syndication_events (channel, attempted_at DESC);

CREATE TABLE IF NOT EXISTS scholarly_publication_records (
    id                    TEXT PRIMARY KEY,
    publication_id        TEXT NOT NULL UNIQUE,
    doi                   TEXT,
    zenodo_deposit_id     TEXT,
    zenodo_doi            TEXT,
    orcid_put_code        INTEGER,        -- returned integer from ORCID POST
    figshare_article_id   TEXT,
    arxiv_submission_id   TEXT,
    openreview_forum_id   TEXT,
    crossref_deposit_id   TEXT,
    researchgate_confirmed INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    -- status: 'draft' | 'deposited' | 'published' | 'retracted'
    published_at          TEXT,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_scholarly_pub_doi
    ON scholarly_publication_records (doi) WHERE doi IS NOT NULL;
"#;
