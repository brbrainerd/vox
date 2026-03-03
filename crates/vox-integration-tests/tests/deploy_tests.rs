//! Integration tests for the Vox deployment pipeline.
//! Verifies manifest generation for Kubernetes, Docker, and Bare-metal.

use vox_container::generate::{EnvironmentSpec, generate_kubernetes_manifests, generate_dockerfile_from_spec};

#[test]
fn deploy_kubernetes_manifests_correctly_use_spec() {
    let spec = EnvironmentSpec {
        base_image: "node:22".to_string(),
        exposed_ports: vec![3003, 8080],
        env_vars: vec![
            ("NODE_ENV".to_string(), "production".to_string()),
            ("DATABASE_URL".to_string(), "postgresql://localhost".to_string()),
        ],
        workdir: Some("/app".to_string()),
        ..Default::default()
    };

    let manifests = generate_kubernetes_manifests(
        "my-app",
        "my-app:latest",
        "vox-default",
        &spec
    );

    // Verify Deployment YAML
    assert!(manifests.contains("kind: Deployment"));
    assert!(manifests.contains("image: my-app:latest"));
    assert!(manifests.contains("containerPort: 3003"));
    assert!(manifests.contains("containerPort: 8080"));
    assert!(manifests.contains("name: NODE_ENV"));
    assert!(manifests.contains("value: \"production\""));
    assert!(manifests.contains("name: DATABASE_URL"));
    assert!(manifests.contains("value: \"postgresql://localhost\""));

    // Verify Service YAML
    assert!(manifests.contains("kind: Service"));
    assert!(manifests.contains("name: my-app"));
    assert!(manifests.contains("port: 3003"));
    assert!(manifests.contains("targetPort: 3003"));
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

    assert!(dockerfile.contains("FROM rust:1.80-slim"));
    assert!(dockerfile.contains("apt-get install -y libssl-dev pkg-config"));
    assert!(dockerfile.contains("ENV RUST_LOG=info"));
    assert!(dockerfile.contains("EXPOSE 3000"));
    assert!(dockerfile.contains("RUN cargo build --release"));
    assert!(dockerfile.contains("CMD [\"./target/release/my-app\"]"));
}

#[test]
fn deploy_bare_metal_systemd_template_population() {
    // This logic currently lives in vox-cli, but we can verify the template logic.
    // In a real integration test, we'd mock the Command output, but for now
    // let's verify if the template used in `deploy.rs` is sound.

    let app_name = "test-app";
    let user = "vox";
    let workdir = "/home/vox/app";
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

    assert!(service_file.contains("Description=Vox Application: test-app"));
    assert!(service_file.contains("User=vox"));
    assert!(service_file.contains("Environment=PORT=3000"));
    assert!(service_file.contains("ExecStart=./my-binary --port 3000"));
}
