//! Slot pools and template fill.

// ─── Slot Pools ──────────────────────────────────────────

pub(crate) static CRATE_POOL: &[&str] = &[
    "vox-compiler",
    "vox-cli",
    "vox-db",
    "vox-orchestrator",
    "vox-gamify",
    "vox-arca",
    "vox-codegen-rust",
    "vox-ssg",
    "vox-lsp",
    "vox-mcp",
    "vox-fabrica",
    "vox-oratio",
];

static TOESTUB_RULE_POOL: &[&str] = &[
    "UnannotatedAxumHandler",
    "MissingDocComment",
    "CircularReExport",
    "RecreatedDeletedModule",
    "FlatSiblingModule",
    "NullStateUsage",
    "UnregisteredCapability",
];

static LANGUAGE_POOL: &[&str] = &["Rust", "Vox", "SQL", "TOML", "Markdown"];

static DOC_CATEGORY_POOL: &[&str] = &[
    "struct fields",
    "pub fns",
    "trait methods",
    "enum variants",
    "module-level items",
];

static BUILD_CRATE_POOL: &[&str] = &[
    "vox-compiler",
    "vox-codegen-rust",
    "vox-ssg",
    "vox-cli",
];

static TEST_MODULE_POOL: &[&str] = &[
    "the tokenizer",
    "the parser's error recovery",
    "HIR lowering",
    "the typeck inference engine",
    "codegen output",
    "the reward policy",
    "the streak tracker",
];

static RESEARCH_TOPIC_POOL: &[&str] = &[
    "actor runtime patterns",
    "Server-rendered HTML vs client React (Vox web SSOT)",
    "QLoRA fine-tuning strategies",
    "Turso vs SQLite benchmarks",
    "MCP server design",
    "Vox vs competitors",
    "Build reproducibility techniques",
];

/// Fill a template string with seeded-random slot values.
///
/// Supported slots: `{CRATE}`, `{RULE}`, `{LANGUAGE}`, `{DOC_CATEGORY}`,
/// `{BUILD_CRATE}`, `{TEST_MODULE}`, `{RESEARCH_TOPIC}`.
pub fn slot_fill(template: &str, seed: u64) -> String {
    fn pick<'a>(pool: &'a [&'a str], seed: u64) -> &'a str {
        pool[(seed as usize) % pool.len()]
    }
    template
        .replace("{CRATE}", pick(CRATE_POOL, seed))
        .replace("{RULE}", pick(TOESTUB_RULE_POOL, seed.wrapping_add(1)))
        .replace("{LANGUAGE}", pick(LANGUAGE_POOL, seed.wrapping_add(2)))
        .replace(
            "{DOC_CATEGORY}",
            pick(DOC_CATEGORY_POOL, seed.wrapping_add(3)),
        )
        .replace(
            "{BUILD_CRATE}",
            pick(BUILD_CRATE_POOL, seed.wrapping_add(4)),
        )
        .replace(
            "{TEST_MODULE}",
            pick(TEST_MODULE_POOL, seed.wrapping_add(5)),
        )
        .replace(
            "{RESEARCH_TOPIC}",
            pick(RESEARCH_TOPIC_POOL, seed.wrapping_add(6)),
        )
}
