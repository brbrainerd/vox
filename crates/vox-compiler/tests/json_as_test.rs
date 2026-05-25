//! Integration tests for `@json_as(TypeName)` — Phase M, Step 6.
//!
//! RFC: json-as-rfc-2026-05-24.md.
//!
//! These tests verify the end-to-end pipeline:
//!
//!   Vox source with `@json_as` annotation
//!   → parse → HIR lower (synthesises `from_json` / `to_json`)
//!   → eval (no special-casing — synthesised fns are ordinary HirFns)
//!
//! Each test calls `fn main()` which exercises the synthesised functions and
//! returns a bool or int result that the assertion checks.

use vox_compiler::eval::value::VoxValue;
use vox_compiler::eval::Interpreter;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;

// ──────────────────────────────────────────────────────────────────────────────
// Test infrastructure
// ──────────────────────────────────────────────────────────────────────────────

fn run(src: &str) -> Result<VoxValue, String> {
    let tokens = lex(src);
    let module =
        parse_script(tokens).map_err(|errs| format!("parse: {} errors", errs.len()))?;
    let lowered = lower_module(&module);
    let mut interp = Interpreter::new(10_000_000);
    interp
        .run_module(&lowered)
        .map_err(|e| format!("module: {e:?}"))?;
    interp.call("main", vec![]).map_err(|e| format!("main: {e:?}"))
}

fn run_bool(src: &str) -> bool {
    match run(src).expect("run_bool") {
        VoxValue::Bool(b) => b,
        VoxValue::Int(n) => n != 0,
        other => panic!("expected Bool, got {other:?}"),
    }
}

fn run_str(src: &str) -> String {
    match run(src).expect("run_str") {
        VoxValue::Str(s) => s,
        other => panic!("expected Str, got {other:?}"),
    }
}

fn run_int(src: &str) -> i64 {
    match run(src).expect("run_int") {
        VoxValue::Int(n) => n,
        other => panic!("expected Int, got {other:?}"),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase M Step 2 wire-in: synthesised fns are in HirModule::functions
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn synthesised_fns_appear_in_hir_module() {
    let tokens = lex(r#"
        @json_as(Widget)
        type Widget {
            id: int,
            name: str,
        }
    "#);
    let module = parse_script(tokens).expect("parse");
    let hir = lower_module(&module);

    let from_json_exists = hir.functions.iter().any(|f| f.name == "Widget_from_json");
    let to_json_exists = hir.functions.iter().any(|f| f.name == "Widget_to_json");
    assert!(from_json_exists, "Widget_from_json not in hir.functions");
    assert!(to_json_exists, "Widget_to_json not in hir.functions");
}

// ──────────────────────────────────────────────────────────────────────────────
// Basic struct: required scalar fields
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn from_json_extracts_required_str_field() {
    let ok = run_bool(r#"
        @json_as(Product)
        type Product {
            name: str,
        }

        fn main() to bool {
            let payload = "{" + "\"name\":\"gadget\"" + "}"
            let r = json.parse(payload)
            if r.is_err() { return false }
            let res = Product_from_json(r.unwrap())
            if res.is_err() { return false }
            let p = res.unwrap()
            return p.get("name").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("") == "gadget"
        }
    "#);
    assert!(ok);
}

#[test]
fn from_json_extracts_required_int_field() {
    let n = run_int(r#"
        @json_as(Counter)
        type Counter {
            value: int,
        }

        fn main() to int {
            let payload = "{" + "\"value\":42" + "}"
            let r = json.parse(payload)
            if r.is_err() { return -1 }
            let res = Counter_from_json(r.unwrap())
            if res.is_err() { return -2 }
            let c = res.unwrap()
            return c.get("value").and_then(fn(j: Json) to Option[int] { j.as_int() }).unwrap_or(-3)
        }
    "#);
    assert_eq!(n, 42);
}

// ──────────────────────────────────────────────────────────────────────────────
// Missing required field → Err
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn from_json_errors_on_missing_required_field() {
    let s = run_str(r#"
        @json_as(Item)
        type Item {
            id: int,
            label: str,
        }

        fn main() to str {
            let payload = "{" + "\"id\":1" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Item_from_json(r.unwrap())
            if res.is_err() { return "missing_field_error" }
            return "ok"
        }
    "#);
    assert_eq!(s, "missing_field_error");
}

// ──────────────────────────────────────────────────────────────────────────────
// Option[T] field — absent → None (no error)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn from_json_option_field_absent_is_none() {
    let s = run_str(r#"
        @json_as(Note)
        type Note {
            title: str,
            body: Option[str],
        }

        fn main() to str {
            let payload = "{" + "\"title\":\"hello\"" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Note_from_json(r.unwrap())
            if res.is_err() { return "from_json_error" }
            let n = res.unwrap()
            // n is an object; n.get("body") wraps the stored value in another Option.
            // The stored value for "body" is itself an Option[str].
            // So: n.get("body") → Some(Option(None)) when absent.
            let body_outer = n.get("body")
            if body_outer.is_none() { return "key_missing" }
            let body_field = body_outer.unwrap()
            if body_field.is_none() { return "none" }
            return "some"
        }
    "#);
    // body key is absent from JSON → Option[str] field is None
    assert_eq!(s, "none");
}

#[test]
fn from_json_option_field_present_is_some() {
    let s = run_str(r#"
        @json_as(Note)
        type Note {
            title: str,
            body: Option[str],
        }

        fn main() to str {
            let payload = "{" + "\"title\":\"hi\"" + ",\"body\":\"content\"" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Note_from_json(r.unwrap())
            if res.is_err() { return "from_json_error" }
            let n = res.unwrap()
            // n.get("body") → Some(Option(Some(Str("content"))))
            let body_outer = n.get("body")
            if body_outer.is_none() { return "key_missing" }
            let body_field = body_outer.unwrap()
            if body_field.is_none() { return "none" }
            return body_field.unwrap()
        }
    "#);
    assert_eq!(s, "content");
}

// ──────────────────────────────────────────────────────────────────────────────
// to_json round-trip
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn to_json_emits_object_with_correct_keys() {
    let s = run_str(r#"
        @json_as(Config)
        type Config {
            host: str,
            port: int,
        }

        fn main() to str {
            let payload = "{" + "\"host\":\"localhost\"" + ",\"port\":8080" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Config_from_json(r.unwrap())
            if res.is_err() { return "from_json_error" }
            let cfg = res.unwrap()
            let out = Config_to_json(cfg)
            return out.get("host").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("no_host")
        }
    "#);
    assert_eq!(s, "localhost");
}

// ──────────────────────────────────────────────────────────────────────────────
// @field_name override
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn field_name_attribute_overrides_json_key() {
    let ok = run_bool(r#"
        @json_as(Order)
        type Order {
            @field_name("order_id") id: str,
        }

        fn main() to bool {
            let payload = "{" + "\"order_id\":\"abc-123\"" + "}"
            let r = json.parse(payload)
            if r.is_err() { return false }
            let res = Order_from_json(r.unwrap())
            if res.is_err() { return false }
            let o = res.unwrap()
            return o.get("id").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("") == "abc-123"
        }
    "#);
    assert!(ok);
}

// ──────────────────────────────────────────────────────────────────────────────
// naming: "camelCase"
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn camel_case_naming_reads_camel_json_keys() {
    let ok = run_bool(r#"
        @json_as(Event, naming: "camelCase")
        type Event {
            event_type: str,
            created_at: str,
        }

        fn main() to bool {
            let payload = "{" + "\"eventType\":\"click\"" + ",\"createdAt\":\"2026-01-01\"" + "}"
            let r = json.parse(payload)
            if r.is_err() { return false }
            let res = Event_from_json(r.unwrap())
            if res.is_err() { return false }
            let e = res.unwrap()
            let typ = e.get("event_type").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("")
            return typ == "click"
        }
    "#);
    assert!(ok);
}

// ──────────────────────────────────────────────────────────────────────────────
// defaults: true — missing field gets zero value, no error
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn defaults_true_missing_field_uses_zero() {
    let s = run_str(r#"
        @json_as(Settings, defaults: true)
        type Settings {
            timeout: int,
            label: str,
        }

        fn main() to str {
            let payload = "{" + "\"label\":\"test\"" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Settings_from_json(r.unwrap())
            if res.is_err() { return "from_json_error" }
            let s = res.unwrap()
            let timeout_val = s.get("timeout")
            if timeout_val.is_none() { return "missing" }
            return "present"
        }
    "#);
    // With defaults: true, missing `timeout` int field gets 0 — key should be present in result
    assert_eq!(s, "present");
}

// ──────────────────────────────────────────────────────────────────────────────
// No @json_as: fns are NOT synthesised
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn unannotated_type_has_no_synthesised_fns() {
    let tokens = lex(r#"
        type Plain {
            x: int,
        }
    "#);
    let module = parse_script(tokens).expect("parse");
    let hir = lower_module(&module);

    let any_json_fns = hir
        .functions
        .iter()
        .any(|f| f.name.contains("Plain_from_json") || f.name.contains("Plain_to_json"));
    assert!(!any_json_fns, "unexpected json fns for unannotated type");
}

// ──────────────────────────────────────────────────────────────────────────────
// Bool field round-trip
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn bool_field_round_trip() {
    let ok = run_bool(r#"
        @json_as(Flag)
        type Flag {
            enabled: bool,
        }

        fn main() to bool {
            let payload = "{" + "\"enabled\":true" + "}"
            let r = json.parse(payload)
            if r.is_err() { return false }
            let res = Flag_from_json(r.unwrap())
            if res.is_err() { return false }
            let f = res.unwrap()
            return f.get("enabled").and_then(fn(j: Json) to Option[bool] { j.as_bool() }).unwrap_or(false)
        }
    "#);
    assert!(ok);
}

// ──────────────────────────────────────────────────────────────────────────────
// Multiple fields: all extracted correctly
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn multi_field_struct_all_fields_extracted() {
    let n = run_int(r#"
        @json_as(Stats)
        type Stats {
            count: int,
            total: int,
        }

        fn main() to int {
            let payload = "{" + "\"count\":3" + ",\"total\":99" + "}"
            let r = json.parse(payload)
            if r.is_err() { return -1 }
            let res = Stats_from_json(r.unwrap())
            if res.is_err() { return -2 }
            let s = res.unwrap()
            let count = s.get("count").and_then(fn(j: Json) to Option[int] { j.as_int() }).unwrap_or(-3)
            let total = s.get("total").and_then(fn(j: Json) to Option[int] { j.as_int() }).unwrap_or(-4)
            return count + total
        }
    "#);
    assert_eq!(n, 102);
}

// ──────────────────────────────────────────────────────────────────────────────
// Tagged-enum ADT (RFC §4.4): build_from_json_adt code path
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn adt_tagged_enum_dispatches_on_tag_field() {
    let s = run_str(r#"
        @json_as(Shape, tag: "type")
        type Shape =
            | Circle { radius: int }
            | Square { side: int }

        fn main() to str {
            let payload = "{" + "\"type\":\"Circle\"" + ",\"radius\":5" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Shape_from_json(r.unwrap())
            if res.is_err() { return "from_json_error: " + res.unwrap_err() }
            let shape = res.unwrap()
            let radius = shape.get("radius").and_then(fn(j: Json) to Option[int] { j.as_int() }).unwrap_or(-1)
            return "radius:" + str(radius)
        }
    "#);
    assert_eq!(s, "radius:5");
}

#[test]
fn adt_tagged_enum_second_variant_dispatches_correctly() {
    let s = run_str(r#"
        @json_as(Shape, tag: "type")
        type Shape =
            | Circle { radius: int }
            | Square { side: int }

        fn main() to str {
            let payload = "{" + "\"type\":\"Square\"" + ",\"side\":7" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Shape_from_json(r.unwrap())
            if res.is_err() { return "from_json_error: " + res.unwrap_err() }
            let shape = res.unwrap()
            let side = shape.get("side").and_then(fn(j: Json) to Option[int] { j.as_int() }).unwrap_or(-1)
            return "side:" + str(side)
        }
    "#);
    assert_eq!(s, "side:7");
}

#[test]
fn adt_tagged_enum_missing_tag_returns_err() {
    let s = run_str(r#"
        @json_as(Shape, tag: "type")
        type Shape =
            | Circle { radius: int }

        fn main() to str {
            let payload = "{" + "\"radius\":5" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Shape_from_json(r.unwrap())
            if res.is_ok() { return "unexpected_ok" }
            return "missing_tag"
        }
    "#);
    assert_eq!(s, "missing_tag");
}

#[test]
fn adt_tagged_enum_unknown_tag_returns_err() {
    let s = run_str(r#"
        @json_as(Shape, tag: "type")
        type Shape =
            | Circle { radius: int }
            | Square { side: int }

        fn main() to str {
            let payload = "{" + "\"type\":\"Triangle\"" + ",\"radius\":5" + "}"
            let r = json.parse(payload)
            if r.is_err() { return "parse_error" }
            let res = Shape_from_json(r.unwrap())
            if res.is_ok() { return "unexpected_ok" }
            return "unknown_tag"
        }
    "#);
    assert_eq!(s, "unknown_tag");
}
