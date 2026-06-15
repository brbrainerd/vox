//! `vox wasm` — raw precompiled WASI module execution via the in-process
//! wasmtime SSOT (`vox-wasm-engine`).
//!
//! This is the real raw-`.wasm` runner that the mesh worker
//! (`run_dispatched_bundle`) and the Populi control-plane dispatch handler shell
//! out to. It replaces the invalid `vox run --mode script --isolation wasm`
//! invocation (`--isolation` is a `vox script` flag, and `vox run` runs `.vox`
//! source, not a precompiled `.wasm`). Not feature-gated — `vox-wasm-engine` is a
//! hard dependency, so raw-`.wasm` execution is always available.

use crate::cli_args::WasmRunArgs;

#[derive(clap::Subcommand)]
pub enum WasmCmd {
    /// Execute a precompiled `.wasm` (WASI preview1) module.
    Run {
        #[command(flatten)]
        args: WasmRunArgs,
    },
}

/// Parse `HOST[:GUEST]` preopen specs into engine `Preopen`s (guest defaults to host).
fn parse_preopens(ro: &[String], rw: &[String]) -> anyhow::Result<Vec<vox_wasm_engine::Preopen>> {
    fn one(spec: &str, write: bool) -> anyhow::Result<vox_wasm_engine::Preopen> {
        let (host, guest) = match spec.split_once(':') {
            Some((h, g)) if !g.is_empty() => (h.to_string(), g.to_string()),
            _ => (spec.to_string(), spec.to_string()),
        };
        if host.is_empty() {
            anyhow::bail!("invalid preopen spec (empty host): {spec:?}");
        }
        Ok(if write {
            vox_wasm_engine::Preopen::read_write(host, guest)
        } else {
            vox_wasm_engine::Preopen::read_only(host, guest)
        })
    }
    let mut out = Vec::with_capacity(ro.len() + rw.len());
    for s in ro {
        out.push(one(s, false)?);
    }
    for s in rw {
        out.push(one(s, true)?);
    }
    Ok(out)
}

/// Parse repeatable `KEY=VALUE` env specs into `(key, value)` pairs exposed to
/// the guest (WASI). Rejects empty keys and entries lacking `=`.
fn parse_env(specs: &[String]) -> anyhow::Result<Vec<(String, String)>> {
    specs
        .iter()
        .map(|s| match s.split_once('=') {
            Some((k, v)) if !k.is_empty() => Ok((k.to_string(), v.to_string())),
            _ => anyhow::bail!("invalid --env spec (expected KEY=VALUE): {s:?}"),
        })
        .collect()
}

/// Run a `vox wasm` subcommand. On success this **exits the process** with the
/// module's exit code (so callers that shell out — the mesh worker — observe the
/// real exit status); returns `Err` only when the module cannot be loaded/run.
pub fn run(cmd: WasmCmd) -> anyhow::Result<()> {
    match cmd {
        WasmCmd::Run { args } => {
            let host = match args.fuel {
                Some(f) if f > 0 => vox_wasm_engine::WasmHost::with_fuel(f)?,
                _ => vox_wasm_engine::WasmHost::new()?,
            };
            let opts = vox_wasm_engine::WasmExecOpts {
                args: args.args.clone(),
                preopens: parse_preopens(&args.preopen_ro, &args.preopen_rw)?,
                fuel_override: None,
                stdin: None,
                env: parse_env(&args.env)?,
            };
            let outcome = host.execute(&args.file, &opts)?;
            print!("{}", outcome.stdout_str());
            eprint!("{}", outcome.stderr_str());
            std::process::exit(outcome.exit_code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_preopens;

    #[test]
    fn parse_preopens_defaults_guest_to_host_and_handles_explicit_guest() {
        // vox-arch-check: allow abs-path
        let ps = parse_preopens(&["/tmp/data".into()], &["/var/out:/out".into()]).unwrap();
        assert_eq!(ps.len(), 2);
    }

    #[test]
    fn parse_preopens_rejects_empty_host() {
        assert!(parse_preopens(&[":guest".into()], &[]).is_err());
    }

    #[test]
    fn parse_env_splits_key_value_and_allows_empty_value() {
        let pairs = super::parse_env(&["A=1".into(), "B=".into(), "C=x=y".into()]).unwrap();
        assert_eq!(
            pairs,
            vec![
                ("A".to_string(), "1".to_string()),
                ("B".to_string(), String::new()),
                ("C".to_string(), "x=y".to_string()), // only the first '=' splits
            ]
        );
    }

    #[test]
    fn parse_env_rejects_missing_eq_or_empty_key() {
        assert!(super::parse_env(&["NOEQ".into()]).is_err());
        assert!(super::parse_env(&["=v".into()]).is_err());
    }
}
