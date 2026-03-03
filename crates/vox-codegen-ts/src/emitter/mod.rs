pub mod infra;
pub mod logic;
pub mod react;
pub mod test;
pub mod ui;

use vox_ast::decl::{Decl, Module};
use crate::adt::generate_types;
use crate::component::generate_component;
use crate::routes::{generate_api_client, generate_routes};
use std::collections::HashSet;

/// Output from the TypeScript code generator.
pub struct CodegenOutput {
    /// List of (filename, content) pairs.
    pub files: Vec<(String, String)>,
}

pub fn generate(module: &Module) -> Result<CodegenOutput, String> {
    let mut files = Vec::new();

    // 1. Types
    let types_content = generate_types(module);
    if !types_content.is_empty() {
        files.push(("types.d.ts".to_string(), types_content));
    }

    // 2. Metadata: Icons & Routes
    let mut icons = Vec::new();
    let mut route_components = HashSet::new();
    for decl in &module.declarations {
        match decl {
            Decl::Import(import) => {
                for path in &import.paths {
                    if path.segments.first().map(|s| s.as_str()) == Some("icons") {
                        if let Some(second) = path.segments.get(1) {
                            icons.push(second.clone());
                        }
                    }
                }
            }
            Decl::Routes(routes_decl) => {
                for entry in &routes_decl.entries {
                    route_components.insert(entry.component_name.clone());
                }
            }
            Decl::Page(page_decl) => {
                route_components.insert(page_decl.func.name.clone());
            }
            _ => {}
        }
    }

    // 3. Components
    for decl in &module.declarations {
        match decl {
            Decl::Component(comp) => {
                let is_route = route_components.contains(&comp.func.name);
                let (filename, content) =
                    generate_component(&comp.func, !comp.styles.is_empty(), &icons, is_route);
                files.push((filename, content));
                if !comp.styles.is_empty() {
                    files.push((format!("{}.css", comp.func.name), ui::generate_component_css(comp)));
                }
            }
            Decl::Page(page) => {
                let (filename, content) =
                    generate_component(&page.func, false, &icons, true);
                files.push((filename, content));
            }
            Decl::Layout(layout) => {
                let (filename, content) = generate_component(&layout.func, false, &icons, false);
                files.push((filename, content));
            }
            Decl::Loading(loading) => {
                let (filename, content) = generate_component(&loading.func, false, &icons, false);
                files.push((filename, content));
            }
            Decl::NotFound(not_found) => {
                let (filename, content) = generate_component(&not_found.func, false, &icons, false);
                files.push((filename, content));
            }
            Decl::ErrorBoundary(eb) => {
                let (filename, content) = generate_component(&eb.func, false, &icons, false);
                files.push((filename, content));
            }
            Decl::Function(func) if func.is_layout => {
                let (filename, content) = generate_component(func, false, &icons, false);
                files.push((filename, content));
            }
            Decl::V0Component(v0) => {
                let filename = format!("{}.tsx", v0.name);
                let prompt_comment = if !v0.prompt.is_empty() {
                    format!("Prompt: {}", v0.prompt)
                } else if let Some(ref img) = v0.image_path {
                    format!("From image: {}", img)
                } else {
                    "No prompt provided".to_string()
                };
                let content = format!(
                    "// @v0 generated component\n// {}\nimport React from \"react\";\n\nexport function {}(): React.ReactElement {{\n  return <div>{{/* AI component definition pending API integration */}}</div>;\n}}\n",
                    prompt_comment, v0.name
                );
                files.push((filename, content));
            }
            _ => {}
        }
    }

    // 4. Routes & API
    let routes_content = generate_routes(module);
    let has_routes = !routes_content.is_empty();
    if has_routes {
        files.push(("server.ts".to_string(), routes_content));
        let api_client_content = generate_api_client(module);
        if !api_client_content.is_empty() {
            files.push(("api.ts".to_string(), api_client_content));
        }
    }

    // Generate App.tsx with React Router for routes: declarations
    for decl in &module.declarations {
        if let Decl::Routes(routes_decl) = decl {
            let mut app = String::new();
            app.push_str("import React from \"react\";\n");
            app.push_str("import { BrowserRouter, Routes, Route } from \"react-router-dom\";\n");
            // Import each referenced component
            for entry in &routes_decl.entries {
                app.push_str(&format!(
                    "import {{ {} }} from \"./{}.tsx\";\n",
                    entry.component_name, entry.component_name
                ));
            }
            app.push_str("\nexport default function App(): React.ReactElement {\n");
            app.push_str("  return (\n");
            app.push_str("    <BrowserRouter>\n");
            app.push_str("      <Routes>\n");
            for entry in &routes_decl.entries {
                app.push_str(&format!(
                    "        <Route path=\"{}\" element={{<{} />}} />\n",
                    entry.path, entry.component_name
                ));
            }
            app.push_str("      </Routes>\n");
            app.push_str("    </BrowserRouter>\n");
            app.push_str("  );\n");
            app.push_str("}\n");
            files.push(("App.tsx".to_string(), app));
        }
    }

    // 5. Logic: Activities, Agents, Messages
    let mut activities = Vec::new();
    let mut messages = Vec::new();
    for decl in &module.declarations {
        match decl {
            Decl::Activity(a) => activities.push(a),
            Decl::Agent(agent) => {
                files.push((format!("{}.ts", agent.name), logic::generate_agent(agent)));
            }
            Decl::Message(msg) => messages.push(msg),
            _ => {}
        }
    }
    if !activities.is_empty() {
        files.push(("activities.ts".to_string(), logic::generate_activities(&activities)));
    }
    if !messages.is_empty() {
        files.push(("messages.ts".to_string(), logic::generate_messages(&messages)));
    }

    // 6. Database: Schema & Handlers
    let schema_content = crate::schema::generate_voxdb_schema(module);
    if !schema_content.is_empty() {
        files.push(("schema.ts".to_string(), schema_content));
    }
    let voxdb_handlers_content = crate::voxdb::generate_voxdb_handlers(module);
    if !voxdb_handlers_content.is_empty() {
        files.push(("voxdb_handlers.ts".to_string(), voxdb_handlers_content));
    }

    // 7. React: Config, Contexts, Providers, Hooks
    let mut config_ts = String::new();
    let mut contexts = Vec::new();
    let mut providers = Vec::new();
    let mut hooks = Vec::new();
    for decl in &module.declarations {
        match decl {
            Decl::Config(cfg) => {
                config_ts.push_str(&format!("export const {} = {{\n", cfg.name));
                for field in &cfg.fields {
                    let is_opt = matches!(&field.type_ann, vox_ast::types::TypeExpr::Generic { name, .. } if name == "Option");
                    if is_opt {
                        config_ts.push_str(&format!("  get {}(): string | undefined {{\n    return process.env.{} ?? undefined;\n  }},\n", field.name, field.name));
                    } else {
                        config_ts.push_str(&format!("  get {}(): string {{\n    const val = process.env.{};\n    if (val === undefined) throw new Error(\"Missing env var: {}\");\n    return val;\n  }},\n", field.name, field.name, field.name));
                    }
                }
                config_ts.push_str("};\n\n");
            }
            Decl::Context(ctx) => contexts.push(ctx),
            Decl::Provider(prov) => providers.push(prov),
            Decl::Hook(hook) => hooks.push(hook),
            _ => {}
        }
    }
    if !config_ts.is_empty() { files.push(("config.ts".to_string(), config_ts)); }
    if !contexts.is_empty() { files.push(("contexts.ts".to_string(), react::generate_contexts(&contexts))); }
    if !providers.is_empty() {
        let mut prov_content = String::new();
        prov_content.push_str("import React from \"react\";\n");
        if !contexts.is_empty() {
            let names: Vec<_> = contexts.iter().map(|c| format!("{}Context", c.name)).collect();
            prov_content.push_str(&format!("import {{ {} }} from \"./contexts\";\n", names.join(", ")));
        }
        for prov in &providers { prov_content.push_str(&react::generate_provider(prov)); }
        files.push(("providers.tsx".to_string(), prov_content));
    }
    if !hooks.is_empty() {
        let mut final_hooks = String::from("import React, { useState, useEffect, useMemo, useRef, useCallback, useReducer } from \"react\";\n\n");
        for hook in &hooks { final_hooks.push_str(&react::generate_custom_hook(hook)); }
        files.push(("custom_hooks.ts".to_string(), final_hooks));
    }

    // 8. UI Extras: Keyframes & Themes
    let mut keyframes = Vec::new();
    for decl in &module.declarations {
        match decl {
            Decl::Keyframes(kf) => keyframes.push(kf),
            Decl::Theme(theme) => {
                files.push((format!("{}.css", theme.name), ui::generate_theme_css(theme)));
                files.push(("ThemeToggle.tsx".to_string(), ui::generate_theme_toggle()));
            }
            _ => {}
        }
    }
    if !keyframes.is_empty() {
        files.push(("animations.css".to_string(), ui::generate_keyframes_css(&keyframes)));
    }

    // 9. Tests
    let mut tests = Vec::new();
    let mut fixtures = Vec::new();
    let mut mocks = Vec::new();
    for decl in &module.declarations {
        match decl {
            Decl::Test(t) => tests.push(&t.func),
            Decl::Fixture(f) => fixtures.push(&f.func),
            Decl::Mock(m) => mocks.push(m),
            _ => {}
        }
    }
    if !tests.is_empty() {
        files.push(("tests/vox.test.ts".to_string(), test::generate_test_suite(&tests, &fixtures, &mocks)));
    }

    // 10. Infrastructure
    let all_ts: String = files.iter().map(|(_, c)| c.as_str()).collect::<Vec<_>>().join("\n");
    files.push(("tsconfig.json".to_string(), infra::generate_tsconfig()));
    files.push(("vitest.config.ts".to_string(), infra::generate_vitest_config()));
    files.push((".env.example".to_string(), infra::generate_env_example(&all_ts)));
    files.push(("package.json".to_string(), infra::generate_package_json(&all_ts, !tests.is_empty(), has_routes)));

    // 11. @environment declarations — generate named Dockerfiles from environment specs
    let mut has_env_dockerfile = false;
    for decl in &module.declarations {
        if let Decl::Environment(env) = decl {
            let spec = vox_container::generate::EnvironmentSpec {
                base_image: env.base_image.as_deref().unwrap_or("node:20-alpine").to_string(),
                packages: env.packages.clone(),
                env_vars: env.env_vars.clone(),
                exposed_ports: env.exposed_ports.clone(),
                volumes: env.volumes.clone(),
                workdir: env.workdir.clone(),
                cmd: env.cmd.clone(),
                copy_instructions: env.copy_instructions.clone(),
                run_commands: env.run_commands.clone(),
                ..Default::default()
            };
            if env.base_image.as_deref() == Some("bare-metal") {
                let service_name = format!("{}.service", env.name);
                files.push((service_name, vox_container::generate_systemd_unit(&spec, "vox-app")));
            } else {
                let dockerfile_name = if env.name == "default" || env.name == "production" {
                    "Dockerfile".to_string()
                } else {
                    format!("Dockerfile.{}", env.name)
                };
                files.push((dockerfile_name, vox_container::generate::generate_dockerfile_from_spec(&spec)));
            }
            has_env_dockerfile = true;
        }
    }

    // Fall back to generic Dockerfile when routes exist but no @environment was declared
    if has_routes && !has_env_dockerfile {
        files.push(("Dockerfile".to_string(), infra::generate_dockerfile()));
        files.push(("docker-compose.yml".to_string(), infra::generate_docker_compose()));
    }

    Ok(CodegenOutput { files })
}
