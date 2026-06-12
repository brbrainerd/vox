//! The RN target must emit lifecycle hooks (`on mount`, effects) — previously
//! it dropped them, so a component that loaded data in `on mount:` rendered its
//! initial state forever. Async `@query`/`@mutation` calls in those bodies must
//! be awaited inside a fire-and-forget async IIFE (a `useEffect` callback can't
//! be async).

use vox_codegen::codegen_ts::CodegenOptions;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};
use vox_rn_codegen::generate_rn;

fn rn_component(src: &str, component: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let out = generate_rn(&hir, &CodegenOptions::default()).expect("rn emit");
    let target = format!("{component}.tsx");
    out.files
        .into_iter()
        .find(|(name, _)| name == &target)
        .unwrap_or_else(|| panic!("no {target} emitted; files: see emit"))
        .1
}

const SRC: &str = r#"
@query fn load_count() to int { return 0 }

component Dashboard() {
    state total: int = 0
    on mount: {
        total = load_count()
    }
    view: column {
        text { "Total: " + str(total) }
    }
}
"#;

#[test]
fn rn_emits_on_mount_useeffect() {
    let tsx = rn_component(SRC, "Dashboard");
    assert!(
        tsx.contains("useEffect(() => {"),
        "RN must emit a useEffect for `on mount`; got:\n{tsx}"
    );
    assert!(
        tsx.lines()
            .any(|l| l.contains("from \"react\"") && l.contains("useEffect")),
        "RN must import useEffect from react; got:\n{tsx}"
    );
}

#[test]
fn rn_on_mount_awaits_async_endpoint_in_iife() {
    let tsx = rn_component(SRC, "Dashboard");
    assert!(
        tsx.contains("set_total(await load_count())"),
        "async endpoint call in on-mount must be awaited; got:\n{tsx}"
    );
    assert!(
        tsx.contains("async () =>"),
        "awaited body must run in an async IIFE; got:\n{tsx}"
    );
}

const FIRE_FORGET_SRC: &str = r#"
@mutation fn record_event(kind: str) to Result[str] { return Ok("x") }

component Logger() {
    state n: int = 0
    view: column(raw_class="root") {
        button(on_click={fn() {
            let _ = record_event("mood")
            n = n + 1
        }}) {
            "Log"
        }
    }
}
"#;

#[test]
fn rn_fire_and_forget_endpoint_call_is_catch_guarded() {
    // A discarded, un-awaited endpoint call in an RN handler must not become an
    // unhandled promise rejection (the live "tap → white screen / log error"
    // bug). It is wrapped in `.catch(...)`.
    let tsx = rn_component(FIRE_FORGET_SRC, "Logger");
    assert!(
        tsx.contains(".catch("),
        "fire-and-forget endpoint call must be catch-guarded; got:\n{tsx}"
    );
    assert!(
        !tsx.contains("const _ = record_event"),
        "discarded endpoint call must not emit a dead `const _ =` promise; got:\n{tsx}"
    );
}

const NAV_SRC: &str = r#"
component Menu() {
    view: row(raw_class="nav") {
        link(href="/timeline") { "Timeline" }
    }
}
"#;

const ROW_SRC: &str = r#"
component Bar() {
    view: column(raw_class="root") {
        row(raw_class="actions") {
            button(on_click={fn() {}}) { "A" }
            button(on_click={fn() {}}) { "B" }
        }
        row(raw_class="nav", scroll="horizontal") {
            link(href="/a") { "A" }
            link(href="/b") { "B" }
        }
    }
}
"#;

#[test]
fn rn_row_wraps_by_default_and_scroll_opts_into_scrollview() {
    let tsx = rn_component(ROW_SRC, "Bar");
    // Bare `row` cannot overflow: its style wraps.
    assert!(
        tsx.contains("flexWrap: \"wrap\""),
        "default row must wrap; got:\n{tsx}"
    );
    // `row(scroll="horizontal")` becomes a horizontal ScrollView.
    assert!(
        tsx.contains("<ScrollView horizontal"),
        "scroll=horizontal row must emit a horizontal ScrollView; got:\n{tsx}"
    );
    assert!(
        tsx.contains("import { View, Text, Pressable, Image, TextInput, ScrollView, StyleSheet }"),
        "ScrollView must be imported; got:\n{tsx}"
    );
}

const SCREEN_SRC: &str = r#"
component NavBar() {
    view: row(raw_class="nav") { text { "nav" } }
}

component Home() {
    view: column(raw_class="root") {
        NavBar()
        text { "hi" }
    }
}

component Bleeder() {
    view: column(bleed=true) {
        text { "edge to edge" }
    }
}

routes {
    "/" to Home
    "/b" to Bleeder
}
"#;

#[test]
fn rn_screen_root_gets_default_edge_padding() {
    let tsx = rn_component(SCREEN_SRC, "Home");
    assert!(
        tsx.contains("<View style={styles.screen}>"),
        "screen-root component must wrap its view in the padded screen container; got:\n{tsx}"
    );
    assert!(
        tsx.contains("screen: { flex: 1, paddingHorizontal: 16 }"),
        "screen style must define horizontal padding; got:\n{tsx}"
    );
}

#[test]
fn rn_nested_component_is_not_screen_padded() {
    // NavBar is rendered inside Home but is not itself a route → no screen pad
    // (prevents double-padding).
    let tsx = rn_component(SCREEN_SRC, "NavBar");
    assert!(
        !tsx.contains("styles.screen"),
        "non-route component must NOT get screen padding; got:\n{tsx}"
    );
}

#[test]
fn rn_bleed_opts_out_of_screen_padding() {
    let tsx = rn_component(SCREEN_SRC, "Bleeder");
    assert!(
        !tsx.contains("styles.screen"),
        "a screen root with `bleed` must opt out of default padding; got:\n{tsx}"
    );
}

#[test]
fn rn_link_emits_expo_router_link() {
    let tsx = rn_component(NAV_SRC, "Menu");
    assert!(
        tsx.contains("import { Link } from \"expo-router\""),
        "link element must import expo-router Link; got:\n{tsx}"
    );
    assert!(
        tsx.contains("<Link href={\"/timeline\"}>"),
        "link(href=...) must become <Link href={{...}}>; got:\n{tsx}"
    );
    // The label must be wrapped in <Text> (RN forbids bare strings in elements).
    assert!(
        tsx.contains("<Text") && tsx.contains("Timeline"),
        "link label must render inside <Text>; got:\n{tsx}"
    );
}

// ── Phase 5 S7: external react component imports on the RN target ──────────────

#[test]
fn rn_imported_react_component_emits_es_import_not_sibling_import() {
    let src = r#"
import react MyButton from "@acme/btn"

component Page() {
    view: column() {
        MyButton()
    }
}
"#;
    let tsx = rn_component(src, "Page");
    assert!(
        tsx.contains("import MyButton from \"@acme/btn\";"),
        "RN target must emit the external ES import, got:\n{tsx}"
    );
    assert!(
        tsx.contains("<MyButton"),
        "RN target must render MyButton as a component tag, got:\n{tsx}"
    );
    assert!(
        !tsx.contains("from \"./MyButton\""),
        "RN target must not emit a sibling ./MyButton import, got:\n{tsx}"
    );
}

#[test]
fn rn_named_react_imports_are_grouped_and_deterministic() {
    let src = r#"
import react { Card, Avatar } from "react-native-paper"

component Page() {
    view: column() {
        Card()
        Avatar()
    }
}
"#;
    let tsx = rn_component(src, "Page");
    assert!(
        tsx.contains("import { Avatar, Card } from \"react-native-paper\";"),
        "RN target must emit a grouped, sorted named import, got:\n{tsx}"
    );
}

#[test]
fn rn_paper_import_emits_provider_guidance() {
    let src = r#"
import react { Card } from "react-native-paper"

component Page() {
    view: column() {
        Card()
    }
}
"#;
    let tsx = rn_component(src, "Page");
    assert!(
        tsx.contains("requires <PaperProvider>"),
        "RN target must emit PaperProvider guidance for react-native-paper, got:\n{tsx}"
    );
}
