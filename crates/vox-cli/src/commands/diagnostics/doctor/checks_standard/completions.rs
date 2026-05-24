use super::super::common::Check;

pub async fn run(checks: &mut Vec<Check>) {
    let home = dirs::home_dir().unwrap_or_default();
    if home.as_os_str().is_empty() {
        checks.push(Check::warn("Shell Completions", "could not determine home directory to verify completions".to_string()));
        return;
    }

    let is_windows = cfg!(windows);
    
    // Check PowerShell first since it's the standard on Windows and common elsewhere
    let mut pwsh_path = home.clone();
    if is_windows {
        pwsh_path.push("Documents");
        pwsh_path.push("WindowsPowerShell");
        pwsh_path.push("Microsoft.PowerShell_profile.ps1");
    } else {
        pwsh_path.push(".config");
        pwsh_path.push("powershell");
        pwsh_path.push("Microsoft.PowerShell_profile.ps1");
    }

    let mut bash_path = home.clone();
    bash_path.push(".bashrc");
    
    let mut zsh_path = home.clone();
    zsh_path.push(".zshrc");

    let mut fish_path = home.clone();
    fish_path.push(".config");
    fish_path.push("fish");
    fish_path.push("config.fish");

    let profiles = vec![
        ("PowerShell", pwsh_path),
        ("Bash", bash_path),
        ("Zsh", zsh_path),
        ("Fish", fish_path),
    ];

    let mut found_any = false;
    let mut found_shells = Vec::new();

    for (name, path) in profiles {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if content.contains("vox completions") {
                    found_any = true;
                    found_shells.push(name);
                }
            }
        }
    }

    if found_any {
        checks.push(Check::pass(
            "Shell Completions",
            format!("installed for: {}", found_shells.join(", ")),
        ));
    } else {
        checks.push(Check::warn(
            "Shell Completions",
            "no completion integration found. Run `vox completions <shell> --install` to improve discoverability.".to_string(),
        ));
    }
}
