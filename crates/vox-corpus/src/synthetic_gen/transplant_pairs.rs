use rand::seq::SliceRandom;
use serde_json::{Value, json};
use std::io::Write;

pub fn generate_transplant_pairs(
    input_path: &std::path::Path,
    out: &mut impl Write,
    count: usize,
) -> anyhow::Result<usize> {
    use std::io::BufRead;
    let file = std::fs::File::open(input_path)?;
    let reader = std::io::BufReader::new(file);
    let mut lines = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            lines.push(v);
        }
    }

    if lines.len() < 2 {
        return Ok(0);
    }

    let mut rng = rand::thread_rng();
    let mut actual = 0;

    for _ in 0..count {
        let pair: Vec<_> = lines.choose_multiple(&mut rng, 2).cloned().collect();
        let source_v = &pair[0];
        let target_v = &pair[1];

        let source_code = source_v
            .get("response")
            .and_then(|r| r.as_str())
            .unwrap_or("");
        let target_code = target_v
            .get("response")
            .and_then(|r| r.as_str())
            .unwrap_or("");

        // Very basic transplant: if source has a 'fn' and target has an 'actor' or '@table'
        if source_code.contains("fn ")
            && (target_code.contains("actor ") || target_code.contains("table "))
        {
            // Find the function block
            if let Some(fn_start) = source_code.find("fn ") {
                let fn_block = &source_code[fn_start..];
                // Find the end of the first block (crude approximation)
                if let Some(fn_end) = fn_block.find('}') {
                    let fn_to_inject = &fn_block[..fn_end + 1];

                    // Inject into target before the last '}'
                    if let Some(last_brace) = target_code.rfind('}') {
                        let mut new_code = target_code[..last_brace].to_string();
                        new_code.push('\n');
                        new_code.push_str("    // Transplanted capability\n");
                        new_code.push_str("    ");
                        new_code.push_str(fn_to_inject);
                        new_code.push('\n');
                        new_code.push_str(&target_code[last_brace..]);

                        let prompt = format!(
                            "Transplant the function logic from the first example into the data structure of the second example.\n\nSource:\n```vox\n{}\n```\nTarget:\n```vox\n{}\n```",
                            source_code, target_code
                        );
                        let record = json!({
                            "prompt": prompt,
                            "response": new_code,
                            "messages": [
                                {"role": "user", "content": prompt},
                                {"role": "assistant", "content": new_code}
                            ],
                            "category": "construct_transplant",
                            "lane": "vox_logic_composition",
                            "schema_version": "vox_dogfood_v1",
                        });

                        writeln!(out, "{}", serde_json::to_string(&record)?)?;
                        actual += 1;
                    }
                }
            }
        }
    }

    Ok(actual)
}
