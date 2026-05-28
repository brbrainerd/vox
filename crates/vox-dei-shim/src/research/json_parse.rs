use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

pub(super) fn parse_json_response<T: DeserializeOwned>(text: &str) -> Result<T> {
    let block = extract_json_block(text);
    serde_json::from_str(block).with_context(|| "parse research JSON response")
}

fn extract_json_block(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find("```json") {
        let rest = &trimmed[start + "```json".len()..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim();
        }
    }
    if let Some(start) = trimmed.find("```") {
        let rest = &trimmed[start + "```".len()..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim();
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Payload {
        value: String,
    }

    #[test]
    fn extracts_json_codeblock() {
        let payload: Payload =
            parse_json_response("before\n```json\n{\"value\":\"ok\"}\n```\nafter").unwrap();

        assert_eq!(payload.value, "ok");
    }
}
