//! Nucleo command palette — fuzzy search over registered slash-commands.

use nucleo_matcher::{
    Config as NuConfig, Matcher, Utf32String,
    pattern::{CaseMatching, Normalization, Pattern},
};

/// A registered slash-command entry.
#[derive(Clone, Debug)]
pub struct Command {
    pub name: String,
    pub description: String,
}

/// Command palette: fuzzy-matches registered commands against a query string.
pub struct Palette {
    commands: Vec<Command>,
}

impl Palette {
    pub fn new(commands: Vec<Command>) -> Self {
        Self { commands }
    }

    /// Returns commands whose name fuzzy-matches `query`, ranked by score.
    pub fn search(&self, query: &str) -> Vec<&Command> {
        if query.is_empty() {
            return self.commands.iter().collect();
        }

        let mut matcher = Matcher::new(NuConfig::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);

        let mut scored: Vec<(u32, &Command)> = self
            .commands
            .iter()
            .filter_map(|cmd| {
                let hay = Utf32String::from(cmd.name.as_str());
                pattern
                    .score(hay.slice(..), &mut matcher)
                    .map(|score| (score, cmd))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, cmd)| cmd).collect()
    }
}

/// Default slash-commands surfaced in the palette.
pub fn default_commands() -> Vec<Command> {
    vec![
        Command {
            name: "/model".into(),
            description: "Switch or list AI models".into(),
        },
        Command {
            name: "/budget".into(),
            description: "Show or set token budget".into(),
        },
        Command {
            name: "/skills".into(),
            description: "List available skills".into(),
        },
        Command {
            name: "/memory".into(),
            description: "Open memory editor".into(),
        },
        Command {
            name: "/context".into(),
            description: "Edit context window".into(),
        },
        Command {
            name: "/sh".into(),
            description: "Run a shell command".into(),
        },
        Command {
            name: "/ai".into(),
            description: "Send a prompt to the agent".into(),
        },
        Command {
            name: "/help".into(),
            description: "Show this palette".into(),
        },
    ]
}
