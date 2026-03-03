//! Code companions — living representations of Vox components.

use crate::util::now_unix;
use serde::{Deserialize, Serialize};

// ─── Constants ───────────────────────────────────────────

const FEED_HEALTH_GAIN: i32 = 10;
const PLAY_ENERGY_COST: i32 = 5;
const REST_ENERGY_GAIN: i32 = 15;

// ─── Mood ────────────────────────────────────────────────

/// Emotional state of a companion, driven by code quality and interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mood {
    Happy,
    Neutral,
    Sad,
    Excited,
    Tired,
}

impl Mood {
    /// Determine mood from a code quality score (0-100).
    pub fn from_quality(quality: u8) -> Self {
        match quality {
            81..=100 => Mood::Happy,
            61..=80 => Mood::Neutral,
            41..=60 => Mood::Sad,
            _ => Mood::Tired,
        }
    }

    /// Emoji representation.
    pub fn emoji(&self) -> &str {
        match self {
            Mood::Happy => "😊",
            Mood::Neutral => "😐",
            Mood::Sad => "😢",
            Mood::Excited => "🤩",
            Mood::Tired => "😴",
        }
    }

    /// Canonical string representation (used for DB storage and Display).
    pub fn as_str(&self) -> &str {
        match self {
            Mood::Happy => "happy",
            Mood::Neutral => "neutral",
            Mood::Sad => "sad",
            Mood::Excited => "excited",
            Mood::Tired => "tired",
        }
    }
}

impl std::str::FromStr for Mood {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "happy" => Ok(Mood::Happy),
            "neutral" => Ok(Mood::Neutral),
            "sad" => Ok(Mood::Sad),
            "excited" => Ok(Mood::Excited),
            "tired" => Ok(Mood::Tired),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for Mood {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Interaction ─────────────────────────────────────────

/// Actions a user can take with a companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// Feed: +health, mood → happy
    Feed,
    /// Play: -energy, mood → excited
    Play,
    /// Rest: +energy, mood → neutral
    Rest,
    /// Task assigned: -energy, mood → excited
    TaskAssigned,
    /// Lock acquired: -energy
    LockAcquired,
    /// Task failed: -health, mood → sad
    TaskFailed,
    /// Task completed: +health, mood → happy
    TaskCompleted,
    /// Writing code: -energy, mood → excited
    Writing,
    /// Idle: +energy, mood → tired
    Idle,
    /// Error encountered: -health, mood → sad
    Error,
}

// ─── Personality ─────────────────────────────────────────

/// A companion's underlying personality archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Personality {
    #[default]
    Cheerful,
    Focused,
    Edgy,
    Wise,
    Quirky,
}

impl Personality {
    pub fn as_str(&self) -> &'static str {
        match self {
            Personality::Cheerful => "cheerful",
            Personality::Focused => "focused",
            Personality::Edgy => "edgy",
            Personality::Wise => "wise",
            Personality::Quirky => "quirky",
        }
    }

    /// Get a contextual reaction for an interaction given this personality.
    pub fn react(&self, action: Interaction) -> &'static str {
        match (self, action) {
            (Personality::Cheerful, Interaction::TaskCompleted) => "Woohoo! We did it! 🎉",
            (Personality::Cheerful, Interaction::TaskFailed) => "Aww... Let's try again! 😊",
            (Personality::Focused, Interaction::TaskCompleted) => {
                "Task complete. Efficiency: optimal."
            }
            (Personality::Focused, Interaction::TaskFailed) => {
                "Suboptimal outcome. Analyzing failure."
            }
            (Personality::Edgy, Interaction::TaskCompleted) => "...whatever, it worked.",
            (Personality::Edgy, Interaction::Error) => "Typical. Everything's broken.",
            (Personality::Wise, Interaction::TaskCompleted) => {
                "To succeed is to persist, iteration by iteration."
            }
            (Personality::Wise, Interaction::Idle) => {
                "In stillness, the solution often reveals itself."
            }
            (Personality::Quirky, Interaction::Writing) => "My quantum quill is dancing!",
            (Personality::Quirky, Interaction::Rest) => {
                "Rebooting neural pathways... or just napping."
            }
            _ => "",
        }
    }
}

impl std::str::FromStr for Personality {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cheerful" => Ok(Personality::Cheerful),
            "focused" => Ok(Personality::Focused),
            "edgy" => Ok(Personality::Edgy),
            "wise" => Ok(Personality::Wise),
            "quirky" => Ok(Personality::Quirky),
            _ => Err(()),
        }
    }
}

// ─── Companion ───────────────────────────────────────────

/// A code companion — a living representation of a Vox component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Companion {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub code_hash: Option<String>,
    pub language: String,
    pub ascii_sprite: Option<String>,
    pub mood: Mood,
    pub health: i32,
    pub max_health: i32,
    pub energy: i32,
    pub max_energy: i32,
    pub code_quality: u8,
    pub last_active: i64,
    #[serde(default)]
    pub personality: Personality,
}

impl Companion {
    /// Create a new companion with default stats.
    pub fn new(
        id: impl Into<String>,
        user_id: impl Into<String>,
        name: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        let now = now_unix();
        Self {
            id: id.into(),
            user_id: user_id.into(),
            name: name.into(),
            description: None,
            code_hash: None,
            language: language.into(),
            ascii_sprite: None,
            mood: Mood::Neutral,
            health: 100,
            max_health: 100,
            energy: 100,
            max_energy: 100,
            code_quality: 50,
            last_active: now,
            personality: Personality::default(),
        }
    }

    /// Apply an interaction effect.
    pub fn interact(&mut self, action: Interaction) {
        match action {
            Interaction::Feed => {
                self.health = (self.health + FEED_HEALTH_GAIN).min(self.max_health);
                self.mood = Mood::Happy;
            }
            Interaction::Play => {
                self.energy = (self.energy - PLAY_ENERGY_COST).max(0);
                self.mood = Mood::Excited;
            }
            Interaction::Rest => {
                self.energy = (self.energy + REST_ENERGY_GAIN).min(self.max_energy);
                self.mood = Mood::Neutral;
            }
            Interaction::TaskAssigned => {
                self.energy = (self.energy - 10).max(0);
                self.mood = Mood::Excited;
            }
            Interaction::LockAcquired => {
                self.energy = (self.energy - 5).max(0);
            }
            Interaction::TaskFailed => {
                self.health = (self.health - 20).max(0);
                self.mood = Mood::Sad;
            }
            Interaction::TaskCompleted => {
                self.health = (self.health + 10).min(self.max_health);
                self.mood = Mood::Happy;
            }
            Interaction::Writing => {
                self.energy = (self.energy - 2).max(0);
                self.mood = Mood::Excited;
            }
            Interaction::Idle => {
                self.energy = (self.energy + 5).min(self.max_energy);
                self.mood = Mood::Tired;
            }
            Interaction::Error => {
                self.health = (self.health - 15).max(0);
                self.mood = Mood::Sad;
            }
        }
        self.update_sprite();
        self.last_active = now_unix();
    }

    /// Update the ASCII sprite based on the current mood
    pub fn update_sprite(&mut self) {
        let sprite = match self.mood {
            Mood::Happy => {
                r#"
  /\_/\
 ( ^.^ )
  > ^ <
"#
            }
            Mood::Neutral => {
                r#"
  /\_/\
 ( -.- )
  > ^ <
"#
            }
            Mood::Sad => {
                r#"
  /\_/\
 ( ;_; )
  > ^ <
"#
            }
            Mood::Excited => {
                r#"
  /\_/\
 ( *.* )
  > ^ <
"#
            }
            Mood::Tired => {
                r#"
  /\_/\
 ( ~.~ )
  > ^ <
"#
            }
        };
        self.ascii_sprite = Some(sprite.to_string());
    }

    /// Render an HTML-safe version of the ASCII sprite, preserving whitespace.
    pub fn render_html_sprite(&self) -> String {
        let raw = self.ascii_sprite.as_deref().unwrap_or("  ?\n (?) ");
        format!(
            "<pre class=\"companion-sprite\">\n{}\n</pre>",
            raw.replace("<", "&lt;").replace(">", "&gt;")
        )
    }

    /// Update mood based on current code quality score.
    pub fn update_mood_from_quality(&mut self) {
        self.mood = Mood::from_quality(self.code_quality);
    }

    /// Check if this companion has enough energy for a battle.
    pub fn can_battle(&self) -> bool {
        self.energy >= 20
    }

    /// Consume energy for a battle. Returns false if insufficient.
    pub fn spend_battle_energy(&mut self) -> bool {
        if self.can_battle() {
            self.energy -= 20;
            true
        } else {
            false
        }
    }

    /// Health as a percentage (0.0 - 1.0).
    pub fn health_pct(&self) -> f64 {
        if self.max_health == 0 {
            return 0.0;
        }
        self.health as f64 / self.max_health as f64
    }

    /// Energy as a percentage (0.0 - 1.0).
    pub fn energy_pct(&self) -> f64 {
        if self.max_energy == 0 {
            return 0.0;
        }
        self.energy as f64 / self.max_energy as f64
    }

    /// Render an ASCII health and energy progress bar.
    pub fn render_status_bar(&self, width: usize) -> String {
        let hp_w = (self.health_pct() * width as f64).round() as usize;
        let diff_hp = width.saturating_sub(hp_w);
        let hp_bar = format!("[{}{}]", "♥".repeat(hp_w), " ".repeat(diff_hp));

        let en_w = (self.energy_pct() * width as f64).round() as usize;
        let diff_en = width.saturating_sub(en_w);
        let en_bar = format!("[{}{}]", "⚡".repeat(en_w), " ".repeat(diff_en));

        format!(
            "{:<12} {} HP: {} EN: {}",
            self.name,
            self.mood.emoji(),
            hp_bar,
            en_bar,
        )
    }
}

/// Renders a multi-agent progress board
pub fn render_multi_agent_status(companions: &[&Companion]) -> String {
    let mut out = String::new();
    out.push_str("========================================================\n");
    out.push_str("                  VOX AGENT HUD                         \n");
    out.push_str("========================================================\n");
    for c in companions {
        out.push_str(&format!("{}\n", c.render_status_bar(10)));
    }
    out.push_str("========================================================\n");
    out
}

// ─── Helpers ─────────────────────────────────────────────

// ─── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_companion_defaults() {
        let c = Companion::new("c-1", "u-1", "TestBot", "vox");
        assert_eq!(c.health, 100);
        assert_eq!(c.energy, 100);
        assert_eq!(c.mood, Mood::Neutral);
        assert_eq!(c.code_quality, 50);
    }

    #[test]
    fn feed_increases_health() {
        let mut c = Companion::new("c-1", "u-1", "Bot", "vox");
        c.health = 80;
        c.interact(Interaction::Feed);
        assert_eq!(c.health, 90);
        assert_eq!(c.mood, Mood::Happy);
    }

    #[test]
    fn feed_caps_at_max_health() {
        let mut c = Companion::new("c-1", "u-1", "Bot", "vox");
        c.health = 95;
        c.interact(Interaction::Feed);
        assert_eq!(c.health, 100); // capped
    }

    #[test]
    fn play_costs_energy() {
        let mut c = Companion::new("c-1", "u-1", "Bot", "vox");
        c.interact(Interaction::Play);
        assert_eq!(c.energy, 95);
        assert_eq!(c.mood, Mood::Excited);
    }

    #[test]
    fn play_energy_floors_at_zero() {
        let mut c = Companion::new("c-1", "u-1", "Bot", "vox");
        c.energy = 2;
        c.interact(Interaction::Play);
        assert_eq!(c.energy, 0);
    }

    #[test]
    fn rest_restores_energy() {
        let mut c = Companion::new("c-1", "u-1", "Bot", "vox");
        c.energy = 50;
        c.interact(Interaction::Rest);
        assert_eq!(c.energy, 65);
        assert_eq!(c.mood, Mood::Neutral);
    }

    #[test]
    fn mood_from_quality() {
        assert_eq!(Mood::from_quality(90), Mood::Happy);
        assert_eq!(Mood::from_quality(70), Mood::Neutral);
        assert_eq!(Mood::from_quality(50), Mood::Sad);
        assert_eq!(Mood::from_quality(30), Mood::Tired);
        assert_eq!(Mood::from_quality(0), Mood::Tired);
    }

    #[test]
    fn battle_energy_check() {
        let mut c = Companion::new("c-1", "u-1", "Bot", "vox");
        assert!(c.can_battle());
        c.energy = 15;
        assert!(!c.can_battle());
        assert!(!c.spend_battle_energy());
    }

    #[test]
    fn health_energy_percentages() {
        let c = Companion::new("c-1", "u-1", "Bot", "vox");
        assert!((c.health_pct() - 1.0).abs() < 0.01);
        assert!((c.energy_pct() - 1.0).abs() < 0.01);
    }
}
