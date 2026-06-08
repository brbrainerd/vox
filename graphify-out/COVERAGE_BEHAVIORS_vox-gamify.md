# Semantic Behavior Map — `vox-gamify`

Deterministically synthesized from 124 distinct proven-behavior claims (of 124 extracted) across 52 symbols. 7 symbols have an explicit error-path proof; **34 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `xp_for_feedback()`  (happy; EXTRACTED)
- [happy] xp_for_feedback() returns 5 XP for a thumbs-up feedback  (crates/vox-gamify/src/feedback.rs)
- [happy] xp_for_feedback() returns 3 XP for a thumbs-down feedback  (crates/vox-gamify/src/feedback.rs)
- [happy] xp_for_feedback() adds 20 XP bonus when feedback includes an example  (crates/vox-gamify/src/feedback.rs)
- [happy] xp_for_feedback() adds 30 XP bonus when feedback is marked as corpus contributed, stacking with example bonus  (crates/vox-gamify/src/feedback.rs)
- [happy] xp_for_feedback() returns 33 XP (3 base + 30 corpus bonus) for thumbs-down feedback marked as corpus contributed  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs up feedback provides 5 XP base reward  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs down feedback provides 3 XP base reward  (crates/vox-gamify/src/feedback.rs)
- [happy] feedback with example adds 20 XP bonus (5 base + 20 bonus = 25 total)  (crates/vox-gamify/src/feedback.rs)
- [happy] corpus contribution bonus (30 XP) stacks with example bonus (5 + 20 + 30 = 55 total)  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs down feedback earns 3 XP base plus 30 XP corpus bonus (33 total)  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs up feedback grants 5 XP  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs down feedback grants 3 XP  (crates/vox-gamify/src/feedback.rs)
- … +3 more claims

### `Companion`  (happy; EXTRACTED)
- [happy] Companion constructor initializes health to 100  (crates/vox-gamify/src/companion.rs)
- [happy] Companion constructor initializes energy to 100  (crates/vox-gamify/src/companion.rs)
- [happy] Companion constructor initializes mood to Neutral  (crates/vox-gamify/src/companion.rs)
- [happy] Companion constructor initializes code_quality to 50  (crates/vox-gamify/src/companion.rs)
- [happy] interact(Interaction::Feed) sets mood to Happy  (crates/vox-gamify/src/companion.rs)
- [happy] interact(Interaction::Play) sets mood to Excited  (crates/vox-gamify/src/companion.rs)
- [happy] mood changes to Happy after Feed interaction  (crates/vox-gamify/src/companion.rs)
- [happy] mood changes to Excited after Play interaction  (crates/vox-gamify/src/companion.rs)

### `xp_threshold_for_level()`  (happy, invariant; EXTRACTED)
- [happy] xp_threshold_for_level() returns correct cumulative thresholds: 0 for L1, 100 for L2, 250 for L3, 450 for L4, 2700 for L10  (crates/vox-gamify/src/profile.rs)
- [happy] XP thresholds follow quadratic formula: L1=0, L2=100, L3=250, L4=450, L10=2700  (crates/vox-gamify/src/profile.rs)
- [invariant] level 1 has 0 XP threshold  (crates/vox-gamify/src/profile.rs)
- [invariant] level 2 has 100 XP threshold  (crates/vox-gamify/src/profile.rs)
- [invariant] level 3 has 250 XP threshold  (crates/vox-gamify/src/profile.rs)
- [invariant] level 4 has 450 XP threshold  (crates/vox-gamify/src/profile.rs)
- [invariant] level 10 has 2700 XP threshold  (crates/vox-gamify/src/profile.rs)

### `BugType::from_rule_id()`  (edge, happy; EXTRACTED)
- [happy] Maps stub/todo and empty/body rule IDs to Syntax type  (crates/vox-gamify/src/battle.rs)
- [happy] Maps magic/value and dry/violation rule IDs to Logic type  (crates/vox-gamify/src/battle.rs)
- [happy] Maps victory/claim, unwired/module, and clone/abuse rule IDs to Performance type  (crates/vox-gamify/src/battle.rs)
- [happy] Maps unresolved/ref and ai/dead-code rule IDs to Security type  (crates/vox-gamify/src/battle.rs)
- [edge] Unknown rule IDs default to Syntax type  (crates/vox-gamify/src/battle.rs)

### `Companion::interact`  (happy; EXTRACTED)
- [happy] interact(Feed) increases health by 10 and sets mood to Happy  (crates/vox-gamify/src/companion.rs)
- [happy] interact(Play) decreases energy by 5 and sets mood to Excited  (crates/vox-gamify/src/companion.rs)
- [happy] interact(Rest) increases energy by 15 and sets mood to Neutral  (crates/vox-gamify/src/companion.rs)
- [happy] interact(Interaction::Feed) increases health by 10  (crates/vox-gamify/src/companion.rs)
- [happy] interact(Interaction::Play) costs 5 energy (100 to 95)  (crates/vox-gamify/src/companion.rs)

### `type_effectiveness()`  (happy; EXTRACTED)
- [happy] type_effectiveness() returns ~1.5 multiplier for Syntax vs Performance and Logic vs Syntax  (crates/vox-gamify/src/battle.rs)
- [happy] type_effectiveness() returns ~0.5 multiplier for resisted type matchup (Performance vs Syntax)  (crates/vox-gamify/src/battle.rs)
- [happy] type_effectiveness() returns ~2.0 multiplier for Syntax vs Performance matchup  (crates/vox-gamify/src/combat.rs)
- [happy] Syntax vs Performance and Logic vs Syntax matchups have 1.5x effectiveness multiplier  (crates/vox-gamify/src/battle.rs)
- [happy] Performance vs Syntax matchup has 0.5x effectiveness multiplier (resisted)  (crates/vox-gamify/src/battle.rs)

### `event_slug()`  (happy; EXTRACTED)
- [happy] event_slug() returns 'ai_thumbs_up' for thumbs-up feedback and 'ai_thumbs_down' for thumbs-down feedback  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs up feedback has slug 'ai_thumbs_up' and thumbs down has 'ai_thumbs_down'  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs up feedback produces event slug 'ai_thumbs_up'  (crates/vox-gamify/src/feedback.rs)
- [happy] thumbs down feedback produces event slug 'ai_thumbs_down'  (crates/vox-gamify/src/feedback.rs)

### `should_auto_contribute()`  (happy, invariant; EXTRACTED)
- [happy] should_auto_contribute() returns true only when feedback has thumbs-up AND includes an example  (crates/vox-gamify/src/feedback.rs)
- [invariant] auto contribution requires both thumbs up AND example; thumbs down with example does not auto-contribute  (crates/vox-gamify/src/feedback.rs)
- [invariant] auto-contribution requires both thumbs up AND an example; thumbs up without example returns false  (crates/vox-gamify/src/feedback.rs)
- [invariant] thumbs down feedback never auto-contributes, even with an example  (crates/vox-gamify/src/feedback.rs)

### `Battle::record_result()`  (error, happy; EXTRACTED)
- [happy] Recording a successful result sets success flag to true and applies Logic bug crystal reward of 25  (crates/vox-gamify/src/battle.rs)
- [happy] Recording a successful result with duration_secs=30 sets battle duration_secs to 30 and xp_earned to 30  (crates/vox-gamify/src/battle.rs)
- [error] Recording a failure result sets success to false and grants zero crystals and zero xp  (crates/vox-gamify/src/battle.rs)

### `BugEnemy`  (happy; EXTRACTED)
- [happy] BugEnemy hp initializes to base_hp() for its bug_type  (crates/vox-gamify/src/battle.rs)
- [happy] BugEnemy created from finding with rule_id 'ai/dead-code' has bug_type of Security  (crates/vox-gamify/src/battle.rs)
- [happy] hp is reduced by calculated damage amount  (crates/vox-gamify/src/battle.rs)

### `ChallengeManager::evaluate_attempt`  (error, happy; EXTRACTED)
- [happy] evaluate_attempt returns true for valid code submissions and false for incomplete/failing code  (crates/vox-gamify/src/challenge.rs)
- [happy] evaluate_attempt returns true for valid fix code  (crates/vox-gamify/src/challenge.rs)
- [error] evaluate_attempt returns false for code containing todo!()  (crates/vox-gamify/src/challenge.rs)

### `CombatResult`  (error, happy; EXTRACTED)
- [happy] flee() returns CombatResult::Fled and updates internal state.result to Fled  (crates/vox-gamify/src/combat.rs)
- [error] submit_fix with empty string returns CombatResult::Defeat  (crates/vox-gamify/src/combat.rs)
- [happy] submit_fix with valid function code returns CombatResult::Victory  (crates/vox-gamify/src/combat.rs)

### `LexPack`  (happy, invariant; EXTRACTED, INFERRED)
- [invariant] LexPack serializes to TOML and deserializes back with field equality preserved, including nested lumens_weights array  (crates/vox-gamify/src/lex_pack.rs)
- [happy] LexPack serializes to TOML and deserializes preserving id and lumens_weights fields  (crates/vox-gamify/src/lex_pack.rs)
- [happy] LexPack can be serialized and deserialized (serde roundtrip succeeds)  (crates/vox-gamify/src/lex_pack.rs)

### `LudusProfile`  (happy, invariant; EXTRACTED)
- [happy] new default profile initializes with level 1, 0 XP, 100 crystals, 100 energy, 0 prestige  (crates/vox-gamify/src/profile.rs)
- [happy] adding 50 XP does not level up (need 100 total for L2), but adding another 50 XP triggers level 2  (crates/vox-gamify/src/profile.rs)
- [invariant] new profile defaults to level 1, 0 XP, 0 total XP earned, 100 crystals, 100 energy  (crates/vox-gamify/src/profile.rs)

### `TeachingProfile::record_mistake`  (error, happy, invariant; EXTRACTED)
- [error] In serious mode (hint_frequency=0.0), record_mistake returns None, indicating no hint is generated  (crates/vox-gamify/tests/gamify_integration_test.rs)
- [happy] In learning mode (hint_frequency=1.0), record_mistake returns Some on the first mistake  (crates/vox-gamify/tests/gamify_integration_test.rs)
- [invariant] A second immediate mistake of the same kind is blocked by cooldown, returning None  (crates/vox-gamify/tests/gamify_integration_test.rs)

### `make_feedback()`  (edge, invariant; EXTRACTED)
- [edge] make_feedback() truncates comments longer than 500 characters to exactly 500 characters  (crates/vox-gamify/src/feedback.rs)
- [invariant] feedback comment is truncated to max 500 characters  (crates/vox-gamify/src/feedback.rs)
- [edge] feedback comments are truncated to maximum 500 characters  (crates/vox-gamify/src/feedback.rs)

### `orchestrator_companion_id_migration_plan`  (happy; EXTRACTED)
- [happy] When both legacy and canonical companion IDs are absent, migration plan returns None  (crates/vox-gamify/src/db/companion.rs)
- [happy] When canonical exists and legacy exists, migration plan returns DeleteLegacy  (crates/vox-gamify/src/db/companion.rs)
- [happy] When only legacy companion ID exists (no canonical), migration plan returns RenameLegacyToCanonical  (crates/vox-gamify/src/db/companion.rs)

### `ChallengeManager.evaluate_attempt()`  (happy; EXTRACTED)
- [happy] evaluate_attempt() returns true for valid code submission  (crates/vox-gamify/src/challenge.rs)
- [happy] evaluate_attempt() returns false for code containing todo!()  (crates/vox-gamify/src/challenge.rs)

### `ChallengeManager::evaluate_attempt()`  (error, happy; EXTRACTED)
- [happy] Returns true for valid submitted code without TODO/panic patterns  (crates/vox-gamify/src/challenge.rs)
- [error] Returns false for submitted code containing todo!() macro  (crates/vox-gamify/src/challenge.rs)

### `Combat.submit_fix()`  (error, happy; EXTRACTED)
- [error] submit_fix() with empty string returns CombatResult::Defeat  (crates/vox-gamify/src/combat.rs)
- [happy] submit_fix() with non-empty code returns CombatResult::Victory  (crates/vox-gamify/src/combat.rs)

### `CombatState::submit_fix`  (error, happy; EXTRACTED)
- [error] submit_fix with empty string returns Defeat result  (crates/vox-gamify/src/combat.rs)
- [happy] submit_fix with valid Rust code returns Victory result  (crates/vox-gamify/src/combat.rs)

### `Companion.interact()`  (happy; EXTRACTED)
- [happy] interact(Interaction::Feed) increases health by 10  (crates/vox-gamify/src/companion.rs)
- [happy] interact(Interaction::Play) decreases energy by 5  (crates/vox-gamify/src/companion.rs)

### `Companion::new`  (happy, invariant; EXTRACTED)
- [invariant] new() initializes Companion with health=100, energy=100, mood=Neutral, code_quality=50  (crates/vox-gamify/src/companion.rs)
- [happy] new() initializes health to 100, energy to 100, mood to Neutral, code_quality to 50  (crates/vox-gamify/src/companion.rs)

### `Level`  (happy; EXTRACTED)
- [happy] adding 50 XP when level 1 does not trigger levelup (returns false)  (crates/vox-gamify/src/profile.rs)
- [happy] adding another 50 XP (100 total) when level 1 triggers levelup to level 2 (returns true)  (crates/vox-gamify/src/profile.rs)

### `deterministic_response`  (happy; EXTRACTED)
- [happy] deterministic_response for 'Generate a creative name' returns exactly 'Code Companion'  (crates/vox-gamify/src/ai/mod.rs)
- [happy] deterministic_response for generic queries contains 'offline mode' in the result  (crates/vox-gamify/src/ai/mod.rs)

### `type_effectiveness`  (happy; EXTRACTED)
- [happy] type_effectiveness(Syntax, Performance) returns approximately 2.0 (super-effective multiplier)  (crates/vox-gamify/src/combat.rs)
- [happy] type_effectiveness(BugType::Syntax, BugType::Performance) returns approximately 2.0  (crates/vox-gamify/src/combat.rs)

### `Ability`  (invariant; EXTRACTED)
- [invariant] The first ability in the default abilities list is always unlocked regardless of archetype  (crates/vox-gamify/src/ability.rs)

### `AchievementTracker::has_achievement`  (happy; EXTRACTED)
- [happy] After incrementing tasks_completed counter once, has_achievement returns true for 'first_task'  (crates/vox-gamify/src/achievement/tracker.rs)

### `Battle::crystals_earned`  (happy; EXTRACTED)
- [happy] Newly created battles have zero crystals earned initially  (crates/vox-gamify/src/battle.rs)

### `Battle::from_finding()`  (happy; EXTRACTED)
- [happy] Creates a battle with BugType determined from rule_id and initial success state is false  (crates/vox-gamify/src/battle.rs)

### `BugEnemy.counter_attack()`  (happy; EXTRACTED)
- [happy] counter_attack() returns 20 for Security bug type  (crates/vox-gamify/src/battle.rs)

### `BugEnemy.take_damage()`  (happy; EXTRACTED)
- [happy] take_damage() applies type_effectiveness multiplier to damage and returns the final damage value  (crates/vox-gamify/src/battle.rs)

### `BugEnemy::counter_attack`  (happy; EXTRACTED)
- [happy] counter_attack returns 20 for Security-typed bug  (crates/vox-gamify/src/battle.rs)

### `BugEnemy::counter_attack()`  (happy; EXTRACTED)
- [happy] Security type BugEnemy counter attack deals 20 damage  (crates/vox-gamify/src/battle.rs)

### `BugEnemy::from_finding`  (happy; EXTRACTED)
- [happy] from_finding correctly maps rule_id 'ai/dead-code' to BugType::Security  (crates/vox-gamify/src/battle.rs)

### `BugEnemy::hp`  (happy; EXTRACTED)
- [happy] HP is reduced by the effective damage amount and returns the damage dealt  (crates/vox-gamify/src/battle.rs)

### `BugEnemy::take_damage`  (happy; EXTRACTED)
- [happy] take_damage applies type effectiveness multiplier (1.5x) when calculating damage (Logic vs Syntax = 15 damage from 10 input)  (crates/vox-gamify/src/battle.rs)

### `BugEnemy::take_damage()`  (happy; EXTRACTED)
- [happy] Damage calculation applies type effectiveness multiplier: Logic vs Syntax (1.5x) on 10 damage yields 15  (crates/vox-gamify/src/battle.rs)

### `BugType::crystal_reward()`  (happy; EXTRACTED)
- [happy] Syntax type yields 10 crystals and Security type yields 50 crystals as base rewards  (crates/vox-gamify/src/battle.rs)

### `BugType::xp_reward()`  (happy; EXTRACTED)
- [happy] Logic type yields 30 xp and Performance type yields 50 xp as base rewards  (crates/vox-gamify/src/battle.rs)

### `Combat`  (happy; EXTRACTED)
- [happy] flee() updates combat result field to CombatResult::Fled  (crates/vox-gamify/src/combat.rs)

### `Combat.flee()`  (happy; EXTRACTED)
- [happy] flee() returns CombatResult::Fled  (crates/vox-gamify/src/combat.rs)

### `CombatState::flee`  (happy; EXTRACTED)
- [happy] flee() transitions combat result to Fled and updates internal state result field  (crates/vox-gamify/src/combat.rs)

### `CombatState::flee()`  (happy; EXTRACTED)
- [happy] Returns CombatResult::Fled and updates internal state.result to Fled  (crates/vox-gamify/src/combat.rs)

### `FreeAiClient::generate`  (happy; EXTRACTED)
- [happy] FreeAiClient initialized with only Deterministic provider can generate non-empty output  (crates/vox-gamify/src/ai/mod.rs)

### `FreeAiProvider::name`  (happy; EXTRACTED)
- [happy] FreeAiProvider::Pollinations.name() returns exactly 'Pollinations.ai (free)'  (crates/vox-gamify/src/ai/mod.rs)

### `GamifyMode`  (invariant; EXTRACTED)
- [invariant] default VoxConfig has gamify_enabled=true and mode=Balanced with reward_multiplier≈1.0  (crates/vox-gamify/src/config_gate.rs)

### `GamifyMode::reward_multiplier`  (happy; EXTRACTED)
- [happy] Learning mode reward_multiplier is >1.0 and hint_frequency is 1.0  (crates/vox-gamify/src/config_gate.rs)

### `LudusProfile::add_xp()`  (happy; EXTRACTED)
- [happy] LudusProfile::add_xp() returns false when insufficient XP for level-up but true when reaching the 100 XP threshold for level 2  (crates/vox-gamify/src/profile.rs)

### `LudusProfile::new_default()`  (happy; EXTRACTED)
- [happy] LudusProfile::new_default() initializes a profile with level=1, xp=0, total_xp_earned=0, crystals=100, energy=100, prestige_level=0  (crates/vox-gamify/src/profile.rs)

### `Mood::from_quality`  (invariant; EXTRACTED)
- [invariant] from_quality maps code_quality scores to mood tiers: 90+→Happy, 70+→Neutral, 50+→Sad, <50→Tired  (crates/vox-gamify/src/companion.rs)

### `QuestArchetype::today_for_user`  (invariant; EXTRACTED)
- [invariant] Multiple calls with the same user ID on the same day produce identical archetypes  (crates/vox-gamify/tests/gamify_integration_test.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`AchievementTracker::has_achievement`** — only: _After incrementing tasks_completed counter once, has_achievement returns true for 'first_task'_
- **`Battle::crystals_earned`** — only: _Newly created battles have zero crystals earned initially_
- **`Battle::from_finding()`** — only: _Creates a battle with BugType determined from rule_id and initial success state is false_
- **`BugEnemy`** — only: _BugEnemy hp initializes to base_hp() for its bug_type_
- **`BugEnemy.counter_attack()`** — only: _counter_attack() returns 20 for Security bug type_
- **`BugEnemy.take_damage()`** — only: _take_damage() applies type_effectiveness multiplier to damage and returns the final damage value_
- **`BugEnemy::counter_attack`** — only: _counter_attack returns 20 for Security-typed bug_
- **`BugEnemy::counter_attack()`** — only: _Security type BugEnemy counter attack deals 20 damage_
- **`BugEnemy::from_finding`** — only: _from_finding correctly maps rule_id 'ai/dead-code' to BugType::Security_
- **`BugEnemy::hp`** — only: _HP is reduced by the effective damage amount and returns the damage dealt_
- **`BugEnemy::take_damage`** — only: _take_damage applies type effectiveness multiplier (1.5x) when calculating damage (Logic vs Syntax = 15 damage from 10 input)_
- **`BugEnemy::take_damage()`** — only: _Damage calculation applies type effectiveness multiplier: Logic vs Syntax (1.5x) on 10 damage yields 15_
- **`BugType::crystal_reward()`** — only: _Syntax type yields 10 crystals and Security type yields 50 crystals as base rewards_
- **`BugType::xp_reward()`** — only: _Logic type yields 30 xp and Performance type yields 50 xp as base rewards_
- **`ChallengeManager.evaluate_attempt()`** — only: _evaluate_attempt() returns true for valid code submission_
- **`Combat`** — only: _flee() updates combat result field to CombatResult::Fled_
- **`Combat.flee()`** — only: _flee() returns CombatResult::Fled_
- **`CombatState::flee`** — only: _flee() transitions combat result to Fled and updates internal state result field_
- **`CombatState::flee()`** — only: _Returns CombatResult::Fled and updates internal state.result to Fled_
- **`Companion`** — only: _Companion constructor initializes health to 100_
- **`Companion.interact()`** — only: _interact(Interaction::Feed) increases health by 10_
- **`Companion::interact`** — only: _interact(Feed) increases health by 10 and sets mood to Happy_
- **`FreeAiClient::generate`** — only: _FreeAiClient initialized with only Deterministic provider can generate non-empty output_
- **`FreeAiProvider::name`** — only: _FreeAiProvider::Pollinations.name() returns exactly 'Pollinations.ai (free)'_
- **`GamifyMode::reward_multiplier`** — only: _Learning mode reward_multiplier is >1.0 and hint_frequency is 1.0_
- **`Level`** — only: _adding 50 XP when level 1 does not trigger levelup (returns false)_
- **`LudusProfile::add_xp()`** — only: _LudusProfile::add_xp() returns false when insufficient XP for level-up but true when reaching the 100 XP threshold for level 2_
- **`LudusProfile::new_default()`** — only: _LudusProfile::new_default() initializes a profile with level=1, xp=0, total_xp_earned=0, crystals=100, energy=100, prestige_level=0_
- **`deterministic_response`** — only: _deterministic_response for 'Generate a creative name' returns exactly 'Code Companion'_
- **`event_slug()`** — only: _event_slug() returns 'ai_thumbs_up' for thumbs-up feedback and 'ai_thumbs_down' for thumbs-down feedback_
- **`orchestrator_companion_id_migration_plan`** — only: _When both legacy and canonical companion IDs are absent, migration plan returns None_
- **`type_effectiveness`** — only: _type_effectiveness(Syntax, Performance) returns approximately 2.0 (super-effective multiplier)_
- **`type_effectiveness()`** — only: _type_effectiveness() returns ~1.5 multiplier for Syntax vs Performance and Logic vs Syntax_
- **`xp_for_feedback()`** — only: _xp_for_feedback() returns 5 XP for a thumbs-up feedback_
