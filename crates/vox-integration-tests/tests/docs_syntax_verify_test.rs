use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use std::fs;
use std::path::PathBuf;
use vox_compiler::lexer::lex;
use vox_compiler::parser::descent::parse;
use walkdir::WalkDir;

#[test]
fn test_all_markdown_vox_blocks_parse() {
    let workspace_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let docs_dir = PathBuf::from(&workspace_root)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("docs")
        .join("src");

    let mut failed = false;
    let mut parsed_blocks = 0;

    for entry in WalkDir::new(&docs_dir) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        // Skip archived docs — historical content is allowed to contain pre-tombstone syntax.
        if entry
            .path()
            .components()
            .any(|c| c.as_os_str() == "archive")
        {
            continue;
        }

        let content = fs::read_to_string(entry.path()).unwrap();
        let parser = Parser::new(&content);

        let mut in_vox_block = false;
        let mut current_block = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang)))
                    if lang.as_ref() == "vox" =>
                {
                    in_vox_block = true;
                    current_block.clear();
                }
                Event::Text(text) if in_vox_block => {
                    current_block.push_str(&text);
                }
                Event::End(TagEnd::CodeBlock) if in_vox_block => {
                    in_vox_block = false;

                    // Ignore skipped blocks or ones with intentional warning placeholders.
                    // `// vox:skip` is the canonical CLAUDE.md skip sentinel.
                    // `{{#include ...}}` blocks are mdBook directives, not raw Vox.
                    if current_block.contains("vox:skip")
                        || current_block.contains("Skip-Test")
                        || current_block.contains("todo!(")
                        || current_block.contains("empty-body")
                        || current_block.contains("{{#include")
                    {
                        continue;
                    }

                    // Ignore things known to be partial snippets unless wrapped in fn
                    // We will just attempt to parse them. If they fail, we can add a skip.

                    let tokens = lex(&current_block);
                    if let Err(errs) = parse(tokens) {
                        println!(
                            "Failed to parse vox block in {:?} \nBlock:\n{}\nErrors:\n",
                            entry.path(),
                            current_block
                        );
                        for e in errs {
                            println!("{:?}", e);
                        }
                        failed = true;
                    } else {
                        parsed_blocks += 1;
                    }
                }
                _ => {}
            }
        }
    }

    println!("Successfully parsed {} vox blocks.", parsed_blocks);
    assert!(
        !failed,
        "One or more markdown vox code blocks failed to parse. See stdout for details."
    );
}
