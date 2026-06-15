//! Accessibility and structure validation for TSX returned by v0.dev (TASK-5.4).
//!
//! Performs a lightweight regex-based JSX element scan and feeds the result
//! into the Web IR a11y validator (`validate_a11y`).  No full parse tree is
//! required — we only need enough structure to surface the most common a11y
//! violations before the generated file is committed to the project.
//!
//! ## What is checked
//!
//! All rules from `web_ir::validate_a11y` (TASK-5.3):
//! - `<img>` without `alt` (Error)
//! - `<button>` with no accessible label (Error)
//! - `<a>` without `href` (Warning)
//! - `role="button"` on non-button elements without keyboard handler (Warning)
//! - `<input>` without accessible label (Warning)
//!
//! ## Limitations
//!
//! The regex extractor handles standard JSX attribute syntax but will miss:
//! - Spread attributes (`{...props}`)
//! - Computed attribute names (rare in v0 output)
//! - Multi-line attr values spanning more than ~3 lines
//!
//! These edge cases are acceptable — we emit warnings conservatively and never
//! block generation on false positives (only `Error` severity items trigger a
//! mandatory user decision; `Warning` items are printed but auto-accepted).

#![allow(dead_code)] // Staged v0.dev integration; exported for future `vox validate` wiring

use regex::Regex;
use vox_codegen::web_ir::{
    BehaviorNode, DomNode, DomNodeId, WebIrDiagnostic, WebIrDiagnosticSeverity, WebIrModule,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate TSX source for a11y issues.  Returns diagnostics sorted by code.
///
/// Errors indicate unambiguous violations (missing `alt`, unlabelled
/// buttons).  Warnings indicate likely violations that may have off-screen
/// labelling the regex cannot see.
pub fn validate_tsx_a11y(tsx: &str) -> Vec<WebIrDiagnostic> {
    let module = tsx_to_web_ir_module(tsx);
    let mut out = Vec::new();
    vox_codegen::web_ir::validate_a11y::validate_a11y(&module, &mut out);
    out.sort_by(|a, b| a.code.cmp(&b.code));
    out
}

/// Format diagnostics for CLI display.  Returns `None` if the slice is empty.
pub fn format_diagnostics(diags: &[WebIrDiagnostic], component_name: &str) -> Option<String> {
    if diags.is_empty() {
        return None;
    }
    let mut buf = format!(
        "⚠️  {component_name}: {n} accessibility issue(s) detected in v0 output:\n",
        n = diags.len(),
    );
    for d in diags {
        let icon = match d.severity() {
            WebIrDiagnosticSeverity::Error => "❌",
            WebIrDiagnosticSeverity::Warning => "⚠️ ",
            WebIrDiagnosticSeverity::Info => "ℹ️ ",
        };
        buf.push_str(&format!("  {icon} [{}] {}\n", d.code, d.message));
    }
    buf.push_str(
        "\nFix these issues in the generated TSX before shipping, or add explicit aria attributes.",
    );
    Some(buf)
}

/// True if any diagnostic has [`WebIrDiagnosticSeverity::Error`] severity.
pub fn has_errors(diags: &[WebIrDiagnostic]) -> bool {
    diags
        .iter()
        .any(|d| d.severity() == WebIrDiagnosticSeverity::Error)
}

// ---------------------------------------------------------------------------
// TSX -> WebIrModule (tag-stack, heuristic)
// ---------------------------------------------------------------------------

/// Build a minimal [`WebIrModule`] from TSX source by processing JSX tags in
/// document order with a tag stack.
///
/// Only element types relevant to the a11y rules are extracted:
/// `img`, `button`, `a`, `input`, `div`, `span`, `section`, `article`,
/// `header`, `footer`, `main`, `nav`, `aside`, `li`, `td`, `th`, `summary`.
///
/// A tag stack tracks open/close pairs so that:
/// - Self-closing tags (`<button />`) never get spurious text children from
///   following siblings.
/// - Text content is attributed only to the element that actually encloses it,
///   not to a preceding sibling that happens to be within the 120-char lookahead
///   window.
/// - Nested relevant elements (e.g. `<button><span>text</span></button>`) are
///   linked as parent -> child in the arena so the a11y recursive walk finds
///   the accessible content through the child chain.
fn tsx_to_web_ir_module(tsx: &str) -> WebIrModule {
    // Opening / self-closing tag scanner.
    // Group 1 = tag name, Group 2 = raw attrs block, Group 3 = "/>" or ">".
    let re_open = Regex::new(
        r#"(?x)
        <                       # open bracket
        ([A-Za-z][A-Za-z0-9]*)  # 1: tag name
        (                        # 2: attrs block
          (?:
            \s+
            (?:
              [A-Za-z_][\w\-:]*  # attr key
              (?:\s*=\s*(?:
                "[^"]*"          # double-quoted value
                |'[^']*'         # single-quoted value
                |\{[^}]*\}       # JSX expression value
                |[^\s/>"'{]+     # bare value
              ))?
            )
          )*
        )
        \s*
        (/>|>)                  # 3: self-closing or open
        "#,
    )
    .expect("static JSX open-tag regex");

    // Closing tag scanner.
    let re_close = Regex::new(r"</([A-Za-z][A-Za-z0-9]*)>").expect("static JSX close-tag regex");

    // Attr extractor: key="val", key={'val'}, key={expr}, key (boolean).
    // Group 1 = name, 2 = double-quoted, 3 = single-quoted, 4 = JSX expr, 5 = bare.
    let re_attr = Regex::new(
        r#"(?x)
        \s+
        ([A-Za-z_][\w\-:]*)  # 1: attr name
        (?:\s*=\s*
          (?:
            "([^"]*)"          # 2: double-quoted value
            |'([^']*)'         # 3: single-quoted value
            |\{([^}]*)\}       # 4: JSX expr content
            |([^\s/>"'{]+)     # 5: bare value
          )
        )?
        "#,
    )
    .expect("static attr regex");

    // Elements checked for a11y; all others are tracked on the stack for
    // correct balancing but do not produce DomNodes.
    const RELEVANT: &[&str] = &[
        "img", "button", "a", "input", "div", "span", "section", "article", "header", "footer",
        "main", "nav", "aside", "li", "td", "th", "summary",
    ];

    // --- Collect parse events sorted by byte offset ---

    enum TagEvent {
        /// An opening (or self-closing) tag.
        Open {
            pos: usize,
            /// Byte offset immediately after the closing `>`.
            end: usize,
            tag: String,
            attrs: Vec<(String, String)>,
            self_closing: bool,
        },
        /// A closing tag.
        Close { pos: usize, tag: String },
    }

    let mut events: Vec<TagEvent> = Vec::new();

    for cap in re_open.captures_iter(tsx) {
        let pos = cap.get(0).unwrap().start();
        let end = cap.get(0).unwrap().end();
        let tag = cap.get(1).unwrap().as_str().to_ascii_lowercase();
        let attrs_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let self_closing = cap.get(3).map(|m| m.as_str()) == Some("/>");

        let mut attrs: Vec<(String, String)> = Vec::new();
        for a in re_attr.captures_iter(attrs_str) {
            let key = a.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            // Pick the first non-None value group (2,3,4,5); boolean attrs get empty string.
            let val = a
                .get(2)
                .or_else(|| a.get(3))
                .or_else(|| a.get(4))
                .or_else(|| a.get(5))
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            if !key.is_empty() {
                attrs.push((key, val));
            }
        }
        events.push(TagEvent::Open {
            pos,
            end,
            tag,
            attrs,
            self_closing,
        });
    }

    for cap in re_close.captures_iter(tsx) {
        let pos = cap.get(0).unwrap().start();
        let tag = cap.get(1).unwrap().as_str().to_ascii_lowercase();
        events.push(TagEvent::Close { pos, tag });
    }

    // Sort by byte position so we process tags in document order.
    events.sort_by_key(|e| match e {
        TagEvent::Open { pos, .. } | TagEvent::Close { pos, .. } => *pos,
    });

    // --- Process events with a tag stack ---

    let mut module = WebIrModule::default();
    let mut id_counter: u32 = 0;

    // Stack entry: (Option<arena_idx>, tag_name, open_end_byte)
    //   arena_idx = Some(i) when the element was added to the arena.
    //   arena_idx = None for non-relevant elements (tracked only for balancing).
    let mut stack: Vec<(Option<usize>, String, usize)> = Vec::new();

    for ev in events {
        match ev {
            TagEvent::Open {
                tag,
                attrs,
                self_closing,
                end,
                ..
            } => {
                let is_relevant = RELEVANT.contains(&tag.as_str());
                let arena_idx: Option<usize> = if is_relevant {
                    let elem_id = DomNodeId(id_counter);
                    id_counter += 1;
                    let idx = module.dom_nodes.len();
                    module.dom_nodes.push(DomNode::Element {
                        id: elem_id,
                        tag: tag.clone(),
                        attrs: attrs.clone(),
                        children: vec![],
                        span: None,
                    });
                    // Link to the nearest relevant ancestor already on the stack.
                    let parent_arena_idx: Option<usize> = stack
                        .iter()
                        .rev()
                        .find(|(i, _, _)| i.is_some())
                        .and_then(|(i, _, _)| *i);
                    if let Some(par) = parent_arena_idx {
                        if let Some(DomNode::Element { children, .. }) =
                            module.dom_nodes.get_mut(par)
                        {
                            children.push(elem_id);
                        }
                    }
                    // Emit BehaviorNode::EventHandler for keyboard event attrs so
                    // check_keyboard_handler can validate them.
                    for (key, val) in &attrs {
                        let event = if key.eq_ignore_ascii_case("onKeyDown") {
                            "keydown"
                        } else if key.eq_ignore_ascii_case("onKeyUp") {
                            "keyup"
                        } else if key.eq_ignore_ascii_case("onKeyPress") {
                            "keypress"
                        } else {
                            continue;
                        };
                        module.behavior_nodes.push(BehaviorNode::EventHandler {
                            target_dom: Some(elem_id),
                            event: event.to_string(),
                            handler: val.clone(),
                            span: None,
                        });
                    }
                    Some(idx)
                } else {
                    None
                };

                if !self_closing {
                    stack.push((arena_idx, tag, end));
                }
            }

            TagEvent::Close {
                tag,
                pos: close_pos,
            } => {
                // Find the innermost matching open tag and pop it.
                if let Some(stack_pos) = stack.iter().rposition(|(_, name, _)| *name == tag) {
                    let (arena_idx, _, open_end) = stack.remove(stack_pos);
                    if let Some(arena_idx) = arena_idx {
                        // Extract text content that belongs to this element's body
                        // (strips child element tags, counts JSX expressions as content).
                        let inner = tsx.get(open_end..close_pos).unwrap_or("");
                        let text = extract_element_text_content(inner);
                        if !text.is_empty() {
                            let text_pos = module.dom_nodes.len() as u32;
                            id_counter += 1;
                            module.dom_nodes.push(DomNode::Text {
                                content: text,
                                span: None,
                            });
                            if let Some(DomNode::Element { children, .. }) =
                                module.dom_nodes.get_mut(arena_idx)
                            {
                                children.push(DomNodeId(text_pos));
                            }
                        }
                    }
                }
            }
        }
    }

    module
}

/// Extract visible text / expression content from the inner body of a JSX element.
///
/// Strips child element tags (`<...>` and `</...>`) and replaces JSX expression
/// blocks (`{...}`) with a sentinel so expressions count as accessible content.
/// Returns the condensed result if it contains any non-whitespace characters,
/// otherwise an empty string.
fn extract_element_text_content(inner: &str) -> String {
    // Replace JSX expression blocks with a sentinel so they count as content.
    let re_expr = Regex::new(r"\{[^}]*\}").expect("static expr regex");
    let s = re_expr.replace_all(inner, " EXPR ");
    // Strip all JSX tags (opening, closing, self-closing).
    let re_tag = Regex::new(r"<[^>]*>").expect("static tag regex");
    let s = re_tag.replace_all(&s, " ");
    let trimmed = s.trim().to_string();
    if trimmed.chars().any(|c| !c.is_whitespace()) {
        trimmed
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vox_codegen::web_ir::WebIrDiagnosticSeverity;

    #[test]
    fn img_without_alt_is_caught() {
        let tsx = r#"
export function Card() {
  return <div><img src="/logo.png" /></div>;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.img_missing_alt"),
            "expected img.missing_alt, got: {diags:?}"
        );
    }

    #[test]
    fn img_with_alt_passes() {
        let tsx = r#"
export function Card() {
  return <img src="/logo.png" alt="Company logo" />;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.img_missing_alt"),
            "img with alt should pass"
        );
    }

    #[test]
    fn empty_button_is_caught() {
        let tsx = r#"
export function Btn() {
  return <button onClick={handleClick} />;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
            "expected button.missing_label, got: {diags:?}"
        );
    }

    #[test]
    fn button_with_text_passes() {
        let tsx = r#"
export function Btn() {
  return <button onClick={handleClick}>Submit</button>;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
            "button with text should pass, got: {diags:?}"
        );
    }

    #[test]
    fn anchor_without_href_warns() {
        let tsx = r#"
export function Nav() {
  return <a className="tab">Home</a>;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.anchor_missing_href"),
            "expected anchor.missing_href, got: {diags:?}"
        );
    }

    #[test]
    fn anchor_with_href_passes() {
        let tsx = r#"
export function Nav() {
  // vox-arch-check: allow abs-path
  return <a href="/home">Home</a>;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.anchor_missing_href"),
            "anchor with href should pass"
        );
    }

    #[test]
    fn input_without_label_warns() {
        let tsx = r#"
export function Form() {
  return <input type="text" placeholder="Search" />;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.input_missing_label"),
            "expected input.missing_label, got: {diags:?}"
        );
    }

    #[test]
    fn input_with_aria_label_passes() {
        let tsx = r#"
export function Form() {
  return <input type="text" aria-label="Search" />;
}
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.input_missing_label"),
            "input with aria-label should pass"
        );
    }

    #[test]
    fn has_errors_false_for_warnings_only() {
        let tsx = r#"<a className="link">Click</a>"#;
        let diags = validate_tsx_a11y(tsx);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity() == WebIrDiagnosticSeverity::Error)
            .collect();
        // Only anchor.missing_href which is Warning
        assert!(
            errors.is_empty(),
            "anchor.missing_href is Warning not Error"
        );
    }

    #[test]
    fn format_diagnostics_none_when_empty() {
        assert!(format_diagnostics(&[], "Widget").is_none());
    }

    #[test]
    fn format_diagnostics_includes_component_name() {
        let tsx = r#"<img src="/x.png" />"#;
        let diags = validate_tsx_a11y(tsx);
        let msg = format_diagnostics(&diags, "MyWidget").unwrap();
        assert!(msg.contains("MyWidget"), "should mention component name");
        assert!(msg.contains("web_ir_validate.a11y.img_missing_alt"));
    }

    #[test]
    fn self_closing_button_not_given_sibling_text() {
        // A self-closing <button /> should NOT absorb text from a following sibling.
        let tsx = r#"
<div>
  <button />
  <span>This text belongs to the div, not the button</span>
</div>
"#;
        let diags = validate_tsx_a11y(tsx);
        // The button has no accessible label — it's self-closing with no content.
        assert!(
            diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
            "self-closing button with no label should fail; got: {diags:?}"
        );
    }

    #[test]
    fn button_with_nested_span_text_passes() {
        // Text inside a nested <span> counts as the button's accessible content.
        let tsx = r#"
<button>
  <span>Click me</span>
</button>
"#;
        let diags = validate_tsx_a11y(tsx);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "web_ir_validate.a11y.interactive_missing_label"),
            "button with nested span text should pass; got: {diags:?}"
        );
    }
}
