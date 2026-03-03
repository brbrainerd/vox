use tower_lsp_server::ls_types::{Position, Uri};
use vox_lsp::definition_at;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cross_file_definition() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let file1 = root.join("main.vox");
    let file2 = root.join("utils.vox");

    // main.vox: `helper` is at line 1, character 12
    fs::write(&file1, "fn main() to Unit:\n    let x = helper()\n").unwrap();
    // utils.vox: `helper` is declared at 0,0
    fs::write(&file2, "fn helper() to Unit:\n    let y = 1\n").unwrap();

    let text = fs::read_to_string(&file1).unwrap();
    let uri: Uri = url::Url::from_file_path(&file1).unwrap().as_str().parse().unwrap();

    // "    let x = helper()" — line 1, `helper` starts at character 12
    let pos = Position { line: 1, character: 12 };

    let def = definition_at(&text, pos, uri, Some(root))
        .expect("definition_at should find helper in utils.vox");

    let def_uri_str = def.uri.as_str().replace('\\', "/");
    assert!(
        def_uri_str.ends_with("utils.vox"),
        "expected utils.vox, got {}",
        def_uri_str
    );
    assert_eq!(def.range.start.line, 0, "helper is on line 0 of utils.vox");
}
