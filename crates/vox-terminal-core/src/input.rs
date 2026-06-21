#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputIntent {
    VoxNative(String),
    Shell(String),
    Agent(String),
    Command { name: String, args: String },
}

pub fn classify(raw: &str) -> InputIntent {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix('!') {
        return InputIntent::Shell(rest.trim().to_string());
    }
    if let Some(rest) = t.strip_prefix('/') {
        let rest = rest.trim_start();
        if rest.is_empty() {
            return InputIntent::VoxNative(t.to_string());
        }
        let (name, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        return match name {
            "sh" | "shell" => InputIntent::Shell(args.trim().to_string()),
            "ai" | "agent" => InputIntent::Agent(args.trim().to_string()),
            _ => InputIntent::Command {
                name: name.to_string(),
                args: args.trim().to_string(),
            },
        };
    }
    InputIntent::VoxNative(t.to_string())
}
