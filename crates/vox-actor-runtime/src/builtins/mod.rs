//! Standard library builtins available to compiled Vox programs.
//!
//! Three-tier hashing strategy:
//! - `vox_hash_fast`   → XXH3-128 (20-80 GB/s, non-cryptographic, 32-char hex)
//! - `vox_hash_secure` → BLAKE3   (6-12 GB/s, cryptographic, 64-char hex)
//! - `vox_uuid`        → monotonic unique ID (timestamp + atomic counter)
//! - `vox_now_ms`      → current UNIX time in milliseconds

use rust_decimal::prelude::FromStr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use vox_openclaw_runtime::{
    DefaultOpenClawRuntimeAdapter, OpenClawRuntimeAdapter, connect_default_runtime_adapter,
};

#[cfg(not(target_arch = "wasm32"))]
fn exit_commands() -> &'static std::sync::Mutex<Vec<(String, Vec<String>)>> {
    static CMDS: OnceLock<std::sync::Mutex<Vec<(String, Vec<String>)>>> = OnceLock::new();
    CMDS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_signal_handler() {
    static HANDLER_INIT: OnceLock<()> = OnceLock::new();
    HANDLER_INIT.get_or_init(|| {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                #[cfg(unix)]
                {
                    use tokio::signal::unix::{SignalKind, signal};
                    if let (Ok(mut sigint), Ok(mut sigterm)) = (
                        signal(SignalKind::interrupt()),
                        signal(SignalKind::terminate()),
                    ) {
                        tokio::select! {
                            _ = sigint.recv() => {}
                            _ = sigterm.recv() => {}
                        }
                    } else {
                        let _ = tokio::signal::ctrl_c().await;
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = tokio::signal::ctrl_c().await;
                }

                let _ = tokio::task::spawn_blocking(|| {
                    execute_exit_commands();
                })
                .await;

                std::process::exit(1);
            });
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn execute_exit_commands() {
    if let Ok(mut cmds) = exit_commands().lock() {
        for (cmd, args) in cmds.drain(..) {
            let mut c = std::process::Command::new(&cmd);
            c.args(args);
            let _ = c.status();
        }
    }
}

pub fn vox_flush_exit_commands() {
    #[cfg(not(target_arch = "wasm32"))]
    execute_exit_commands();
}

/// Fast non-cryptographic hash (XXH3-128, 128-bit output) for object identity,
/// dedup keys, and ephemeral cache keying.
///
/// Use for: HashMap keys, cache keys, dedup within a process, activity IDs
/// in hot workflow paths where you control the input (no adversarial keys).
///
/// Output: 32-char lowercase hex string (128-bit → 2 × u64 in hex).
/// Deterministic for the same input within a process; also cross-machine
/// deterministic (XXH3-128 is unkeyed / uses a fixed internal secret).
///
/// For provenance, signatures, or any security-sensitive hashing, use
/// `vox_hash_secure` (BLAKE3-based) instead.
pub fn vox_hash_fast(input: &str) -> String {
    use xxhash_rust::xxh3::xxh3_128;
    let h = xxh3_128(input.as_bytes());
    format!("{h:032x}")
}

/// Cryptographic hash using BLAKE3 (256-bit output).
///
/// Use for: `input_hash` provenance stored in DB, content-addressable IDs
/// shared across machines / process lifetimes, data integrity verification.
///
/// Output: 64-char lowercase hex string (256-bit).
/// Fully deterministic, cross-machine stable, collision probability ≈ 2^-128.
///
/// ✅ Cryptographically secure. Safe to store permanently.
pub fn vox_hash_secure(input: &str) -> String {
    let hash = blake3::hash(input.as_bytes());
    hash.to_hex().to_string()
}

/// Generate a unique identifier.
///
/// Combines nanosecond-precision UNIX timestamp with a monotonic atomic counter
/// to guarantee uniqueness even within the same nanosecond (parallel workflow steps).
///
/// Format: `vox-{nanos_hex}-{counter_hex}`
/// Example: `vox-17a8c3f2d8b00000-0000000000000001`
pub fn vox_uuid() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("vox-{:016x}-{:016x}", nanos, count)
}

/// Current UNIX time in milliseconds.
pub fn vox_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn vox_log_debug(message: &str) {
    tracing::debug!(target: "vox_actor_runtime::builtins", "{message}");
}

pub fn vox_log_info(message: &str) {
    tracing::info!(target: "vox_actor_runtime::builtins", "{message}");
}

pub fn vox_log_warn(message: &str) {
    tracing::warn!(target: "vox_actor_runtime::builtins", "{message}");
}

pub fn vox_log_error(message: &str) {
    tracing::error!(target: "vox_actor_runtime::builtins", "{message}");
}

// ── Regex (std.regex) ───────────────────────────────────────────────

/// Compiled regex value handed back to Vox as `Result[Regex]`.
#[derive(Debug, Clone)]
pub struct VoxRegex(pub regex::Regex);

/// One match in a haystack — exposes captures via `group(idx)`.
#[derive(Debug, Clone)]
pub struct VoxMatch {
    pub groups: Vec<Option<String>>,
}

impl VoxRegex {
    pub fn matches(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
    pub fn find(&self, text: &str) -> Option<VoxMatch> {
        self.0.captures(text).map(captures_to_match)
    }
    pub fn find_all(&self, text: &str) -> Vec<VoxMatch> {
        self.0.captures_iter(text).map(captures_to_match).collect()
    }
}

impl VoxMatch {
    pub fn group(&self, idx: i64) -> Option<String> {
        if idx < 0 {
            return None;
        }
        self.groups.get(idx as usize).cloned().flatten()
    }
}

fn captures_to_match(caps: regex::Captures<'_>) -> VoxMatch {
    let groups = (0..caps.len())
        .map(|i| caps.get(i).map(|m| m.as_str().to_string()))
        .collect();
    VoxMatch { groups }
}

/// Compile a Vox regex pattern. Returns `Err(message)` on invalid pattern.
pub fn vox_regex_compile(pattern: &str) -> Result<VoxRegex, String> {
    regex::Regex::new(pattern)
        .map(VoxRegex)
        .map_err(|e| e.to_string())
}

// ── JSON (std.json.parse + opaque Json with typed accessors) ────────

/// Opaque JSON value handed back to Vox as `Result[Json]`. Wraps `serde_json::Value`.
#[derive(Debug, Clone)]
pub struct VoxJson(pub serde_json::Value);

impl VoxJson {
    // ── Navigation (single hop, returns Option) ─────────────────────────
    // RFC json-ergonomics-rfc-2026-05-23 §4.2.

    /// Object key access. `None` if receiver is not an object or key is absent.
    pub fn get(&self, key: String) -> Option<VoxJson> {
        self.0
            .as_object()
            .and_then(|o| o.get(&key))
            .map(|v| VoxJson(v.clone()))
    }

    /// Array index access. `None` if receiver is not an array or index is OOB.
    /// Negative indices are not supported and yield `None`.
    pub fn at(&self, idx: i64) -> Option<VoxJson> {
        if idx < 0 {
            return None;
        }
        self.0
            .as_array()
            .and_then(|a| a.get(idx as usize))
            .map(|v| VoxJson(v.clone()))
    }

    /// RFC 6901 JSON Pointer navigation. `None` if the path does not resolve.
    /// `data.pointer("/products/0/name")` is the canonical deep-access tool.
    pub fn pointer(&self, path: String) -> Option<VoxJson> {
        self.0.pointer(&path).map(|v| VoxJson(v.clone()))
    }

    // ── Leaf coercion (returns Option[T]) ───────────────────────────────
    // RFC §4.3. None on wrong type OR is-null (mirrors serde_json's `as_*`).

    pub fn as_str(&self) -> Option<String> {
        self.0.as_str().map(|s| s.to_string())
    }
    pub fn as_int(&self) -> Option<i64> {
        self.0.as_i64()
    }
    pub fn as_float(&self) -> Option<f64> {
        self.0.as_f64()
    }
    pub fn as_bool(&self) -> Option<bool> {
        self.0.as_bool()
    }
    pub fn as_array(&self) -> Option<Vec<VoxJson>> {
        self.0
            .as_array()
            .map(|a| a.iter().map(|v| VoxJson(v.clone())).collect())
    }
    /// Asserts the receiver is a JSON object. Returns the receiver itself so
    /// chains continue navigating; use `keys()` / `has()` to inspect.
    pub fn as_object(&self) -> Option<VoxJson> {
        if self.0.is_object() {
            Some(VoxJson(self.0.clone()))
        } else {
            None
        }
    }

    // ── Inspection (no Option) ──────────────────────────────────────────
    // RFC §4.4.

    /// True iff the value is the JSON `null` literal (the *value*; not absence).
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    /// Convenience for `self.get(key).is_some()`. Sugar shipped by user
    /// request 2026-05-23.
    pub fn has(&self, key: String) -> bool {
        self.0.as_object().is_some_and(|o| o.contains_key(&key))
    }

    /// `Some(n)` when receiver is an array (n=len) or object (n=#keys);
    /// `None` for scalars/null. Changed from `i64` to `Option[i64]` in the
    /// strict-Option API — scalar receivers have no length.
    pub fn length(&self) -> Option<i64> {
        if let Some(a) = self.0.as_array() {
            Some(a.len() as i64)
        } else if let Some(o) = self.0.as_object() {
            Some(o.len() as i64)
        } else {
            None
        }
    }

    /// `Some(keys)` when receiver is an object; `None` otherwise.
    pub fn keys(&self) -> Option<Vec<String>> {
        self.0
            .as_object()
            .map(|o| o.keys().cloned().collect())
    }
}

impl std::fmt::Display for VoxJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Parse a JSON string into the opaque `VoxJson`. Returns `Err(message)` on bad JSON.
pub fn vox_json_parse(s: &str) -> Result<VoxJson, String> {
    serde_json::from_str::<serde_json::Value>(s)
        .map(VoxJson)
        .map_err(|e| e.to_string())
}

/// Render a `VoxJson` value to a JSON string.
pub fn vox_json_render(v: &VoxJson) -> Result<String, String> {
    serde_json::to_string(&v.0).map_err(|e| e.to_string())
}

/// Read a process environment variable (`std.env.get` in Vox scripts).
pub fn vox_env_get(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// List directory entries (non-recursive) as file names (`std.fs.list_dir`).
pub fn vox_list_dir(path: &str) -> Result<Vec<String>, String> {
    let rd = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        out.push(ent.file_name().to_string_lossy().into_owned());
    }
    Ok(out)
}

pub use vox_shell_stdlib_types::fs_types::VoxFileRecord;

fn vox_file_record_from_meta(
    full_path: &str,
    name: &str,
    meta: &std::fs::Metadata,
) -> VoxFileRecord {
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let ft = meta.file_type();
    let len = meta.len();
    let size = i64::try_from(len).unwrap_or(i64::MAX);
    VoxFileRecord {
        name: name.to_string(),
        path: full_path.to_string(),
        size,
        modified_ms,
        is_dir: ft.is_dir(),
        is_file: ft.is_file(),
        is_symlink: ft.is_symlink(),
    }
}

/// Structured directory listing (`std.fs.list_dir_detailed`).
pub fn vox_fs_list_dir_detailed(dir: &str) -> Result<Vec<VoxFileRecord>, String> {
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
        out.push(vox_file_record_from_meta(&full, &name, &meta));
    }
    Ok(out)
}

/// Metadata for a single path (`std.fs.stat`).
pub fn vox_fs_stat(path: &str) -> Result<VoxFileRecord, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let name = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    Ok(vox_file_record_from_meta(path, &name, &meta))
}

/// RFC 4180-style CSV parse → JSON array of string arrays (`std.csv.parse`).
pub fn vox_csv_parse(text: &str) -> Result<serde_json::Value, String> {
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

/// CSV with header row → JSON array of objects (`std.csv.parse_records`).
pub fn vox_csv_parse_records(text: &str) -> Result<serde_json::Value, String> {
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

/// Serialize rows as CSV (`std.csv.render`).
pub fn vox_csv_render(rows: &[Vec<String>]) -> Result<String, String> {
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

/// TOML documents must be tables at the root. Json arrays/scalars are wrapped for render and
/// unwrapped on parse — kept in sync with `vox_compiler::eval::shell_stdlib`.
const VOX_TOML_JSON_ROOT_KEY: &str = "__vox_json_root";

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

/// Parse TOML text to JSON (`std.toml.parse`).
pub fn vox_toml_parse(text: &str) -> Result<serde_json::Value, String> {
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

/// Render JSON as pretty TOML (`std.toml.render`).
pub fn vox_toml_render(value: &serde_json::Value) -> Result<String, String> {
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

/// Parse YAML text to JSON (`std.yaml.parse`).
pub fn vox_yaml_parse(text: &str) -> Result<serde_json::Value, String> {
    serde_yaml::from_str::<serde_json::Value>(text).map_err(|e| e.to_string())
}

/// Render JSON as YAML (`std.yaml.render`).
pub fn vox_yaml_render(value: &serde_json::Value) -> Result<String, String> {
    serde_yaml::to_string(value).map_err(|e| e.to_string())
}

fn vox_csv_save_from_json(value: &serde_json::Value) -> Result<String, String> {
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
            vox_csv_render(&out_rows)
        }
        _ => Err("csv save: expected JSON array".into()),
    }
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

/// Polymorphic read by extension (`std.io.open`).
pub fn vox_io_open(path: &str) -> Result<serde_json::Value, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    match ext.as_str() {
        "json" => serde_json::from_str(&text).map_err(|e| e.to_string()),
        "toml" => vox_toml_parse(&text),
        "yaml" | "yml" => vox_yaml_parse(&text),
        "csv" => vox_csv_parse_records(&text),
        _ => Ok(serde_json::Value::String(text)),
    }
}

/// Polymorphic write by extension (`std.io.save`).
pub fn vox_io_save(path: &str, value: &serde_json::Value) -> Result<(), String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let data: Vec<u8> = match ext.as_str() {
        "json" => serde_json::to_string_pretty(value)
            .map_err(|e| e.to_string())?
            .into_bytes(),
        "toml" => vox_toml_render(value)?.into_bytes(),
        "yaml" | "yml" => vox_yaml_render(value)?.into_bytes(),
        "csv" => vox_csv_save_from_json(value)?.into_bytes(),
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

/// Run subprocess and parse stdout as JSON (`std.process.run_capture_json`).
pub fn vox_process_run_capture_json(
    cmd: &str,
    args: &[String],
) -> Result<serde_json::Value, String> {
    let cap = vox_process_run_capture(cmd, args)?;
    serde_json::from_str(cap.stdout.trim()).map_err(|e| format!("stdout is not valid JSON: {e}"))
}

/// Run subprocess; on success return stdout split into lines (`std.process.run_capture_lines`).
pub fn vox_process_run_capture_lines(cmd: &str, args: &[String]) -> Result<Vec<String>, String> {
    let cap = vox_process_run_capture(cmd, args)?;
    if cap.exit != 0 {
        return Err(format!(
            "process exited with code {} (stderr: {})",
            cap.exit, cap.stderr
        ));
    }
    Ok(cap
        .stdout
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<_>>())
}

/// Spawn a subprocess; on success returns exit code (`std.process.run`).
///
/// Non-zero exit is surfaced as `Err` so Vox `Result` can represent failure.
pub fn vox_process_run(cmd: &str, args: &[String]) -> Result<i32, String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    let st = c.status().map_err(|e| e.to_string())?;
    if st.success() {
        Ok(st.code().unwrap_or(0))
    } else {
        Err(format!("exit status {:?}", st.code()))
    }
}

/// Resolve `cmd` on the process `PATH` (`std.process.which` in Vox scripts).
///
/// Returns the absolute path as a string when found, or `None` when not found or resolution fails.
/// This is argv-first tooling: pass a single executable name or filename, not a shell pipeline.
pub fn vox_process_which(cmd: &str) -> Option<String> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }
    which::which(cmd)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// MCP canonical tool name → ACI `mutation_kind` string (`std.agentos.mutation_kind_for_tool`).
#[must_use]
pub fn agentos_mutation_kind_for_tool(name: &str) -> String {
    vox_foundation::primitives::agentos_mutation::mutation_kind_for_tool(name).to_string()
}

/// Terminate the current process with an exit code (`std.process.exit` in Vox scripts).
pub fn vox_process_exit(code: i32) -> ! {
    std::process::exit(code)
}

/// Captured stdout/stderr/exit from a subprocess (`std.process.run_capture` in Vox scripts).
///
/// Unlike [`vox_process_run`], this always returns **`Ok`** when the process was spawned and
/// output was read; non-zero exits are represented by the `exit` field (guard-style scripts).
/// I/O or spawn failures return `Err`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxProcessCapture {
    /// Process exit code (platform convention; may be negative if no code was available).
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn vox_process_run_capture(cmd: &str, args: &[String]) -> Result<VoxProcessCapture, String> {
    vox_process_run_capture_ex(cmd, args, "", &[])
}

/// Like [`vox_process_run_capture`], with optional working directory and extra `KEY=value` env pairs.
///
/// When `cwd` is empty, the subprocess inherits the current working directory. When `env_pairs`
/// is empty, no extra variables are applied (the process still inherits the parent environment).
pub fn vox_process_run_capture_ex(
    cmd: &str,
    args: &[String],
    cwd: &str,
    env_pairs: &[String],
) -> Result<VoxProcessCapture, String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    if !cwd.is_empty() {
        c.current_dir(cwd);
    }
    for pair in env_pairs {
        if let Some((k, v)) = pair.split_once('=') {
            if !k.is_empty() {
                c.env(k, v);
            }
        }
    }
    let out = c.output().map_err(|e| e.to_string())?;
    Ok(VoxProcessCapture {
        exit: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Like [`vox_process_run`], with optional working directory and extra `KEY=value` env pairs.
pub fn vox_process_run_ex(
    cmd: &str,
    args: &[String],
    cwd: &str,
    env_pairs: &[String],
) -> Result<i32, String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    if !cwd.is_empty() {
        c.current_dir(cwd);
    }
    for pair in env_pairs {
        if let Some((k, v)) = pair.split_once('=') {
            if !k.is_empty() {
                c.env(k, v);
            }
        }
    }
    let st = c.status().map_err(|e| e.to_string())?;
    if st.success() {
        Ok(st.code().unwrap_or(0))
    } else {
        Err(format!("exit status {:?}", st.code()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn vox_process_spawn_background(cmd: &str, args: &[String]) -> Result<i64, String> {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => return Err("spawn_background must be run within a Tokio runtime".to_string()),
    };

    let mut c = tokio::process::Command::new(cmd);
    c.args(args);
    match c.spawn() {
        Ok(mut child) => {
            let id = child.id().unwrap_or(0);
            handle.spawn(async move {
                let _ = child.wait().await;
            });
            Ok(id as i64)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn vox_process_spawn_background(_cmd: &str, _args: &[String]) -> Result<i64, String> {
    Err("spawn_background is not supported in WASI scripts".to_string())
}

#[cfg(unix)]
#[cfg(not(target_arch = "wasm32"))]
pub fn vox_process_exec(cmd: &str, args: &[String]) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    let err = c.exec();
    Err(err.to_string())
}

#[cfg(not(unix))]
#[cfg(not(target_arch = "wasm32"))]
pub fn vox_process_exec(cmd: &str, args: &[String]) -> Result<(), String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    match c.status() {
        Ok(st) => {
            vox_flush_exit_commands();
            std::process::exit(st.code().unwrap_or(1))
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn vox_process_exec(_cmd: &str, _args: &[String]) -> Result<(), String> {
    Err("exec is not supported in WASI scripts".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn vox_process_register_exit_command(cmd: &str, args: &[String]) -> Result<(), String> {
    ensure_signal_handler();
    if let Ok(mut cmds) = exit_commands().lock() {
        cmds.push((cmd.to_string(), args.to_vec()));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn vox_process_register_exit_command(_cmd: &str, _args: &[String]) -> Result<(), String> {
    Err("register_exit_command is not supported in WASI scripts".to_string())
}

/// Remove a directory tree (`std.fs.remove_dir_all`).
pub fn vox_fs_remove_dir_all(path: &str) -> Result<(), String> {
    std::fs::remove_dir_all(path).map_err(|e| e.to_string())
}

/// Copy a file (`src` → `dst`).
pub fn vox_fs_copy(src: &str, dst: &str) -> Result<(), String> {
    std::fs::copy(src, dst)
        .map_err(|e| e.to_string())
        .map(|_| ())
}

/// Join path segments with the platform separator. Empty input yields `"."`.
pub fn vox_path_join_many(segments: &[String]) -> String {
    if segments.is_empty() {
        return ".".to_string();
    }
    let mut p = std::path::PathBuf::from(&segments[0]);
    for s in segments.iter().skip(1) {
        p.push(s);
    }
    p.to_string_lossy().into_owned()
}

/// Read a string field from a JSON object (top-level). Returns error if JSON is invalid or not an object.
pub fn vox_json_read_str(json: &str, key: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let obj = v
        .as_object()
        .ok_or_else(|| "JSON root must be an object".to_string())?;
    let val = obj.get(key).ok_or_else(|| format!("missing key {key:?}"))?;
    val.as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("key {key:?} is not a string"))
}

/// Read an `f64` field from a JSON object (top-level). Integers are coerced to float.
pub fn vox_json_read_f64(json: &str, key: &str) -> Result<f64, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let obj = v
        .as_object()
        .ok_or_else(|| "JSON root must be an object".to_string())?;
    let val = obj.get(key).ok_or_else(|| format!("missing key {key:?}"))?;
    val.as_f64()
        .or_else(|| val.as_i64().map(|i| i as f64))
        .ok_or_else(|| format!("key {key:?} is not a number"))
}

/// JSON-encode a string value (quotes and escapes).
pub fn vox_json_quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

/// HTTP GET text response body (`std.http.get_text`).
pub fn vox_http_get_text(url: &str) -> Result<String, String> {
    run_http_op(HttpOp::GetText {
        url: url.to_string(),
    })
}

/// HTTP POST with JSON body; returns text response body (`std.http.post_json`).
pub fn vox_http_post_json(url: &str, body_json: &str) -> Result<String, String> {
    run_http_op(HttpOp::PostJson {
        url: url.to_string(),
        body_json: body_json.to_string(),
    })
}

/// OpenClaw WS control-plane call from Vox scripts (`OpenClaw.call`).
pub fn vox_openclaw_call(method: &str, params_json: &str) -> Result<String, String> {
    run_openclaw_op(OpenClawOp::GatewayCall {
        method: method.to_string(),
        params_json: params_json.to_string(),
    })
}

/// OpenClaw convenience: list remote skills as JSON (`OpenClaw.list_skills`).
pub fn vox_openclaw_list_skills() -> Result<String, String> {
    run_openclaw_op(OpenClawOp::ListSkills)
}

/// OpenClaw convenience: subscribe domain (`OpenClaw.subscribe`).
pub fn vox_openclaw_subscribe(domain: &str) -> Result<String, String> {
    run_openclaw_op(OpenClawOp::Subscribe {
        domain: domain.to_string(),
    })
}

/// OpenClaw convenience: unsubscribe domain (`OpenClaw.unsubscribe`).
pub fn vox_openclaw_unsubscribe(domain: &str) -> Result<String, String> {
    run_openclaw_op(OpenClawOp::Unsubscribe {
        domain: domain.to_string(),
    })
}

/// OpenClaw convenience: notify domain (`OpenClaw.notify`).
pub fn vox_openclaw_notify(domain: &str, message: &str) -> Result<String, String> {
    run_openclaw_op(OpenClawOp::Notify {
        domain: domain.to_string(),
        message: message.to_string(),
    })
}

async fn connect_openclaw_adapter() -> Result<DefaultOpenClawRuntimeAdapter, String> {
    let secrets_token = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken)
        .expose()
        .map(std::string::ToString::to_string);
    connect_default_runtime_adapter(secrets_token)
        .await
        .map_err(|e| format!("openclaw adapter connect failed: {e}"))
}

enum OpenClawOp {
    GatewayCall { method: String, params_json: String },
    ListSkills,
    Subscribe { domain: String },
    Unsubscribe { domain: String },
    Notify { domain: String, message: String },
}

enum HttpOp {
    GetText { url: String },
    PostJson { url: String, body_json: String },
}

struct HttpRequest {
    op: HttpOp,
    reply_tx: std::sync::mpsc::Sender<Result<String, String>>,
}

struct HttpWorker {
    tx: std::sync::mpsc::Sender<HttpRequest>,
}

fn http_worker() -> &'static HttpWorker {
    static WORKER: OnceLock<HttpWorker> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<HttpRequest>();
        std::thread::Builder::new()
            .name("vox-http-runtime".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("http runtime init failed: {e}"));
                match runtime {
                    Ok(rt) => {
                        let client = vox_http_client::client();
                        while let Ok(req) = rx.recv() {
                            let result = rt.block_on(handle_http_op(&client, req.op));
                            let _ = req.reply_tx.send(result);
                        }
                    }
                    Err(err) => {
                        while let Ok(req) = rx.recv() {
                            let _ = req.reply_tx.send(Err(err.clone()));
                        }
                    }
                }
            })
            .expect("spawn http runtime worker");
        HttpWorker { tx }
    })
}

fn run_http_op(op: HttpOp) -> Result<String, String> {
    let worker = http_worker();
    run_http_op_with_worker(worker, op)
}

fn run_http_op_with_worker(worker: &HttpWorker, op: HttpOp) -> Result<String, String> {
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    worker
        .tx
        .send(HttpRequest { op, reply_tx })
        .map_err(|e| format!("http worker send failed: {e}"))?;
    reply_rx
        .recv()
        .map_err(|e| format!("http worker recv failed: {e}"))?
}

async fn handle_http_op(client: &reqwest::Client, op: HttpOp) -> Result<String, String> {
    match op {
        HttpOp::GetText { url } => {
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if status.is_success() {
                Ok(text)
            } else {
                Err(format!("GET {url} failed with status {status}: {text}"))
            }
        }
        HttpOp::PostJson { url, body_json } => {
            let body: serde_json::Value =
                serde_json::from_str(&body_json).map_err(|e| format!("invalid JSON body: {e}"))?;
            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if status.is_success() {
                Ok(text)
            } else {
                Err(format!("POST {url} failed with status {status}: {text}"))
            }
        }
    }
}

struct OpenClawRequest {
    op: OpenClawOp,
    reply_tx: std::sync::mpsc::Sender<Result<String, String>>,
}

struct OpenClawWorker {
    tx: std::sync::mpsc::Sender<OpenClawRequest>,
}

fn openclaw_worker() -> &'static OpenClawWorker {
    static WORKER: OnceLock<OpenClawWorker> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<OpenClawRequest>();
        std::thread::Builder::new()
            .name("vox-openclaw-runtime".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("openclaw runtime init failed: {e}"));
                match runtime {
                    Ok(rt) => {
                        let mut adapter: Option<DefaultOpenClawRuntimeAdapter> = None;
                        while let Ok(req) = rx.recv() {
                            let result = rt.block_on(handle_openclaw_op(&mut adapter, req.op));
                            let _ = req.reply_tx.send(result);
                        }
                    }
                    Err(err) => {
                        while let Ok(req) = rx.recv() {
                            let _ = req.reply_tx.send(Err(err.clone()));
                        }
                    }
                }
            })
            .expect("spawn openclaw runtime worker");
        OpenClawWorker { tx }
    })
}

fn run_openclaw_op(op: OpenClawOp) -> Result<String, String> {
    let worker = openclaw_worker();
    run_openclaw_op_with_worker(worker, op)
}

fn run_openclaw_op_with_worker(worker: &OpenClawWorker, op: OpenClawOp) -> Result<String, String> {
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    worker
        .tx
        .send(OpenClawRequest { op, reply_tx })
        .map_err(|e| format!("openclaw worker send failed: {e}"))?;
    reply_rx
        .recv()
        .map_err(|e| format!("openclaw worker recv failed: {e}"))?
}

async fn handle_openclaw_op(
    adapter: &mut Option<DefaultOpenClawRuntimeAdapter>,
    op: OpenClawOp,
) -> Result<String, String> {
    match op {
        OpenClawOp::GatewayCall {
            method,
            params_json,
        } => {
            let params = serde_json::from_str::<serde_json::Value>(&params_json)
                .map_err(|e| format!("invalid params_json: {e}"))?;
            if adapter.is_none() {
                *adapter = Some(connect_openclaw_adapter().await?);
            }
            let adapter = adapter
                .as_mut()
                .ok_or_else(|| "openclaw adapter unavailable".to_string())?;
            let payload = adapter
                .gateway_call(&method, params)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&payload).map_err(|e| e.to_string())
        }
        OpenClawOp::ListSkills => {
            if adapter.is_none() {
                *adapter = Some(connect_openclaw_adapter().await?);
            }
            let adapter = adapter
                .as_mut()
                .ok_or_else(|| "openclaw adapter unavailable".to_string())?;
            let skills = adapter
                .list_remote_skills()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&serde_json::json!({ "skills": skills }))
                .map_err(|e| e.to_string())
        }
        OpenClawOp::Subscribe { domain } => {
            if adapter.is_none() {
                *adapter = Some(connect_openclaw_adapter().await?);
            }
            let adapter = adapter
                .as_mut()
                .ok_or_else(|| "openclaw adapter unavailable".to_string())?;
            let payload = adapter
                .subscribe_domain(&domain)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&payload).map_err(|e| e.to_string())
        }
        OpenClawOp::Unsubscribe { domain } => {
            if adapter.is_none() {
                *adapter = Some(connect_openclaw_adapter().await?);
            }
            let adapter = adapter
                .as_mut()
                .ok_or_else(|| "openclaw adapter unavailable".to_string())?;
            let payload = adapter
                .unsubscribe_domain(&domain)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&payload).map_err(|e| e.to_string())
        }
        OpenClawOp::Notify { domain, message } => {
            if adapter.is_none() {
                *adapter = Some(connect_openclaw_adapter().await?);
            }
            let adapter = adapter
                .as_mut()
                .ok_or_else(|| "openclaw adapter unavailable".to_string())?;
            let payload = adapter
                .notify_domain(&domain, &message)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&payload).map_err(|e| e.to_string())
        }
    }
}

// ── Browser (CDP / chromiumoxide; dispatches through vox-plugin-host) ─────────
//
// The BrowserAutomation sabi trait is synchronous — the plugin manages its own
// tokio runtime internally. We no longer need a dedicated background thread or
// channel machinery here.

fn with_browser_backend<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&vox_plugin_host::loader::LoadedCodePlugin) -> Result<T, String>,
{
    let plugin = vox_plugin_host::cached_code_plugin("browser")
        .map_err(|e| format!("browser plugin load: {e}"))?;
    if plugin
        .plugin
        .as_browser_automation()
        .into_option()
        .is_none()
    {
        return Err("browser plugin loaded but BrowserAutomation accessor returned None".into());
    }
    f(plugin)
}

macro_rules! browser_call {
    ($result:expr) => {
        $result.into_result().map_err(|e| format!("browser: {e}"))
    };
}

/// `Browser.open` — returns opaque `page_id` (`Result[str]` in Vox).
pub fn vox_browser_open(url: &str, headless: bool) -> Result<String, String> {
    let url = url.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.open(url.as_str().into(), headless)).map(|s| s.into_string())
    })
}

pub fn vox_browser_close(page_id: &str) -> Result<(), String> {
    let page_id = page_id.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.close(page_id.as_str().into()))
    })
}

pub fn vox_browser_goto(page_id: &str, url: &str) -> Result<(), String> {
    let page_id = page_id.to_string();
    let url = url.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.goto(page_id.as_str().into(), url.as_str().into()))
    })
}

pub fn vox_browser_click(page_id: &str, target: &str) -> Result<(), String> {
    let page_id = page_id.to_string();
    let target = target.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.click(page_id.as_str().into(), target.as_str().into()))
    })
}

pub fn vox_browser_fill(page_id: &str, target: &str, value: &str) -> Result<(), String> {
    let page_id = page_id.to_string();
    let target = target.to_string();
    let value = value.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.fill(
            page_id.as_str().into(),
            target.as_str().into(),
            value.as_str().into()
        ))
    })
}

pub fn vox_browser_wait_for(page_id: &str, target: &str, timeout_secs: u64) -> Result<(), String> {
    let page_id = page_id.to_string();
    let target = target.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.wait_for(
            page_id.as_str().into(),
            target.as_str().into(),
            timeout_secs
        ))
    })
}

pub fn vox_browser_text(page_id: &str, target: &str) -> Result<String, String> {
    let page_id = page_id.to_string();
    let target = target.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.text(page_id.as_str().into(), target.as_str().into()))
            .map(|s| s.into_string())
    })
}

pub fn vox_browser_html(page_id: &str, target: &str) -> Result<String, String> {
    let page_id = page_id.to_string();
    let target = target.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.html(page_id.as_str().into(), target.as_str().into()))
            .map(|s| s.into_string())
    })
}

pub fn vox_browser_screenshot(page_id: &str, path: &str) -> Result<String, String> {
    let page_id = page_id.to_string();
    let path = path.to_string();
    with_browser_backend(|p| {
        let b = p
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("checked above");
        browser_call!(b.screenshot(page_id.as_str().into(), path.as_str().into()))
            .map(|s| s.into_string())
    })
}

/// Expand a glob pattern and return sorted paths as strings (`std.fs.glob`).
///
/// Patterns follow the Rust [`glob`] crate (e.g. `*.rs`, `target/**/*.toml`). Invalid patterns
/// return `Err`.
pub fn vox_fs_glob(pattern: &str) -> Result<Vec<String>, String> {
    let mut paths: Vec<String> = Vec::new();
    for entry in glob::glob(pattern).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?;
        paths.push(p.to_string_lossy().into_owned());
    }
    paths.sort();
    Ok(paths)
}

pub fn vox_fs_read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

pub fn vox_fs_write(path: &str, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| e.to_string())
}

/// Create a directory recursively (`std.fs.mkdir`).
pub fn vox_fs_mkdir(path: &str) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())
}

// ── Phase K stdlib parity (2026-05-23) ──────────────────────────────────
// 14 primitives bringing `--mode script` (actor-runtime) up to parity with
// `--mode interp` (eval). Punchlist source:
// `docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md` §Phase K.

/// `std.fs.exists(path) -> bool` — does the path exist on disk?
pub fn vox_fs_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

/// `std.fs.is_file(path) -> bool` — exists AND is a regular file.
pub fn vox_fs_is_file(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

/// `std.fs.is_dir(path) -> bool` — exists AND is a directory.
pub fn vox_fs_is_dir(path: &str) -> bool {
    std::path::Path::new(path).is_dir()
}

/// `std.fs.remove(path) -> Result[Unit]` — delete a file (NOT a directory;
/// use `vox_fs_remove_dir_all` for trees).
pub fn vox_fs_remove(path: &str) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

/// `std.path.extension(p) -> Option[str]` — file extension without the dot;
/// `None` if the path has no extension.
pub fn vox_path_extension(p: &str) -> Option<String> {
    std::path::Path::new(p)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// `std.path.parent(p) -> Option[str]` — parent directory of a path; `None`
/// for the root or empty path.
pub fn vox_path_parent(p: &str) -> Option<String> {
    std::path::Path::new(p)
        .parent()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
}

/// `std.path.file_name(p) -> Option[str]` — final component of a path,
/// including extension; `None` if the path ends in `..`.
pub fn vox_path_file_name(p: &str) -> Option<String> {
    std::path::Path::new(p)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// `std.path.stem(p) -> Option[str]` — final component without the extension.
pub fn vox_path_stem(p: &str) -> Option<String> {
    std::path::Path::new(p)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// `std.path.is_absolute(p) -> bool` — true when the path is absolute on the
/// current platform (drive letter on Windows, leading `/` on Unix).
pub fn vox_path_is_absolute(p: &str) -> bool {
    std::path::Path::new(p).is_absolute()
}

/// `std.path.resolve(p) -> Result[str]` — canonicalize the path against the
/// current working directory. Errors when the path does not exist.
pub fn vox_path_resolve(p: &str) -> Result<String, String> {
    std::fs::canonicalize(p)
        .map_err(|e| e.to_string())
        .and_then(|pb| {
            pb.to_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "vox_path_resolve: non-utf8 path".to_string())
        })
}

/// `std.env.args() -> list[str]` — process command-line arguments
/// (including the program name at index 0, matching `std::env::args` Rust
/// behavior).
pub fn vox_env_args() -> Vec<String> {
    std::env::args().collect()
}

/// `std.env.set(key, value)` — set an environment variable for the current
/// process and its children. Unconditional; matches `std::env::set_var`.
#[allow(unsafe_code)]
pub fn vox_env_set(key: &str, value: &str) {
    // SAFETY: `set_var` was marked unsafe in Rust 2024 due to multithreaded
    // hazards. Vox scripts call this from the main thread before spawning
    // subprocesses, matching the documented safe pattern.
    unsafe { std::env::set_var(key, value); }
}

/// `std.regex.replace(pattern, haystack, replacement) -> Result[str]` —
/// replace all matches of `pattern` in `haystack` with `replacement`.
pub fn vox_regex_replace(
    pattern: &str,
    haystack: &str,
    replacement: &str,
) -> Result<String, String> {
    let re = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
    Ok(re.replace_all(haystack, replacement).to_string())
}

/// `std.regex.find(pattern, haystack) -> Result[Option[str]]` — first match
/// substring; `Ok(None)` when no match, `Err(msg)` only on bad pattern.
pub fn vox_regex_find(
    pattern: &str,
    haystack: &str,
) -> Result<Option<String>, String> {
    let re = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
    Ok(re.find(haystack).map(|m| m.as_str().to_string()))
}

/// Convert a `dec` value to a string (`std.dec.to_string`).
pub fn vox_dec_to_str(d: rust_decimal::Decimal) -> String {
    d.to_string()
}

/// Parse a string into a `dec` value (`std.dec.from_str`).
pub fn vox_str_to_dec(s: &str) -> Result<rust_decimal::Decimal, String> {
    rust_decimal::Decimal::from_str(s).map_err(|e| e.to_string())
}

/// Return JSON list of runtime capabilities (`std.meta.capabilities`).
pub fn vox_meta_capabilities() -> String {
    let mut caps = vec!["hashing", "fs", "process", "http"];
    #[cfg(feature = "database")]
    caps.push("database");

    if std::env::var("VOX_CHROME_EXECUTABLE").is_ok()
        || which::which("google-chrome").is_ok()
        || which::which("chromium").is_ok()
    {
        caps.push("browser");
    }

    serde_json::to_string(&caps).unwrap_or_else(|_| "[]".to_string())
}

/// Return JSON list of registered tools (`std.meta.tools`).
pub fn vox_meta_tools() -> String {
    // Note: To avoid circular dependency on vox-mcp-registry, this currently returns an empty list
    // until the registry is flattened or moved to a core crate.
    "[]".to_string()
}

#[cfg(test)]
mod tests;
