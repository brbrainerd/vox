/// Task 8G: React interop C4 — imported npm packages land in package.json.
///
/// When a Vox program imports a React component from an npm specifier
/// (e.g. `import react {Dialog} from "@radix-ui/react-dialog"`), that package
/// — and any peer deps from `external_libs.rs` — must appear in the generated
/// `package.json` `dependencies` section.
use vox_codegen_ts::scaffold::package_json_for_test;

#[test]
fn imported_lib_lands_in_package_json() {
    let pkg = package_json_for_test(
        r#"import react {Dialog} from "@radix-ui/react-dialog"
component App() { }"#,
    );
    assert!(
        pkg.contains("@radix-ui/react-dialog"),
        "imported package must appear in dependencies\ngot: {pkg}"
    );
}

#[test]
fn known_lib_peers_are_included() {
    // @mantine/core has peers: ["@mantine/hooks"] per external_libs.rs
    let pkg = package_json_for_test(
        r#"import react {Button} from "@mantine/core"
component App() { }"#,
    );
    assert!(
        pkg.contains("@mantine/core"),
        "@mantine/core must appear in dependencies\ngot: {pkg}"
    );
    assert!(
        pkg.contains("@mantine/hooks"),
        "peer @mantine/hooks must appear in dependencies\ngot: {pkg}"
    );
}

#[test]
fn multiple_imports_all_land() {
    let pkg = package_json_for_test(
        r#"import react {A} from "@acme/ui"
import react {B} from "@acme/icons"
component App() { }"#,
    );
    assert!(pkg.contains("@acme/ui"), "@acme/ui must appear\ngot: {pkg}");
    assert!(
        pkg.contains("@acme/icons"),
        "@acme/icons must appear\ngot: {pkg}"
    );
}
