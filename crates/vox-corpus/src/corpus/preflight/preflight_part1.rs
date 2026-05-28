/// Compute xxh3 fingerprint over all watched files from the repo root.
/// Returns a zero-padded 16-char hex string.
pub fn compute_corpus_fingerprint(repo_root: &Path) -> String {
    let mut combined: u64 = 0;
    for rel in WATCHED_FILES {
        let full = repo_root.join(rel);
        let bytes = std::fs::read(&full).unwrap_or_default();
        // XOR fold with the path itself so renames invalidate too
        let path_hash = xxh3_64(rel.as_bytes());
        let content_hash = xxh3_64(&bytes);
        combined = combined.wrapping_add(content_hash ^ path_hash);
    }
    format!("{combined:016x}")
}

/// Returns `true` if the corpus fingerprint stored in `snapshot_file` matches
/// the current fingerprint (i.e., corpus is fresh and does not need regeneration).
pub fn corpus_is_fresh(repo_root: &Path, snapshot_file: &Path) -> bool {
    match vox_bounded_fs::read_utf8_path_capped(snapshot_file) {
        Ok(stored) => stored.trim() == compute_corpus_fingerprint(repo_root),
        Err(_) => false,
    }
}

/// Write the current fingerprint to a snapshot file.
pub fn write_fingerprint_snapshot(repo_root: &Path, snapshot_file: &Path) -> Result<()> {
    if let Some(parent) = snapshot_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(snapshot_file, compute_corpus_fingerprint(repo_root))?;
    Ok(())
}

/// Target cleanup: remove the mixed train file and cache so a fresh
/// regeneration doesn't stale-layer over old data.
pub fn clean_corpus_targets(repo_root: &Path) -> Result<()> {
    let targets = [
        "target/dogfood/train_mixed.jsonl",
        "target/dogfood/.corpus_cache/",
    ];
    for rel in &targets {
        let path = repo_root.join(rel);
        if path.is_file() {
            std::fs::remove_file(&path)?;
            tracing::info!("[preflight] removed {}", path.display());
        } else if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
            tracing::info!("[preflight] removed dir {}", path.display());
        }
    }
    Ok(())
}

// ── Compact Vox generator ────────────────────────────────────────────────────

/// Convert pretty-printed Vox source to compact single-line form.
/// Preserves all semantics while removing indentation and extra whitespace.
/// This is the canonical serializable/transport form of Vox code.
pub fn to_compact(src: &str) -> String {
    src.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Generate compact Vox variants from an existing organic pair.
/// Returns a JSONL string adding a compact-format training pair.
pub fn compact_variant(prompt: &str, pretty_response: &str, category: &str) -> String {
    let compact = to_compact(pretty_response);
    let prompt_full = format!("{prompt} (compact, no whitespace)");
    serde_json::json!({
        "prompt": prompt_full,
        "response": compact,
        "messages": [
            {"role": "user", "content": prompt_full},
            {"role": "assistant", "content": compact}
        ],
        "category": format!("{category}_compact"),
        "format": "vox_organic_compact",
        "schema_version": "vox_dogfood_v1",
    })
    .to_string()
}

// ── Multi-turn conversation generator ────────────────────────────────────────

/// A multi-turn conversation turn.
#[derive(Debug, Clone)]
pub struct Turn {
    /// Role: "user" or "assistant".
    pub role: &'static str,
    /// Message content.
    pub content: String,
}

/// Generate a 3-turn iterative refinement conversation for a given Vox construct type.
/// Turn 1: create it, Turn 2: add error handling with real `Result[T]`, Turn 3: production-ready.
pub fn gen_multiturn_vox(
    construct: &str,
    name: &str,
    base_code: &str,
    template_idx: usize,
) -> Vec<Turn> {
    match template_idx % 4 {
        0 => vec![
            Turn {
                role: "user",
                content: format!("Write a Vox {construct} called `{name}`"),
            },
            Turn {
                role: "assistant",
                content: base_code.to_string(),
            },
            Turn {
                role: "user",
                content: format!("Add error handling and logging to `{name}`"),
            },
            Turn {
                role: "assistant",
                content: format!(
                    "// Error handling via Result[T] — null is banned\n\
                 @traced\n\
                 fn {name}(x: int) -> Result[str]:\n\
                     if x < 0:\n\
                         return Err(\"invalid: x must be non-negative\")\n\
                     return Ok(\"done\")"
                ),
            },
            Turn {
                role: "user",
                content: format!("Add a @test for `{name}` covering the error case"),
            },
            Turn {
                role: "assistant",
                content: format!(
                    "@test\n\
                 fn test_{name}_rejects_negative() -> Unit:\n\
                     let result = {name}(-1)\n\
                     match result:\n\
                         Err(msg) -> assert(msg.contains(\"invalid\"))\n\
                         Ok(_) -> fail(\"expected error\")"
                ),
            },
        ],
        1 => vec![
            Turn {
                role: "user",
                content: format!(
                    "I have this {construct} called `{name}`. Explain how it works:\n```vox\n{base_code}\n```"
                ),
            },
            Turn {
                role: "assistant",
                content: format!(
                    "This Vox {construct} named `{name}` initializes and manages state. It uses strong typing and explicit error handling via Option[T]/Result[T] — null is never used."
                ),
            },
            Turn {
                role: "user",
                content: "Can you refactor it to be more performant?".to_string(),
            },
            Turn {
                role: "assistant",
                content: format!(
                    "// Refactored: inlined hot path, removed intermediate allocations\n\
                 @inline\n\
                 fn {name}(x: int) -> Result[str]:\n\
                     if x < 0: return Err(\"invalid\")\n\
                     return Ok(\"done\")"
                ),
            },
        ],
        2 => vec![
            Turn {
                role: "user",
                content: format!("Create a {construct} named `{name}`."),
            },
            Turn {
                role: "assistant",
                content: base_code.to_string(),
            },
            Turn {
                role: "user",
                content: "Now make it return Option[T] for the absent case.".to_string(),
            },
            Turn {
                role: "assistant",
                content: format!(
                    "// Option[T] exhaustive match\n\
                 fn {name}(id: int) -> Option[str]:\n\
                     if id == 0: return None\n\
                     return Some(\"found\")"
                ),
            },
        ],
        _ => vec![
            Turn {
                role: "user",
                content: format!("Write a {construct} for `{name}`"),
            },
            Turn {
                role: "assistant",
                content: base_code.to_string(),
            },
            Turn {
                role: "user",
                content: "Add call-count tracking to it.".to_string(),
            },
            Turn {
                role: "assistant",
                content: format!(
                    "// Call tracking via actor state\n\
                 actor {name}Tracker:\n\
                     state count: int = 0\n\
                     on increment() -> Unit:\n\
                         self.count = self.count + 1"
                ),
            },
        ],
    }
}

/// Serialize a multi-turn conversation to JSONL (ChatML-compatible format).
///
/// Always includes top-level `prompt` (first user turn) and `response` (first assistant turn)
/// so every row satisfies the uniform schema contract checked by corpus validation tools.
pub fn multiturn_to_jsonl(turns: &[Turn], category: &str) -> String {
    let messages: Vec<serde_json::Value> = turns
        .iter()
        .map(|t| serde_json::json!({"role": t.role, "content": t.content}))
        .collect();
    // Extract first user and first assistant turn for the required top-level prompt/response fields.
    let prompt = turns
        .iter()
        .find(|t| t.role == "user")
        .map(|t| t.content.as_str())
        .unwrap_or("");
    let response = turns
        .iter()
        .find(|t| t.role == "assistant")
        .map(|t| t.content.as_str())
        .unwrap_or("");
    serde_json::json!({
        "prompt": prompt,
        "response": response,
        "messages": messages,
        "category": category,
        "format": "multiturn_chat",
        "schema_version": "vox_dogfood_v1",
    })
    .to_string()
}

// ── Error → Fix pair generator ────────────────────────────────────────────────

/// A category of intentional syntax/semantic error.
///
/// Variants cover all common beginner mistakes identified in the gap analysis.
/// New variants must have a corresponding `break_vox` arm.
#[derive(Debug, Clone, Copy)]
pub enum BrokenKind {
    MissingReturnArrow,
    UnclosedBrace,
    KeywordTypo,
    MissingRet,
    WrongType,
    MissingToUnit,
    TypeMismatch,
    OptionUnwrapMissing,
    BadReturnType,
    /// Generic instantiated with wrong number of type parameters, e.g. `List[]` instead of `List[int]`.
    UnresolvedGenericArity,
    /// Branches return different types, causing ambiguous inference.
    InferenceAmbiguity,
    /// Match arm appears after a wildcard `_` arm, making it dead code.
    UnreachableMatchArm,
}

/// Apply a specific kind of breakage to valid Vox source.
pub fn break_vox(src: &str, kind: BrokenKind) -> (String, String) {
    match kind {
        BrokenKind::MissingReturnArrow => {
            let broken = src.replace("-> ", "");
            let explanation = "Missing `->` return type arrow in function signature. \
                               Vox requires explicit return type annotations."
                .to_string();
            (broken, explanation)
        }
        BrokenKind::UnclosedBrace => {
            let broken = if src.contains('{') {
                let mut s = src.to_string();
                if let Some(pos) = s.rfind('}') {
                    s.remove(pos);
                }
                s
            } else {
                src.to_string()
            };
            let explanation = "Unclosed brace `{`. Every `{` must have a matching `}`.".to_string();
            (broken, explanation)
        }
        BrokenKind::KeywordTypo => {
            let broken = src.replace("fn ", "fun ").replace("actor ", "actr ");
            let explanation = "Keyword typo: `fun` → `fn`, `actr` → `actor`. \
                               Vox keywords are exact."
                .to_string();
            (broken, explanation)
        }
        BrokenKind::MissingRet => {
            let broken = src.replace("    return ", "    ");
            let explanation = "Missing `return` keyword. Vox uses explicit `return` for returns, \
                               not bare expressions."
                .to_string();
            (broken, explanation)
        }
        BrokenKind::WrongType => {
            let broken = src
                .replace(": int", ": integer")
                .replace(": str", ": string");
            let explanation = "Wrong type names: `integer` → `int`, `string` → `str`. \
                               Vox primitive types are: `int`, `str`, `bool`, `float`."
                .to_string();
            (broken, explanation)
        }
        BrokenKind::MissingToUnit => {
            let broken = src.replace(" -> Unit", "");
            let explanation = "Missing `-> Unit` return type. Functions that perform side-effects \
                               but return no value must explicitly declare `-> Unit`."
                .to_string();
            (broken, explanation)
        }
        BrokenKind::TypeMismatch => {
            let broken = src.replace("= 0", "= \"0\"");
            let explanation = "Type mismatch: assigned `str` where `int` was expected.".to_string();
            (broken, explanation)
        }
        BrokenKind::OptionUnwrapMissing => {
            let broken = src.replacen("Some(", "", 1).replacen(')', "", 1);
            let explanation =
                "Attempting to use `Option[T]` as `T` directly without unwrap or matching."
                    .to_string();
            (broken, explanation)
        }
        BrokenKind::BadReturnType => {
            let broken = src.replace("-> ", "returns ");
            let explanation =
                "Invalid return type syntax: use `->` instead of `returns`.".to_string();
            (broken, explanation)
        }
        BrokenKind::UnresolvedGenericArity => {
            // Replace `List[int]` with `List[]` — missing type argument
            let broken = src
                .replace("List[int]", "List[]")
                .replace("Option[str]", "Option[]");
            let explanation = "Generic type `List` requires exactly one type argument. \
                               `List[]` is invalid — use `List[int]`, `List[str]`, etc."
                .to_string();
            (broken, explanation)
        }
        BrokenKind::InferenceAmbiguity => {
            // Create a branch where types differ — int vs str
            let broken = src.replace("return 0", "return if true { 0 } else { \"zero\" }");
            let explanation = "Inference ambiguity: `if` branches return `int` and `str`. \
                               Both arms of an `if` expression must return the same type."
                .to_string();
            (broken, explanation)
        }
        BrokenKind::UnreachableMatchArm => {
            // Add an arm after a wildcard
            let broken = src.replace("_ => false", "_ => false\n        true => false");
            let explanation = "`true => false` is unreachable — the `_` wildcard arm above it \
                               captures all remaining cases. Remove the dead arm or reorder."
                .to_string();
            (broken, explanation)
        }
    }
}
