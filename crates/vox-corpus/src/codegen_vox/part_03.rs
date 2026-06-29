
// ── Multi-construct program templates ────────────────────────────────────────

fn gen_full_stack_program(rng: &mut Rng, variant: usize) -> OrganicPair {
    let noun = NOUNS[rng.usize(NOUNS.len())];
    let tn = {
        let mut s = noun[..1].to_uppercase();
        s.push_str(&noun[1..]);
        s
    };
    let verb = VERBS[rng.usize(VERBS.len())];
    let templates = [
        // Template 0: CRUD API
        format!(
            "table {tn} {{\n    id: int\n    name: str\n    active: bool\n}}\n\n@query\nfn get_{noun}(id: int) to str {{\n    return db.{tn}.find(id).name\n}}\n\n@mutation\nfn create_{noun}(name: str) to Unit {{\n    db.{tn}.insert(name)\n}}\n\n@get(\"/api/{noun}\")\nfn {noun}_handler(req: str) to str {{\n    return get_{noun}(1)\n}}\n\n@test\nfn test_{noun}() to Unit {{\n    create_{noun}(\"test\")\n    assert(get_{noun}(1) == \"test\")\n}}"
        ),
        // Template 1: Agent pipeline
        format!(
            "message {tn}Event {{\n    id: int\n    data: str\n}}\n\nactor {tn}Worker {{\n    state count: int = 0\n    on {verb}() to str {{\n        count = count + 1\n        return \"processed\"\n    }}\n}}\n\nworkflow {noun}_pipeline(input: str) to str {{\n    let worker = spawn({tn}Worker)\n    let result = {verb}(input)\n    return result\n}}"
        ),
        // Template 2: UI app
        format!(
            "type {tn}Status = Loading | Ready(data: str) | Error(msg: str)\n\ncomponent {tn}View() {{\n    state status: str = \"ready\"\n    view: <div className=\"{noun}\">\n        <h1>{{\"{tn}\"}}</h1>\n        <p>{{status}}</p>\n    </div>\n}}\n\nlayout fn {tn}Layout(children: Element) to Element {{\n    return <main>{{children}}</main>\n}}\n\nroutes {{\n    \"/\" to {tn}View\n}}"
        ),
    ];

    OrganicPair {
        prompt: format!("Write a complete Vox program for {tn} with multiple constructs"),
        response: templates[variant % templates.len()].clone(),
        category: "vox_full_program".into(),
        verified: false,
        complexity: 8,
        coverage_tags: vec!["multi_construct".into()],
    }
}

// ── Main generation entry point ──────────────────────────────────────────────

/// Generate the organic corpus by iterating over `TAXONOMY_FROM_AST`.
///
/// For each taxonomy entry, generates `variants_per_construct` variants,
/// ensuring every language construct has training coverage.
pub fn generate_organic_corpus(seed: u64) -> Vec<OrganicPair> {
    let mut rng = Rng::new(seed);
    let mut pairs = Vec::new();
    // 7 = the minimum required by `compute_variety_requirements` for body constructs
    // (function, actor, workflow, component, etc.). Raising from 5 eliminates the
    // "Under-covered (16)" warning across all such constructs.
    let variants_per_construct: usize = 7;

    // Dynamic: iterate over TAXONOMY_FROM_AST (auto-derived from Decl enum)
    for tag in TAXONOMY_FROM_AST {
        for v in 0..variants_per_construct {
            if let Some(pair) = generate_for_taxonomy_entry(tag, &mut rng, v) {
                pairs.push(pair);
            }
        }
    }

    // Multi-construct programs
    for v in 0..5 {
        pairs.push(gen_full_stack_program(&mut rng, v));
    }

    // Parser round-trip verification
    for pair in &mut pairs {
        pair.verified = verify_parse(&pair.response);
    }

    let total = pairs.len();
    let verified = pairs.iter().filter(|p| p.verified).count();
    let taxonomy_count = TAXONOMY_FROM_AST.len();
    eprintln!(
        "  [organic] {total} pairs for {taxonomy_count} taxonomy entries, {verified} verified ({:.0}%)",
        if total > 0 {
            verified as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    );

    pairs
}

/// Generate the Chain-of-Thought (CoT) organic corpus by iterating over `TAXONOMY_FROM_AST`.
/// Prepends structured reasoning blocks before the generated Vox code.
pub fn generate_cot_organic_corpus(seed: u64) -> Vec<OrganicPair> {
    let base_pairs = generate_organic_corpus(seed);
    let mut cot_pairs = Vec::new();

    for pair in base_pairs {
        let mut new_pair = pair.clone();
        let construct_name = new_pair.category.replace("vox_", "");
        
        let think_block = format!(
            "<think>\n1. Plan: The user wants to generate a `{construct_name}` construct in Vox.\n\
             2. Syntax considerations: A valid `{construct_name}` requires correct keywords, valid identifiers, and proper brace formatting based on the Vox AST specification.\n\
             3. Implementation: I will synthesize a realistic, minimal example of a `{construct_name}` satisfying these requirements.\n\
             </think>\n\n"
        );
        
        new_pair.response = format!("{think_block}{}", new_pair.response);
        new_pair.category = "vox_gen_cot".into();
        cot_pairs.push(new_pair);
    }
    
    cot_pairs
}

/// Parse verification (heuristic fallback when parser feature not enabled).
fn verify_parse(source: &str) -> bool {
    #[cfg(feature = "parser-verify")]
    {
        let tokens = vox_compiler::lexer::lex(source);
        vox_compiler::parser::parse(tokens).is_ok()
    }
    #[cfg(not(feature = "parser-verify"))]
    {
        let open = source.chars().filter(|&c| c == '{').count();
        let close = source.chars().filter(|&c| c == '}').count();
        open == close && !source.is_empty()
    }
}

/// Write organic pairs to JSONL.
pub fn write_organic_to_jsonl(
    pairs: &[OrganicPair],
    output: &std::path::Path,
    verified_only: bool,
) -> anyhow::Result<usize> {
    use std::io::Write;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output)?;
    let mut count = 0;
    for pair in pairs {
        if verified_only && !pair.verified {
            continue;
        }
        writeln!(f, "{}", pair.to_jsonl())?;
        count += 1;
    }
    Ok(count)
}

// ── Variety calculation and coverage analysis ────────────────────────────────

/// Coverage report for the generated corpus.
#[derive(Debug)]
pub struct CoverageReport {
    /// Total pairs generated.
    pub total_pairs: usize,
    /// Pairs that passed parser verification.
    pub verified_pairs: usize,
    /// Number of TAXONOMY entries with generators.
    pub taxonomy_covered: usize,
    /// Total TAXONOMY entries.
    pub taxonomy_total: usize,
    /// Per-construct pair counts.
    pub per_construct: Vec<(String, usize, usize)>, // (tag, actual, required)
    /// Constructs below minimum required variety.
    pub under_covered: Vec<String>,
    /// Unique Expr variant tags observed.
    pub expr_variants_seen: usize,
    /// Total Expr variant count from AST (24).
    pub expr_variants_total: usize,
}

/// Minimum pairs required per construct based on language complexity.
///
/// **Formula**: constructs with bodies (function, actor, workflow, etc.) need
/// more examples because a user can ask for them in wildly different ways.
/// Structural constructs (import, const, index) need fewer.
///
/// - **Body constructs** (function, actor, workflow, component, etc.): 7 pairs
/// - **Decorator constructs** (test, fixture, mock, scheduled, etc.): 5 pairs
/// - **Structural constructs** (import, const, index, message, etc.): 3 pairs
pub fn compute_variety_requirements() -> Vec<(&'static str, usize)> {
    TAXONOMY_FROM_AST
        .iter()
        .map(|tag| {
            let min = match *tag {
                // High-body-complexity: many parameters, complex bodies, many use cases
                "function" | "actor" | "workflow" | "component" | "activity" | "server_fn"
                | "agent_def" | "agent" | "skill" | "action" | "trait" | "impl" | "hook"
                | "provider" | "page" => 7,
                // Medium: decorator-style with predictable structure
                "test" | "fixture" | "mock" | "scheduled" | "mcp_tool" | "mcp_resource"
                | "query" | "mutation" | "http_route" | "error_boundary" | "layout" | "loading"
                | "not_found" | "context" | "config" | "type_def" | "v0_component" => 5,
                // Low: structural / declarative with little variation
                "import" | "const" | "message" | "table" | "collection" | "index"
                | "vector_index" | "search_index" | "keyframes" | "theme" | "environment"
                | "routes" => 3,
                // Unknown new construct — conservative default
                _ => 5,
            };
            (*tag, min)
        })
        .collect()
}

/// Analyze generated pairs and produce a coverage report.
pub fn compute_coverage_report(pairs: &[OrganicPair]) -> CoverageReport {
    use std::collections::{HashMap, HashSet};

    let mut per_construct: HashMap<String, usize> = HashMap::new();
    let mut expr_tags: HashSet<String> = HashSet::new();

    for pair in pairs {
        *per_construct.entry(pair.category.clone()).or_default() += 1;
        for tag in &pair.coverage_tags {
            if tag.starts_with("expr:") {
                expr_tags.insert(tag.clone());
            }
        }
    }

    let requirements = compute_variety_requirements();
    let mut construct_report = Vec::new();
    let mut under_covered = Vec::new();
    let mut covered = 0;

    for (tag, min_required) in &requirements {
        let category = format!("vox_{tag}");
        let actual = per_construct.get(&category).copied().unwrap_or(0);
        construct_report.push((tag.to_string(), actual, *min_required));
        if actual > 0 {
            covered += 1;
        }
        if actual < *min_required {
            under_covered.push(format!("{tag} ({actual}/{min_required})"));
        }
    }

    CoverageReport {
        total_pairs: pairs.len(),
        verified_pairs: pairs.iter().filter(|p| p.verified).count(),
        taxonomy_covered: covered,
        taxonomy_total: TAXONOMY_FROM_AST.len(),
        per_construct: construct_report,
        under_covered,
        expr_variants_seen: expr_tags.len(),
        // Dynamic: AST_EXPR_TOTAL auto-derived from vox-ast Expr enum by build.rs
        expr_variants_total: AST_EXPR_TOTAL,
    }
}

/// Print a human-readable coverage report.
pub fn print_coverage_report(report: &CoverageReport) {
    eprintln!("═══════════════════════════════════════════");
    eprintln!("  Vox Organic Corpus — Coverage Report");
    eprintln!("═══════════════════════════════════════════");
    let verified_pct = if report.total_pairs > 0 {
        report.verified_pairs as f64 / report.total_pairs as f64 * 100.0
    } else {
        0.0
    };
    eprintln!(
        "Total pairs : {} ({} verified, {:.0}%)",
        report.total_pairs, report.verified_pairs, verified_pct
    );
    eprintln!(
        "Taxonomy    : {}/{} constructs ({:.0}%)",
        report.taxonomy_covered,
        report.taxonomy_total,
        report.taxonomy_covered as f64 / report.taxonomy_total.max(1) as f64 * 100.0
    );
    eprintln!(
        "Expr        : {}/{} variants ({:.0}%)  [AST_EXPR_TOTAL={}]",
        report.expr_variants_seen,
        report.expr_variants_total,
        report.expr_variants_seen as f64 / report.expr_variants_total.max(1) as f64 * 100.0,
        AST_EXPR_TOTAL
    );
    eprintln!(
        "BinOp       : {}/{} operators  [AST_BINOP_TOTAL={}]",
        BINOP_VARIANTS.len(),
        AST_BINOP_TOTAL,
        AST_BINOP_TOTAL
    );
    eprintln!(
        "TypeExpr    : {}/{} types  [AST_TYPE_EXPR_TOTAL={}]",
        TYPE_EXPR_VARIANTS.len(),
        AST_TYPE_EXPR_TOTAL,
        AST_TYPE_EXPR_TOTAL
    );
    eprintln!(
        "Pattern     : {}/{} patterns  [AST_PATTERN_TOTAL={}]",
        PATTERN_VARIANTS.len(),
        AST_PATTERN_TOTAL,
        AST_PATTERN_TOTAL
    );
    eprintln!(
        "Stmt        : {}/{} statements  [AST_STMT_TOTAL={}]",
        STMT_VARIANTS.len(),
        AST_STMT_TOTAL,
        AST_STMT_TOTAL
    );
    eprintln!("───────────────────────────────────────────");
    if report.under_covered.is_empty() {
        eprintln!("✓ All constructs meet minimum variety requirements");
    } else {
        eprintln!("⚠ Under-covered ({}):", report.under_covered.len());
        for entry in &report.under_covered {
            eprintln!("  - {entry}");
        }
    }
    eprintln!("═══════════════════════════════════════════");
}
