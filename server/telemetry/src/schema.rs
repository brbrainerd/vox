//! ClickHouse DDL generation from `collection-taxonomy.v1.json`.
//!
//! # Invariants
//! - Every field whose `type` is `enum` or `hash` → `LowCardinality(String)`
//! - `int` fields → `Nullable(Int64)` (optional numeric context)
//! - `bool` fields → `UInt8`
//! - Every table has: `install_id String`, `event_name LowCardinality(String)`,
//!   `ts DateTime64(3, 'UTC')`, and a 180-day TTL on `ts`.
//! - Materialized views roll up daily counts per `event_name`.

use serde::Deserialize;
use std::fmt::Write as _;

// ── Taxonomy types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Taxonomy {
    pub version: u32,
    pub k_anonymity: u32,
    pub categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct Category {
    pub name: String,
    pub otlp_event_name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub allowed: Vec<String>,
}

// ── DDL generation ────────────────────────────────────────────────────────────

/// Maps a taxonomy field type to a ClickHouse column type (already nullable-wrapped
/// where needed — `LowCardinality` cannot be nested inside `Nullable`).
fn ch_type(field_type: &str) -> &'static str {
    match field_type {
        // LowCardinality(Nullable(String)) is the correct nullable enum form.
        "enum" | "hash" => "LowCardinality(Nullable(String))",
        "int" => "Nullable(Int64)",
        "bool" => "Nullable(UInt8)",
        _ => "Nullable(String)",
    }
}

/// Generate the complete DDL for the `events_raw` table plus per-category
/// materialized views.  Returns a single SQL string with statements separated
/// by `;\n\n`.
pub fn gen_ddl(taxonomy: &Taxonomy) -> String {
    let mut out = String::new();

    // ── events_raw ────────────────────────────────────────────────────────────
    out.push_str("-- Generated from collection-taxonomy.v1.json — do not edit manually.\n\n");
    out.push_str("CREATE TABLE IF NOT EXISTS events_raw\n(\n");
    out.push_str("    install_id     String,\n");
    out.push_str("    event_name     LowCardinality(String),\n");
    out.push_str("    ts             DateTime64(3, 'UTC'),\n");

    // Collect all unique field names across every category.
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for cat in &taxonomy.categories {
        for f in &cat.fields {
            seen.insert(f.name.clone());
        }
    }

    // Build a name → type map (first definition wins if there is a conflict).
    let mut field_map: std::collections::BTreeMap<String, &str> = std::collections::BTreeMap::new();
    for cat in &taxonomy.categories {
        for f in &cat.fields {
            field_map
                .entry(f.name.clone())
                .or_insert_with(|| ch_type(&f.field_type));
        }
    }

    for (name, ch_t) in &field_map {
        let _ = writeln!(out, "    {name:<40} {ch_t},");
    }
    // Remove trailing comma from last field line — insert a sentinel non-nullable column.
    out.push_str("    _schema_version UInt8 DEFAULT 1\n");
    out.push_str(")\n");
    out.push_str("ENGINE = MergeTree()\n");
    out.push_str("PARTITION BY toYYYYMM(ts)\n");
    out.push_str("ORDER BY (event_name, install_id, ts)\n");
    out.push_str("TTL toDateTime(ts) + INTERVAL 180 DAY;\n");

    // ── per-category materialized views ───────────────────────────────────────
    for cat in &taxonomy.categories {
        let view = format!("mv_{}", cat.name);
        let _ = write!(
            out,
            "\n\nCREATE MATERIALIZED VIEW IF NOT EXISTS {view}\nENGINE = SummingMergeTree()\n"
        );
        out.push_str("ORDER BY (event_name, day)\n");
        out.push_str("POPULATE\nAS\n");
        out.push_str("SELECT\n");
        out.push_str("    event_name,\n");
        out.push_str("    toDate(ts)   AS day,\n");
        for f in &cat.fields {
            let _ = writeln!(out, "    {name},", name = f.name);
        }
        out.push_str("    count()      AS cnt\n");
        out.push_str("FROM events_raw\n");
        out.push_str(&format!("WHERE event_name = '{}'\n", cat.otlp_event_name));
        out.push_str("GROUP BY ALL;\n");
    }

    out
}

// ── boot migration ────────────────────────────────────────────────────────────

/// Split generated DDL into individual executable statements, dropping
/// comment-only lines and any trailing `;`. `gen_ddl` joins statements with
/// `;\n\n`, so the ClickHouse client (which wants exactly one statement per
/// `query()`, no terminator) gets a clean list.
pub fn split_statements(ddl: &str) -> Vec<String> {
    ddl.split(";\n\n")
        .map(|raw| {
            raw.lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .map(|s| s.trim().trim_end_matches(';').trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Apply the generated schema idempotently on startup. Every statement is
/// `CREATE ... IF NOT EXISTS`, so this is safe to run on every boot — a fresh
/// Coolify volume is migrated without a separate one-shot job.
pub async fn ensure_schema(ch: &clickhouse::Client) -> anyhow::Result<()> {
    let taxonomy = load_taxonomy()?;
    for stmt in split_statements(&gen_ddl(&taxonomy)) {
        ch.query(&stmt).execute().await?;
    }
    Ok(())
}

// ── read the vendored contract ────────────────────────────────────────────────

// Path is relative to this source file. After flattening the formerly
// double-nested crate into `server/telemetry/`, `contracts/` sits one level up
// from `src/` (was two levels in the old `vox-server/vox-server/` layout).
const TAXONOMY_JSON: &str = include_str!("../contracts/collection-taxonomy.v1.json");

pub fn load_taxonomy() -> Result<Taxonomy, serde_json::Error> {
    serde_json::from_str(TAXONOMY_JSON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddl_contains_events_raw_table() {
        let t = load_taxonomy().expect("taxonomy must parse");
        let ddl = gen_ddl(&t);
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS events_raw"));
    }

    #[test]
    fn ddl_has_ttl_clause() {
        let t = load_taxonomy().expect("taxonomy must parse");
        let ddl = gen_ddl(&t);
        assert!(ddl.contains("TTL toDateTime(ts) + INTERVAL 180 DAY"));
    }

    #[test]
    fn ddl_has_install_id_and_ts_columns() {
        let t = load_taxonomy().expect("taxonomy must parse");
        let ddl = gen_ddl(&t);
        assert!(
            ddl.contains("install_id"),
            "events_raw must have install_id column"
        );
        assert!(
            ddl.contains("DateTime64(3, 'UTC')"),
            "events_raw must have DateTime64 ts column"
        );
    }

    #[test]
    fn ddl_has_materialized_view_per_category() {
        let t = load_taxonomy().expect("taxonomy must parse");
        let ddl = gen_ddl(&t);
        for cat in &t.categories {
            let view = format!("mv_{}", cat.name);
            assert!(
                ddl.contains(&view),
                "DDL must contain materialized view {view}"
            );
        }
    }

    #[test]
    fn ddl_enum_fields_use_low_cardinality() {
        let t = load_taxonomy().expect("taxonomy must parse");
        let ddl = gen_ddl(&t);
        assert!(
            ddl.contains("LowCardinality(Nullable(String))"),
            "enum/hash fields must use LowCardinality(Nullable(String))"
        );
    }
}
