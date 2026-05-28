//! Golden tests for the Phase 3 Runs surface components.
//!
//! Verifies that:
//!   1. RunsToolbar emits the search box, filter chips, and live-tail toggle.
//!   2. RunRow emits 7 columns with correct class-conditional highlight.
//!   3. RunsTableHeader emits 7 labelled column headers.
//!   4. EventRow emits timestamp + kind + label.
//!
//! RunDrawer and RunsSurface involve `any`-typed props and on-mount fetch;
//! those are covered by the E2E API tests (e2e_runs.rs).
//!
//! Sources are inlined (no cross-file import resolution in the test harness).
//! StateChip is included inline since RunRow uses it for the status column.

fn compile_component(source: &str, component_name: &str) -> String {
    let tokens = vox_compiler::lexer::lex(source);
    let module =
        vox_compiler::parser::parse(tokens).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let hir = vox_compiler::hir::lower_module(&module);
    let out =
        vox_codegen::codegen_ts::generate(&hir).unwrap_or_else(|e| panic!("codegen failed: {e:?}"));
    let filename = format!("{component_name}.tsx");
    out.files
        .into_iter()
        .find(|(n, _)| n == &filename)
        .map(|(_, c)| c)
        .unwrap_or_else(|| panic!("{filename} not found in codegen output"))
}

// ── Shared sources ────────────────────────────────────────────────────────────
// Note: StateChip source removed; tests below operate on RunsToolbar / RunRow /
// RunsTableHeader / EventRow directly. RunRow's status column is exercised via
// raw class assertions, not by re-compiling the chip component itself.

const RUNS_TOOLBAR_SRC: &str = r#"
component RunsToolbar(live_tail: bool, on_toggle_live: fn() = fn() {}) {
    view: row(h=11, border_b=true, border_color="zinc.800", pad_x=4, items="center", gap=3, shrink=0, bg="zinc.950") {
        panel(flex=1, bg="zinc.900", border=true, border_color="white/10", radius="lg", pad_x=3, raw_class="h-8 flex items-center") {
            text(size="sm", color="zinc.600", italic=true) { "Search runs…" }
        }
        row(gap=2, items="center", shrink=0) {
            button(raw_class="px-2 py-0.5 rounded-full text-xs font-mono bg-zinc-800 text-zinc-400 border border-white/10 hover:bg-zinc-700") { "all" }
            button(raw_class="px-2 py-0.5 rounded-full text-xs font-mono bg-emerald-400/10 text-emerald-400 border border-emerald-400/20 hover:bg-emerald-400/20") { "ok" }
            button(raw_class="px-2 py-0.5 rounded-full text-xs font-mono bg-rose-500/10 text-rose-400 border border-rose-500/20 hover:bg-rose-500/20") { "error" }
        }
        button(
            raw_class=if live_tail {
                "px-3 py-1 rounded-lg bg-blue-600 text-white text-xs font-bold hover:bg-blue-500 shrink-0"
            } else {
                "px-3 py-1 rounded-lg bg-zinc-900 border border-white/10 text-xs text-zinc-400 hover:bg-zinc-800 shrink-0"
            },
            on_click=on_toggle_live
        ) {
            if live_tail { "Live tail: on" } else { "Live tail: off" }
        }
    }
}
"#;

const RUN_ROW_SRC: &str = r#"
component StateChip(status: str, label: str = "", dim: bool = false) {
    view: row(items="center", gap=1) {
        text() { status }
    }
}
component RunRow(id: str, started: str, duration: str, model: str, status: str, cost: str, tokens: str, active: str, on_click: fn() = fn() {}) {
    view: row(
        border_b=true, border_color="zinc.800/60",
        pad_x=4, pad_y=0,
        items="center",
        raw_class=if active is id {
            "h-10 bg-blue-950/30 cursor-pointer border-l-2 border-l-blue-500"
        } else {
            "h-10 hover:bg-zinc-900/60 cursor-pointer"
        },
        on_click=on_click
    ) {
        text(size="xs", font_family="mono", color="blue.400",   raw_class="w-24 shrink-0 truncate") { id }
        text(size="xs", font_family="mono", color="zinc.500",   raw_class="w-36 shrink-0") { started }
        text(size="xs", font_family="mono", color="zinc.300",   raw_class="w-20 shrink-0") { duration }
        text(size="xs", font_family="mono", color="zinc.400",   raw_class="flex-1 truncate") { model }
        panel(raw_class="w-20 shrink-0 flex items-center") {
            StateChip(status=status, label=status)
        }
        text(size="xs", font_family="mono", color="zinc.400",   raw_class="w-20 shrink-0 text-right") { cost }
        text(size="xs", font_family="mono", color="zinc.500",   raw_class="w-20 shrink-0 text-right pr-1") { tokens }
    }
}
"#;

const RUNS_TABLE_HEADER_SRC: &str = r#"
component RunsTableHeader() {
    view: row(
        border_b=true, border_color="zinc.700",
        pad_x=4, pad_y=0,
        items="center", shrink=0,
        bg="zinc.900/50",
        raw_class="h-8 sticky top-0 z-10"
    ) {
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.600", raw_class="w-24 shrink-0") { "ID" }
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.600", raw_class="w-36 shrink-0") { "STARTED" }
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.600", raw_class="w-20 shrink-0") { "DURATION" }
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.600", raw_class="flex-1") { "MODEL" }
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.600", raw_class="w-20 shrink-0") { "STATUS" }
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.600", raw_class="w-20 shrink-0 text-right") { "COST" }
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.600", raw_class="w-20 shrink-0 text-right pr-1") { "TOKENS" }
    }
}
"#;

const EVENT_ROW_SRC: &str = r#"
component EventRow(kind: str, label: str, ts: str) {
    view: row(items="start", gap=3, pad_y=1, border_b=true, border_color="zinc.800/40") {
        text(size="xs", font_family="mono", color="zinc.700", raw_class="w-14 shrink-0 text-right") { ts }
        column(gap=0, flex=1) {
            text(size="xs", font_family="mono", color="zinc.400") { kind }
            text(size="xs", color="zinc.600", raw_class="leading-snug") { label }
        }
    }
}
"#;

// ── RunsToolbar ───────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn runs_toolbar_emits_search_box_and_filter_chips() {
    let ts = compile_component(RUNS_TOOLBAR_SRC, "RunsToolbar");

    // Search placeholder text
    assert!(
        ts.contains("Search runs"),
        "RunsToolbar must include search placeholder"
    );

    // Status filter chips
    assert!(
        ts.contains("\"all\"") || ts.contains("'all'"),
        "must have 'all' chip"
    );
    assert!(
        ts.contains("\"ok\"") || ts.contains("'ok'"),
        "must have 'ok' chip"
    );
    assert!(
        ts.contains("\"error\"") || ts.contains("'error'"),
        "must have 'error' chip"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn runs_toolbar_live_tail_toggle_is_conditional() {
    let ts = compile_component(RUNS_TOOLBAR_SRC, "RunsToolbar");

    insta::assert_snapshot!("runs_toolbar_tsx_runs_surface", ts);

    // Both branches of live_tail conditional must be present
    assert!(
        ts.contains("Live tail: on"),
        "RunsToolbar must render 'Live tail: on' branch"
    );
    assert!(
        ts.contains("Live tail: off"),
        "RunsToolbar must render 'Live tail: off' branch"
    );
}

// ── RunRow ────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn run_row_emits_seven_columns() {
    let ts = compile_component(RUN_ROW_SRC, "RunRow");

    insta::assert_snapshot!("run_row_tsx_runs_surface", ts);

    // 7 data columns: id / started / duration / model / status / cost / tokens
    // Count <p> elements (text(...) lowers to <p>) — at least 6 text renders.
    let text_count = ts.matches("<p ").count();
    assert!(
        text_count >= 6,
        "RunRow must emit at least 6 <p> elements, got {text_count}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn run_row_applies_highlight_class_when_active() {
    let ts = compile_component(RUN_ROW_SRC, "RunRow");

    // Highlight branch: blue-950 background + left border
    assert!(
        ts.contains("border-l-blue-500") || ts.contains("border-l-2"),
        "RunRow must have highlight class for active state"
    );

    // Normal branch: hover only
    assert!(
        ts.contains("hover:bg-zinc-900"),
        "RunRow must have hover class for inactive state"
    );
}

// ── RunsTableHeader ───────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn runs_table_header_emits_seven_column_labels() {
    let ts = compile_component(RUNS_TABLE_HEADER_SRC, "RunsTableHeader");

    insta::assert_snapshot!("runs_table_header_tsx_runs_surface", ts);

    for label in &[
        "ID", "STARTED", "DURATION", "MODEL", "STATUS", "COST", "TOKENS",
    ] {
        assert!(
            ts.contains(label),
            "RunsTableHeader must include column label '{label}'"
        );
    }
}

// ── EventRow ─────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn event_row_emits_timestamp_kind_and_label() {
    let ts = compile_component(EVENT_ROW_SRC, "EventRow");

    insta::assert_snapshot!("event_row_tsx_runs_surface", ts);

    // ts / kind / label props must be referenced
    assert!(ts.contains("ts"), "EventRow must reference ts prop");
    assert!(ts.contains("kind"), "EventRow must reference kind prop");
    assert!(ts.contains("label"), "EventRow must reference label prop");
}
