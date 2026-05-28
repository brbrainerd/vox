//! Render a `ScaffoldInput` to a standalone `\documentclass{article}` LaTeX
//! source document.

use crate::manuscript::scaffold::{AuthorEntry, CitedFact, FigureEntry, ResultsRow, ScaffoldInput};

use super::escape::escape_latex;

/// Render a [`ScaffoldInput`] to LaTeX. The output is a complete,
/// `pdflatex`-compatible source document — no preamble surgery required.
///
/// Forbidden sections (Abstract, Introduction, Discussion, Significance,
/// Conclusion) emit LaTeX comments listing cited facts the human should
/// compose around, mirroring the markdown scaffolder's TODO blocks.
pub fn render_latex(input: &ScaffoldInput) -> String {
    let mut out = String::with_capacity(4096);
    write_preamble(&mut out, input);
    out.push_str("\n\\begin{document}\n\n");
    write_title_block(&mut out, input);
    write_abstract_block(&mut out);
    write_section_todo(&mut out, "Introduction", &input.cited_facts);
    write_methods(&mut out, input.methods_summary.as_deref());
    write_results(&mut out, &input.results_rows);
    write_figures(&mut out, &input.figures);
    write_limitations(&mut out, &input.limitations);
    write_section_todo(&mut out, "Discussion", &input.cited_facts);
    write_section_todo(&mut out, "Significance", &[]);
    write_section_todo(&mut out, "Conclusion", &[]);
    write_references(&mut out, &input.cited_facts);
    write_ai_disclosure(&mut out, input.ai_disclosure_markdown.as_deref());
    write_competing_interests(&mut out, input.competing_interests.as_deref());
    out.push_str("\n\\end{document}\n");
    out
}

fn write_preamble(out: &mut String, input: &ScaffoldInput) {
    out.push_str(
        "\\documentclass[11pt,a4paper]{article}\n\
         \\usepackage[utf8]{inputenc}\n\
         \\usepackage[T1]{fontenc}\n\
         \\usepackage{graphicx}\n\
         \\usepackage{hyperref}\n\
         \\usepackage{booktabs}\n\
         \\usepackage{longtable}\n\
         \\usepackage{geometry}\n\
         \\geometry{margin=1in}\n\
         \\hypersetup{colorlinks=true,linkcolor=blue,urlcolor=blue}\n",
    );
    out.push_str("\\title{");
    out.push_str(&escape_latex(&input.title_hint));
    out.push_str("}\n");
    out.push_str("\\author{");
    let authors_joined: Vec<String> = input
        .authors
        .iter()
        .map(|a| render_author_inline(a))
        .collect();
    if authors_joined.is_empty() {
        // Required by \maketitle; comment marks it as TODO.
        out.push_str("Anonymous \\thanks{TODO: replace with author list}");
    } else {
        out.push_str(&authors_joined.join(" \\and "));
    }
    out.push_str("}\n");
    out.push_str("\\date{}\n");
}

fn render_author_inline(a: &AuthorEntry) -> String {
    let mut s = escape_latex(&a.name);
    if let Some(orcid) = &a.orcid {
        s.push_str(&format!(" \\href{{{orcid}}}{{\\textsuperscript{{ORCID}}}}"));
    }
    s
}

fn write_title_block(out: &mut String, _input: &ScaffoldInput) {
    out.push_str("\\maketitle\n\n");
}

fn write_abstract_block(out: &mut String) {
    out.push_str(
        "\\begin{abstract}\n\
         % TODO(narrative): the worthiness rubric forbids auto-generating\n\
         % abstracts. Write 150-250 words summarizing motivation, methods,\n\
         % key results (each tied to a Results-section claim), and a single\n\
         % line on implications. Do not introduce new claims here.\n\
         \\end{abstract}\n\n",
    );
}

fn write_section_todo(out: &mut String, section: &str, facts: &[CitedFact]) {
    out.push_str("\\section{");
    out.push_str(&escape_latex(section));
    out.push_str("}\n");
    out.push_str(
        "% TODO(narrative): write the ",
    );
    out.push_str(section);
    out.push_str(" yourself.\n");
    out.push_str(
        "% The worthiness rubric forbids auto-generating novelty / significance\n\
         % / causal-mechanism prose. Use the cited facts below to compose this section.\n",
    );
    if facts.is_empty() {
        out.push_str("% (No cited facts supplied for this section.)\n\n");
    } else {
        out.push_str("% Cited facts (DOI/URL preserved):\n");
        for f in facts {
            out.push_str("%   - ");
            out.push_str(&escape_latex(&f.claim_text));
            out.push_str(" [");
            out.push_str(&escape_latex(&f.citation_key));
            out.push_str("] ");
            out.push_str(&f.doi_or_url);
            out.push('\n');
        }
        out.push('\n');
    }
}

fn write_methods(out: &mut String, summary: Option<&str>) {
    out.push_str("\\section{Methods}\n");
    match summary {
        Some(s) => {
            out.push_str(&markdown_to_latex(s));
            out.push_str("\n\n");
        }
        None => {
            out.push_str(
                "% TODO(methods): no operator-approved methods summary was supplied.\n\
                 % The worthiness rubric requires Methods to be human-authored\n\
                 % and machine-verifiable against the RO-Crate mainEntity declaration.\n\n",
            );
        }
    }
}

fn write_results(out: &mut String, rows: &[ResultsRow]) {
    out.push_str("\\section{Results}\n");
    if rows.is_empty() {
        out.push_str(
            "% TODO(results): no verified claims supplied. Run the claim extractor\n\
             % and worthiness preflight before scaffolding again.\n\n",
        );
        return;
    }
    out.push_str(
        "\\begin{longtable}{p{0.42\\textwidth} l p{0.18\\textwidth} p{0.18\\textwidth}}\n\
         \\toprule\n\
         Claim & Verdict & CI95 & Trusty URI \\\\\n\
         \\midrule\n",
    );
    for r in rows {
        let ci = match r.ci95 {
            Some((lo, hi)) => format!("[{lo:.3}, {hi:.3}]"),
            None => "---".to_string(),
        };
        out.push_str(&escape_latex(&r.claim_text));
        out.push_str(" & ");
        out.push_str(&escape_latex(&r.verdict));
        out.push_str(" & ");
        out.push_str(&escape_latex(&ci));
        out.push_str(" & ");
        out.push_str(&format!("\\url{{{}}}", r.trusty_uri.replace('}', "")));
        out.push_str(" \\\\\n");
    }
    out.push_str("\\bottomrule\n\\end{longtable}\n\n");
}

fn write_figures(out: &mut String, figures: &[FigureEntry]) {
    out.push_str("\\section{Figures}\n");
    if figures.is_empty() {
        out.push_str(
            "% No figures supplied. Add `figures` entries to the RO-Crate\n\
             % mainEntity.figures and re-render to surface them here.\n\n",
        );
        return;
    }
    for (idx, f) in figures.iter().enumerate() {
        let n = idx + 1;
        out.push_str("\\begin{figure}[h]\n\\centering\n");
        out.push_str(&format!(
            "\\includegraphics[width=0.8\\textwidth]{{{}}}\n",
            f.path
        ));
        out.push_str(
            "% TODO(figure-caption): write a one-line factual caption.\n\
             % The worthiness rubric forbids auto-generating captions for\n\
             % measurement-implying figures; the renderer leaves this slot empty.\n",
        );
        if let Some(hint) = &f.caption_hint {
            out.push_str("% Caption hint (machine-suggested; not authoritative): ");
            out.push_str(hint);
            out.push('\n');
        }
        out.push_str(&format!("\\caption{{Figure {n}.}}\n"));
        out.push_str(&format!("\\label{{fig:{n}}}\n"));
        out.push_str("\\end{figure}\n");
        // Provenance footer as a comment for reviewer reference.
        out.push_str(&format!(
            "% Provenance: path={} sha3_256={} source_script={}\n\n",
            f.path, f.sha3_256_hex, f.source_script
        ));
    }
}

fn write_limitations(out: &mut String, limitations: &[String]) {
    out.push_str("\\section{Limitations}\n");
    if limitations.is_empty() {
        out.push_str(
            "% TODO(limitations): preflight surfaced no operator-flagged limitations.\n\
             % Either confirm none apply (and remove this block) or add the ones\n\
             % reviewers will ask about.\n\n",
        );
        return;
    }
    out.push_str("\\begin{itemize}\n");
    for l in limitations {
        out.push_str("  \\item ");
        out.push_str(&escape_latex(l));
        out.push('\n');
    }
    out.push_str("\\end{itemize}\n\n");
}

fn write_references(out: &mut String, facts: &[CitedFact]) {
    out.push_str("\\section{References}\n");
    if facts.is_empty() {
        out.push_str(
            "% TODO(references): no verified prior-art citations supplied.\n\
             % Run the novelty / SPECTER2 retrieval before submission.\n\n",
        );
        return;
    }
    out.push_str("\\begin{thebibliography}{99}\n");
    for f in facts {
        out.push_str(&format!(
            "\\bibitem{{{}}} {} \\url{{{}}}\n",
            escape_latex_label(&f.citation_key),
            escape_latex(&f.claim_text),
            f.doi_or_url
        ));
    }
    out.push_str("\\end{thebibliography}\n\n");
}

fn write_ai_disclosure(out: &mut String, block: Option<&str>) {
    out.push_str("\\section*{AI Tool Disclosure}\n");
    if let Some(b) = block {
        out.push_str(&markdown_to_latex(b));
        out.push_str("\n\n");
    } else {
        out.push_str(
            "% TODO(ai_disclosure): no AiDisclosureBlock supplied. Required\n\
             % for any venue with an AI-disclosure policy.\n\n",
        );
    }
}

fn write_competing_interests(out: &mut String, ci: Option<&str>) {
    out.push_str("\\section*{Competing Interests}\n");
    match ci {
        Some(s) => {
            out.push_str(&escape_latex(s));
            out.push_str("\n\n");
        }
        None => {
            out.push_str(
                "% TODO(competing_interests): rubric requires an explicit\n\
                 % statement (use \"The authors declare no competing interests.\"\n\
                 % when applicable).\n\n",
            );
        }
    }
}

/// LaTeX `\bibitem` labels can't contain spaces, braces, or special chars.
/// Collapse non-alphanumeric to `-`.
fn escape_latex_label(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Minimal markdown→LaTeX for short user-supplied blocks (methods summary,
/// AI-disclosure body). Handles paragraphs, code spans, and inline emphasis;
/// drops other constructs gracefully (text-only passthrough with escape).
pub(crate) fn markdown_to_latex(md: &str) -> String {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let parser = Parser::new(md);
    let mut out = String::with_capacity(md.len() + 64);
    let mut in_code = false;
    for event in parser {
        match event {
            Event::Text(t) => {
                if in_code {
                    out.push_str(&t);
                } else {
                    out.push_str(&escape_latex(&t));
                }
            }
            Event::Code(c) => {
                out.push_str("\\texttt{");
                out.push_str(&escape_latex(&c));
                out.push('}');
            }
            Event::Start(Tag::Emphasis) => out.push_str("\\emph{"),
            Event::End(TagEnd::Emphasis) => out.push('}'),
            Event::Start(Tag::Strong) => out.push_str("\\textbf{"),
            Event::End(TagEnd::Strong) => out.push('}'),
            Event::Start(Tag::CodeBlock(_)) => {
                in_code = true;
                out.push_str("\\begin{verbatim}\n");
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                out.push_str("\\end{verbatim}\n");
            }
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::End(TagEnd::Paragraph) => out.push_str("\n\n"),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manuscript::scaffold::{AuthorEntry, CitedFact, FigureEntry, ResultsRow};

    fn sample() -> ScaffoldInput {
        ScaffoldInput {
            title_hint: "Fast Foo".into(),
            authors: vec![AuthorEntry {
                name: "Alice Test".into(),
                orcid: Some("https://orcid.org/0000-0002-1825-0097".into()),
                affiliation_ror: None,
            }],
            results_rows: vec![ResultsRow {
                claim_text: "p95 latency dropped 23%".into(),
                trusty_uri: "https://np.example/RA1234".into(),
                evidence_source: "ExecTimeRecord".into(),
                verdict: "Supported".into(),
                ci95: Some((0.18, 0.27)),
            }],
            cited_facts: vec![CitedFact {
                claim_text: "Prior linear scan".into(),
                citation_key: "Doe 2024".into(),
                doi_or_url: "https://doi.org/10.0/x".into(),
            }],
            methods_summary: Some("We ran *three* configurations.".into()),
            limitations: vec!["x86_64 only".into()],
            ai_disclosure_markdown: Some("Scaffolded by `vox-manuscript-latex`.".into()),
            competing_interests: Some("None declared.".into()),
            figures: vec![FigureEntry {
                path: "figures/fig-01-p95.svg".into(),
                sha3_256_hex: "abcd1234".into(),
                source_script: "scripts/plot.py".into(),
                caption_hint: Some("p95 drop".into()),
            }],
        }
    }

    #[test]
    fn rendered_document_starts_with_documentclass_and_ends_with_end_document() {
        let tex = render_latex(&sample());
        assert!(tex.starts_with("\\documentclass[11pt,a4paper]{article}"));
        assert!(tex.trim_end().ends_with("\\end{document}"));
    }

    #[test]
    fn title_and_author_are_escaped_and_present() {
        let tex = render_latex(&sample());
        assert!(tex.contains("\\title{Fast Foo}"));
        assert!(tex.contains("Alice Test"));
        assert!(tex.contains("\\href{https://orcid.org/0000-0002-1825-0097}"));
    }

    #[test]
    fn forbidden_sections_appear_as_section_with_todo_comments_only() {
        let tex = render_latex(&sample());
        for section in ["Introduction", "Discussion", "Significance", "Conclusion"] {
            let heading = format!("\\section{{{section}}}");
            let idx = tex
                .find(&heading)
                .unwrap_or_else(|| panic!("missing section heading {section}"));
            // Body up to next \section or \end{document}.
            let after = &tex[idx + heading.len()..];
            let next_section = after.find("\\section").unwrap_or(after.len());
            let body = &after[..next_section];
            // Body must contain "% TODO(narrative)" and no \textbf or \emph
            // (no auto-emitted prose).
            assert!(
                body.contains("% TODO(narrative)"),
                "section {section} missing TODO narrative comment"
            );
            assert!(
                !body.contains("\\textbf{") && !body.contains("\\emph{"),
                "section {section} contains auto-emitted prose"
            );
        }
    }

    #[test]
    fn results_table_uses_longtable_with_each_claim_row() {
        let tex = render_latex(&sample());
        assert!(tex.contains("\\begin{longtable}"));
        assert!(tex.contains("p95 latency dropped 23\\%")); // % escaped
        assert!(tex.contains("Supported"));
        assert!(tex.contains("[0.180, 0.270]"));
        // \url for Trusty URI.
        assert!(tex.contains("\\url{https://np.example/RA1234}"));
    }

    #[test]
    fn figure_block_includes_graphics_and_provenance_comment() {
        let tex = render_latex(&sample());
        assert!(tex.contains("\\includegraphics[width=0.8\\textwidth]{figures/fig-01-p95.svg}"));
        assert!(tex.contains("sha3_256=abcd1234"));
        assert!(tex.contains("source_script=scripts/plot.py"));
        assert!(tex.contains("% TODO(figure-caption)"));
    }

    #[test]
    fn references_emit_thebibliography_block() {
        let tex = render_latex(&sample());
        assert!(tex.contains("\\begin{thebibliography}"));
        assert!(tex.contains("\\bibitem{Doe-2024}")); // space → -
        assert!(tex.contains("Prior linear scan"));
        assert!(tex.contains("\\url{https://doi.org/10.0/x}"));
    }

    #[test]
    fn missing_methods_emits_todo_comment() {
        let mut input = sample();
        input.methods_summary = None;
        let tex = render_latex(&input);
        assert!(tex.contains("% TODO(methods)"));
    }

    #[test]
    fn empty_authors_emits_anonymous_placeholder() {
        let mut input = sample();
        input.authors.clear();
        let tex = render_latex(&input);
        assert!(tex.contains("Anonymous"));
        assert!(tex.contains("TODO: replace with author list"));
    }

    #[test]
    fn special_chars_in_title_are_escaped_to_safe_latex() {
        let mut input = sample();
        input.title_hint = "100% Better $cost_model".into();
        let tex = render_latex(&input);
        // %, $, _ all escaped.
        assert!(tex.contains("100\\% Better \\$cost\\_model"));
    }

    #[test]
    fn markdown_to_latex_translates_emphasis_strong_and_code() {
        let tex = markdown_to_latex("This is *emph* and **bold** and `code`.");
        assert!(tex.contains("\\emph{emph}"));
        assert!(tex.contains("\\textbf{bold}"));
        assert!(tex.contains("\\texttt{code}"));
    }

    #[test]
    fn markdown_to_latex_preserves_paragraph_breaks() {
        let tex = markdown_to_latex("First.\n\nSecond.");
        // pulldown-cmark emits two paragraphs; we render a blank line between.
        let first_idx = tex.find("First.").unwrap();
        let second_idx = tex.find("Second.").unwrap();
        assert!(first_idx < second_idx);
        assert!(&tex[first_idx..second_idx].contains("\n\n"));
    }

    #[test]
    fn markdown_code_block_uses_verbatim_environment() {
        let tex = markdown_to_latex("```\nfn x() { 42 }\n```");
        assert!(tex.contains("\\begin{verbatim}"));
        assert!(tex.contains("fn x() { 42 }"));
        assert!(tex.contains("\\end{verbatim}"));
    }
}
