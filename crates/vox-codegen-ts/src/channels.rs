//! Channel-contract SSOT loader (`contracts/channels.v1.yaml`) for the frontend
//! stream-subscription primitive. The contract maps a `.vox`-facing channel name
//! to its wire URI, payload type, replace/fold semantics, and optional polling
//! fallback. A parity test enforces that the contract's URI set matches the event
//! constants hand-declared in `crates/vox-gui/ui/src/transport.ts` so the two
//! cannot drift.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ChannelPoll {
    pub command: String,
    pub every_ms: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ChannelDef {
    pub name: String,
    pub uri: String,
    pub payload: String,
    pub semantics: String,
    #[serde(default)]
    pub poll: Option<ChannelPoll>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelContract {
    pub schema_version: u32,
    pub channels: Vec<ChannelDef>,
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/vox-codegen-ts → workspace root is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Load and parse the channel contract from the workspace `contracts/` dir.
pub fn load_channel_contract() -> ChannelContract {
    let path = workspace_root().join("contracts/channels.v1.yaml");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Look up a channel by its `.vox`-facing name.
pub fn channel_by_name<'a>(c: &'a ChannelContract, name: &str) -> Option<&'a ChannelDef> {
    c.channels.iter().find(|ch| ch.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn contract_parses_and_names_are_unique() {
        let c = load_channel_contract();
        assert_eq!(c.schema_version, 1);
        let names: BTreeSet<_> = c.channels.iter().map(|ch| ch.name.clone()).collect();
        assert_eq!(
            names.len(),
            c.channels.len(),
            "channel names must be unique"
        );
        for ch in &c.channels {
            assert!(
                ch.semantics == "replace" || ch.semantics == "fold",
                "channel {} has invalid semantics {}",
                ch.name,
                ch.semantics
            );
        }
    }

    /// Drift guard: the contract's URI set must equal the `vox://…` event-name
    /// string literals declared in transport.ts.
    #[test]
    fn contract_uris_match_transport_ts() {
        let c = load_channel_contract();
        let contract_uris: BTreeSet<String> = c.channels.iter().map(|ch| ch.uri.clone()).collect();

        let ts =
            std::fs::read_to_string(workspace_root().join("crates/vox-gui/ui/src/transport.ts"))
                .expect("read transport.ts");
        let mut ts_uris: BTreeSet<String> = BTreeSet::new();
        for (i, _) in ts.match_indices("'vox://") {
            let rest = &ts[i + 1..]; // skip opening quote
            if let Some(end) = rest.find('\'') {
                ts_uris.insert(rest[..end].to_string());
            }
        }

        let missing: Vec<_> = ts_uris.difference(&contract_uris).collect();
        let extra: Vec<_> = contract_uris.difference(&ts_uris).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "channel contract drifted from transport.ts.\n  in transport.ts but not contract: {missing:?}\n  in contract but not transport.ts: {extra:?}"
        );
    }
}
