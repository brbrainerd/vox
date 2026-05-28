use serde::Serialize;
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
    args: serde_json::Value,
) -> Result<ExecuteOutput, String> {
    let mut shell_args = path;
    
    if let serde_json::Value::Object(map) = args {
        for (k, v) in map {
            shell_args.push(format!("--{}", k));
            if let serde_json::Value::String(s) = v {
                shell_args.push(s);
            } else {
                shell_args.push(v.to_string());
            }
        }
    }

    let output = app.shell().sidecar(VOX_SIDECAR_NAME)
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
