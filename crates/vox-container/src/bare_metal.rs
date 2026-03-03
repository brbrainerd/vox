use crate::generate::EnvironmentSpec;

/// Generate a systemd .service file from an EnvironmentSpec.
///
/// Use this when `@environment` has `base: "bare-metal"` — instead of a
/// Dockerfile you get a drop-in systemd unit file ready for deployment.
pub fn generate_systemd_unit(spec: &EnvironmentSpec, app_name: &str) -> String {
    let mut unit = String::new();

    unit.push_str("[Unit]\n");
    unit.push_str(&format!("Description=Vox Application - {}\n", app_name));
    unit.push_str("After=network.target\n\n");

    unit.push_str("[Service]\n");
    unit.push_str("Type=simple\n");

    if let Some(ref wd) = spec.workdir {
        unit.push_str(&format!("WorkingDirectory={}\n", wd));
    }

    for (k, v) in &spec.env_vars {
        unit.push_str(&format!("Environment=\"{}={}\"\n", k, v));
    }

    let exec_start = if spec.cmd.is_empty() {
        format!("./{}", app_name)
    } else {
        spec.cmd.join(" ")
    };
    unit.push_str(&format!("ExecStart={}\n", exec_start));

    unit.push_str("Restart=always\n");
    unit.push_str("RestartSec=5\n\n");

    unit.push_str("[Install]\n");
    unit.push_str("WantedBy=multi-user.target\n");

    unit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_systemd_unit_minimal() {
        let spec = EnvironmentSpec {
            base_image: "bare-metal".to_string(),
            workdir: Some("/opt/my-app".to_string()),
            env_vars: vec![("PORT".to_string(), "8080".to_string())],
            cmd: vec!["./my-app".to_string(), "--port".to_string(), "8080".to_string()],
            ..Default::default()
        };
        let unit = generate_systemd_unit(&spec, "my-app");
        assert!(unit.contains("[Unit]"), "Missing [Unit] section");
        assert!(unit.contains("[Service]"), "Missing [Service] section");
        assert!(unit.contains("WorkingDirectory=/opt/my-app"), "Missing WorkingDirectory");
        assert!(unit.contains("Environment=\"PORT=8080\""), "Missing env var");
        assert!(unit.contains("ExecStart=./my-app --port 8080"), "Missing ExecStart");
        assert!(unit.contains("WantedBy=multi-user.target"), "Missing [Install]");
    }

    #[test]
    fn test_generate_systemd_unit_default_exec() {
        let spec = EnvironmentSpec {
            base_image: "bare-metal".to_string(),
            cmd: vec![],
            ..Default::default()
        };
        let unit = generate_systemd_unit(&spec, "my-server");
        assert!(unit.contains("ExecStart=./my-server"), "Should fall back to app_name");
    }
}
