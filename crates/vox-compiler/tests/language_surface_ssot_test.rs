//! Guardrails: LSP decorator docs must map to lexer `@` tokens (see `language_surface.rs`).

use vox_compiler::language_surface;

#[test]
fn lsp_decorator_spellings_exist_in_lexer_list() {
    for &(d, _) in language_surface::LSP_DECORATOR_DOCS {
        assert!(
            language_surface::LEXER_DECORATORS.contains(&d),
            "{d} is documented for LSP but missing from LEXER_DECORATORS — add to lexer `Token` or trim LSP list"
        );
    }
}
#[test]
fn retired_decorators_not_in_lsp_list() {
    // @component is retired; must not appear in LSP suggestions
    assert!(
        !language_surface::LEXER_DECORATORS.contains(&"@component"),
        "@component is retired and must not be in LEXER_DECORATORS"
    );
}

#[test]
fn decorator_feature_count_matches_lexer_at_tokens() {
    use vox_compiler::feature_matrix::DecoratorFeature;
    assert_eq!(
        DecoratorFeature::ALL.len(),
        language_surface::LEXER_AT_DECORATORS.len(),
        "feature_matrix decorators must mirror lexer At* decorator tokens"
    );
}

#[test]
fn decorator_feature_names_match_lexer_at_decorators() {
    use std::collections::BTreeSet;

    use vox_compiler::feature_matrix::DecoratorFeature;

    let matrix: BTreeSet<_> = DecoratorFeature::ALL
        .iter()
        .map(|&d| d.lexer_spelling())
        .collect();
    let lexer: BTreeSet<_> = language_surface::LEXER_AT_DECORATORS
        .iter()
        .copied()
        .collect();

    assert_eq!(
        matrix, lexer,
        "feature_matrix decorator spellings must match LEXER_AT_DECORATORS membership \
         (count-only checks miss same-length drift)"
    );
    assert!(
        language_surface::decorator_feature_lexer_parity_mismatch().is_none(),
        "parity helper should agree with set equality: {:?}",
        language_surface::decorator_feature_lexer_parity_mismatch()
    );
}

#[test]
fn deprecated_keywords_not_in_lexer_list() {
    // ret is deprecated; must not appear in main keyword list
    assert!(
        !language_surface::LEXER_KEYWORDS.contains(&"ret"),
        "ret is deprecated and must not be in LEXER_KEYWORDS"
    );
}
