//! Format → re-parse → compare AST (span-stripped JSON).
//!
//! Starts from **`examples/golden/hello.vox`** (minimal documented golden). Broader golden coverage
//! is gated on formatter idempotency work — several larger goldens do not re-parse after `fmt::format` yet.

use serde_json::Value;
use std::path::PathBuf;

const FORMAT_ROUNDTRIP_FILES: &[&str] = &["examples/golden/hello.vox"];

fn normalize_ast_json(v: Value) -> Value {
    match v {
        Value::Object(mut m) => {
            m.remove("span");
            let mut out = serde_json::Map::new();
            for (k, vv) in m {
                out.insert(k, normalize_ast_json(vv));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(normalize_ast_json).collect()),
        other => other,
    }
}

#[test]
fn golden_corpus_format_preserves_ast_structure() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    for rel in FORMAT_ROUNDTRIP_FILES {
        let path = repo_root.join(rel);
        let src = std::fs::read_to_string(&path).expect("read golden allowlist file");
        let tokens = vox_compiler::lexer::lex(&src);
        let Ok(m1) = vox_compiler::parser::parse(tokens) else {
            panic!("golden must parse: {}", path.display());
        };
        let formatted = vox_compiler::fmt::format(&src);
        let tokens2 = vox_compiler::lexer::lex(&formatted);
        let m2 = vox_compiler::parser::parse(tokens2).unwrap_or_else(|e| {
            panic!(
                "re-parse after format failed for {}: {:?}",
                path.display(),
                e
            )
        });

        let j1 = normalize_ast_json(serde_json::to_value(&m1).expect("serde m1"));
        let j2 = normalize_ast_json(serde_json::to_value(&m2).expect("serde m2"));
        assert_eq!(
            j1, j2,
            "AST structure drift after format for {}",
            path.display()
        );
    }

    // Synthetic snippet — must always round-trip (regression guard independent of golden churn).
    {
        let src = "fn a() to int {\n    return 1\n}\n";
        let tokens = vox_compiler::lexer::lex(src);
        let m1 = vox_compiler::parser::parse(tokens).expect("parse synthetic");
        let formatted = vox_compiler::fmt::format(src);
        let m2 = vox_compiler::parser::parse(vox_compiler::lexer::lex(&formatted))
            .expect("re-parse synthetic after format");
        let j1 = normalize_ast_json(serde_json::to_value(&m1).unwrap());
        let j2 = normalize_ast_json(serde_json::to_value(&m2).unwrap());
        assert_eq!(j1, j2, "synthetic AST drift:\n{src}");
    }
}
