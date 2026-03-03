# Crate API: vox-gamify

## Overview

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

---

## Module: `vox-gamify\src\achievement.rs`

Achievement system for gamifying agent activities.

Tracks milestones like first task completion, first handoff,
error-free streaks, and cost efficiency. Achievements are
persisted and shown on the dashboard.


### `struct AchievementId`

Unique achievement identifier.


### `struct Achievement`

An achievement that can be unlocked by agents.


### `enum AchievementCategory`

Achievement categories.


### `struct UnlockedAchievement`

Record of an unlocked achievement.


### `struct AchievementTracker`

Tracks achievements per agent.


## Module: `vox-gamify\src\ai.rs`

Free AI client with multi-provider fallback.

Supports a cascade of providers so Vox is fully redistributable:
1. **Ollama** (local) — zero auth, best quality, no network
2. **Pollinations.ai** — zero API key, zero signup, HTTP GET
3. **Gemini Flash** — free tier, requires env var `GEMINI_API_KEY`
4. **Deterministic** — always works, no AI, pattern-based responses


### `enum FreeAiProvider`

Which AI backend to attempt.


### `struct FreeAiClient`

AI client that tries providers in order until one succeeds.


### `fn deterministic_response`

Always-available fallback that returns pattern-based responses.

This is NOT AI — it's a simple keyword matcher that ensures
Vox never fails when AI providers are unavailable.


## Module: `vox-gamify\src\battle.rs`

Bug battle system seeded from TOESTUB findings.


### `enum BugType`

Bug categories with associated reward tiers.


### `struct Battle`

A bug battle instance.


## Module: `vox-gamify\src\challenge.rs`

Coding challenges, leaderboards, and manager.


### `enum ChallengeType`

Categories of coding challenges.


### `struct Challenge`

A coding challenge that can be attempted by users for XP and crystals.


### `struct TestCase`

A specific input/output pair or condition to test a challenge solution.


### `struct ChallengeAttempt`

A user's attempt at solving a challenge.


### `struct ChallengeLeaderboardEntry`

A leaderboard entry for coding challenges.


### `struct ChallengeManager`

Manager for generating and scoring challenges.


## Module: `vox-gamify\src\companion.rs`

Code companions — living representations of Vox components.


### `enum Mood`

Emotional state of a companion, driven by code quality and interactions.


### `enum Interaction`

Actions a user can take with a companion.


### `enum Personality`

A companion's underlying personality archetype.


### `struct Companion`

A code companion — a living representation of a Vox component.


### `fn render_multi_agent_status`

Renders a multi-agent progress board


## Module: `vox-gamify\src\cost.rs`

Cost aggregation and per-session/per-agent cost tracking.

Tracks costs incurred by AI API calls (OpenRouter, Gemini, etc.)
and provides aggregation, budget alerts, and reporting.


### `struct CostRecord`

A single cost record for an API call.


### `struct CostSummary`

Aggregated cost summary for an agent or session.


### `struct CostAggregator`

In-memory cost aggregator.

Tracks costs per agent (and optionally per session) and provides
budget alert functionality.


## Module: `vox-gamify\src\db.rs`

Database persistence for gamification layer.


### `fn get_profile`

Load a gamify profile from the DB.


### `fn upsert_profile`

Upsert a gamify profile to the DB.


### `fn list_companions`

Load all companions for a user.


### `fn upsert_companion`

Upsert a companion.


### `fn get_companion`

Get a specific companion.


### `fn delete_companion`

Delete a companion.


### `fn list_quests`

Load all active quests for a user.


### `fn upsert_quest`

Upsert a quest.


### `fn get_quest`

Get a specific quest by ID.


### `fn delete_quest`

Delete a quest.


### `fn count_quests`

Count active quests for a user.


### `fn list_battles`

Load recent battles for a user.


### `fn insert_battle`

Insert a new battle record.


### `fn get_battle`

Get a specific battle by ID.


### `fn update_battle`

Update a battle.


### `fn count_battles`

Count battles played by a user.


### `fn leaderboard`

Get top users by XP for the leaderboard.


### `fn get_profile_stats`

Get aggregate profile stats (e.g. total completed quests, total battles won, etc.).


### `fn get_events`

Load recent events for an agent.


### `fn insert_event`

Insert a new agent event.


### `fn insert_cost_record`

Insert a cost record.


### `fn get_agent_cost_usd`

Get total cost for an agent.


### `fn list_cost_records`

Get cost records for an agent, most recent first.


### `fn insert_a2a_message`

Insert an A2A message into persistent storage.


### `fn get_pending_messages`

Get unacknowledged messages for a receiver.


### `fn list_a2a_messages`

List recent A2A messages (audit trail).


### `fn acknowledge_message`

Acknowledge an A2A message by ID.


### `fn insert_agent_session`

Insert a new agent session.


### `fn update_agent_session`

Update session status and optional context.


### `fn end_agent_session`

End a session by setting ended_at and status.


### `fn list_active_sessions`

Get active sessions.


### `fn upsert_agent_metric`

Upsert an aggregated metric for an agent.


### `fn get_agent_metrics`

Get all metrics for an agent in a given period.


### `fn process_event_rewards`

Process an orchestrator event for gamification rewards (XP, crystals, companion stats).

Handles all `AgentEventKind` variants by delegating companion stat changes to
`Companion::interact()` (SSOT) and awarding profile XP/crystals as appropriate.


## Module: `vox-gamify\src\leaderboard.rs`

Agent leaderboard for ranking agents by various metrics.

Tracks tasks completed, code quality, speed, cost efficiency,
and generates sortable leaderboard views.


### `enum LeaderboardMetric`

A metric category for leaderboard ranking.


### `struct AgentStats`

Per-agent stats tracked for the leaderboard.


### `struct LeaderboardEntry`

A row in the leaderboard.


### `struct Leaderboard`

Agent leaderboard.


## Module: `vox-gamify\src\lib.rs`

# vox-gamify

Gamification layer for the Vox programming language.

Provides code companions, daily quests, bug battles, ASCII sprites,
and a free multi-provider AI client (Pollinations / Ollama / Gemini).

All features work fully offline with deterministic fallbacks.


## Module: `vox-gamify\src\notifications.rs`

Gamification notifications and messaging.


### `enum NotificationType`

The type of gamification notification.


### `struct Notification`

A notification meant for the user.


### `struct NotificationManager`

Local storage of unread notifications during a session.


## Module: `vox-gamify\src\profile.rs`

Player profile with XP, leveling, energy, and crystals.


### `struct GamifyProfile`

A player's gamification profile.


## Module: `vox-gamify\src\quest.rs`

Daily quest system with templated generation and progress tracking.


### `enum QuestType`

Categories of daily quests.


### `struct QuestTemplate`

A template for generating quests, with difficulty tiers.


### `struct Quest`

An active quest instance.


### `fn generate_daily_quests`

Generate daily quests by picking random templates.

Uses a simple deterministic shuffle based on the day to ensure
the same quests appear for the same user on the same day.


## Module: `vox-gamify\src\schema.rs`

V5 database schema: Gamification tables.

Extends the existing vox-pm schema with tables for player profiles,
companions, quests, and battles.


## Module: `vox-gamify\src\sprite.rs`

ASCII sprite generation — deterministic + AI-powered.

Every companion gets a visual identity. The deterministic generator
always works offline; AI generation is an optional enhancement.


### `fn generate_deterministic`

Generate a deterministic ASCII sprite based on mood.

Always succeeds — no network, no AI required.


### `fn generate_ai_sprite`

Generate an AI-powered ASCII sprite, falling back to deterministic.

Uses the FreeAiClient's fallback chain. If all AI providers fail,
returns the deterministic sprite (never errors).


## Module: `vox-gamify\src\streak.rs`

Streak tracking with bonus XP and grace periods.


### `struct StreakTracker`

Tracks daily activity streaks.


### `enum StreakResult`

The result of attempting to record daily activity.


## Module: `vox-gamify\src\util.rs`

Shared utility functions for the gamification crate.


### `fn now_unix`

Current Unix timestamp in seconds.


