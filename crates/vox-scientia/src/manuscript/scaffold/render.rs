//! IMRaD markdown rendering.

use super::safe_slots::ForbiddenSection;
use super::section_tree::{
    AuthorEntry, CitedFact, FigureEntry, ResultsRow, ScaffoldInput,
};

/// Render a [`ScaffoldInput`] to a complete IMRaD markdown document.
///
/// Section order: Title, Authors, Abstract (TODO), Introduction (TODO),
/// Methods, Results, Limitations, Discussion (TODO), Significance (TODO),
/// Conclusion (TODO), References, AI Disclosure, Competing Interests.
///
/// Forbidden sections (Abstract, Introduction, Discussion, Significance,
/// Conclusion) emit explicit `<!-- TODO(narrative): -->` blocks listing the
/// cited facts the human should compose around. The rendered markdown is
/// always a valid input for downstream pandoc / preflight pipelines.
pub fn render_imrad(input: &ScaffoldInput) -> String {
    let mut out = String::new();
    write_title(&mut out, &input.title_hint);
    write_authors(&mut out, &input.authors);
    write_abstract_todo(&mut out);
    write_forbidden_section_with_facts(&mut out, ForbiddenSection::Introduction, &input.cited_facts);
    write_methods(&mut out, input.methods_summary.as_deref());
    write_results(&mut out, &input.results_rows);
    write_figures(&mut out, &input.figures);
    write_limitations(&mut out, &input.limitations);
    write_forbidden_section_with_facts(&mut out, ForbiddenSection::Discussion, &input.cited_facts);
    write_forbidden_section_with_facts(&mut out, ForbiddenSection::Significance, &[]);
    write_forbidden_section_with_facts(&mut out, ForbiddenSection::Conclusion, &[]);
    write_references(&mut out, &input.cited_facts);
    write_ai_disclosure(&mut out, input.ai_disclosure_markdown.as_deref());
    write_competing_interests(&mut out, input.competing_interests.as_deref());
    out
}

fn write_title(out: &mut String, title: &str) {
    out.push_str("# ");
    out.push_str(title);
    out.push_str("\n\n");
    out.push_str(
        "<!-- machine_suggested: title lifted from FindingCandidate.title_hint; \
         requires_human_review before submission. -->\n\n",
    );
}

fn write_authors(out: &mut String, authors: &[AuthorEntry]) {
    out.push_str("## Authors\n\n");
    if authors.is_empty() {
        out.push_str(
            "<!-- TODO(author): no authors supplied via ScaffoldInput. \
             Add at least one author with ORCID before submission. -->\n\n",
        );
        return;
    }
    for a in authors {
        out.push_str("- ");
        out.push_str(&a.name);
        if let Some(orcid) = &a.orcid {
            out.push_str(" ([ORCID](");
            out.push_str(orcid);
            out.push_str("))");
        }
        if let Some(ror) = &a.affiliation_ror {
            out.push_str(" ([ROR](");
            out.push_str(ror);
            out.push_str("))");
        }
        out.push('\n');
    }
    out.push('\n');
}

fn write_abstract_todo(out: &mut String) {
    out.push_str("## Abstract\n\n");
    out.push_str(
        "<!-- TODO(narrative): the worthiness rubric forbids auto-generating \
         abstracts. Write 150-250 words summarizing motivation, methods, \
         key results (each tied to a Results-section claim), and a single \
         line on implications. Do not introduce new claims here. -->\n\n",
    );
}

fn write_forbidden_section_with_facts(
    out: &mut String,
    section: ForbiddenSection,
    facts: &[CitedFact],
) {
    out.push_str("## ");
    out.push_str(section.as_title());
    out.push_str("\n\n");
    out.push_str("<!-- TODO(narrative): write the ");
    out.push_str(section.as_title());
    out.push_str(
        " yourself.\n\n     \
         The worthiness rubric forbids auto-generating novelty / significance \
         / causal-mechanism prose. Use the cited facts listed below to \
         compose this section.\n",
    );
    if facts.is_empty() {
        out.push_str("\n     (No cited facts supplied for this section.)\n");
    } else {
        out.push_str("\n     Cited facts (Trusty / DOI provenance preserved):\n");
        for f in facts {
            out.push_str("     - ");
            out.push_str(&f.claim_text);
            out.push_str(" [");
            out.push_str(&f.citation_key);
            out.push_str("](");
            out.push_str(&f.doi_or_url);
            out.push_str(")\n");
        }
    }
    out.push_str("-->\n\n");
}

fn write_methods(out: &mut String, summary: Option<&str>) {
    out.push_str("## Methods\n\n");
    if let Some(s) = summary {
        out.push_str(s);
        out.push_str("\n\n");
    } else {
        out.push_str(
            "<!-- TODO(methods): no operator-approved methods summary was \
             supplied. The worthiness rubric requires Methods to be human-\
             authored and machine-verifiable against the RO-Crate \
             mainEntity declaration. -->\n\n",
        );
    }
}

fn write_results(out: &mut String, rows: &[ResultsRow]) {
    out.push_str("## Results\n\n");
    if rows.is_empty() {
        out.push_str(
            "<!-- TODO(results): no verified claims supplied. Run the \
             claim extractor and worthiness preflight before scaffolding \
             again. -->\n\n",
        );
        return;
    }
    out.push_str("| Claim | Verdict | CI95 | Evidence | Trusty URI |\n");
    out.push_str("|-------|---------|------|----------|------------|\n");
    for r in rows {
        let ci = match r.ci95 {
            Some((lo, hi)) => format!("[{lo:.3}, {hi:.3}]"),
            None => "—".into(),
        };
        out.push_str("| ");
        out.push_str(&escape_pipe(&r.claim_text));
        out.push_str(" | ");
        out.push_str(&r.verdict);
        out.push_str(" | ");
        out.push_str(&ci);
        out.push_str(" | ");
        out.push_str(&escape_pipe(&r.evidence_source));
        out.push_str(" | [");
        // Show short fingerprint of Trusty URI as the visible link text.
        let short: String = r.trusty_uri.chars().rev().take(8).collect::<String>();
        let short: String = short.chars().rev().collect();
        out.push_str(&short);
        out.push_str("](");
        out.push_str(&r.trusty_uri);
        out.push_str(") |\n");
    }
    out.push('\n');
}

fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn write_figures(out: &mut String, figures: &[FigureEntry]) {
    out.push_str("## Figures\n\n");
    if figures.is_empty() {
        out.push_str(
            "<!-- No figures supplied. Add `figures` entries to the RO-Crate \
             `mainEntity.figures` and re-scaffold to surface them here. -->\n\n",
        );
        return;
    }
    for (idx, f) in figures.iter().enumerate() {
        let n = idx + 1;
        // Image embed using the figure path. The renderer cannot generate
        // a caption (rubric forbids auto-captioning measurement-implying
        // figures), so caption is a TODO block.
        out.push_str("### Figure ");
        out.push_str(&n.to_string());
        out.push_str("\n\n");
        out.push_str("![Figure ");
        out.push_str(&n.to_string());
        out.push_str("](");
        out.push_str(&f.path);
        out.push_str(")\n\n");
        out.push_str(
            "<!-- TODO(figure-caption): write a one-line factual caption.\n     \
             The worthiness rubric forbids auto-generating captions for \
             measurement-implying figures; the renderer leaves this slot \
             empty for the human author.\n",
        );
        if let Some(hint) = &f.caption_hint {
            out.push_str("\n     Caption hint (machine-suggested; not authoritative): ");
            out.push_str(hint);
            out.push('\n');
        }
        out.push_str("-->\n\n");
        // Provenance footer — replay-eligibility breadcrumbs.
        out.push_str("**Provenance.** Path: `");
        out.push_str(&f.path);
        out.push_str("` · SHA3-256: `");
        out.push_str(&f.sha3_256_hex);
        out.push_str("` · Source script: `");
        out.push_str(&f.source_script);
        out.push_str("`.\n\n");
    }
}

fn write_limitations(out: &mut String, limitations: &[String]) {
    out.push_str("## Limitations\n\n");
    if limitations.is_empty() {
        out.push_str(
            "<!-- TODO(limitations): preflight surfaced no operator-flagged \
             limitations. Either confirm none apply (and remove this block) \
             or add the ones reviewers will ask about. -->\n\n",
        );
        return;
    }
    for l in limitations {
        out.push_str("- ");
        out.push_str(l);
        out.push('\n');
    }
    out.push('\n');
}

fn write_references(out: &mut String, facts: &[CitedFact]) {
    out.push_str("## References\n\n");
    if facts.is_empty() {
        out.push_str(
            "<!-- TODO(references): no verified prior-art citations supplied. \
             Run the novelty / SPECTER2 retrieval before submission. -->\n\n",
        );
        return;
    }
    for f in facts {
        out.push_str("- [");
        out.push_str(&f.citation_key);
        out.push_str("] ");
        out.push_str(&f.claim_text);
        out.push_str(" — <");
        out.push_str(&f.doi_or_url);
        out.push_str(">\n");
    }
    out.push('\n');
}

fn write_ai_disclosure(out: &mut String, block: Option<&str>) {
    out.push_str("## AI Tool Disclosure\n\n");
    if let Some(b) = block {
        out.push_str(b);
        out.push_str("\n\n");
    } else {
        out.push_str(
            "<!-- TODO(ai_disclosure): no AiDisclosureBlock supplied. \
             Required for any venue with an AI-disclosure policy. -->\n\n",
        );
    }
}

fn write_competing_interests(out: &mut String, ci: Option<&str>) {
    out.push_str("## Competing Interests\n\n");
    if let Some(s) = ci {
        out.push_str(s);
        out.push_str("\n\n");
    } else {
        out.push_str(
            "<!-- TODO(competing_interests): rubric requires an explicit \
             statement (use \"The authors declare no competing interests.\" \
             when applicable). -->\n\n",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::section_tree::AuthorEntry;

    fn sample_input() -> ScaffoldInput {
        ScaffoldInput {
            title_hint: "Fast Foo Bars".into(),
            authors: vec![AuthorEntry {
                name: "Alice Test".into(),
                orcid: Some("https://orcid.org/0000-0002-1825-0097".into()),
                affiliation_ror: None,
            }],
            results_rows: vec![ResultsRow {
                claim_text: "p95 latency dropped 23%".into(),
                trusty_uri: "https://np.example/RA1234567890abcdef".into(),
                evidence_source: "ExecTimeRecord rows 1..2400".into(),
                verdict: "Supported".into(),
                ci95: Some((0.180, 0.275)),
            }],
            cited_facts: vec![CitedFact {
                claim_text: "Prior work uses linear scan".into(),
                citation_key: "Doe2024".into(),
                doi_or_url: "https://doi.org/10.0000/test".into(),
            }],
            methods_summary: Some("We re-ran the existing benchmark suite under three configurations.".into()),
            limitations: vec!["Sample restricted to x86_64 hosts.".into()],
            ai_disclosure_markdown: Some("This manuscript was scaffolded by `vox-manuscript-scaffold` v0.5.".into()),
            competing_interests: Some("The authors declare no competing interests.".into()),
            figures: vec![],
        }
    }

    fn sample_figure() -> FigureEntry {
        FigureEntry {
            path: "figures/fig-01-p95.svg".into(),
            sha3_256_hex: "abcd1234".into(),
            source_script: "scripts/plot_p95.py".into(),
            caption_hint: Some("p95 latency dropped 23% post-rewrite".into()),
        }
    }

    #[test]
    fn rendered_document_includes_all_expected_section_headings() {
        let md = render_imrad(&sample_input());
        for heading in [
            "# Fast Foo Bars",
            "## Authors",
            "## Abstract",
            "## Introduction",
            "## Methods",
            "## Results",
            "## Figures",
            "## Limitations",
            "## Discussion",
            "## Significance",
            "## Conclusion",
            "## References",
            "## AI Tool Disclosure",
            "## Competing Interests",
        ] {
            assert!(
                md.contains(heading),
                "rendered manuscript missing `{heading}`; got:\n{md}"
            );
        }
    }

    #[test]
    fn empty_figures_emits_placeholder_comment() {
        let md = render_imrad(&sample_input());
        // Default sample has no figures.
        assert!(md.contains("## Figures"));
        assert!(md.contains("No figures supplied"));
    }

    #[test]
    fn figure_entry_renders_image_provenance_and_todo_caption() {
        let mut input = sample_input();
        input.figures = vec![sample_figure()];
        let md = render_imrad(&input);
        // Numbered heading.
        assert!(md.contains("### Figure 1"));
        // Markdown image embed.
        assert!(md.contains("![Figure 1](figures/fig-01-p95.svg)"));
        // TODO caption block — never auto-emitted prose.
        assert!(md.contains("<!-- TODO(figure-caption)"));
        // Caption hint surfaced as machine-suggested but not authoritative.
        assert!(md.contains("p95 latency dropped 23% post-rewrite"));
        assert!(md.contains("machine-suggested; not authoritative"));
        // Provenance footer with all three breadcrumbs.
        assert!(md.contains("SHA3-256: `abcd1234`"));
        assert!(md.contains("Source script: `scripts/plot_p95.py`"));
    }

    #[test]
    fn multiple_figures_number_in_input_order() {
        let mut input = sample_input();
        let mut f2 = sample_figure();
        f2.path = "figures/fig-02-ablation.svg".into();
        input.figures = vec![sample_figure(), f2];
        let md = render_imrad(&input);
        assert!(md.contains("### Figure 1"));
        assert!(md.contains("### Figure 2"));
        // Order preserved — Figure 1 appears before Figure 2 in the output.
        let i1 = md.find("### Figure 1").unwrap();
        let i2 = md.find("### Figure 2").unwrap();
        assert!(i1 < i2);
    }

    #[test]
    fn figure_without_caption_hint_omits_hint_block() {
        let mut input = sample_input();
        input.figures = vec![FigureEntry {
            caption_hint: None,
            ..sample_figure()
        }];
        let md = render_imrad(&input);
        assert!(!md.contains("machine-suggested; not authoritative"));
        // Still has TODO block.
        assert!(md.contains("<!-- TODO(figure-caption)"));
    }

    #[test]
    fn forbidden_sections_always_emit_todo_blocks_no_auto_prose() {
        let md = render_imrad(&sample_input());
        for section in ForbiddenSection::ALL {
            // The section heading is present, and immediately followed by a
            // TODO(narrative) block — no other prose between heading and the
            // closing `-->`.
            let title = section.as_title();
            let heading = format!("## {title}\n\n");
            let idx = md
                .find(&heading)
                .unwrap_or_else(|| panic!("missing heading for {title}"));
            let after = &md[idx + heading.len()..];
            assert!(
                after.trim_start().starts_with("<!-- TODO(narrative)"),
                "{title}: expected TODO(narrative) block immediately after heading; got `{}…`",
                &after[..after.len().min(120)]
            );
        }
    }

    #[test]
    fn results_table_renders_claim_row_with_trusty_link() {
        let md = render_imrad(&sample_input());
        assert!(md.contains("p95 latency dropped 23%"));
        assert!(md.contains("Supported"));
        assert!(md.contains("[0.180, 0.275]"));
        assert!(md.contains("https://np.example/RA1234567890abcdef"));
    }

    #[test]
    fn empty_authors_emits_todo_block() {
        let mut input = sample_input();
        input.authors.clear();
        let md = render_imrad(&input);
        assert!(md.contains("<!-- TODO(author)"));
    }

    #[test]
    fn empty_results_emits_todo_block_not_table_header() {
        let mut input = sample_input();
        input.results_rows.clear();
        let md = render_imrad(&input);
        assert!(md.contains("<!-- TODO(results)"));
        assert!(!md.contains("| Claim |"));
    }

    #[test]
    fn methods_summary_is_lifted_when_provided() {
        let md = render_imrad(&sample_input());
        assert!(md.contains("We re-ran the existing benchmark suite"));
    }

    #[test]
    fn ai_disclosure_block_passes_through_verbatim() {
        let md = render_imrad(&sample_input());
        assert!(md.contains("scaffolded by `vox-manuscript-scaffold`"));
    }

    #[test]
    fn pipe_in_claim_text_is_escaped() {
        let mut input = sample_input();
        input.results_rows[0].claim_text = "a | b".into();
        let md = render_imrad(&input);
        assert!(md.contains("a \\| b"));
    }

    #[test]
    fn cited_facts_appear_in_introduction_todo_block() {
        let md = render_imrad(&sample_input());
        // Pull just the introduction block.
        let intro_start = md.find("## Introduction").unwrap();
        let intro_block = &md[intro_start..];
        let intro_end = intro_block.find("\n## Methods").unwrap();
        let intro = &intro_block[..intro_end];
        assert!(intro.contains("Doe2024"));
        assert!(intro.contains("Prior work uses linear scan"));
    }

    #[test]
    fn rubric_compliance_no_prose_appears_in_any_forbidden_section() {
        // Property-style check: scan the rendered output and assert that
        // for every forbidden section, the body between the heading and the
        // next heading contains ONLY HTML comment lines (or whitespace).
        let md = render_imrad(&sample_input());
        for section in ForbiddenSection::ALL {
            let heading = format!("## {}\n", section.as_title());
            let idx = md.find(&heading).unwrap();
            // Find the next `## ` heading.
            let after = &md[idx + heading.len()..];
            let next = after.find("\n## ").unwrap_or(after.len());
            let body = &after[..next];
            // Body must consist entirely of: blank lines, lines starting
            // with `<!--`, lines containing the closing `-->` marker, or
            // continuation lines of an HTML comment.
            let mut inside_comment = false;
            for line in body.lines() {
                let t = line.trim();
                if t.is_empty() {
                    continue;
                }
                if !inside_comment && t.starts_with("<!--") {
                    inside_comment = true;
                }
                if !inside_comment {
                    panic!(
                        "forbidden section `{}` contained non-comment prose line: `{}`",
                        section.as_title(),
                        line
                    );
                }
                if t.ends_with("-->") {
                    inside_comment = false;
                }
            }
        }
    }
}
