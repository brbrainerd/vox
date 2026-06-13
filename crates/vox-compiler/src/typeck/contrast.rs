//! Compile-time contrast-ratio validator for design-token color pairs.
//!
//! Enforces the WCAG 2.1 AA minimum of 4.5:1 for normal text (3:1 for large
//! text / UI components — we apply the stricter 4.5 for now). A token pair
//! that violates the ratio emits `vox/tokens/contrast-violation` at compile
//! time, making the design contract structurally unrepresentable per P0.
//!
//! Hex parsing handles `#RRGGBB` and `#RGB` forms. Malformed hex values
//! emit `vox/tokens/invalid-hex` before contrast is checked.

use crate::ast::span::Span;
use crate::hir::nodes::tokens::{HirColorToken, HirTokensDecl};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// Minimum WCAG 2.1 AA contrast ratio for normal text (4.5:1).
const MIN_CONTRAST_RATIO: f64 = 4.5;

/// Validate all token declarations in a module.
///
/// Returns diagnostics for:
/// - `vox/tokens/invalid-hex` — malformed color value.
/// - `vox/tokens/contrast-violation` — light/dark pair fails WCAG 4.5:1 AA.
/// - `vox/tokens/raw-color` — reserved for use in component emit validation
///   (not checked here; component emit is responsible for that gate).
pub fn check_tokens(decls: &[HirTokensDecl]) -> Vec<Diagnostic> {
    let mut diags = vec![];
    for decl in decls {
        // First validate every color's hex is parseable (both variants).
        for tok in &decl.colors {
            if parse_hex_luminance(&tok.light).is_err() {
                diags.push(invalid_hex_diag(&tok.name, &tok.light, tok.span));
            }
            if parse_hex_luminance(&tok.dark).is_err() {
                diags.push(invalid_hex_diag(&tok.name, &tok.dark, tok.span));
            }
        }
        // Then check declared fg-on-bg pairs (`on:`). A token with no `on:` makes
        // no contrast claim — its light/dark variants are never rendered together,
        // so comparing them is meaningless (the historical bug we are removing).
        for tok in &decl.colors {
            check_color_pair(tok, &decl.colors, &mut diags);
        }
    }
    diags
}

/// For a `fg on: bg` pairing, check the WCAG ratio in BOTH variants
/// (light fg on light bg, dark fg on dark bg). Either failing emits a violation.
fn check_color_pair(fg: &HirColorToken, all: &[HirColorToken], diags: &mut Vec<Diagnostic>) {
    let Some(bg_name) = &fg.pair_bg else {
        return;
    };
    let Some(bg) = all.iter().find(|c| &c.name == bg_name) else {
        diags.push(Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "Token `{}` declares `on: {bg_name}`, but no color token named `{bg_name}` is defined in this @tokens block.",
                fg.name
            ),
            span: fg.span,
            code: Some("vox/tokens/dangling-pair".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec![format!("Define `color {bg_name} light: \"#…\" dark: \"#…\"` or fix the `on:` name.")],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("a defined background token".into()),
            found_type: Some(bg_name.clone()),
            context: None,
            ast_node_kind: None,
        });
        return;
    };

    for (variant, fg_hex, bg_hex) in [
        ("light", &fg.light, &bg.light),
        ("dark", &fg.dark, &bg.dark),
    ] {
        let (Ok(fg_lum), Ok(bg_lum)) = (parse_hex_luminance(fg_hex), parse_hex_luminance(bg_hex))
        else {
            continue; // hex validity already reported above.
        };
        let ratio = contrast_ratio(fg_lum, bg_lum);
        if ratio < MIN_CONTRAST_RATIO {
            diags.push(Diagnostic {
                severity: TypeckSeverity::Error,
                message: format!(
                    "Token `{}` on `{}` ({variant}): foreground {fg_hex} on background {bg_hex} has contrast {ratio:.2}:1, below the WCAG AA minimum of {MIN_CONTRAST_RATIO:.1}:1.",
                    fg.name, bg.name
                ),
                span: fg.span,
                code: Some("vox/tokens/contrast-violation".into()),
                category: DiagnosticCategory::Typecheck,
                suggestions: vec![format!(
                    "Darken/lighten `{}` ({variant}) or its `{}` background to reach {MIN_CONTRAST_RATIO:.1}:1. Current: {ratio:.2}:1.",
                    fg.name, bg.name
                )],
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                expected_type: Some(format!("contrast >= {MIN_CONTRAST_RATIO}")),
                found_type: Some(format!("{ratio:.2}")),
                context: None,
                ast_node_kind: None,
            });
        }
    }
}

fn invalid_hex_diag(name: &str, value: &str, span: Span) -> Diagnostic {
    Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "Token `{name}`: invalid hex color `{value}`. Expected `#RRGGBB` or `#RGB`."
        ),
        span,
        code: Some("vox/tokens/invalid-hex".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec!["Use a 6-digit hex color, e.g. `#3B82F6`.".into()],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some("#RRGGBB".into()),
        found_type: Some(value.to_string()),
        context: None,
        ast_node_kind: None,
    }
}

/// Parse a hex color string (`#RRGGBB` or `#RGB`) into its WCAG relative luminance.
///
/// Returns `Err(())` for any parse failure; callers treat hex-validity as a
/// boolean signal rather than a structured error, so a unit error is the
/// minimum-allocation choice here.
#[allow(clippy::result_unit_err)]
pub fn parse_hex_luminance(hex: &str) -> Result<f64, ()> {
    let hex = hex.trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ())?;
            (r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).map_err(|_| ())?;
            let g = u8::from_str_radix(&hex[1..2], 16).map_err(|_| ())?;
            let b = u8::from_str_radix(&hex[2..3], 16).map_err(|_| ())?;
            (r * 17, g * 17, b * 17)
        }
        _ => return Err(()),
    };
    Ok(relative_luminance(r, g, b))
}

/// WCAG 2.1 relative luminance from sRGB components.
fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn linearize(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// WCAG 2.1 contrast ratio from two relative luminance values.
pub fn contrast_ratio(l1: f64, l2: f64) -> f64 {
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_passes() {
        let l_white = parse_hex_luminance("#FFFFFF").unwrap();
        let l_black = parse_hex_luminance("#000000").unwrap();
        let ratio = contrast_ratio(l_white, l_black);
        assert!((ratio - 21.0).abs() < 0.1, "expected ~21:1, got {ratio}");
        assert!(ratio >= MIN_CONTRAST_RATIO);
    }

    #[test]
    fn similar_grays_fail() {
        let l1 = parse_hex_luminance("#888888").unwrap();
        let l2 = parse_hex_luminance("#999999").unwrap();
        let ratio = contrast_ratio(l1, l2);
        assert!(
            ratio < MIN_CONTRAST_RATIO,
            "similar grays should fail: {ratio}"
        );
    }

    #[test]
    fn invalid_hex_returns_err() {
        assert!(parse_hex_luminance("not-a-color").is_err());
        assert!(parse_hex_luminance("#GGGGGG").is_err());
        assert!(parse_hex_luminance("#12").is_err());
    }

    #[test]
    fn shorthand_hex_parses() {
        let l = parse_hex_luminance("#FFF").unwrap();
        let l2 = parse_hex_luminance("#FFFFFF").unwrap();
        assert!((l - l2).abs() < 1e-9, "shorthand should match full form");
    }

    fn span() -> crate::ast::span::Span {
        crate::ast::span::Span { start: 0, end: 0 }
    }

    fn color(name: &str, light: &str, dark: &str, on: Option<&str>) -> HirColorToken {
        HirColorToken {
            name: name.into(),
            light: light.into(),
            dark: dark.into(),
            pair_bg: on.map(str::to_string),
            span: span(),
        }
    }

    fn decl(colors: Vec<HirColorToken>) -> HirTokensDecl {
        HirTokensDecl {
            span: span(),
            colors,
            spacing: vec![],
            radius: vec![],
            shadows: vec![],
            fonts: vec![],
        }
    }

    #[test]
    fn paired_fg_on_bg_below_aa_emits_violation() {
        // gray-300 (#d1d5db) text on white refuses; ~1.46:1 in light mode.
        let d = decl(vec![
            color("surface_page", "#ffffff", "#1a1a1a", None),
            color("text_muted", "#d1d5db", "#6b7280", Some("surface_page")),
        ]);
        let diags = check_tokens(&[d]);
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some("vox/tokens/contrast-violation")),
            "gray-on-white pair must violate: {diags:?}"
        );
    }

    #[test]
    fn unpaired_token_with_close_variants_is_fine() {
        // Near-identical light/dark variants are NOT a contrast claim when the
        // token has no `on:` pairing — this is the historical bug we removed.
        let d = decl(vec![color("brand", "#ffffff", "#f8f8f8", None)]);
        let diags = check_tokens(&[d]);
        assert!(
            diags
                .iter()
                .all(|d| d.code.as_deref() != Some("vox/tokens/contrast-violation")),
            "unpaired near-identical variants must not be flagged: {diags:?}"
        );
    }

    #[test]
    fn valid_paired_contrast_emits_no_violation() {
        let d = decl(vec![
            color("surface_page", "#ffffff", "#1a1a1a", None),
            color("text_body", "#111111", "#eeeeee", Some("surface_page")),
        ]);
        let diags = check_tokens(&[d]);
        assert!(
            diags.is_empty(),
            "high-contrast pair should emit nothing: {diags:?}"
        );
    }

    #[test]
    fn dangling_pair_reference_is_reported() {
        let d = decl(vec![color("text", "#111", "#eee", Some("nonexistent"))]);
        let diags = check_tokens(&[d]);
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some("vox/tokens/dangling-pair")),
            "dangling on: must be reported: {diags:?}"
        );
    }
}
