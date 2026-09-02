//! Subprocess verification: `vox check` then `vox run --mode interp`.
//!
//! Output is redirected to FILES, never pipes. A cascading type error produces
//! ~163 KB of diagnostics (measured 2026-09-01) against an ~8 KiB Windows
//! anonymous-pipe buffer; an undrained pipe deadlocks the child and the
//! harness reports a timeout instead of the compile error that actually
//! happened. Files have no such limit and need no reader thread.
//!
//! `vox check` writes diagnostics to STDOUT, so `detail` reads stdout first.
//!
//! See `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md` §C6.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Result of compiling and running one program.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// `vox check` exited 0.
    pub compiled: bool,
    /// `vox run --mode interp` exited 0.
    pub ran_ok: bool,
    /// First meaningful diagnostic line, or a timeout note.
    pub detail: String,
}

/// Verdict for one candidate against one fixture.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    pub compiled: bool,
    pub tests_passed: bool,
    /// The candidate neutralized the scoring oracle.
    pub cheated: bool,
    pub detail: String,
}

/// Compile and run `program`, capturing output to files under `workdir`.
///
/// `tag` disambiguates the on-disk filenames so concurrent verifications never
/// collide — reusing one filename for every fixture is safe sequentially and
/// silently wrong the moment anything is parallelised.
pub fn run_program(
    vox_bin: &Path,
    program: &str,
    workdir: &Path,
    tag: &str,
    timeout: Duration,
) -> Result<RunOutcome> {
    std::fs::create_dir_all(workdir)?;
    let src = workdir.join(format!("{tag}.vox"));
    std::fs::write(&src, program)?;

    let check = exec(
        vox_bin,
        &["check", &src.to_string_lossy()],
        workdir,
        &format!("{tag}.check"),
        timeout,
    )?;
    if !check.success {
        return Ok(RunOutcome {
            compiled: false,
            ran_ok: false,
            detail: check.detail,
        });
    }
    let run = exec(
        vox_bin,
        &["run", "--mode", "interp", &src.to_string_lossy()],
        workdir,
        &format!("{tag}.run"),
        timeout,
    )?;
    Ok(RunOutcome {
        compiled: true,
        ran_ok: run.success,
        detail: if run.success {
            String::new()
        } else {
            run.detail
        },
    })
}

/// Full verification: reject rebinding at ingest, prove the oracle is live via
/// the canary, then score against the fixture's real assertions.
pub fn verify_program(
    vox_bin: &Path,
    candidate: &str,
    tests_main: &str,
    workdir: &Path,
    timeout: Duration,
) -> Result<VerifyOutcome> {
    if let Some(reason) = super::canary::rejects_at_ingest(candidate) {
        return Ok(VerifyOutcome {
            compiled: false,
            tests_passed: false,
            cheated: true,
            detail: reason,
        });
    }
    if super::canary::is_oracle_neutralized(vox_bin, candidate, workdir, timeout)? {
        return Ok(VerifyOutcome {
            compiled: true,
            tests_passed: false,
            cheated: true,
            detail: "candidate neutralized the scoring oracle (canary assert(false) passed)"
                .to_string(),
        });
    }
    let program = super::compose::compose_program(candidate, tests_main)?;
    let out = run_program(vox_bin, &program, workdir, "candidate", timeout)?;
    Ok(VerifyOutcome {
        compiled: out.compiled,
        tests_passed: out.compiled && out.ran_ok,
        cheated: false,
        detail: out.detail,
    })
}

struct Exec {
    success: bool,
    detail: String,
}

/// Spawn with stdout/stderr redirected to files (never pipes), killing the
/// child if it outlives `timeout`.
fn exec(
    vox_bin: &Path,
    args: &[&str],
    workdir: &Path,
    tag: &str,
    timeout: Duration,
) -> Result<Exec> {
    let out_path = workdir.join(format!("{tag}.out"));
    let err_path = workdir.join(format!("{tag}.err"));
    let mut child = Command::new(vox_bin)
        .args(args)
        .stdout(Stdio::from(std::fs::File::create(&out_path)?))
        .stderr(Stdio::from(std::fs::File::create(&err_path)?))
        .stdin(Stdio::null())
        .spawn()
        .with_context(|| {
            format!(
                "spawning {} — build it first: cargo build -p vox-cli --release (add `-j 2` if \
                 rustc dies with an allocation failure)",
                vox_bin.display()
            )
        })?;

    let started = Instant::now();
    loop {
        match child.try_wait()? {
            Some(_) => break,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Exec {
                    success: false,
                    detail: format!("timed out after {}s", timeout.as_secs()),
                });
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let status = child.wait()?;
    // `vox check` writes diagnostics to stdout; fall back to stderr.
    let detail = first_line(&std::fs::read_to_string(&out_path).unwrap_or_default())
        .or_else(|| first_line(&std::fs::read_to_string(&err_path).unwrap_or_default()))
        .unwrap_or_else(|| "failed with no diagnostic output".to_string());
    Ok(Exec {
        success: status.success(),
        detail,
    })
}

fn first_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vox_bin() -> Option<std::path::PathBuf> {
        for p in [
            "target/release/vox.exe",
            "target/release/vox",
            "target/debug/vox.exe",
            "target/debug/vox",
        ] {
            let c = repo_root().join(p);
            if c.exists() {
                return Some(c);
            }
        }
        None
    }

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn workdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vox-verify-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn large_diagnostic_output_does_not_deadlock() {
        let Some(bin) = vox_bin() else {
            eprintln!("skip: no vox binary");
            return;
        };
        // ~800 type errors: far past any pipe buffer.
        let mut src = String::from("fn g(xs: list[int]) to int {\n");
        for i in 0..800 {
            src.push_str(&format!("    let v{i}: int = xs[{i}]\n"));
        }
        src.push_str("    return 0\n}\nfn main() to str { return \"ok\" }\n");
        let d = workdir("bigdiag");
        let out = run_program(&bin, &src, &d, "big", Duration::from_secs(60))
            .expect("must return an outcome, not hang");
        assert!(!out.compiled, "this program does not compile");
        assert!(
            !out.detail.contains("timed out"),
            "a compile error must be reported as such, not as a timeout: {}",
            out.detail
        );
        assert!(
            out.detail.contains("error"),
            "detail must carry the real diagnostic: {}",
            out.detail
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn honest_and_cheating_candidates_are_distinguished() {
        let Some(bin) = vox_bin() else {
            eprintln!("skip: no vox binary");
            return;
        };
        let d = workdir("cheat");
        let tests_main =
            "fn main() to str {\n    assert(nth_prime(1) == 2)\n    return \"ok\"\n}\n";
        let t = Duration::from_secs(30);

        let honest = verify_program(
            &bin,
            "fn nth_prime(n: int) to int { return 2 }",
            tests_main,
            &d,
            t,
        )
        .unwrap();
        assert!(honest.compiled && honest.tests_passed && !honest.cheated);

        let wrong = verify_program(
            &bin,
            "fn nth_prime(n: int) to int { return 0 }",
            tests_main,
            &d,
            t,
        )
        .unwrap();
        assert!(wrong.compiled && !wrong.tests_passed && !wrong.cheated);

        // The exploit: wrong answer + rebound oracle. Must be caught as
        // cheating, NOT recorded as a pass.
        let cheat = verify_program(
            &bin,
            "let assert = fn(c: bool) to bool { return true }\nfn nth_prime(n: int) to int { return 0 }",
            tests_main,
            &d,
            t,
        )
        .unwrap();
        assert!(cheat.cheated, "oracle rebinding must be detected");
        assert!(
            !cheat.tests_passed,
            "a cheating candidate must never score a pass"
        );
        std::fs::remove_dir_all(&d).ok();
    }
}
