use super::shell_stdlib::{
    interp_csv_parse, interp_csv_parse_records, interp_csv_render, interp_fs_list_dir_detailed,
    interp_fs_stat, interp_io_open, interp_io_save, interp_process_run_capture_json,
    interp_process_run_capture_lines, interp_toml_parse, interp_toml_render, interp_yaml_parse,
    interp_yaml_render,
};
use super::value::VoxValue;
use secrecy::ExposeSecret;
use std::sync::Mutex;
use std::sync::OnceLock;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn exit_commands() -> &'static Mutex<Vec<(String, Vec<String>)>> {
    static CMDS: OnceLock<Mutex<Vec<(String, Vec<String>)>>> = OnceLock::new();
    CMDS.get_or_init(|| Mutex::new(Vec::new()))
}

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
    execute_exit_commands();
}

fn voxvalue_as_table_str(v: &VoxValue) -> Option<Vec<Vec<String>>> {
    let VoxValue::List(rows) = v else {
        return None;
    };
    let mut out = Vec::new();
    for row in rows {
        let VoxValue::List(cells) = row else {
            return None;
        };
        let mut line = Vec::new();
        for c in cells {
            let VoxValue::Str(s) = c else {
                return None;
            };
            line.push(s.clone());
        }
        out.push(line);
    }
    Some(out)
}

/// Dispatch a method call on a runtime value. Returns `None` if the method is
/// not known — callers should surface a user-visible `MethodNotFound` error.
pub fn call_builtin_method(
    obj: &VoxValue,
    method: &str,
    args: Vec<VoxValue>,
    caps: Option<&std::collections::HashSet<String>>,
) -> Option<VoxValue> {
    match obj {
        // ── List ──────────────────────────────────────────────────────
        VoxValue::List(v) => match method {
            "len" => Some(VoxValue::Int(v.len() as i64)),
            "is_empty" => Some(VoxValue::Bool(v.is_empty())),
            "push" => {
                let mut owned = v.clone();
                if let Some(val) = args.into_iter().next() {
                    owned.push(val);
                }
                Some(VoxValue::List(owned))
            }
            "pop" => {
                let mut owned = v.clone();
                let popped = owned.pop().unwrap_or(VoxValue::Null);
                Some(popped)
            }
            "get" => {
                let idx = args.into_iter().next()?;
                if let VoxValue::Int(i) = idx {
                    let val = v.get(i as usize).cloned().map(Box::new);
                    Some(VoxValue::Option(val))
                } else {
                    Some(VoxValue::Option(None))
                }
            }
            "first" => Some(v.first().cloned().unwrap_or(VoxValue::Null)),
            "last" => Some(v.last().cloned().unwrap_or(VoxValue::Null)),
            "contains" => {
                let target = args.into_iter().next().unwrap_or(VoxValue::Null);
                Some(VoxValue::Bool(v.contains(&target)))
            }
            "join" => {
                let sep = match args.into_iter().next() {
                    Some(VoxValue::Str(s)) => s,
                    _ => String::new(),
                };
                let strings: Vec<String> = v.iter().map(|x| format!("{x:?}")).collect();
                Some(VoxValue::Str(strings.join(&sep)))
            }
            "reverse" => {
                let mut owned = v.clone();
                owned.reverse();
                Some(VoxValue::List(owned))
            }
            // Json-shaped arrays (`std.json.parse`, `std.csv.parse`, …) use these names in typecheck + native `VoxJson`.
            "length" => Some(VoxValue::Int(v.len() as i64)),
            "at" => {
                let idx = match args.first().cloned() {
                    Some(VoxValue::Int(i)) => i,
                    _ => {
                        return Some(VoxValue::Result(Err(
                            "json: invalid array index".into(),
                        )));
                    }
                };
                if idx < 0 {
                    return Some(VoxValue::Result(Err(format!(
                        "json: negative array index {idx}"
                    ))));
                }
                let i = idx as usize;
                match v.get(i) {
                    Some(el) => Some(VoxValue::Result(Ok(Box::new(el.clone())))),
                    None => Some(VoxValue::Result(Err(format!(
                        "json: index {idx} out of bounds (len={})",
                        v.len()
                    )))),
                }
            }
            "is_null" => Some(VoxValue::Bool(false)),
            "keys" => Some(VoxValue::List(vec![])),
            "to_string" => {
                let j = vox_to_json(VoxValue::List(v.clone()));
                Some(VoxValue::Str(serde_json::to_string(&j).unwrap_or_default()))
            }
            "get_str" | "get_int" | "get_float" | "get_bool" | "get_object" | "get_array" => {
                Some(VoxValue::Result(Err(
                    "json: receiver is not an object".into(),
                )))
            }
            _ => None,
        },

        // ── Str ───────────────────────────────────────────────────────
        VoxValue::Str(s) => match method {
            "len" => Some(VoxValue::Int(s.len() as i64)),
            "is_empty" => Some(VoxValue::Bool(s.is_empty())),
            "to_upper" | "to_uppercase" => Some(VoxValue::Str(s.to_uppercase())),
            "to_lower" | "to_lowercase" => Some(VoxValue::Str(s.to_lowercase())),
            "trim" => Some(VoxValue::Str(s.trim().to_string())),
            "trim_start" => Some(VoxValue::Str(s.trim_start().to_string())),
            "trim_end" => Some(VoxValue::Str(s.trim_end().to_string())),
            "contains" => {
                let needle = match args.into_iter().next() {
                    Some(VoxValue::Str(n)) => n,
                    _ => return Some(VoxValue::Bool(false)),
                };
                Some(VoxValue::Bool(s.contains(&*needle)))
            }
            "starts_with" => {
                let prefix = match args.into_iter().next() {
                    Some(VoxValue::Str(p)) => p,
                    _ => return Some(VoxValue::Bool(false)),
                };
                Some(VoxValue::Bool(s.starts_with(&*prefix)))
            }
            "ends_with" => {
                let suffix = match args.into_iter().next() {
                    Some(VoxValue::Str(sf)) => sf,
                    _ => return Some(VoxValue::Bool(false)),
                };
                Some(VoxValue::Bool(s.ends_with(&*suffix)))
            }
            "split" => {
                let delim = match args.into_iter().next() {
                    Some(VoxValue::Str(d)) => d,
                    _ => " ".to_string(),
                };
                let parts: Vec<VoxValue> = s
                    .split(&*delim)
                    .map(|p| VoxValue::Str(p.to_string()))
                    .collect();
                Some(VoxValue::List(parts))
            }
            "replace" => {
                let mut it = args.into_iter();
                let from = match it.next() {
                    Some(VoxValue::Str(f)) => f,
                    _ => return Some(VoxValue::Str(s.clone())),
                };
                let to = match it.next() {
                    Some(VoxValue::Str(t)) => t,
                    _ => String::new(),
                };
                Some(VoxValue::Str(s.replace(&*from, &to)))
            }
            "repeat" => {
                let n = match args.into_iter().next() {
                    Some(VoxValue::Int(n)) => n as usize,
                    _ => 1,
                };
                Some(VoxValue::Str(s.repeat(n)))
            }
            "chars_count" => Some(VoxValue::Int(s.chars().count() as i64)),
            "to_str" | "to_string" => Some(VoxValue::Str(s.clone())),
            "slice" => {
                let mut it = args.into_iter();
                let start = match it.next() {
                    Some(VoxValue::Int(n)) => n.max(0) as usize,
                    _ => 0,
                };
                let end = match it.next() {
                    Some(VoxValue::Int(n)) => n.max(0) as usize,
                    _ => s.chars().count(),
                };
                let end = end.min(s.chars().count());
                let start = start.min(end);
                let out: String = s.chars().skip(start).take(end - start).collect();
                Some(VoxValue::Str(out))
            }
            "char_at" => {
                let idx = match args.into_iter().next() {
                    Some(VoxValue::Int(n)) if n >= 0 => n as usize,
                    _ => return Some(VoxValue::Option(None)),
                };
                match s.chars().nth(idx) {
                    Some(c) => Some(VoxValue::Option(Some(Box::new(VoxValue::Str(
                        c.to_string(),
                    ))))),
                    None => Some(VoxValue::Option(None)),
                }
            }
            "index_of" => {
                let needle = match args.into_iter().next() {
                    Some(VoxValue::Str(n)) => n,
                    _ => return Some(VoxValue::Option(None)),
                };
                match s.find(&*needle) {
                    Some(byte_pos) => {
                        // Convert byte position to char index for Vox semantics.
                        let char_idx = s[..byte_pos].chars().count() as i64;
                        Some(VoxValue::Option(Some(Box::new(VoxValue::Int(char_idx)))))
                    }
                    None => Some(VoxValue::Option(None)),
                }
            }
            _ => None,
        },

        // ── Int ───────────────────────────────────────────────────────
        VoxValue::Int(n) => match method {
            "to_str" | "to_string" => Some(VoxValue::Str(n.to_string())),
            "abs" => Some(VoxValue::Int(n.unsigned_abs() as i64)),
            "min" => {
                let other = match args.into_iter().next() {
                    Some(VoxValue::Int(m)) => m,
                    _ => *n,
                };
                Some(VoxValue::Int(*n.min(&other)))
            }
            "max" => {
                let other = match args.into_iter().next() {
                    Some(VoxValue::Int(m)) => m,
                    _ => *n,
                };
                Some(VoxValue::Int(*n.max(&other)))
            }
            _ => None,
        },

        // ── Float ─────────────────────────────────────────────────────
        VoxValue::Float(f) => match method {
            "to_str" | "to_string" => Some(VoxValue::Str(f.to_string())),
            "abs" => Some(VoxValue::Float(f.abs())),
            "floor" => Some(VoxValue::Float(f.floor())),
            "ceil" => Some(VoxValue::Float(f.ceil())),
            "round" => Some(VoxValue::Float(f.round())),
            "sqrt" => Some(VoxValue::Float(f.sqrt())),
            _ => None,
        },

        // ── Bool ──────────────────────────────────────────────────────
        VoxValue::Bool(b) => match method {
            "to_str" | "to_string" => Some(VoxValue::Str(b.to_string())),
            _ => None,
        },
        // ── Option ───────────────────────────────────────────────────
        VoxValue::Option(opt) => match method {
            "is_some" => Some(VoxValue::Bool(opt.is_some())),
            "is_none" => Some(VoxValue::Bool(opt.is_none())),
            // `unwrap()` panics on None. The _Panic sentinel is caught
            // upstream in `eval/expr.rs` and converted to an EvalError.
            // Prior behavior (returning Null on None) was a silent-wrong-
            // output footgun; see audit doc §10.4.
            "unwrap" => Some(match opt.as_ref() {
                Some(v) => (**v).clone(),
                None => VoxValue::_Panic(
                    "called `Option.unwrap()` on a None value".to_string(),
                ),
            }),
            // `unwrap_or(default)` — never panics; this is the "safe" form.
            "unwrap_or" => {
                let default = args.into_iter().next().unwrap_or(VoxValue::Null);
                Some(
                    opt.as_ref()
                        .map(|v| (**v).clone())
                        .unwrap_or(default),
                )
            }
            // `unwrap_or_default` — interp uses Null as the universal default
            // since we don't track per-type Default impls. Safe form.
            "unwrap_or_default" => Some(
                opt.as_ref()
                    .map(|v| (**v).clone())
                    .unwrap_or(VoxValue::Null),
            ),
            // `expect(msg)` — like `unwrap` but uses the supplied message
            // in the panic. The whole point of `expect` is to give the
            // programmer a chance to explain WHY they're unwrapping.
            "expect" => Some(match opt.as_ref() {
                Some(v) => (**v).clone(),
                None => {
                    let msg = match args.into_iter().next() {
                        Some(VoxValue::Str(s)) => s,
                        _ => "expected Some, found None".to_string(),
                    };
                    VoxValue::_Panic(format!("Option.expect: {msg}"))
                }
            }),
            _ => None,
        },
        // ── Result ───────────────────────────────────────────────────
        VoxValue::Result(res) => match method {
            "is_ok" => Some(VoxValue::Bool(res.is_ok())),
            "is_err" => Some(VoxValue::Bool(res.is_err())),
            // `ok()` — returns Some(value) for Ok, None for Err.
            "ok" => Some(VoxValue::Option(
                res.as_ref().ok().map(|v| Box::new((**v).clone())),
            )),
            // `err()` — returns Some(err_msg) for Err, None for Ok.
            "err" => Some(VoxValue::Option(
                res.as_ref()
                    .err()
                    .map(|e| Box::new(VoxValue::Str(e.clone()))),
            )),
            // `unwrap()` panics on Err with the Err message. The _Panic
            // sentinel is caught upstream and converted to an EvalError.
            "unwrap" => Some(match res.as_ref() {
                Ok(v) => (**v).clone(),
                Err(e) => VoxValue::_Panic(format!(
                    "called `Result.unwrap()` on an Err value: {e}"
                )),
            }),
            // `unwrap_err()` panics on Ok — the inverse of unwrap. Prior
            // impl returned empty Str on Ok, masking the misuse silently.
            "unwrap_err" => Some(match res.as_ref() {
                Err(e) => VoxValue::Str(e.clone()),
                Ok(_) => VoxValue::_Panic(
                    "called `Result.unwrap_err()` on an Ok value".to_string(),
                ),
            }),
            "unwrap_or" => {
                let default = args.into_iter().next().unwrap_or(VoxValue::Null);
                Some(
                    res.as_ref()
                        .ok()
                        .map(|v| (**v).clone())
                        .unwrap_or(default),
                )
            }
            "unwrap_or_default" => Some(
                res.as_ref()
                    .ok()
                    .map(|v| (**v).clone())
                    .unwrap_or(VoxValue::Null),
            ),
            // `expect(msg)` panics on Err with the supplied context.
            "expect" => Some(match res.as_ref() {
                Ok(v) => (**v).clone(),
                Err(e) => {
                    let ctx = match args.into_iter().next() {
                        Some(VoxValue::Str(s)) => s,
                        _ => "expected Ok, found Err".to_string(),
                    };
                    VoxValue::_Panic(format!("Result.expect: {ctx} ({e})"))
                }
            }),
            _ => None,
        },

        // ── Object (including Namespaces) ───────────────────────────
        VoxValue::Object(fields) => {
            let ns = fields
                .iter()
                .find(|(k, _)| k == "__namespace__")
                .and_then(|(_, v)| {
                    if let VoxValue::Str(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

            if ns.is_none() && method == "get" {
                let key = match args.into_iter().next() {
                    Some(VoxValue::Str(s)) => s,
                    _ => return Some(VoxValue::Option(None)),
                };
                // Record/Object.get returns Option[T] — matches the typecheck
                // signature so corpus scripts that call `.unwrap()` on the
                // result type-check AND run consistently. Prior to 2026-05-23
                // this returned the bare value (or Null on miss), which
                // typecheck-eval'd inconsistently.
                return Some(
                    fields
                        .iter()
                        .find(|(k, _)| k == &key)
                        .map(|(_, v)| VoxValue::Option(Some(Box::new(v.clone()))))
                        .unwrap_or(VoxValue::Option(None)),
                );
            }

            if let Some(ns_str) = ns
                && let Some(c) = caps
                && matches!(ns_str, "fs" | "io" | "process" | "env" | "secrets")
            {
                let ok = (ns_str == "fs" || ns_str == "io") && c.contains("fs")
                    || ns_str == "process" && (c.contains("process") || c.contains("subprocess"))
                    || ns_str == "env" && c.contains("env")
                    || ns_str == "secrets" && c.contains("secrets");
                if !ok {
                    println!(
                        "Capability denied: script missing capability for '{ns_str}' namespace"
                    );
                    return Some(VoxValue::Null);
                }
            }

            match ns {
                Some("fs") => match method {
                    // `read_to_string` is the Rust-style alias; `read` and
                    // `read_file` are the canonical Vox names. All three
                    // share the same impl per audit doc §10.4.
                    "read" | "read_file" | "read_to_string" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::read_to_string(path) {
                            Ok(s) => Ok(Box::new(VoxValue::Str(s))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    // `write_to_file` is the Rust-style alias of write/write_file.
                    "write" | "write_file" | "write_to_file" => {
                        let mut it = args.into_iter();
                        let path = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let content = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::write(path, content) {
                            Ok(_) => Ok(Box::new(VoxValue::Bool(true))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    // `cwd` — current working directory. Mirrors
                    // `std::env::current_dir()`. Returns a Result because
                    // the OS can deny access to the cwd (unlikely but
                    // surfaceable).
                    "cwd" => {
                        let res = match std::env::current_dir() {
                            Ok(p) => Ok(Box::new(VoxValue::Str(
                                p.to_string_lossy().to_string(),
                            ))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    // `copy(src, dst)` — copies a file. Audit doc §10
                    // confirmed this as a needed primitive (no good substitute).
                    "copy" => {
                        let mut it = args.into_iter();
                        let src = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let dst = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::copy(&src, &dst) {
                            Ok(_) => Ok(Box::new(VoxValue::Bool(true))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    // `remove(path)` — deletes a file. For directories use
                    // `remove_dir_all` (already registered).
                    "remove" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::remove_file(&path) {
                            Ok(_) => Ok(Box::new(VoxValue::Bool(true))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    // `walk(dir)` — recursive lister. Eval delegates to
                    // the glob impl via `**/*` since fs.walk and fs.glob are
                    // the same operation conceptually (audit doc §11).
                    // Kept as an alias rather than dropped because two
                    // scripts in `mens-corpus/` already use this name; the
                    // alias avoids unnecessary corpus churn.
                    "walk" | "list_recursive" => {
                        let root = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let pattern = format!("{root}/**/*");
                        let mut entries: Vec<VoxValue> = Vec::new();
                        match glob::glob(&pattern) {
                            Ok(it) => {
                                for entry in it.flatten() {
                                    if entry.is_file() {
                                        entries.push(VoxValue::Str(
                                            entry.to_string_lossy().to_string(),
                                        ));
                                    }
                                }
                                Some(VoxValue::Result(Ok(Box::new(VoxValue::List(entries)))))
                            }
                            Err(e) => Some(VoxValue::Result(Err(e.to_string()))),
                        }
                    }
                    "exists" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Bool(false)),
                        };
                        Some(VoxValue::Bool(std::path::Path::new(&path).exists()))
                    }
                    "is_file" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Bool(false)),
                        };
                        Some(VoxValue::Bool(std::path::Path::new(&path).is_file()))
                    }
                    "is_dir" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Bool(false)),
                        };
                        Some(VoxValue::Bool(std::path::Path::new(&path).is_dir()))
                    }
                    "remove_dir_all" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::remove_dir_all(&path) {
                            Ok(()) => Ok(Box::new(VoxValue::Null)),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    "list_dir" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => ".".to_string(),
                        };
                        let res = if let Ok(entries) = std::fs::read_dir(path) {
                            let list: Vec<VoxValue> = entries
                                .filter_map(|e| e.ok())
                                .map(|e| VoxValue::Str(e.file_name().to_string_lossy().to_string()))
                                .collect();
                            Ok(Box::new(VoxValue::List(list)))
                        } else {
                            Err("failed to list directory".to_string())
                        };
                        Some(VoxValue::Result(res))
                    }
                    "glob" => {
                        let pattern = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match glob::glob(&pattern) {
                            Ok(paths) => {
                                let list: Vec<VoxValue> = paths
                                    .filter_map(
                                        |p: std::result::Result<
                                            std::path::PathBuf,
                                            glob::GlobError,
                                        >| p.ok(),
                                    )
                                    .map(|p: std::path::PathBuf| {
                                        VoxValue::Str(p.to_string_lossy().to_string())
                                    })
                                    .collect();
                                Ok(Box::new(VoxValue::List(list)))
                            }
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    "list_dir_detailed" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match interp_fs_list_dir_detailed(&path) {
                            Ok(rows) => {
                                let list: Vec<VoxValue> = rows
                                    .into_iter()
                                    .map(|r| {
                                        VoxValue::Object(vec![
                                            ("name".into(), VoxValue::Str(r.name)),
                                            ("path".into(), VoxValue::Str(r.path)),
                                            ("size".into(), VoxValue::Int(r.size)),
                                            ("modified_ms".into(), VoxValue::Int(r.modified_ms)),
                                            ("is_dir".into(), VoxValue::Bool(r.is_dir)),
                                            ("is_file".into(), VoxValue::Bool(r.is_file)),
                                            ("is_symlink".into(), VoxValue::Bool(r.is_symlink)),
                                        ])
                                    })
                                    .collect();
                                Ok(Box::new(VoxValue::List(list)))
                            }
                            Err(e) => Err(e),
                        };
                        Some(VoxValue::Result(res))
                    }
                    "stat" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match interp_fs_stat(&path) {
                            Ok(r) => Ok(Box::new(VoxValue::Object(vec![
                                ("name".into(), VoxValue::Str(r.name)),
                                ("path".into(), VoxValue::Str(r.path)),
                                ("size".into(), VoxValue::Int(r.size)),
                                ("modified_ms".into(), VoxValue::Int(r.modified_ms)),
                                ("is_dir".into(), VoxValue::Bool(r.is_dir)),
                                ("is_file".into(), VoxValue::Bool(r.is_file)),
                                ("is_symlink".into(), VoxValue::Bool(r.is_symlink)),
                            ]))),
                            Err(e) => Err(e),
                        };
                        Some(VoxValue::Result(res))
                    }
                    "mkdir" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::create_dir_all(&path) {
                            Ok(()) => Ok(Box::new(VoxValue::Bool(true))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    _ => None,
                },
                Some("env") => match method {
                    "get" => {
                        let name = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let val = std::env::var(name).ok().map(|s| Box::new(VoxValue::Str(s)));
                        Some(VoxValue::Option(val))
                    }
                    "args" => {
                        let args: Vec<VoxValue> = std::env::args().map(VoxValue::Str).collect();
                        Some(VoxValue::List(args))
                    }
                    "set" => {
                        let mut it = args.into_iter();
                        let key = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let val = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let _guard = ENV_MUTEX.lock().unwrap();
                        #[allow(unsafe_code)]
                        // SAFETY: Access to environment variables is synchronized via ENV_MUTEX
                        // to avoid data races in multi-threaded contexts as required by Rust 1.81+.
                        unsafe {
                            std::env::set_var(key, val);
                        }
                        Some(VoxValue::Null)
                    }
                    _ => None,
                },
                Some("path") => match method {
                    "join" => {
                        let mut it = args.into_iter();
                        let a = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let b = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let joined = std::path::Path::new(&a).join(b);
                        Some(VoxValue::Str(joined.to_string_lossy().to_string()))
                    }
                    "extension" => {
                        let p = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Str(String::new())),
                        };
                        let ext = std::path::Path::new(&p)
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        Some(VoxValue::Str(ext))
                    }
                    "parent" => {
                        let p = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Str(String::new())),
                        };
                        let parent = std::path::Path::new(&p)
                            .parent()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        Some(VoxValue::Str(parent))
                    }
                    "file_name" => {
                        let p = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Str(String::new())),
                        };
                        let name = std::path::Path::new(&p)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        Some(VoxValue::Str(name))
                    }
                    "stem" => {
                        let p = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Str(String::new())),
                        };
                        let stem = std::path::Path::new(&p)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        Some(VoxValue::Str(stem))
                    }
                    "is_absolute" => {
                        let p = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Bool(false)),
                        };
                        Some(VoxValue::Bool(std::path::Path::new(&p).is_absolute()))
                    }
                    _ => None,
                },
                Some("secrets") => match method {
                    "resolve" => {
                        let name = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };

                        let id = match std::str::FromStr::from_str(&name) {
                            Ok(id) => id,
                            Err(_) => return Some(VoxValue::Null),
                        };

                        let resolved = vox_secrets::resolve_secret_with_context(id, "script");
                        if let Some(val) = resolved.value {
                            Some(VoxValue::Str(val.expose_secret().to_string()))
                        } else {
                            Some(VoxValue::Null)
                        }
                    }
                    _ => None,
                },
                Some("process") => match method {
                    "spawn" | "run" => {
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| {
                                    if let VoxValue::Str(s) = v {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };

                        let output = std::process::Command::new(cmd_name).args(cmd_args).output();

                        match output {
                            Ok(out) => {
                                let res = vec![
                                    (
                                        "stdout".to_string(),
                                        VoxValue::Str(
                                            String::from_utf8_lossy(&out.stdout).to_string(),
                                        ),
                                    ),
                                    (
                                        "stderr".to_string(),
                                        VoxValue::Str(
                                            String::from_utf8_lossy(&out.stderr).to_string(),
                                        ),
                                    ),
                                    (
                                        "code".to_string(),
                                        VoxValue::Int(out.status.code().unwrap_or(0) as i64),
                                    ),
                                ];
                                // Wrap in Option(Some(...)) to match the
                                // typecheck signature `Option[Record]`.
                                // Prior to 2026-05-23 this returned the bare
                                // Object, causing scripts that followed the
                                // typeck contract (`proc.unwrap()`) to fail
                                // at eval with "Method unwrap not found"
                                // even though their `vox check` passed.
                                Some(VoxValue::Option(Some(Box::new(VoxValue::Object(res)))))
                            }
                            Err(_) => Some(VoxValue::Option(None)),
                        }
                    }
                    "run_ex" => {
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| {
                                    if let VoxValue::Str(s) = v {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };
                        let cwd = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let _env_list = it.next();

                        // Match the typeck signature `Result[Record]` —
                        // return the full {stdout, stderr, code} record so
                        // callers can inspect all three without a second
                        // shell-out. Prior impl returned bare Int (exit code
                        // only), which created a typeck/eval mismatch that
                        // surfaced as either "type error" at check OR
                        // "Cannot access field 'code'" at run depending on
                        // which side was authoritative.
                        let output = std::process::Command::new(&cmd_name)
                            .args(&cmd_args)
                            .current_dir(&cwd)
                            .output();
                        let res = match output {
                            Ok(out) => Ok(Box::new(VoxValue::Object(vec![
                                (
                                    "stdout".to_string(),
                                    VoxValue::Str(
                                        String::from_utf8_lossy(&out.stdout).to_string(),
                                    ),
                                ),
                                (
                                    "stderr".to_string(),
                                    VoxValue::Str(
                                        String::from_utf8_lossy(&out.stderr).to_string(),
                                    ),
                                ),
                                (
                                    "code".to_string(),
                                    VoxValue::Int(out.status.code().unwrap_or(0) as i64),
                                ),
                            ]))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    "spawn_background" => {
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| {
                                    if let VoxValue::Str(s) = v {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };

                        let handle = match tokio::runtime::Handle::try_current() {
                            Ok(h) => h,
                            Err(_) => {
                                return Some(VoxValue::Result(Err(
                                    "spawn_background must be run within a Tokio runtime"
                                        .to_string(),
                                )));
                            }
                        };

                        match tokio::process::Command::new(cmd_name)
                            .args(cmd_args)
                            .spawn()
                        {
                            Ok(mut child) => {
                                let id = child.id().unwrap_or(0);
                                handle.spawn(async move {
                                    let _ = child.wait().await;
                                });
                                Some(VoxValue::Result(Ok(Box::new(VoxValue::Int(id as i64)))))
                            }
                            Err(e) => Some(VoxValue::Result(Err(e.to_string()))),
                        }
                    }
                    "exec" => {
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| {
                                    if let VoxValue::Str(s) = v {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };

                        #[cfg(unix)]
                        {
                            use std::os::unix::process::CommandExt;
                            let err = std::process::Command::new(cmd_name).args(cmd_args).exec();
                            Some(VoxValue::Result(Err(err.to_string())))
                        }
                        #[cfg(not(unix))]
                        {
                            match std::process::Command::new(cmd_name).args(cmd_args).status() {
                                Ok(st) => {
                                    vox_flush_exit_commands();
                                    std::process::exit(st.code().unwrap_or(1))
                                }
                                Err(e) => Some(VoxValue::Result(Err(e.to_string()))),
                            }
                        }
                    }
                    "register_exit_command" => {
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| {
                                    if let VoxValue::Str(s) = v {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };

                        ensure_signal_handler();
                        if let Ok(mut cmds) = exit_commands().lock() {
                            cmds.push((cmd_name, cmd_args));
                        }
                        Some(VoxValue::Result(Ok(Box::new(VoxValue::Null))))
                    }
                    "exit" => {
                        let code = match args.into_iter().next() {
                            Some(VoxValue::Int(c)) => c as i32,
                            _ => 0,
                        };
                        vox_flush_exit_commands();
                        std::process::exit(code);
                    }
                    "run_capture_json" => {
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| {
                                    if let VoxValue::Str(s) = v {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };
                        let res = interp_process_run_capture_json(&cmd_name, &cmd_args);
                        Some(VoxValue::Result(match res {
                            Ok(v) => Ok(Box::new(json_to_vox(v))),
                            Err(e) => Err(e),
                        }))
                    }
                    "run_capture_lines" => {
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| {
                                    if let VoxValue::Str(s) = v {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };
                        let res = interp_process_run_capture_lines(&cmd_name, &cmd_args);
                        Some(VoxValue::Result(match res {
                            Ok(lines) => Ok(Box::new(VoxValue::List(
                                lines.into_iter().map(VoxValue::Str).collect(),
                            ))),
                            Err(e) => Err(e),
                        }))
                    }
                    // `process.cwd` — same op as `fs.cwd`, aliased here for the
                    // call sites that reach for it under the `process` namespace.
                    // Both resolve via `std::env::current_dir`.
                    "cwd" => {
                        let res = match std::env::current_dir() {
                            Ok(p) => Ok(Box::new(VoxValue::Str(
                                p.to_string_lossy().to_string(),
                            ))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    // `process.which(cmd)` — locate a binary on PATH, returning
                    // its absolute path or None if not found. Cross-platform —
                    // uses the `which` crate which handles `.exe` extension on
                    // Windows and PATHEXT lookups correctly. Audit doc §10
                    // confirmed this as a needed primitive over the
                    // platform-specific `process.run("which", ...)` workaround.
                    "which" => {
                        let cmd = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Option(None)),
                        };
                        match ::which::which(&cmd) {
                            Ok(p) => Some(VoxValue::Option(Some(Box::new(VoxValue::Str(
                                p.to_string_lossy().to_string(),
                            ))))),
                            Err(_) => Some(VoxValue::Option(None)),
                        }
                    }
                    _ => None,
                },
                Some("agentos") => match method {
                    "mutation_kind_for_tool" => {
                        let name = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Str("read_only".to_string())),
                        };
                        Some(VoxValue::Str(
                            vox_foundation::primitives::agentos_mutation::mutation_kind_for_tool(&name).to_string(),
                        ))
                    }
                    _ => None,
                },
                Some("csv") => match method {
                    "parse" => {
                        let s = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        Some(VoxValue::Result(match interp_csv_parse(&s) {
                            Ok(v) => Ok(Box::new(json_to_vox(v))),
                            Err(e) => Err(e),
                        }))
                    }
                    "parse_records" => {
                        let s = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        Some(VoxValue::Result(match interp_csv_parse_records(&s) {
                            Ok(v) => Ok(Box::new(json_to_vox(v))),
                            Err(e) => Err(e),
                        }))
                    }
                    "render" => {
                        let rows_v = match args.into_iter().next() {
                            Some(v) => v,
                            _ => return Some(VoxValue::Null),
                        };
                        let rows = match voxvalue_as_table_str(&rows_v) {
                            Some(r) => r,
                            None => {
                                return Some(VoxValue::Result(Err(
                                    "csv.render: expected list[list[str]]".into(),
                                )));
                            }
                        };
                        Some(VoxValue::Result(match interp_csv_render(&rows) {
                            Ok(s) => Ok(Box::new(VoxValue::Str(s))),
                            Err(e) => Err(e),
                        }))
                    }
                    _ => None,
                },
                Some("toml") => match method {
                    "parse" => {
                        let s = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        Some(VoxValue::Result(match interp_toml_parse(&s) {
                            Ok(v) => Ok(Box::new(json_to_vox(v))),
                            Err(e) => Err(e),
                        }))
                    }
                    "render" => {
                        let v = match args.into_iter().next() {
                            Some(val) => val,
                            _ => return Some(VoxValue::Null),
                        };
                        let j = vox_to_json(v);
                        Some(VoxValue::Result(match interp_toml_render(&j) {
                            Ok(s) => Ok(Box::new(VoxValue::Str(s))),
                            Err(e) => Err(e),
                        }))
                    }
                    _ => None,
                },
                Some("yaml") => match method {
                    "parse" => {
                        let s = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        Some(VoxValue::Result(match interp_yaml_parse(&s) {
                            Ok(v) => Ok(Box::new(json_to_vox(v))),
                            Err(e) => Err(e),
                        }))
                    }
                    "render" => {
                        let v = match args.into_iter().next() {
                            Some(val) => val,
                            _ => return Some(VoxValue::Null),
                        };
                        let j = vox_to_json(v);
                        Some(VoxValue::Result(match interp_yaml_render(&j) {
                            Ok(s) => Ok(Box::new(VoxValue::Str(s))),
                            Err(e) => Err(e),
                        }))
                    }
                    _ => None,
                },
                Some("io") => match method {
                    "open" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        Some(VoxValue::Result(match interp_io_open(&path) {
                            Ok(v) => Ok(Box::new(json_to_vox(v))),
                            Err(e) => Err(e),
                        }))
                    }
                    "save" => {
                        let mut it = args.into_iter();
                        let path = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let val = match it.next() {
                            Some(v) => v,
                            _ => return Some(VoxValue::Null),
                        };
                        let j = vox_to_json(val);
                        Some(VoxValue::Result(match interp_io_save(&path, &j) {
                            Ok(()) => Ok(Box::new(VoxValue::Null)),
                            Err(e) => Err(e),
                        }))
                    }
                    _ => None,
                },
                Some("json") => match method {
                    "parse" => {
                        let s = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        match serde_json::from_str::<serde_json::Value>(&s) {
                            Ok(v) => Some(json_to_vox(v)),
                            Err(_) => Some(VoxValue::Null),
                        }
                    }
                    "render" | "stringify" | "encode" => {
                        let v = match args.into_iter().next() {
                            Some(v) => v,
                            _ => return Some(VoxValue::Null),
                        };
                        let j = vox_to_json(v);
                        let res = serde_json::to_string(&j).map_err(|e| e.to_string());
                        Some(VoxValue::Result(match res {
                            Ok(s) => Ok(Box::new(VoxValue::Str(s))),
                            Err(e) => Err(e),
                        }))
                    }
                    _ => None,
                },
                Some("regex") => {
                    // regex.replace(haystack, pattern, replacement) -> str
                    // regex.is_match(haystack, pattern) -> bool
                    // regex.captures(haystack, pattern) -> Option[List[str]]
                    //
                    // Patterns that fail to compile yield: empty string (replace),
                    // false (is_match), or None (captures). Loud-error variants
                    // can be added later if needed — the current corpus uses
                    // patterns that are statically known to compile.
                    let mut it = args.into_iter();
                    match method {
                        "replace" => {
                            let haystack = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Str(String::new())),
                            };
                            let pattern = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Str(haystack)),
                            };
                            let replacement = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Str(haystack)),
                            };
                            match regex::Regex::new(&pattern) {
                                Ok(re) => Some(VoxValue::Str(
                                    re.replace_all(&haystack, replacement.as_str()).to_string(),
                                )),
                                Err(_) => Some(VoxValue::Str(haystack)),
                            }
                        }
                        "is_match" => {
                            let haystack = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Bool(false)),
                            };
                            let pattern = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Bool(false)),
                            };
                            Some(VoxValue::Bool(
                                regex::Regex::new(&pattern)
                                    .map(|re| re.is_match(&haystack))
                                    .unwrap_or(false),
                            ))
                        }
                        "captures" => {
                            let haystack = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Option(None)),
                            };
                            let pattern = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Option(None)),
                            };
                            let re = match regex::Regex::new(&pattern) {
                                Ok(re) => re,
                                Err(_) => return Some(VoxValue::Option(None)),
                            };
                            match re.captures(&haystack) {
                                Some(caps) => {
                                    let groups: Vec<VoxValue> = caps
                                        .iter()
                                        .map(|m| {
                                            VoxValue::Str(
                                                m.map(|x| x.as_str().to_string())
                                                    .unwrap_or_default(),
                                            )
                                        })
                                        .collect();
                                    Some(VoxValue::Option(Some(Box::new(VoxValue::List(groups)))))
                                }
                                None => Some(VoxValue::Option(None)),
                            }
                        }
                        // `regex.compile(pattern) -> Result[Regex]` — pre-compile
                        // a pattern for repeated use in hot loops. We don't have
                        // a dedicated `Regex` runtime value yet; for the
                        // interp tier the compiled-regex use case is rare and
                        // the existing `regex.replace`/`is_match`/`captures`
                        // already compile on each call. So we return the
                        // pattern string back wrapped in Ok — callers that
                        // pass this to subsequent calls will recompile (no
                        // perf win), but the symbol resolves cleanly. A real
                        // compiled-Regex value type can land later if hot-loop
                        // regex shows up in profiles.
                        "compile" => {
                            let pattern = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => {
                                    return Some(VoxValue::Result(Err(
                                        "regex.compile expected a string pattern".to_string(),
                                    )))
                                }
                            };
                            match regex::Regex::new(&pattern) {
                                Ok(_) => Some(VoxValue::Result(Ok(Box::new(VoxValue::Str(
                                    pattern,
                                ))))),
                                Err(e) => Some(VoxValue::Result(Err(e.to_string()))),
                            }
                        }
                        _ => None,
                    }
                }
                Some("log") => {
                    let msg = args
                        .iter()
                        .map(vox_value_display)
                        .collect::<Vec<_>>()
                        .join(" ");
                    match method {
                        "debug" => tracing::debug!("{msg}"),
                        "info" => tracing::info!("{msg}"),
                        "warn" => tracing::warn!("{msg}"),
                        "error" => tracing::error!("{msg}"),
                        _ => {}
                    }
                    Some(VoxValue::Null)
                }
                _ => {
                    if ns.is_none() {
                        interp_json_object_methods(fields, method, args.as_slice())
                    } else {
                        None
                    }
                }
            }
        }

        _ => None,
    }
}

fn lookup_json_field<'a>(
    fields: &'a [(String, VoxValue)],
    key: &str,
) -> Option<&'a VoxValue> {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Json accessor surface for plain `Object` values produced by `json_to_vox` (no `__namespace__`).
/// Matches [`vox_actor_runtime::builtins::VoxJson`] behavior for scripts running in the interpreter.
fn interp_json_object_methods(
    fields: &[(String, VoxValue)],
    method: &str,
    args: &[VoxValue],
) -> Option<VoxValue> {
    let res_ok = |v: VoxValue| Some(VoxValue::Result(Ok(Box::new(v))));
    let res_err = |msg: String| Some(VoxValue::Result(Err(msg)));

    match method {
        "get_str" => {
            let key = match args.first() {
                Some(VoxValue::Str(s)) => s.as_str(),
                Some(_) => return res_err("json: expected string key".into()),
                None => return res_err("json: missing key argument".into()),
            };
            match lookup_json_field(fields, key) {
                Some(VoxValue::Str(s)) => res_ok(VoxValue::Str(s.clone())),
                Some(_) => res_err(format!("json: key '{key}' is not a string")),
                None => res_err(format!("json: missing key '{key}'")),
            }
        }
        "get_int" => {
            let key = match args.first() {
                Some(VoxValue::Str(s)) => s.as_str(),
                Some(_) => return res_err("json: expected string key".into()),
                None => return res_err("json: missing key argument".into()),
            };
            match lookup_json_field(fields, key) {
                Some(VoxValue::Int(i)) => res_ok(VoxValue::Int(*i)),
                Some(VoxValue::Float(f)) => res_ok(VoxValue::Int(*f as i64)),
                Some(_) => res_err(format!("json: key '{key}' is not an integer")),
                None => res_err(format!("json: missing key '{key}'")),
            }
        }
        "get_float" => {
            let key = match args.first() {
                Some(VoxValue::Str(s)) => s.as_str(),
                Some(_) => return res_err("json: expected string key".into()),
                None => return res_err("json: missing key argument".into()),
            };
            match lookup_json_field(fields, key) {
                Some(VoxValue::Int(i)) => res_ok(VoxValue::Float(*i as f64)),
                Some(VoxValue::Float(f)) => res_ok(VoxValue::Float(*f)),
                Some(_) => res_err(format!("json: key '{key}' is not a number")),
                None => res_err(format!("json: missing key '{key}'")),
            }
        }
        "get_bool" => {
            let key = match args.first() {
                Some(VoxValue::Str(s)) => s.as_str(),
                Some(_) => return res_err("json: expected string key".into()),
                None => return res_err("json: missing key argument".into()),
            };
            match lookup_json_field(fields, key) {
                Some(VoxValue::Bool(b)) => res_ok(VoxValue::Bool(*b)),
                Some(_) => res_err(format!("json: key '{key}' is not a bool")),
                None => res_err(format!("json: missing key '{key}'")),
            }
        }
        "get_object" => {
            let key = match args.first() {
                Some(VoxValue::Str(s)) => s.as_str(),
                Some(_) => return res_err("json: expected string key".into()),
                None => return res_err("json: missing key argument".into()),
            };
            match lookup_json_field(fields, key) {
                Some(VoxValue::Object(o)) => res_ok(VoxValue::Object(o.clone())),
                Some(_) => res_err(format!("json: key '{key}' is not an object")),
                None => res_err(format!("json: missing key '{key}'")),
            }
        }
        "get_array" => {
            let key = match args.first() {
                Some(VoxValue::Str(s)) => s.as_str(),
                Some(_) => return res_err("json: expected string key".into()),
                None => return res_err("json: missing key argument".into()),
            };
            match lookup_json_field(fields, key) {
                Some(VoxValue::List(l)) => res_ok(VoxValue::List(l.clone())),
                Some(_) => res_err(format!("json: key '{key}' is not an array")),
                None => res_err(format!("json: missing key '{key}'")),
            }
        }
        "is_null" => Some(VoxValue::Bool(false)),
        "length" => Some(VoxValue::Int(0)),
        "at" => res_err("json: receiver is not an array".into()),
        "keys" => {
            let ks: Vec<VoxValue> = fields
                .iter()
                .map(|(k, _)| VoxValue::Str(k.clone()))
                .collect();
            Some(VoxValue::List(ks))
        }
        "to_string" => {
            let j = vox_to_json(VoxValue::Object(fields.to_vec()));
            Some(VoxValue::Str(serde_json::to_string(&j).unwrap_or_default()))
        }
        _ => None,
    }
}

fn vox_to_json(v: VoxValue) -> serde_json::Value {
    match v {
        VoxValue::Int(n) => serde_json::Value::Number(n.into()),
        VoxValue::Float(f) => serde_json::json!(f),
        VoxValue::Str(s) => serde_json::Value::String(s),
        VoxValue::Bool(b) => serde_json::Value::Bool(b),
        VoxValue::Null => serde_json::Value::Null,
        VoxValue::List(ls) => serde_json::Value::Array(ls.into_iter().map(vox_to_json).collect()),
        VoxValue::Object(fields) => {
            let mut map = serde_json::Map::new();
            for (k, v) in fields {
                if k == "__namespace__" {
                    continue;
                }
                map.insert(k, vox_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        VoxValue::Tuple(ls) => serde_json::Value::Array(ls.into_iter().map(vox_to_json).collect()),
        _ => serde_json::Value::Null,
    }
}

fn json_to_vox(v: serde_json::Value) -> VoxValue {
    match v {
        serde_json::Value::Null => VoxValue::Null,
        serde_json::Value::Bool(b) => VoxValue::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                VoxValue::Int(i)
            } else {
                VoxValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => VoxValue::Str(s),
        serde_json::Value::Array(arr) => VoxValue::List(arr.into_iter().map(json_to_vox).collect()),
        serde_json::Value::Object(obj) => {
            let mut fields = Vec::new();
            for (k, v) in obj {
                fields.push((k, json_to_vox(v)));
            }
            VoxValue::Object(fields)
        }
    }
}

/// Attempt to call a global built-in function (not a method).
/// Returns `None` if `name` is not a known global.
pub fn call_global_builtin(name: &str, args: Vec<VoxValue>) -> Option<VoxValue> {
    match name {
        "print" => {
            let msg = args
                .iter()
                .map(vox_value_display)
                .collect::<Vec<_>>()
                .join(" ");
            println!("{msg}");
            Some(VoxValue::Null)
        }
        "assert" => {
            let cond = args.first();
            let ok = matches!(cond, Some(VoxValue::Bool(true)));
            if !ok {
                let msg = args
                    .get(1)
                    .map(vox_value_display)
                    .unwrap_or_else(|| "Assertion failed".to_string());
                eprintln!("assertion failed: {msg}");
                // Surface as Null — callers can check via EvalError::AssertionFailed
                return None; // signals caller to raise AssertionFailed
            }
            Some(VoxValue::Null)
        }
        "len" => {
            let v = args.into_iter().next()?;
            match v {
                VoxValue::List(ls) => Some(VoxValue::Int(ls.len() as i64)),
                VoxValue::Str(s) => Some(VoxValue::Int(s.len() as i64)),
                VoxValue::Object(o) => Some(VoxValue::Int(o.len() as i64)),
                _ => Some(VoxValue::Null),
            }
        }
        "str" => {
            let v = args.into_iter().next().unwrap_or(VoxValue::Null);
            Some(VoxValue::Str(vox_value_display(&v)))
        }
        "int" => {
            let v = args.into_iter().next().unwrap_or(VoxValue::Null);
            match v {
                VoxValue::Int(n) => Some(VoxValue::Int(n)),
                VoxValue::Float(f) => Some(VoxValue::Int(f as i64)),
                VoxValue::Str(s) => Some(VoxValue::Int(s.trim().parse::<i64>().unwrap_or(0))),
                VoxValue::Bool(b) => Some(VoxValue::Int(if b { 1 } else { 0 })),
                _ => Some(VoxValue::Int(0)),
            }
        }
        "float" => {
            let v = args.into_iter().next().unwrap_or(VoxValue::Null);
            match v {
                VoxValue::Float(f) => Some(VoxValue::Float(f)),
                VoxValue::Int(n) => Some(VoxValue::Float(n as f64)),
                VoxValue::Str(s) => Some(VoxValue::Float(s.trim().parse::<f64>().unwrap_or(0.0))),
                _ => Some(VoxValue::Float(0.0)),
            }
        }
        "bool" => {
            let v = args.into_iter().next().unwrap_or(VoxValue::Null);
            let b = match v {
                VoxValue::Bool(b) => b,
                VoxValue::Int(n) => n != 0,
                VoxValue::Float(f) => f != 0.0,
                VoxValue::Str(s) => !s.is_empty(),
                VoxValue::Null => false,
                VoxValue::List(l) => !l.is_empty(),
                _ => true,
            };
            Some(VoxValue::Bool(b))
        }
        "range" => {
            let mut it = args.into_iter();
            let (start, end) = match (it.next(), it.next()) {
                (Some(VoxValue::Int(e)), None) => (0, e),
                (Some(VoxValue::Int(s)), Some(VoxValue::Int(e))) => (s, e),
                _ => return Some(VoxValue::List(vec![])),
            };
            let list: Vec<VoxValue> = (start..end).map(VoxValue::Int).collect();
            Some(VoxValue::List(list))
        }
        "type_of" => {
            let v = args.into_iter().next().unwrap_or(VoxValue::Null);
            let t = match v {
                VoxValue::Int(_) => "int",
                VoxValue::Float(_) => "float",
                VoxValue::Str(_) => "str",
                VoxValue::Bool(_) => "bool",
                VoxValue::List(_) => "List",
                VoxValue::Object(_) => "Object",
                VoxValue::Tuple(_) => "Tuple",
                VoxValue::Null => "null",
                VoxValue::Fn { .. } => "fn",
                VoxValue::Option(_) => "Option",
                VoxValue::Result(_) => "Result",
                _ => "unknown",
            };
            Some(VoxValue::Str(t.to_string()))
        }
        _ => None,
    }
}

/// Short human-readable name of a VoxValue's runtime type. Used in
/// error messages for binary/unary op type mismatches (eval/expr.rs).
pub fn vox_value_type_name(v: &VoxValue) -> &'static str {
    match v {
        VoxValue::Int(_) => "Int",
        VoxValue::Float(_) => "Float",
        VoxValue::Str(_) => "Str",
        VoxValue::Bool(_) => "Bool",
        VoxValue::List(_) => "List",
        VoxValue::Object(_) => "Object",
        VoxValue::Tuple(_) => "Tuple",
        VoxValue::Null => "Null",
        VoxValue::Fn { .. } => "Fn",
        VoxValue::Option(_) => "Option",
        VoxValue::Result(_) => "Result",
        VoxValue::Constructor(_) => "Constructor",
        VoxValue::Tagged { .. } => "Tagged",
        VoxValue::_Return(_) => "_Return",
        VoxValue::_Break => "_Break",
        VoxValue::_Continue => "_Continue",
        VoxValue::_Panic(_) => "_Panic",
    }
}

pub fn vox_value_display(v: &VoxValue) -> String {
    match v {
        VoxValue::Int(n) => n.to_string(),
        VoxValue::Float(f) => f.to_string(),
        VoxValue::Str(s) => s.clone(),
        VoxValue::Bool(b) => b.to_string(),
        VoxValue::Null => "null".to_string(),
        VoxValue::List(ls) => {
            let items: Vec<String> = ls.iter().map(vox_value_display).collect();
            format!("[{}]", items.join(", "))
        }
        VoxValue::Object(o) => {
            let fields: Vec<String> = o
                .iter()
                .map(|(k, v)| format!("{k}: {}", vox_value_display(v)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
        VoxValue::Tuple(t) => {
            let items: Vec<String> = t.iter().map(vox_value_display).collect();
            format!("({})", items.join(", "))
        }
        _ => format!("{v:?}"),
    }
}
