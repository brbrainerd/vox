# vox-gamify

Gamification layer for the Vox programming language. Code companions, daily quests, bug battles, and ASCII sprites — all working fully offline.

## Features

| Module | Description |
|--------|-------------|
| `companion.rs` | Code companions with mood, interaction, and energy systems |
| `quest.rs` | Daily quests with randomized objectives |
| `battle.rs` | Bug battle encounters with typed bug categories |
| `sprite.rs` | ASCII art sprites for companions and bugs |
| `ai.rs` | Free multi-provider AI client (Pollinations / Ollama / Gemini) |
| `profile.rs` | Player profile with XP, level, and statistics |
| `db.rs` | Turso persistence for game state |
| `schema.rs` | Database schema (V5) for all gamification data |

## CLI Commands

```bash
vox gamify status              # Show your profile
vox gamify companion list      # List code companions
vox gamify companion adopt     # Adopt a new companion
vox gamify quest list           # View daily quests
vox gamify battle start         # Start a bug battle
```

## Design

All features work offline with deterministic fallbacks. The AI client (`FreeAiClient`) attempts multiple providers in order and falls back to template-based responses if all providers are unavailable.
