use std::fs;
use std::path::Path;

fn main() {
    let docs_dir = Path::new("docs/src");

    if !docs_dir.exists() {
        println!("Docs dir not found. Attempting to run from root...");
        return;
    }

    let mut links = String::new();
    let mut files = Vec::new();

    // Iterate through md files in docs/src.
    if let Ok(entries) = fs::read_dir(docs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or_default() == "md" {
                let file_name = path.file_name().unwrap().to_str().unwrap();
                if file_name != "SUMMARY.md" {
                    files.push(file_name.to_string());
                }
            }
        }
    }

    // Sort files cleanly so index.md comes first if applicable.
    files.sort();

    links.push_str("# Summary\n\n");
    for file in files {
        let title = file.replace(".md", "").replace("_", " ");
        let title = title_case(&title);

        links.push_str(&format!("- [{}]({})\n", title, file));
    }

    let summary_path = docs_dir.join("SUMMARY.md");
    if let Err(e) = fs::write(&summary_path, links) {
        eprintln!("Failed to write SUMMARY.md: {}", e);
    } else {
        println!("Successfully generated SUMMARY.md");
    }
}

// Helper to make title case
fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
