use vox_parser::parse;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_parse_examples() {
    let mut examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    examples_dir.pop();
    examples_dir.pop();
    examples_dir.push("examples");

    if !examples_dir.exists() {
        return;
    }

    // Collect and sort for determinism
    let mut entries: Vec<_> = fs::read_dir(&examples_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "vox"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut failed = 0;
    let total = entries.len();

    for entry in entries {
        let path = entry.path();
        let content = fs::read_to_string(&path).unwrap();
        let tokens = vox_lexer::lex(&content);
        let result = parse(tokens);
        match result {
            Ok(_) => println!("Parsed {}", path.display()),
            Err(errs) => {
                println!("Failed to parse {}:", path.display());
                for e in errs {
                    println!("  {:?}", e);
                }
                failed += 1;
            }
        }
    }

    println!("Total failures: {}/{}", failed, total);
    assert_eq!(failed, 0, "Failed to parse {} out of {} example files", failed, total);
}
