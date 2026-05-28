use crate::extractor::LanguageExtractor;
use crate::features::{ExtractedFeatures, LiteralContext, LiteralLoc, Loc, NumericLoc};
use anyhow::Result;
use std::path::Path;
use vox_code_audit::rules::Language;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::lexer::token::Token;

pub struct VoxExtractor;

impl LanguageExtractor for VoxExtractor {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures> {
        let mut features = ExtractedFeatures::new(path.to_path_buf(), Language::Vox);
        features.crate_name = crate::extractor::crate_name_from_path(path);
        features.allowed_lines = crate::extractor::parse_drift_allow_comments(content);

        let tokens = lex(content);
        for spanned in &tokens {
            match &spanned.token {
                Token::StringLit(s) | Token::SingleStringLit(s) => {
                    let line = byte_offset_to_line(content, spanned.span.start);
                    features.string_literals.push(LiteralLoc {
                        value: s.clone(),
                        loc: Loc { line, col: 0 },
                        ctx: LiteralContext::Code,
                    });
                }
                Token::DecLit(s) => {
                    if let Ok(v) = s.parse::<f64>() {
                        let line = byte_offset_to_line(content, spanned.span.start);
                        features.numeric_literals.push(NumericLoc {
                            value: v,
                            unit: None,
                            loc: Loc { line, col: 0 },
                            in_const: false,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(features)
    }
}

fn byte_offset_to_line(src: &str, offset: usize) -> usize {
    src[..offset.min(src.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &str) -> ExtractedFeatures {
        VoxExtractor.extract(Path::new("test.vox"), src).unwrap()
    }

    #[test]
    fn extracts_string_literal_vox() {
        let f = extract(r#"let x = "hello world""#);
        assert!(f.string_literals.iter().any(|l| l.value == "hello world"));
    }
}
