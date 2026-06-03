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
    // ── Json leaf coercion on scalar receivers ─────────────────────────
    // Json values flow through Vox as plain scalars (a JSON string is
    // VoxValue::Str, a JSON number is Int/Float, etc.). The strict-Option
    // leaf accessors (`as_str`, `as_int`, ...) need to work uniformly on
    // any scalar so chains like `data.get("k").and_then(fn(j) { j.as_str() })`
    // resolve cleanly regardless of the actual JSON shape. Per RFC
    // json-ergonomics-rfc-2026-05-23 §4.3: None on wrong-type or null.
    match method {
        "as_str" => {
            return Some(VoxValue::Option(match obj {
                VoxValue::Str(s) => Some(Box::new(VoxValue::Str(s.clone()))),
                _ => None,
            }));
        }
        "as_int" => {
            return Some(VoxValue::Option(match obj {
                VoxValue::Int(i) => Some(Box::new(VoxValue::Int(*i))),
                VoxValue::Float(f) => Some(Box::new(VoxValue::Int(*f as i64))),
                _ => None,
            }));
        }
        "as_float" => {
            return Some(VoxValue::Option(match obj {
                VoxValue::Float(f) => Some(Box::new(VoxValue::Float(*f))),
                VoxValue::Int(i) => Some(Box::new(VoxValue::Float(*i as f64))),
                _ => None,
            }));
        }
        "as_bool" => {
            return Some(VoxValue::Option(match obj {
                VoxValue::Bool(b) => Some(Box::new(VoxValue::Bool(*b))),
                _ => None,
            }));
        }
        _ => {} // fall through to per-type dispatch below
    }

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
            "first" => Some(VoxValue::Option(v.first().cloned().map(Box::new))),
            "last" => Some(VoxValue::Option(v.last().cloned().map(Box::new))),
            "contains" => {
                let target = args.into_iter().next().unwrap_or(VoxValue::Null);
                Some(VoxValue::Bool(v.contains(&target)))
            }
            // index(val) → int: first index of val, or -1 if absent.
            "index" | "find_index" => {
                let target = args.into_iter().next().unwrap_or(VoxValue::Null);
                let idx = v
                    .iter()
                    .position(|x| x == &target)
                    .map(|i| i as i64)
                    .unwrap_or(-1);
                Some(VoxValue::Int(idx))
            }
            // count(val) → int: number of occurrences.
            "count" => {
                let target = args.into_iter().next().unwrap_or(VoxValue::Null);
                let n = v.iter().filter(|x| *x == &target).count() as i64;
                Some(VoxValue::Int(n))
            }
            // extend(other) → List[T]: append all elements of other.
            "extend" => {
                let mut owned = v.clone();
                if let Some(VoxValue::List(other)) = args.into_iter().next() {
                    owned.extend(other);
                }
                Some(VoxValue::List(owned))
            }
            // remove(val) → List[T]: new list with first occurrence of val removed.
            "remove" => {
                let target = args.into_iter().next().unwrap_or(VoxValue::Null);
                let mut owned = v.clone();
                if let Some(pos) = owned.iter().position(|x| x == &target) {
                    owned.remove(pos);
                }
                Some(VoxValue::List(owned))
            }
            // remove_at(i) → List[T]: new list without element at index i.
            "remove_at" => {
                let mut owned = v.clone();
                if let Some(VoxValue::Int(i)) = args.into_iter().next() {
                    if i >= 0 && (i as usize) < owned.len() {
                        owned.remove(i as usize);
                    }
                }
                Some(VoxValue::List(owned))
            }
            // zip(other) → List[List[T]]: pairs of [a, b] elements.
            "zip" => {
                let other = match args.into_iter().next() {
                    Some(VoxValue::List(o)) => o,
                    _ => return Some(VoxValue::List(Vec::new())),
                };
                let pairs: Vec<VoxValue> = v
                    .iter()
                    .cloned()
                    .zip(other)
                    .map(|(a, b)| VoxValue::List(vec![a, b]))
                    .collect();
                Some(VoxValue::List(pairs))
            }
            // enumerate() → List[List[T]]: [[0, a], [1, b], ...].
            "enumerate" => {
                let pairs: Vec<VoxValue> = v
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, x)| VoxValue::List(vec![VoxValue::Int(i as i64), x]))
                    .collect();
                Some(VoxValue::List(pairs))
            }
            // slice(start, end?) → List[T]: sub-list from start to end (exclusive).
            "slice_list" => {
                let mut it = args.into_iter();
                let start = match it.next() {
                    Some(VoxValue::Int(i)) => i.max(0) as usize,
                    _ => 0,
                };
                let end = match it.next() {
                    Some(VoxValue::Int(i)) => i.min(v.len() as i64) as usize,
                    _ => v.len(),
                };
                let slice = v.get(start..end.min(v.len())).unwrap_or(&[]).to_vec();
                Some(VoxValue::List(slice))
            }
            "join" => {
                let sep = match args.into_iter().next() {
                    Some(VoxValue::Str(s)) => s,
                    _ => String::new(),
                };
                let strings: Vec<String> = v.iter().map(|x| vox_value_display(x)).collect();
                Some(VoxValue::Str(strings.join(&sep)))
            }
            "reverse" => {
                let mut owned = v.clone();
                owned.reverse();
                Some(VoxValue::List(owned))
            }
            "reversed" => {
                let mut owned = v.clone();
                owned.reverse();
                Some(VoxValue::List(owned))
            }
            "sorted" => {
                let mut owned = v.clone();
                owned.sort_by(|a, b| vox_value_cmp(a, b));
                Some(VoxValue::List(owned))
            }
            "sum" => {
                let mut int_sum: i64 = 0;
                let mut float_sum: f64 = 0.0;
                let mut is_float = false;
                for item in v.iter() {
                    match item {
                        VoxValue::Int(n) => {
                            int_sum += n;
                            float_sum += *n as f64;
                        }
                        VoxValue::Float(f) => {
                            is_float = true;
                            float_sum += f;
                        }
                        _ => {}
                    }
                }
                if is_float {
                    Some(VoxValue::Float(float_sum))
                } else {
                    Some(VoxValue::Int(int_sum))
                }
            }
            "max" if args.is_empty() => {
                let result = v.iter().max_by(|a, b| vox_value_cmp(a, b)).cloned();
                Some(VoxValue::Option(result.map(Box::new)))
            }
            "min" if args.is_empty() => {
                let result = v.iter().min_by(|a, b| vox_value_cmp(a, b)).cloned();
                Some(VoxValue::Option(result.map(Box::new)))
            }
            "flatten" => {
                let mut flat = Vec::new();
                for item in v.iter() {
                    if let VoxValue::List(inner) = item {
                        flat.extend(inner.iter().cloned());
                    } else {
                        flat.push(item.clone());
                    }
                }
                Some(VoxValue::List(flat))
            }
            // Json-shaped arrays (`std.json.parse`, `std.csv.parse`, …) use these names in typecheck + native `VoxJson`.
            // Strict-Option API per json-ergonomics-rfc-2026-05-23.
            "length" => Some(VoxValue::Option(Some(Box::new(VoxValue::Int(
                v.len() as i64
            ))))),
            "at" => {
                let idx = match args.first().cloned() {
                    Some(VoxValue::Int(i)) => i,
                    _ => return Some(VoxValue::Option(None)),
                };
                if idx < 0 {
                    return Some(VoxValue::Option(None));
                }
                let i = idx as usize;
                match v.get(i) {
                    Some(el) => Some(VoxValue::Option(Some(Box::new(el.clone())))),
                    None => Some(VoxValue::Option(None)),
                }
            }
            "pointer" => {
                let path = match args.first() {
                    Some(VoxValue::Str(s)) => s.clone(),
                    _ => return Some(VoxValue::Option(None)),
                };
                let serde_val = vox_to_json(VoxValue::List(v.clone()));
                match serde_val.pointer(&path) {
                    Some(found) => {
                        Some(VoxValue::Option(Some(Box::new(json_to_vox(found.clone())))))
                    }
                    None => Some(VoxValue::Option(None)),
                }
            }
            "as_array" => Some(VoxValue::Option(Some(Box::new(VoxValue::List(v.clone()))))),
            "as_object" | "as_str" | "as_int" | "as_float" | "as_bool" => {
                Some(VoxValue::Option(None))
            }
            "is_null" => Some(VoxValue::Bool(false)),
            "has" => Some(VoxValue::Bool(false)),
            // Note: list's `get(int)` (line ~152) handles the int-indexed
            // case; the json-API `get(str)` doesn't apply on a list receiver.
            "keys" => Some(VoxValue::Option(None)),
            "to_string" => {
                let j = vox_to_json(VoxValue::List(v.clone()));
                Some(VoxValue::Str(serde_json::to_string(&j).unwrap_or_default()))
            }
            _ => None,
        },

        // ── Null (JSON null literal) ──────────────────────────────────
        // Json-RFC strict-Option API: a null receiver answers `is_null`
        // affirmatively, has no fields, and produces None for any
        // navigation/coercion. Total methods only.
        VoxValue::Null => match method {
            "is_null" => Some(VoxValue::Bool(true)),
            "has" => Some(VoxValue::Bool(false)),
            "get" | "at" | "pointer" | "as_object" | "as_array" | "length" | "keys" => {
                Some(VoxValue::Option(None))
            }
            "to_string" => Some(VoxValue::Str("null".to_string())),
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
            // count(sub) → int: number of non-overlapping occurrences.
            "count" => {
                let sub = match args.into_iter().next() {
                    Some(VoxValue::Str(p)) => p,
                    _ => return Some(VoxValue::Int(0)),
                };
                if sub.is_empty() {
                    // Python semantics: "" matches between every char + at ends
                    return Some(VoxValue::Int(s.chars().count() as i64 + 1));
                }
                let mut count = 0i64;
                let mut start = 0;
                while let Some(pos) = s[start..].find(&*sub) {
                    count += 1;
                    start += pos + sub.len();
                }
                Some(VoxValue::Int(count))
            }
            // is_alpha() — all chars are alphabetic
            "is_alpha" => Some(VoxValue::Bool(
                !s.is_empty() && s.chars().all(|c| c.is_alphabetic()),
            )),
            // is_digit() — all chars are ASCII digits
            "is_digit" => Some(VoxValue::Bool(
                !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
            )),
            // is_alnum() — all chars are alphanumeric
            "is_alnum" => Some(VoxValue::Bool(
                !s.is_empty() && s.chars().all(|c| c.is_alphanumeric()),
            )),
            // is_upper() — all cased chars are uppercase
            "is_upper" => Some(VoxValue::Bool(
                !s.is_empty()
                    && s.chars().any(|c| c.is_alphabetic())
                    && s.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()),
            )),
            // is_lower() — all cased chars are lowercase
            "is_lower" => Some(VoxValue::Bool(
                !s.is_empty()
                    && s.chars().any(|c| c.is_alphabetic())
                    && s.chars().all(|c| !c.is_alphabetic() || c.is_lowercase()),
            )),
            "ord" => {
                // Return the Unicode code point of the first character.
                Some(VoxValue::Int(
                    s.chars().next().map(|c| c as i64).unwrap_or(0),
                ))
            }
            "chars" => {
                let list: Vec<VoxValue> = s.chars().map(|c| VoxValue::Str(c.to_string())).collect();
                Some(VoxValue::List(list))
            }
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
            "to_int" => Some(VoxValue::Option(
                s.trim()
                    .parse::<i64>()
                    .ok()
                    .map(|n| Box::new(VoxValue::Int(n))),
            )),
            "to_float" => Some(VoxValue::Option(
                s.trim()
                    .parse::<f64>()
                    .ok()
                    .map(|f| Box::new(VoxValue::Float(f))),
            )),
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
                None => VoxValue::_Panic("called `Option.unwrap()` on a None value".to_string()),
            }),
            // `unwrap_or(default)` — never panics; this is the "safe" form.
            "unwrap_or" => {
                let default = args.into_iter().next().unwrap_or(VoxValue::Null);
                Some(opt.as_ref().map(|v| (**v).clone()).unwrap_or(default))
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
                Err(e) => {
                    VoxValue::_Panic(format!("called `Result.unwrap()` on an Err value: {e}"))
                }
            }),
            // `unwrap_err()` panics on Ok — the inverse of unwrap. Prior
            // impl returned empty Str on Ok, masking the misuse silently.
            "unwrap_err" => Some(match res.as_ref() {
                Err(e) => VoxValue::Str(e.clone()),
                Ok(_) => {
                    VoxValue::_Panic("called `Result.unwrap_err()` on an Ok value".to_string())
                }
            }),
            "unwrap_or" => {
                let default = args.into_iter().next().unwrap_or(VoxValue::Null);
                Some(res.as_ref().ok().map(|v| (**v).clone()).unwrap_or(default))
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

            if ns.is_none() {
                match method {
                    // ── dict / map methods ─────────────────────────────────
                    "get" | "get_or" => {
                        let mut it = args.into_iter();
                        let key = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Option(None)),
                        };
                        let found = fields
                            .iter()
                            .find(|(k, _)| k == &key)
                            .map(|(_, v)| v.clone());
                        if method == "get" {
                            // Returns Option[T]
                            return Some(VoxValue::Option(found.map(Box::new)));
                        } else {
                            // get_or(key, default) → T
                            let default = it.next().unwrap_or(VoxValue::Null);
                            return Some(found.unwrap_or(default));
                        }
                    }
                    "contains_key" | "has_key" | "has" => {
                        let key = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Bool(false)),
                        };
                        return Some(VoxValue::Bool(fields.iter().any(|(k, _)| k == &key)));
                    }
                    "len" => {
                        return Some(VoxValue::Int(
                            fields.iter().filter(|(k, _)| k != "__namespace__").count() as i64,
                        ));
                    }
                    "is_empty" => {
                        return Some(VoxValue::Bool(
                            fields.iter().all(|(k, _)| k == "__namespace__"),
                        ));
                    }
                    "keys" => {
                        let keys: Vec<VoxValue> = fields
                            .iter()
                            .filter(|(k, _)| k != "__namespace__")
                            .map(|(k, _)| VoxValue::Str(k.clone()))
                            .collect();
                        return Some(VoxValue::List(keys));
                    }
                    "values" => {
                        let vals: Vec<VoxValue> = fields
                            .iter()
                            .filter(|(k, _)| k != "__namespace__")
                            .map(|(_, v)| v.clone())
                            .collect();
                        return Some(VoxValue::List(vals));
                    }
                    "items" | "entries" => {
                        // Returns list of [key, value] 2-element lists (tuples
                        // would be ideal but list is simpler to destructure
                        // in for-loops with the current interpreter).
                        let items: Vec<VoxValue> = fields
                            .iter()
                            .filter(|(k, _)| k != "__namespace__")
                            .map(|(k, v)| VoxValue::List(vec![VoxValue::Str(k.clone()), v.clone()]))
                            .collect();
                        return Some(VoxValue::List(items));
                    }
                    "insert" | "set" => {
                        // Returns new Object with key set (for statement-level
                        // auto-reassignment to work, the caller's variable is
                        // updated by the same kind-match heuristic used for List).
                        let mut it = args.into_iter();
                        let key = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Object(fields.to_vec())),
                        };
                        let val = it.next().unwrap_or(VoxValue::Null);
                        let mut owned = fields.to_vec();
                        if let Some(entry) = owned.iter_mut().find(|(k, _)| k == &key) {
                            entry.1 = val;
                        } else {
                            owned.push((key, val));
                        }
                        return Some(VoxValue::Object(owned));
                    }
                    "remove" | "delete" => {
                        let key = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Object(fields.to_vec())),
                        };
                        let owned: Vec<_> =
                            fields.iter().filter(|(k, _)| k != &key).cloned().collect();
                        return Some(VoxValue::Object(owned));
                    }
                    "update" => {
                        // Merge another Object (right wins on duplicate keys).
                        let other = match args.into_iter().next() {
                            Some(VoxValue::Object(o)) => o,
                            _ => return Some(VoxValue::Object(fields.to_vec())),
                        };
                        let mut owned = fields.to_vec();
                        for (k, v) in other {
                            if let Some(entry) = owned.iter_mut().find(|(ek, _)| ek == &k) {
                                entry.1 = v;
                            } else {
                                owned.push((k, v));
                            }
                        }
                        return Some(VoxValue::Object(owned));
                    }
                    _ => {}
                }
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
                    // Interp parity: typecheck + native codegen expose these fs ops,
                    // so the interpreter must too (else "Method not found" at --interp).
                    "read_bytes" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::read(&path) {
                            Ok(bytes) => Ok(Box::new(VoxValue::Str(
                                String::from_utf8_lossy(&bytes).to_string(),
                            ))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    "canonicalize" => {
                        let path = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::canonicalize(&path) {
                            Ok(p) => Ok(Box::new(VoxValue::Str(p.to_string_lossy().to_string()))),
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
                            Ok(p) => Ok(Box::new(VoxValue::Str(p.to_string_lossy().to_string()))),
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
                Some("time") => match method {
                    // `std.time.now_ms()` — current UNIX time in milliseconds.
                    // Interpreter parity with native codegen
                    // (vox_actor_runtime::builtins::vox_now_ms) and the typeck
                    // signature (`time.now_ms -> Int`) in builtin_registry.rs.
                    "now_ms" => {
                        let ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        Some(VoxValue::Int(ms))
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
                    // Interp parity for registered path methods missing here:
                    // basename/dirname/join_many/resolve (typeck + native codegen
                    // already know them; the interpreter previously errored with
                    // "Method <name> not found"). Return types match builtin_registry.rs.
                    "basename" => {
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
                    "dirname" => {
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
                    "join_many" => {
                        let segments = match args.into_iter().next() {
                            Some(VoxValue::List(items)) => items,
                            _ => return Some(VoxValue::Str(String::new())),
                        };
                        let mut acc = std::path::PathBuf::new();
                        for seg in segments {
                            if let VoxValue::Str(s) = seg {
                                acc.push(s);
                            }
                        }
                        Some(VoxValue::Str(acc.to_string_lossy().to_string()))
                    }
                    "resolve" => {
                        let p = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = match std::fs::canonicalize(&p) {
                            Ok(abs) => Ok(Box::new(VoxValue::Str(
                                abs.to_string_lossy().to_string(),
                            ))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
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
                        // `process.run(cmd, args)` — cwd-less variant. For a
                        // command run in a specific directory, use
                        // `process.run_ex(cmd, args, cwd)` (Result-returning).
                        // Keeping these as distinct methods rather than
                        // overloading `run` matches the K-complexity-low
                        // policy (one method, one arity).
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
                                    VoxValue::Str(String::from_utf8_lossy(&out.stdout).to_string()),
                                ),
                                (
                                    "stderr".to_string(),
                                    VoxValue::Str(String::from_utf8_lossy(&out.stderr).to_string()),
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
                    // Interp parity: run_capture / run_capture_ex return the full
                    // Result[{exit, stdout, stderr}] record (typeck + codegen have
                    // these; the interpreter previously did not).
                    "run_capture" | "run_capture_ex" => {
                        let with_cwd = method == "run_capture_ex";
                        let mut it = args.into_iter();
                        let cmd_name = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let cmd_args = match it.next() {
                            Some(VoxValue::List(ls)) => ls
                                .into_iter()
                                .filter_map(|v| if let VoxValue::Str(s) = v { Some(s) } else { None })
                                .collect::<Vec<_>>(),
                            _ => vec![],
                        };
                        let mut cmd = std::process::Command::new(&cmd_name);
                        cmd.args(&cmd_args);
                        if with_cwd {
                            if let Some(VoxValue::Str(cwd)) = it.next() {
                                cmd.current_dir(&cwd);
                            }
                            // env list (arg 4) intentionally ignored here, matching
                            // the run_ex interpreter behavior.
                        }
                        let res = match cmd.output() {
                            Ok(out) => Ok(Box::new(VoxValue::Object(vec![
                                (
                                    "exit".to_string(),
                                    VoxValue::Int(out.status.code().unwrap_or(0) as i64),
                                ),
                                (
                                    "stdout".to_string(),
                                    VoxValue::Str(String::from_utf8_lossy(&out.stdout).to_string()),
                                ),
                                (
                                    "stderr".to_string(),
                                    VoxValue::Str(String::from_utf8_lossy(&out.stderr).to_string()),
                                ),
                            ]))),
                            Err(e) => Err(e.to_string()),
                        };
                        Some(VoxValue::Result(res))
                    }
                    // `process.cwd` — same op as `fs.cwd`, aliased here for the
                    // call sites that reach for it under the `process` namespace.
                    // Both resolve via `std::env::current_dir`.
                    "cwd" => {
                        let res = match std::env::current_dir() {
                            Ok(p) => Ok(Box::new(VoxValue::Str(p.to_string_lossy().to_string()))),
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
                Some("agentos") => {
                    match method {
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
                    }
                }
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
                        // Strict-Option JSON RFC 2026-05-23: parse returns
                        // Result[Json] so callers can branch on parse failure
                        // and then use the chainable typed accessors on Ok.
                        let s = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => {
                                return Some(VoxValue::Result(Err(
                                    "json.parse: expected string argument".into(),
                                )));
                            }
                        };
                        match serde_json::from_str::<serde_json::Value>(&s) {
                            Ok(v) => Some(VoxValue::Result(Ok(Box::new(json_to_vox(v))))),
                            Err(e) => Some(VoxValue::Result(Err(format!("json.parse: {e}")))),
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
                    // Interp parity with vox_json_read_str/read_f64/quote (native).
                    "read_str" => {
                        let mut it = args.into_iter();
                        let json_s = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let key = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = (|| -> Result<String, String> {
                            let v: serde_json::Value =
                                serde_json::from_str(&json_s).map_err(|e| e.to_string())?;
                            let obj = v
                                .as_object()
                                .ok_or_else(|| "JSON root must be an object".to_string())?;
                            let val = obj.get(&key).ok_or_else(|| format!("missing key {key:?}"))?;
                            val.as_str()
                                .map(str::to_string)
                                .ok_or_else(|| format!("key {key:?} is not a string"))
                        })();
                        Some(VoxValue::Result(match res {
                            Ok(s) => Ok(Box::new(VoxValue::Str(s))),
                            Err(e) => Err(e),
                        }))
                    }
                    "read_f64" => {
                        let mut it = args.into_iter();
                        let json_s = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let key = match it.next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        let res = (|| -> Result<f64, String> {
                            let v: serde_json::Value =
                                serde_json::from_str(&json_s).map_err(|e| e.to_string())?;
                            let obj = v
                                .as_object()
                                .ok_or_else(|| "JSON root must be an object".to_string())?;
                            let val = obj.get(&key).ok_or_else(|| format!("missing key {key:?}"))?;
                            val.as_f64()
                                .or_else(|| val.as_i64().map(|i| i as f64))
                                .ok_or_else(|| format!("key {key:?} is not a number"))
                        })();
                        Some(VoxValue::Result(match res {
                            Ok(f) => Ok(Box::new(VoxValue::Float(f))),
                            Err(e) => Err(e),
                        }))
                    }
                    "quote" => {
                        let s = match args.into_iter().next() {
                            Some(VoxValue::Str(s)) => s,
                            _ => return Some(VoxValue::Null),
                        };
                        Some(VoxValue::Str(
                            serde_json::to_string(&s).unwrap_or_else(|_| "\"\"".to_string()),
                        ))
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
                        "find" => {
                            // regex.find(haystack, pattern) → Option[str]
                            let haystack = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Option(None)),
                            };
                            let pattern = match it.next() {
                                Some(VoxValue::Str(s)) => s,
                                _ => return Some(VoxValue::Option(None)),
                            };
                            match regex::Regex::new(&pattern) {
                                Ok(re) => Some(VoxValue::Option(
                                    re.find(&haystack)
                                        .map(|m| Box::new(VoxValue::Str(m.as_str().to_string()))),
                                )),
                                Err(_) => Some(VoxValue::Option(None)),
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
                                    )));
                                }
                            };
                            match regex::Regex::new(&pattern) {
                                Ok(_) => {
                                    Some(VoxValue::Result(Ok(Box::new(VoxValue::Str(pattern)))))
                                }
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

fn lookup_json_field<'a>(fields: &'a [(String, VoxValue)], key: &str) -> Option<&'a VoxValue> {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Json accessor surface for plain `Object` values produced by `json_to_vox` (no `__namespace__`).
/// Strict-Option API per json-ergonomics-rfc-2026-05-23: every fallible
/// access returns `Option[T]`; only `to_string`/`is_null`/`has` are total.
/// Matches [`vox_actor_runtime::builtins::VoxJson`] behavior for the
/// `--mode interp` path.
fn interp_json_object_methods(
    fields: &[(String, VoxValue)],
    method: &str,
    args: &[VoxValue],
) -> Option<VoxValue> {
    let opt_some = |v: VoxValue| Some(VoxValue::Option(Some(Box::new(v))));
    let opt_none = || Some(VoxValue::Option(None));

    // Treat absent fields as JSON-null-equivalent for `is_null` and friends,
    // following serde_json's convention that a missing key and an explicit
    // null both produce `None` at the navigation layer.
    let lookup = |key: &str| lookup_json_field(fields, key);
    let arg_key = || -> Option<&str> {
        match args.first() {
            Some(VoxValue::Str(s)) => Some(s.as_str()),
            _ => None,
        }
    };

    match method {
        // ── Navigation (Option[Json]) ─────────────────────────────────
        "get" => {
            let Some(key) = arg_key() else {
                return opt_none();
            };
            match lookup(key) {
                Some(v) => opt_some(v.clone()),
                None => opt_none(),
            }
        }
        "at" => opt_none(), // object receiver has no integer index
        "pointer" => {
            let Some(path) = arg_key() else {
                return opt_none();
            };
            // Delegate to serde_json::Value::pointer for RFC 6901 semantics.
            let serde_val = vox_to_json(VoxValue::Object(fields.to_vec()));
            match serde_val.pointer(path) {
                Some(v) => opt_some(json_to_vox(v.clone())),
                None => opt_none(),
            }
        }

        // ── Leaf coercion on Object receiver: object IS object; nothing
        // else matches. ───────────────────────────────────────────────
        "as_object" => opt_some(VoxValue::Object(fields.to_vec())),
        "as_str" | "as_int" | "as_float" | "as_bool" | "as_array" => opt_none(),

        // ── Inspection ────────────────────────────────────────────────
        "is_null" => Some(VoxValue::Bool(false)),
        "has" => {
            let Some(key) = arg_key() else {
                return Some(VoxValue::Bool(false));
            };
            Some(VoxValue::Bool(lookup(key).is_some()))
        }
        "length" => opt_some(VoxValue::Int(fields.len() as i64)),
        "keys" => {
            let ks: Vec<VoxValue> = fields
                .iter()
                .filter(|(k, _)| k != "__namespace__")
                .map(|(k, _)| VoxValue::Str(k.clone()))
                .collect();
            opt_some(VoxValue::List(ks))
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
        "abs" => match args.into_iter().next()? {
            VoxValue::Int(n) => Some(VoxValue::Int(n.abs())),
            VoxValue::Float(f) => Some(VoxValue::Float(f.abs())),
            _ => None,
        },
        "max" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                // max(a, b) — two-argument form
                (Some(VoxValue::Int(a)), Some(VoxValue::Int(b))) => Some(VoxValue::Int(a.max(b))),
                (Some(VoxValue::Float(a)), Some(VoxValue::Float(b))) => {
                    Some(VoxValue::Float(a.max(b)))
                }
                // max(list) — single list argument
                (Some(VoxValue::List(items)), None) => {
                    items.into_iter().max_by(|a, b| vox_value_cmp(a, b))
                }
                _ => None,
            }
        }
        "min" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                // min(a, b) — two-argument form
                (Some(VoxValue::Int(a)), Some(VoxValue::Int(b))) => Some(VoxValue::Int(a.min(b))),
                (Some(VoxValue::Float(a)), Some(VoxValue::Float(b))) => {
                    Some(VoxValue::Float(a.min(b)))
                }
                // min(list) — single list argument
                (Some(VoxValue::List(items)), None) => {
                    items.into_iter().min_by(|a, b| vox_value_cmp(a, b))
                }
                _ => None,
            }
        }
        // sorted(list) → List[T] — ascending sort (free-function form)
        "sorted" => {
            let v = args.into_iter().next()?;
            if let VoxValue::List(mut items) = v {
                items.sort_by(|a, b| vox_value_cmp(a, b));
                Some(VoxValue::List(items))
            } else {
                None
            }
        }
        // sum(list) → int | float — sum of numeric list (free-function form)
        "sum" => {
            let v = args.into_iter().next()?;
            if let VoxValue::List(items) = v {
                let mut int_sum: i64 = 0;
                let mut float_sum: f64 = 0.0;
                let mut is_float = false;
                for item in &items {
                    match item {
                        VoxValue::Int(n) => {
                            int_sum += n;
                            float_sum += *n as f64;
                        }
                        VoxValue::Float(f) => {
                            is_float = true;
                            float_sum += f;
                        }
                        _ => {}
                    }
                }
                if is_float {
                    Some(VoxValue::Float(float_sum))
                } else {
                    Some(VoxValue::Int(int_sum))
                }
            } else {
                None
            }
        }
        // chr(code: int) → str  — complement of s.ord()
        "chr" => match args.into_iter().next()? {
            VoxValue::Int(n) => {
                let ch = char::from_u32(n as u32).unwrap_or(char::REPLACEMENT_CHARACTER);
                Some(VoxValue::Str(ch.to_string()))
            }
            _ => None,
        },
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

/// Total ordering for comparable VoxValues. Used by `sorted()`, list
/// `max()`, and list `min()`. Mixed types compare as equal to avoid panics.
pub(super) fn vox_value_cmp(a: &VoxValue, b: &VoxValue) -> std::cmp::Ordering {
    match (a, b) {
        (VoxValue::Int(x), VoxValue::Int(y)) => x.cmp(y),
        (VoxValue::Float(x), VoxValue::Float(y)) => {
            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (VoxValue::Str(x), VoxValue::Str(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
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

#[cfg(test)]
mod time_namespace_interp_tests {
    use super::*;

    /// Construct the `std.time` namespace object exactly as `eval/mod.rs` does
    /// (a plain Object carrying the `__namespace__` marker).
    fn time_namespace() -> VoxValue {
        VoxValue::Object(vec![(
            "__namespace__".to_string(),
            VoxValue::Str("time".to_string()),
        )])
    }

    /// `std.time.now_ms()` must dispatch in the interpreter, matching typeck +
    /// native codegen (both map it to `vox_actor_runtime::builtins::vox_now_ms`).
    /// Regression guard: previously the interpreter had no `Some("time")` arm, so
    /// this returned `None` → `"Method now_ms not found"` under `vox run --interp`.
    #[test]
    fn std_time_now_ms_dispatches_in_interpreter() {
        let result = call_builtin_method(&time_namespace(), "now_ms", vec![], None);
        match result {
            Some(VoxValue::Int(ms)) => {
                // A real epoch-ms timestamp is far above this 2001-09 floor.
                assert!(
                    ms > 1_000_000_000_000,
                    "std.time.now_ms should return epoch milliseconds, got {ms}"
                );
            }
            other => panic!("std.time.now_ms did not dispatch in the interpreter: {other:?}"),
        }
    }

    fn path_namespace() -> VoxValue {
        VoxValue::Object(vec![(
            "__namespace__".to_string(),
            VoxValue::Str("path".to_string()),
        )])
    }

    /// `std.path.{basename,dirname,join_many,resolve}` must dispatch in the
    /// interpreter. They are registered for typeck + native codegen but were
    /// previously absent from the interpreter's `Some("path")` arm → runtime
    /// "Method <name> not found" despite passing `vox check`.
    #[test]
    fn std_path_basename_dirname_dispatch_in_interpreter() {
        let p = path_namespace();
        let base = call_builtin_method(
            &p,
            "basename",
            vec![VoxValue::Str("a/b/c.txt".to_string())],
            None,
        );
        assert_eq!(base, Some(VoxValue::Str("c.txt".to_string())));

        let dir = call_builtin_method(
            &p,
            "dirname",
            vec![VoxValue::Str("a/b/c.txt".to_string())],
            None,
        );
        match dir {
            Some(VoxValue::Str(s)) => assert!(s.ends_with("b"), "dirname got {s}"),
            other => panic!("std.path.dirname did not dispatch: {other:?}"),
        }
    }

    #[test]
    fn std_path_join_many_dispatches_in_interpreter() {
        let p = path_namespace();
        let joined = call_builtin_method(
            &p,
            "join_many",
            vec![VoxValue::List(vec![
                VoxValue::Str("a".to_string()),
                VoxValue::Str("b".to_string()),
                VoxValue::Str("c".to_string()),
            ])],
            None,
        );
        match joined {
            Some(VoxValue::Str(s)) => {
                let n = s.replace('\\', "/");
                assert_eq!(n, "a/b/c", "join_many got {s}");
            }
            other => panic!("std.path.join_many did not dispatch: {other:?}"),
        }
    }
}
