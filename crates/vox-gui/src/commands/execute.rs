use serde::Serialize;
use serde_json::Value;
use tauri_plugin_shell::ShellExt;

const VOX_SIDECAR_NAME: &str = "vox";

#[derive(Serialize)]
pub struct ExecuteOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[tauri::command]
pub async fn execute_command(
    app: tauri::AppHandle,
    path: Vec<String>,
    args: Value,
) -> Result<ExecuteOutput, String> {
    let mut shell_args = path;

    if let Some(argv) = args.get("__argv").and_then(Value::as_array) {
        for token in argv.iter().filter_map(Value::as_str) {
            if !token.trim().is_empty() {
                shell_args.push(token.to_string());
            }
        }
    } else if let Value::Object(map) = args {
        for (k, v) in map {
            if k == "__argv" {
                continue;
            }
            if k == "__positionals" {
                if let Value::Array(values) = v {
                    for value in values {
                        if let Some(s) = value.as_str() {
                            shell_args.push(s.to_string());
                        } else if !value.is_null() {
                            shell_args.push(value.to_string());
                        }
                    }
                }
                continue;
            }
            if k == "__flags" {
                if let Value::Array(flags) = v {
                    for flag in flags.iter().filter_map(Value::as_str) {
                        shell_args.push(format!("--{flag}"));
                    }
                }
                continue;
            }
            shell_args.push(format!("--{}", k.replace('_', "-")));
            match v {
                Value::Null => {}
                Value::String(s) if s.is_empty() => {}
                Value::String(s) => shell_args.push(s),
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    let _ = shell_args.pop();
                }
                Value::Array(items) => {
                    let _ = shell_args.pop();
                    for item in items {
                        shell_args.push(format!("--{}", k.replace('_', "-")));
                        if let Some(s) = item.as_str() {
                            shell_args.push(s.to_string());
                        } else if !item.is_null() {
                            shell_args.push(item.to_string());
                        }
                    }
                }
                other => shell_args.push(other.to_string()),
            }
        }
    }

    let output = app
        .shell()
        .sidecar(VOX_SIDECAR_NAME)
        .map_err(|e| e.to_string())?
        .args(shell_args)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    Ok(ExecuteOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
