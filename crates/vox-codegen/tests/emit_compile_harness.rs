//! Emit→compile harness: generate a full Rust **script** crate from a Vox
//! program, write it to a temp dir, and run `cargo build` on it — proving the
//! generated *body* actually type-checks, not merely that emitted strings look
//! right.
//!
//! This is the verification net for ownership / borrow codegen changes
//! (Workstream B escape analysis): a `&str`-vs-`String` parameter mismatch is a
//! `rustc` *type* error that snapshot/`*_compiles.rs` (symbol-link) tests do not
//! catch — only compiling the generated crate does.
//!
//! Most tests run by default; only tests with known pre-existing blockers are
//! `#[ignore]`d with a specific reason. Compiling a generated crate (tokio +
//! `vox-actor-runtime`) takes a while. Run explicitly:
//!
//! ```sh
//! cargo test -p vox-codegen --test emit_compile_harness -- --ignored --nocapture
//! ```
//!
//! A stable `CARGO_TARGET_DIR` under the OS temp dir is reused across runs, so
//! after the first (cold) compile of dependencies subsequent runs are fast.
//!
//! Nested `cargo build` passes `--config build.rustc-wrapper=""` so agent shells
//! without `sccache` on PATH (see root `.cargo/config.toml`) can run these tests.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use vox_codegen::codegen_rust::generate_script;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module;

// All compile-harness tests share ONE stable target dir so that `tokio` +
// `vox-actor-runtime` are compiled only once (cold ~100s; warm ~5s each).
// A mutex serialises access so parallel test threads don't stomp on Cargo's
// lock files. Place it in `%TEMP%/vox-emit-harness-target` so it survives
// across `cargo test` invocations (sccache-style warm cache).
static COMPILE_LOCK: Mutex<()> = Mutex::new(());

fn shared_target_dir() -> PathBuf {
    std::env::temp_dir().join("vox-emit-harness-target")
}

/// Workspace root (contains `examples/golden/`).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Absolute path to the `vox-actor-runtime` crate the generated script depends on.
fn runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vox-actor-runtime")
}

/// Standalone script crates built outside the workspace need the same
/// `[patch.crates-io]` entries as the repo root (pure-rust `aegis` on Windows).
/// Also normalize the generated `turso` dep to match workspace 0.6 + sync.
fn inject_workspace_patches(project_dir: &Path) {
    let cargo_path = project_dir.join("Cargo.toml");
    let Ok(mut toml) = std::fs::read_to_string(&cargo_path) else {
        return;
    };
    const CANON_TURSO: &str =
        "turso = { version = \"0.6\", default-features = false, features = [\"sync\"] }";
    if toml.contains("vox-db") {
        if let Some(start) = toml.find("turso = {") {
            if let Some(line_end) = toml[start..].find('\n') {
                let end = start + line_end;
                toml.replace_range(start..end, CANON_TURSO);
            }
        } else if let Some(idx) = toml.find("[dependencies]") {
            let insert_at = toml[idx..]
                .find('\n')
                .map(|off| idx + off + 1)
                .unwrap_or(toml.len());
            toml.insert_str(insert_at, &format!("{CANON_TURSO}\n"));
        }
    }
    let aegis_path = repo_root()
        .join("patches/aegis-0.9.8")
        .to_string_lossy()
        .replace('\\', "/");
    if !toml.contains("[patch.crates-io]") {
        toml.push_str(&format!(
            "\n[patch.crates-io]\naegis = {{ path = \"{aegis_path}\" }}\n"
        ));
    } else if !toml.contains("aegis = ") {
        toml = toml.replace(
            "[patch.crates-io]\n",
            &format!("[patch.crates-io]\naegis = {{ path = \"{aegis_path}\" }}\n"),
        );
    }
    let _ = std::fs::write(cargo_path, toml);
}

/// Generate a native script crate for `src`, write it to a fresh temp dir, and
/// `cargo build` it. `Ok(())` iff it compiled; otherwise the captured cargo
/// stderr.
fn compile_vox_script(src: &str) -> Result<(), String> {
    let module = parse_script(lex(src)).map_err(|e| format!("parse failed: {e:?}"))?;
    let mut hir = lower_module(&module);
    // Run typecheck so `inferred_types` is populated — required for list/str method
    // disambiguation (e.g. `count`/`contains` shared between str and List receivers).
    let _ = typecheck_hir_module(src, &mut hir);
    let output = generate_script(&hir, "vox-script", Some(&runtime_path()))
        .map_err(|e| format!("codegen failed: {e}"))?;

    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    output
        .write_to_dir(dir.path())
        .map_err(|e| format!("write_to_dir: {e}"))?;
    inject_workspace_patches(dir.path());

    // Shared stable target dir + lock so every test reuses compiled deps.
    // The mutex serialises cargo invocations; tests still run in any order.
    let _guard = COMPILE_LOCK.lock().unwrap();
    let target_dir = shared_target_dir();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(cargo)
        .current_dir(dir.path())
        .args(["build", "--config", "build.rustc-wrapper=\"\""])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env_remove("RUSTC_WRAPPER")
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// Read `examples/golden/{rel}`, then parse → typecheck → generate → `cargo build`.
fn compile_golden_file(rel: &str) -> Result<(), String> {
    let path = repo_root().join("examples/golden").join(rel);
    let src =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    compile_vox_script(&src)
}

/// Assert a Vox program's generated Rust compiles, surfacing cargo's error.
fn assert_compiles(src: &str) {
    if let Err(e) = compile_vox_script(src) {
        panic!("generated Rust did not compile:\n{e}");
    }
}

/// Assert a golden `.vox` file's generated Rust compiles, surfacing cargo's error.
fn assert_golden_compiles(rel: &str) {
    if let Err(e) = compile_golden_file(rel) {
        panic!("golden {rel} did not compile:\n{e}");
    }
}

#[test]
fn minimal_script_compiles() {
    assert_compiles("fn main() { print(\"hi\") }");
}

/// A value-returning `main` (`fn main() to int`) must compile: the generated
/// wrapper runs the body in a closure and discards the result, rather than
/// inlining `return <value>` into `fn main() -> ()`. (Regression test for the
/// entry-wrapper bug this harness originally surfaced.)
#[test]
fn value_returning_main_compiles() {
    assert_compiles("fn main() to int { return 1 + 2 }");
}

/// Value-semantic list `.push` must compile: Vox `xs = xs.push(y)` returns the new
/// list, so it must emit a value-returning block (not Rust `Vec::push` → `()`).
#[test]
fn list_push_compiles() {
    assert_compiles(
        r#"
fn main() {
    let mut xs = ["a"]
    xs = xs.push("b")
    xs = xs.push("c")
    print(str(xs.len()))
}
"#,
    );
}

/// `<list>.get(i) is Some(..)` must compile: `Vec::get` returns `Option<&T>`, so
/// the get side is `.cloned()` to an owned `Option<T>` to match the `Some(..)`
/// side — in both a plain `is` and the `assert(x is y)` → `assert_eq!` path.
#[test]
fn get_is_some_compiles() {
    assert_compiles(
        r#"
fn check(xs: List[str]) to bool {
    return xs.get(0) is Some("a")
}
fn main() {
    let xs = ["a", "b"]
    assert(xs.get(0) is Some("a"))
    assert(xs.get(5) isnt Some("z"))
    print(str(check(xs)))
}
"#,
    );
}

/// Regression net for the `crate-build-audit.vox` compiled-mode family:
/// - string-keyed JSON `get`/subscript must lower to `VoxJson::get(String)`
///   (was: `(j).get((k) as usize).cloned()` → E0308/E0605/E0599);
/// - `int(x)` must lower to a numeric cast (was: bare `int(..)` call → E0425);
/// - a no-`else` `if` whose branch tail is value-typed (e.g. a `match` with
///   Vox `{}` empty-object arms) must discard the tail (was: E0317);
/// - mixed int/float arithmetic must promote the int side (was: E0277);
/// - a value used inside a loop must not be moved as a "last use" (was: E0382);
/// - mixed int/float equality must promote the int side (was: E0277 — interp
///   `eval/value.rs` compares `(Int, Float)` as `(*a as f64) == *b`);
/// - a concatenated string key (`HirExpr::Binary` typed `str`) must take the
///   object-lookup path, not `as usize` list indexing.
#[test]
fn json_string_key_and_numeric_lowerings_compile() {
    assert_compiles(
        r#"
fn jnum(j: Json, k: str) to float {
    return j.get(k).and_then(fn(x: Json) to Option[float] { x.as_float() }).unwrap_or(0.0)
}
fn main() {
    let j = json.parse("{\"a\": 2}").unwrap()
    let k = "a"
    print(str(jnum(j, k)))
    print(str(jnum(j, "a")))
    let scaled = int(3.7)
    let ratio = scaled / 2.0
    print(str(ratio))
    if not fs.exists("vox-harness-nonexistent-dir") {
        match json.parse("{}") { Ok(_) => {} Error(_) => {} }
    }
    let names = ["a", "b"]
    let mut i = 0
    let mut total = 0
    while i < names.len() {
        if jnum(j, names.get(i).unwrap()) > 0.0 { total = total + 1 }
        i = i + 1
    }
    print(str(total))
    let whole = 2
    if whole is 2.0 { print("eq-promoted") }
    if whole isnt 2.5 { print("neq-promoted") }
    let k1 = "a"
    let k2 = ""
    print(str(jnum(j, k1 + k2)))
}
"#,
    );
}

#[test]
fn list_ops_compile() {
    assert_compiles(
        r#"
fn build(n: int) to list[int] {
    let mut acc: list[int] = []
    let mut i = 0
    while i < n {
        acc.push(i)
        i = i + 1
    }
    return acc
}
fn main() {
    let xs = build(3)
    print(str(len(xs)))
}
"#,
    );
}

/// String parameters exercised in both owned (returned/concatenated) and
/// argument positions — the shapes escape analysis (Workstream B) changes.
/// This is the regression guard for borrowed-`&str` signature emission: it must
/// keep compiling once params can be emitted as `&str`.
#[test]
fn string_param_shapes_compile() {
    assert_compiles(
        r#"
fn greet(name: str) to str {
    return "hello, " + name
}
fn shout(msg: str) to str {
    return greet(msg)
}
fn main() {
    print(shout("world"))
}
"#,
    );
}

/// `str + <numeric>` type-checks as `str` (the interpreter auto-stringifies).
/// Codegen must emit `format!`, not `String + i64`. Regression guard for the
/// previously-miscompiling mixed-type concatenation.
#[test]
fn str_plus_numeric_compiles() {
    assert_compiles(
        r#"
fn label(n: int) to str {
    return "item #" + n
}
fn main() {
    print(label(42))
}
"#,
    );
}

/// Exercises the new list-method surface added in this PR: every method that the
/// codegen now handles must also compile under `--mode script` (codegen path).
/// This is the compile-net counterpart to the fast `list_method_emit` tests.
///
/// `sum` / `sorted` / `zip` / `enumerate` / `flatten` are SKIPPED (see the
/// `try_emit_list_method` comments): `sum` needs an element-type-derived
/// `.sum::<T>()` annotation (E0283 otherwise); the rest produce nested/heterogeneous
/// types codegen can't yet resolve monomorphically.
#[test]
fn list_methods_compile() {
    assert_compiles(
        r#"
fn main() {
    let xs: list[str] = ["c", "a", "b"]

    let rev = xs.reverse()
    let rev2 = xs.reversed()

    let ys: list[str] = ["d", "e"]
    let ext = xs.extend(ys)

    let without_a = xs.remove("a")
    let without_0 = xs.remove_at(0)

    let sl = xs.slice_list(0, 2)
    let sl2 = xs.slice_list(1)

    let j = xs.join(", ")

    let idx = xs.index("b")
    let idx2 = xs.find_index("c")

    let cnt = xs.count("a")

    let has = xs.contains("b")

    // first/last → owned Option<T> (.cloned()).
    let f = xs.first()
    let l = xs.last()

    print(j)
}
"#,
    );
}

/// Exercises the full Vox string-method surface: every method that the
/// interpreter handles must also compile under `--mode script` (codegen path).
/// This is the compile-net counterpart to the fast `str_method_emit` tests.
/// Minimal `@json_as` must compile: signatures use `serde_json::Value` (or a
/// `type Json = …` alias), never a bare undefined `Json` type (CR-F2 #6).
#[test]
fn json_as_minimal_compiles() {
    assert_compiles(
        r#"
        @json_as(Widget)
        type Widget {
            name: str,
        }

        fn main() to str {
            let r = json.parse("{\"name\":\"x\"}")
            match r {
                Error(_) => return "err"
                Ok(j) => {
                    let res = Widget_from_json(j)
                    match res {
                        Error(_) => return "decode_err"
                        Ok(w) => return w.name
                    }
                }
            }
        }
        "#,
    );
}

#[test]
fn str_methods_compile() {
    assert_compiles(
        r#"
fn main() {
    let s = "Hello, World!"

    let n = s.len()
    let is_mt = s.is_empty()
    let up = s.to_upper()
    let lo = s.to_lower()
    let tr = "  hi  ".trim()
    let ts = "  hi  ".trim_start()
    let te = "  hi  ".trim_end()

    let has = s.contains("World")
    let sw = s.starts_with("Hello")
    let ew = s.ends_with("!")

    let parts = s.split(", ")
    let replaced = s.replace("World", "Vox")
    let rep = "ab".repeat(3)

    let cc = s.chars_count()
    let cnt = s.count("l")

    let ia = "abc".is_alpha()
    let id = "123".is_digit()
    let ian = "abc123".is_alnum()
    let iu = "ABC".is_upper()
    let il = "abc".is_lower()

    let o = "A".ord()
    let chars = "hi".chars()

    let ts2 = s.to_str()
    let sl = s.slice(0, 5)
    let ca = s.char_at(0)
    let io = s.index_of("World")
    let ti = "42".to_int()
    let tf = "3.14".to_float()

    print(up)
}
"#,
    );
}

#[test]
fn golden_noop_compiles() {
    assert_golden_compiles("mesh/noop.vox");
}

#[test]
fn golden_regex_free_functions_compiles() {
    assert_golden_compiles("regex_free_functions.vox");
}

#[test]
fn golden_decimal_math_compiles() {
    assert_golden_compiles("decimal_math.vox");
}

#[test]
fn golden_while_loop_algorithms_compiles() {
    assert_golden_compiles("while_loop_algorithms.vox");
}

#[test]
fn golden_json_as_typed_compiles() {
    assert_golden_compiles("json_as_typed.vox");
}
#[test]
fn golden_tuple_destructure_compiles() {
    assert_golden_compiles("tuple_destructure.vox");
}

#[test]
fn golden_match_arm_stmts_compiles() {
    assert_golden_compiles("match_arm_stmts.vox");
}

#[test]
fn golden_control_flow_if_compiles() {
    assert_golden_compiles("control_flow_if.vox");
}

#[test]
fn golden_error_propagation_compiles() {
    assert_golden_compiles("error_propagation.vox");
}

// ── Canonical ladder (required gate subset) ─────────────────────────────────
// These mirror `contracts/pipeline/canonical-ladder.v1.yaml` rust-script entries.

#[test]
fn ladder_hello_golden_compiles() {
    assert_golden_compiles("hello.vox");
}

#[test]
fn ladder_crud_api_golden_compiles() {
    assert_golden_compiles("crud_api.vox");
}

#[test]
fn ladder_durable_workflow_real_golden_compiles() {
    assert_golden_compiles("durable_workflow_real.vox");
}

#[test]
fn ladder_scheduled_tick_golden_compiles() {
    assert_golden_compiles("scheduled_tick.vox");
}

#[test]
fn ladder_db_native_ir_golden_compiles() {
    assert_golden_compiles("db_native_ir.vox");
}

#[test]
fn ladder_web_routing_fullstack_golden_compiles() {
    assert_golden_compiles("web_routing_fullstack.vox");
}

#[test]
fn ladder_auth_patterns_golden_compiles() {
    assert_golden_compiles("auth_patterns.vox");
}

#[test]
fn ladder_mcp_tools_golden_compiles() {
    assert_golden_compiles("mcp_tools.vox");
}

#[test]
fn ladder_json_as_typed_golden_compiles() {
    assert_golden_compiles("json_as_typed.vox");
}

#[test]
fn golden_closures_hof_compiles() {
    assert_golden_compiles("closures_hof.vox");
}
#[test]
fn golden_option_type_compiles() {
    assert_golden_compiles("option_type.vox");
}

#[test]
fn traced_fn_compiles() {
    // TRACE-D P6: verifies that @traced functions emit valid Rust (vox-telemetry dep present).
    assert_compiles("@traced\nfn greet(name: str) to str { return name }");
}

#[test]
fn traced_fn_span_emitted() {
    // Behavioral assertion: generated Rust contains the vox_telemetry span call and the
    // generated Cargo.toml contains the vox-telemetry dependency.  This is the structural
    // equivalent of "a span was recorded" without executing the crate at test time.
    let src = "@traced\nfn greet(name: str) to str { return name }";
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let output = generate_script(&hir, "vox-script", Some(&runtime_path()))
        .expect("generate_script should succeed for a @traced fn");

    let lib_rs = output
        .files
        .get("src/lib.rs")
        .expect("lib.rs must be generated");
    assert!(
        lib_rs.contains("vox_telemetry::current_trace_context"),
        "@traced fn must emit current_trace_context() call in generated lib.rs;\ngot:\n{lib_rs}"
    );
    assert!(
        lib_rs.contains("tracing::instrument") || lib_rs.contains("tracing::Span"),
        "@traced fn must emit a tracing span attribute in generated lib.rs;\ngot:\n{lib_rs}"
    );

    let cargo_toml = output
        .files
        .get("Cargo.toml")
        .expect("Cargo.toml must be generated");
    assert!(
        cargo_toml.contains("vox-telemetry"),
        "generated Cargo.toml must include vox-telemetry dep;\ngot:\n{cargo_toml}"
    );
}
