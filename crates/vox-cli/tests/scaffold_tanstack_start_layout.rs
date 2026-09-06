//! Assert TanStack Start scaffold file layout (no Node).
//! Blueprint OP-0169: Start-mode scaffold stays compatible with Express `server.ts` emission flags
//! (`VOX_EMIT_EXPRESS_SERVER`); route tree is client-only here.
#![allow(missing_docs)]

use std::fs;

use vox_cli::frontend;
use vox_cli::templates;

#[test]
fn tanstack_start_with_route_manifest_uses_file_route_fallback() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ts_out = tmp.path().join("ts_out");
    let app = tmp.path().join("app");
    fs::create_dir_all(&ts_out).expect("ts_out");
    fs::write(
        ts_out.join("routes.manifest.ts"),
        "export const voxRoutes = [] as never[];\n",
    )
    .expect("manifest");
    fs::write(
        ts_out.join("Home.tsx"),
        "export function Home() { return null; }\n",
    )
    .expect("home");

    frontend::scaffold_react_app(&app, &ts_out, true).expect("scaffold");

    let workspace =
        fs::read_to_string(app.join("pnpm-workspace.yaml")).expect("pnpm-workspace.yaml");
    assert!(
        workspace.contains("allowBuilds:") && workspace.contains("esbuild: true"),
        "scaffold must emit pnpm 11 allowBuilds for esbuild, got:\n{workspace}"
    );

    let components = fs::read_to_string(app.join("components.json")).expect("components.json");
    assert!(
        components.contains("\"rsc\": false"),
        "scaffold should ship shadcn-compatible components.json with rsc:false"
    );
    let tsconfig = fs::read_to_string(app.join("tsconfig.json")).expect("tsconfig");
    assert!(
        tsconfig.contains("\"@/*\": [\"./src/*\"]"),
        "tsconfig should alias @/* to src for shadcn-style imports"
    );

    assert!(
        app.join("src/routes/index.tsx").is_file(),
        "Start + manifest (no legacy VoxTanStackRouter) should fall back to file routes"
    );
    assert!(app.join("src/router.tsx").is_file());
    assert!(app.join("src/routes/__root.tsx").is_file());
    let adapter =
        fs::read_to_string(app.join("src/vox-manifest-route-adapter.tsx")).expect("adapter");
    assert!(
        adapter.contains("buildChildRoutes"),
        "Start scaffold should ship shared manifest adapter when routes.manifest.ts is present"
    );
}

#[test]
fn tanstack_start_file_route_fallback_writes_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ts_out = tmp.path().join("ts_out");
    let app = tmp.path().join("app");
    fs::create_dir_all(&ts_out).expect("ts_out");
    fs::write(
        ts_out.join("Chat.tsx"),
        "export function Chat() { return null; }\n",
    )
    .expect("chat");

    frontend::scaffold_react_app(&app, &ts_out, true).expect("scaffold");

    assert!(app.join("src/routes/index.tsx").is_file());
    let idx = fs::read_to_string(app.join("src/routes/index.tsx")).expect("idx");
    assert!(idx.contains("Chat"));
}

#[test]
fn route_tree_seed_template_contains_router_type_import() {
    let s = templates::tanstack_start_route_tree_gen();
    assert!(s.contains("routeTree"));
    assert!(s.contains("./router.tsx"));
}
