use tempfile::tempdir;
use std::fs;

#[test]
fn test_cli_graphify_rebuild_success() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("crates");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "fn hello() {}").unwrap();

    let output_file = tmp.path().join("graph.json");
    let cache_dir = tmp.path().join("cache");

    let res = vox_graphify_reader::rebuild::rebuild_graph(
        tmp.path(),
        &src,
        &output_file,
        &cache_dir,
    );
    
    assert!(res.is_ok());
    assert!(output_file.exists());
    let graph_content = fs::read_to_string(output_file).unwrap();
    assert!(graph_content.contains("hello"));
}
