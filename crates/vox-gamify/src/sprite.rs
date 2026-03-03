//! ASCII sprite generation — deterministic + AI-powered.
//!
//! Every companion gets a visual identity. The deterministic generator
//! always works offline; AI generation is an optional enhancement.

use crate::ai::FreeAiClient;
use crate::companion::Mood;

// ─── Constants ───────────────────────────────────────────

/// Prompt template for AI sprite generation. {NAME}, {LANG}, {MOOD} are replaced.
const SPRITE_PROMPT_TEMPLATE: &str = "\
Generate a small ASCII art character (max 6 lines, max 12 chars wide) for a code companion named '{NAME}'. \
The companion represents {LANG} code and is feeling {MOOD}. \
Use only ASCII characters. Output ONLY the ASCII art, nothing else.";

// ─── Deterministic Sprites ───────────────────────────────

/// Sprite templates indexed by mood. Each is a compact ASCII art character.
const SPRITE_HAPPY: &str = "\
  /\\_/\\
 ( ^.^ )
  > ^ <
 /|   |\\
(_|   |_)";

const SPRITE_NEUTRAL: &str = "\
  /\\_/\\
 ( o.o )
  > ^ <
 /|   |\\
(_|   |_)";

const SPRITE_SAD: &str = "\
  /\\_/\\
 ( T.T )
  > ~ <
 /|   |\\
(_|   |_)";

const SPRITE_EXCITED: &str = "\
  /\\_/\\
 ( *.* )
  > ! <
 /|   |\\
(_|   |_)";

const SPRITE_TIRED: &str = "\
  /\\_/\\
 ( -.- )
  > z <
 /|   |\\
(_|   |_)";

/// Generate a deterministic ASCII sprite based on mood.
///
/// Always succeeds — no network, no AI required.
pub fn generate_deterministic(name: &str, mood: Mood) -> String {
    let base = match mood {
        Mood::Happy => SPRITE_HAPPY,
        Mood::Neutral => SPRITE_NEUTRAL,
        Mood::Sad => SPRITE_SAD,
        Mood::Excited => SPRITE_EXCITED,
        Mood::Tired => SPRITE_TIRED,
    };

    // Add a name tag below the sprite
    let name_tag = if name.len() <= 12 {
        format!(" {:^12}", name)
    } else {
        format!(" {:.12}", name)
    };

    format!("{}\n{}", base, name_tag)
}

/// Generate an AI-powered ASCII sprite, falling back to deterministic.
///
/// Uses the FreeAiClient's fallback chain. If all AI providers fail,
/// returns the deterministic sprite (never errors).
pub async fn generate_ai_sprite(
    client: &FreeAiClient,
    name: &str,
    language: &str,
    mood: Mood,
) -> String {
    let prompt = SPRITE_PROMPT_TEMPLATE
        .replace("{NAME}", name)
        .replace("{LANG}", language)
        .replace("{MOOD}", mood.as_str());

    match client.generate(&prompt).await {
        Ok(sprite) => {
            let trimmed = sprite.trim().to_string();
            // Validate: must have at least 2 lines and some non-whitespace
            if trimmed.lines().count() >= 2 && trimmed.chars().any(|c| !c.is_whitespace()) {
                trimmed
            } else {
                generate_deterministic(name, mood)
            }
        }
        Err(_) => generate_deterministic(name, mood),
    }
}

// ─── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_happy() {
        let sprite = generate_deterministic("Bot", Mood::Happy);
        assert!(sprite.contains("^.^"));
        assert!(sprite.contains("Bot"));
    }

    #[test]
    fn deterministic_sad() {
        let sprite = generate_deterministic("Sad Bot", Mood::Sad);
        assert!(sprite.contains("T.T"));
    }

    #[test]
    fn deterministic_excited() {
        let sprite = generate_deterministic("Wow", Mood::Excited);
        assert!(sprite.contains("*.*"));
    }

    #[test]
    fn deterministic_tired() {
        let sprite = generate_deterministic("Zzz", Mood::Tired);
        assert!(sprite.contains("-.-"));
    }

    #[test]
    fn deterministic_neutral() {
        let sprite = generate_deterministic("Norm", Mood::Neutral);
        assert!(sprite.contains("o.o"));
    }

    #[test]
    fn all_moods_non_empty() {
        for mood in &[
            Mood::Happy,
            Mood::Neutral,
            Mood::Sad,
            Mood::Excited,
            Mood::Tired,
        ] {
            let sprite = generate_deterministic("Test", *mood);
            assert!(
                sprite.lines().count() >= 5,
                "Mood {:?} sprite too short",
                mood
            );
        }
    }

    #[test]
    fn long_name_truncated() {
        let sprite = generate_deterministic("VeryLongCompanionName", Mood::Happy);
        // Name should still appear (truncated)
        assert!(sprite.contains("VeryLongComp"));
    }
}
