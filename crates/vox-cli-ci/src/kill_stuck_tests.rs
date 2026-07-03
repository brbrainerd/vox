use anyhow::Result;
use std::path::Path;
use sysinfo::System;

pub fn run(root: &Path, what_if: bool) -> Result<()> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let root_str = root.to_string_lossy().to_string();
    let target_debug_deps = root
        .join("target")
        .join("debug")
        .join("deps")
        .to_string_lossy()
        .to_string();
    let target_release_deps = root
        .join("target")
        .join("release")
        .join("deps")
        .to_string_lossy()
        .to_string();

    let mut found = false;

    for (pid, process) in sys.processes() {
        // Safe check for missing cmd args in modern sysinfo
        if process.cmd().is_empty() {
            continue;
        }

        let cmd: Vec<_> = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        let cmd_str = cmd.join(" ");

        if cmd_str.is_empty() {
            continue;
        }

        let is_target_deps =
            cmd_str.contains(&target_debug_deps) || cmd_str.contains(&target_release_deps);
        let name = process.name().to_string_lossy().to_string().to_lowercase();
        let is_cargo_test = name == "cargo" || name == "cargo.exe";
        let is_running_test =
            is_cargo_test && cmd_str.contains(" test ") && cmd_str.contains(&root_str);

        if is_target_deps || is_running_test {
            found = true;
            if what_if {
                println!("Would stop PID {}: {}", pid, cmd_str);
            } else if process.kill() {
                println!("Stopped PID {}", pid);
            } else {
                println!("Failed to stop PID {}", pid);
            }
        }
    }

    if !found {
        println!("No matching cargo test / workspace test-binary processes found.");
    }

    Ok(())
}
