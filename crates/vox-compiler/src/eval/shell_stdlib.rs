//! Interpreter-only implementations for structured shell-tier stdlib (`std.fs.*`, `std.csv`, etc.).
//!
//! Codegen lowers the same surface to `vox_actor_runtime::builtins`.
//! The shared file-record type lives in `vox-shell-stdlib-types` to avoid the
//! Cargo cycle (`vox-compiler` → `vox-actor-runtime` → … → `vox-compiler`).

pub(crate) use vox_shell_stdlib_types::fs_types::VoxFileRecord as InterpFileRecord;

fn file_record_from_meta(
    full_path: &str,
    name: &str,
    meta: &std::fs::Metadata,
) -> InterpFileRecord {
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let ft = meta.file_type();
    let len = meta.len();
    let size = i64::try_from(len).unwrap_or(i64::MAX);
    InterpFileRecord {
        name: name.to_string(),
        path: full_path.to_string(),
        size,
        modified_ms,
        is_dir: ft.is_dir(),
        is_file: ft.is_file(),
        is_symlink: ft.is_symlink(),
    }
}

pub(crate) fn interp_fs_list_dir_detailed(dir: &str) -> Result<Vec<InterpFileRecord>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        let path_buf = ent.path();
        let name = ent.file_name().to_string_lossy().into_owned();
        let meta = match std::fs::symlink_metadata(&path_buf) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let full = path_buf.to_string_lossy().into_owned();
        out.push(file_record_from_meta(&full, &name, &meta));
    }
    Ok(out)
}

pub(crate) fn interp_fs_stat(path: &str) -> Result<InterpFileRecord, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let name = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    Ok(file_record_from_meta(path, &name, &meta))
}

pub(crate) fn interp_csv_parse(text: &str) -> Result<serde_json::Value, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| e.to_string())?;
        let row: Vec<serde_json::Value> = rec
            .iter()
            .map(|f| serde_json::Value::String(f.to_string()))
            .collect();
        rows.push(serde_json::Value::Array(row));
    }
    Ok(serde_json::Value::Array(rows))
}

pub(crate) fn interp_csv_parse_records(text: &str) -> Result<serde_json::Value, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| e.to_string())?;
        let mut obj = serde_json::Map::new();
        for (i, cell) in rec.iter().enumerate() {
            let key = headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("column_{i}"));
            obj.insert(key, serde_json::Value::String(cell.to_string()));
        }
        rows.push(serde_json::Value::Object(obj));
    }
    Ok(serde_json::Value::Array(rows))
}

pub(crate) fn interp_csv_render(rows: &[Vec<String>]) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    for row in rows {
        wtr.write_record(row).map_err(|e| e.to_string())?;
    }
    wtr.into_inner()
        .map_err(|e| e.to_string())
        .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
}

/// TOML documents must be tables at the root. Json arrays/scalars from `std.csv` / `std.yaml`
/// are wrapped under this key for `std.toml.render` and peeled by `std.toml.parse`.
const VOX_TOML_JSON_ROOT_KEY: &str = "__vox_json_root";

pub(crate) fn interp_toml_parse(text: &str) -> Result<serde_json::Value, String> {
    let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let j = toml_value_to_json(&v)?;
    Ok(match j {
        serde_json::Value::Object(ref map)
            if map.len() == 1 && map.contains_key(VOX_TOML_JSON_ROOT_KEY) =>
        {
            map.get(VOX_TOML_JSON_ROOT_KEY).cloned().unwrap_or(j)
        }
        _ => j,
    })
}

fn toml_value_to_json(v: &toml::Value) -> Result<serde_json::Value, String> {
    Ok(match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(toml_value_to_json(item)?);
            }
            serde_json::Value::Array(out)
        }
        toml::Value::Table(t) => {
            let mut map = serde_json::Map::new();
            for (k, val) in t {
                map.insert(k.clone(), toml_value_to_json(val)?);
            }
            serde_json::Value::Object(map)
        }
    })
}

fn json_value_to_toml(j: &serde_json::Value) -> Result<toml::Value, String> {
    Ok(match j {
        serde_json::Value::Null => toml::Value::String(String::new()),
        serde_json::Value::Bool(b) => toml::Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(u) = n.as_u64() {
                toml::Value::Integer(
                    i64::try_from(u).map_err(|_| "json number too large for TOML Integer".to_string())?,
                )
            } else {
                toml::Value::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => toml::Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(json_value_to_toml(item)?);
            }
            toml::Value::Array(out)
        }
        serde_json::Value::Object(map) => {
            let mut table = toml::map::Map::new();
            for (k, v) in map {
                table.insert(k.clone(), json_value_to_toml(v)?);
            }
            toml::Value::Table(table)
        }
    })
}

pub(crate) fn interp_toml_render(value: &serde_json::Value) -> Result<String, String> {
    let wrapped = match value {
        serde_json::Value::Object(_) => value.clone(),
        serde_json::Value::Array(_)
        | serde_json::Value::String(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::Bool(_)
        | serde_json::Value::Null => {
            let mut m = serde_json::Map::new();
            m.insert(VOX_TOML_JSON_ROOT_KEY.to_string(), value.clone());
            serde_json::Value::Object(m)
        }
    };
    let tv = json_value_to_toml(&wrapped)?;
    toml::to_string_pretty(&tv).map_err(|e| e.to_string())
}

pub(crate) fn interp_yaml_parse(text: &str) -> Result<serde_json::Value, String> {
    serde_yaml::from_str::<serde_json::Value>(text).map_err(|e| e.to_string())
}

pub(crate) fn interp_yaml_render(value: &serde_json::Value) -> Result<String, String> {
    serde_yaml::to_string(value).map_err(|e| e.to_string())
}

fn json_scalar_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

fn interp_csv_save_from_json(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::Array(rows) => {
            if rows.is_empty() {
                return Ok(String::new());
            }
            let mut out_rows: Vec<Vec<String>> = Vec::new();
            if rows.iter().all(|r| r.is_object()) {
                let keys: Vec<String> = rows[0]
                    .as_object()
                    .expect("object row")
                    .keys()
                    .cloned()
                    .collect();
                out_rows.push(keys.clone());
                for row in rows {
                    let obj = row
                        .as_object()
                        .ok_or_else(|| "csv save: expected object".to_string())?;
                    let mut line = Vec::new();
                    for k in &keys {
                        let cell = obj.get(k).map(json_scalar_to_string).unwrap_or_default();
                        line.push(cell);
                    }
                    out_rows.push(line);
                }
            } else if rows.iter().all(|r| r.is_array()) {
                for row in rows {
                    let arr = row
                        .as_array()
                        .ok_or_else(|| "csv save: expected array row".to_string())?;
                    let line: Vec<String> = arr.iter().map(json_scalar_to_string).collect();
                    out_rows.push(line);
                }
            } else {
                return Err("csv save: array must be all objects or all arrays".into());
            }
            interp_csv_render(&out_rows)
        }
        _ => Err("csv save: expected JSON array".into()),
    }
}

pub(crate) fn interp_io_open(path: &str) -> Result<serde_json::Value, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    match ext.as_str() {
        "json" => serde_json::from_str(&text).map_err(|e| e.to_string()),
        "toml" => interp_toml_parse(&text),
        "yaml" | "yml" => interp_yaml_parse(&text),
        "csv" => interp_csv_parse_records(&text),
        _ => Ok(serde_json::Value::String(text)),
    }
}

pub(crate) fn interp_io_save(path: &str, value: &serde_json::Value) -> Result<(), String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let data: Vec<u8> = match ext.as_str() {
        "json" => serde_json::to_string_pretty(value)
            .map_err(|e| e.to_string())?
            .into_bytes(),
        "toml" => interp_toml_render(value)?.into_bytes(),
        "yaml" | "yml" => interp_yaml_render(value)?.into_bytes(),
        "csv" => interp_csv_save_from_json(value)?.into_bytes(),
        _ => {
            let s = match value {
                serde_json::Value::String(s) => s.clone(),
                _ => {
                    return Err(
                        "std.io.save: non-structured extension expects a JSON string value".into(),
                    );
                }
            };
            s.into_bytes()
        }
    };
    std::fs::write(path, data).map_err(|e| e.to_string())
}

pub(crate) fn interp_process_run_capture_json(
    cmd: &str,
    args: &[String],
) -> Result<serde_json::Value, String> {
    let out = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    serde_json::from_str(stdout.trim()).map_err(|e| format!("stdout is not valid JSON: {e}"))
}

pub(crate) fn interp_process_run_capture_lines(
    cmd: &str,
    args: &[String],
) -> Result<Vec<String>, String> {
    let out = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    let code = out.status.code().unwrap_or(-1);
    if code != 0 {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(format!(
            "process exited with code {code} (stderr: {stderr})"
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

#[cfg(test)]
mod shell_stdlib_interp_tests {
    use super::*;

    #[test]
    fn csv_parse_and_render_roundtrip() {
        let rows = vec![vec!["a".into(), "b".into()], vec!["1".into(), "two".into()]];
        let s = interp_csv_render(&rows).unwrap();
        let v = interp_csv_parse(&s).unwrap();
        assert!(v.is_array());
    }

    /// Mirrors `examples/golden/format_conversion.vox::csv_toml_yaml_roundtrip_smoke`.
    #[test]
    fn csv_yaml_toml_roundtrip_smoke() {
        let recs = interp_csv_parse_records("k,v\na,1\n").unwrap();
        let y = interp_yaml_render(&recs).unwrap();
        let back = interp_yaml_parse(&y).unwrap();
        let t = interp_toml_render(&back).unwrap();
        interp_toml_parse(&t).expect("toml parse final step should succeed");
    }
}
