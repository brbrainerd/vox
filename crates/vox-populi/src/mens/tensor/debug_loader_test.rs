#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use vox_tensor::data::load_all;

    #[test]
    fn debug_loader() {
        let p = PathBuf::from(r"C:\Users\Owner\vox\target\dogfood\train.jsonl");
        println!("DEBUG: Loading from {}", p.display());
        if !p.exists() {
            panic!("File {} not found", p.display());
        }
        let pairs = load_all(&p, 3).expect("failed to load");
        println!("DEBUG: Loaded {} pairs", pairs.len());
        if pairs.is_empty() {
            // Let's try to parse one line manualy to see the error
            let raw = std::fs::read_to_string(&p).unwrap();
            for (i, line) in raw.lines().enumerate().take(3) {
                match serde_json::from_str::<vox_tensor::data::TrainingPair>(line) {
                    Ok(p) => println!(
                        "Line {}: rating={:?}, prompt_len={}",
                        i,
                        p.rating,
                        p.prompt.len()
                    ),
                    Err(e) => println!("Line {}: Error - {:?}", i, e),
                }
            }
        }
    }
}
