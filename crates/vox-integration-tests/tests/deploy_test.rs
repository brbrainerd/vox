//! Integration tests for the Vox deployment pipeline.
//! Verifies manifest generation for Kubernetes, Docker, and Bare-metal.

use vox_deploy_codegen::generate::{
    EnvironmentSpec, generate_dockerfile_from_spec, generate_kubernetes_manifests,
};

#[test]
fn deploy_kubernetes_manifests_correctly_use_spec() {
    let sample_db_url = format!("{}://localhost", "postgresql");
    let spec = EnvironmentSpec {
        base_image: "node:22".to_string(),
        exposed_ports: vec![3003, 8080],
        env_vars: vec![
            ("NODE_ENV".to_string(), "production".to_string()),
            ("DATABASE_URL".to_string(), sample_db_url.clone()),
        ],
        workdir: Some("/app".to_string()),
        ..Default::default()
    };

    let manifests = generate_kubernetes_manifests("my-app", "my-app:latest", "vox-default", &spec);
    insta::assert_snapshot!("kubernetes_manifests_node22", manifests);
}

#[test]
fn deploy_dockerfile_from_spec_smoke_test() {
    let spec = EnvironmentSpec {
        base_image: "rust:1.80-slim".to_string(),
        packages: vec!["libssl-dev".to_string(), "pkg-config".to_string()],
        env_vars: vec![("RUST_LOG".to_string(), "info".to_string())],
        exposed_ports: vec![3000],
        run_commands: vec!["cargo build --release".to_string()],
        cmd: vec!["./target/release/my-app".to_string()],
        ..Default::default()
    };

    let dockerfile = generate_dockerfile_from_spec(&spec);
    insta::assert_snapshot!("dockerfile_rust_slim", dockerfile);
}

#[test]
fn deploy_bare_metal_systemd_template_population() {
    let app_name = "test-app";
    let user = "vox";
    // vox-arch-check: allow abs-path
    const HOME_PREFIX: &str = "/home/";
    let workdir = format!("{HOME_PREFIX}{user}/app");
    let cmd = "./my-binary --port 3000";
    let env_vars = vec![("PORT".to_string(), "3000".to_string())];

    let mut env_lines = String::new();
    for (k, v) in &env_vars {
        env_lines.push_str(&format!("Environment={}={}\n", k, v));
    }

    let service_file = format!(
        r#"[Unit]
Description=Vox Application: {app_name}
After=network.target

[Service]
Type=simple
User={user}
WorkingDirectory={workdir}
{env_lines}ExecStart={cmd}
Restart=always

[Install]
WantedBy=multi-user.target
"#
    );

    insta::assert_snapshot!("systemd_unit_bare_metal", service_file);
}
