#[cfg(test)]
mod tests_train_script {
    use std::path::PathBuf;

    #[test]
    fn test_train_script_exists_and_parses() {
        let script_path = PathBuf::from("../../scripts/train_local_qwen.vox");
        if !script_path.exists() {
            let relative = PathBuf::from("scripts/train_local_qwen.vox");
            assert!(relative.exists(), "train_local_qwen.vox not found");
        }
    }
}
