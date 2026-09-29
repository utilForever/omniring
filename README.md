<p align="center">
  <picture>
    <img src="https://raw.githubusercontent.com/utilForever/omniring/refs/heads/main/assets/logo.png" width="400"/>
  </picture>
</p>
<p align="center">
  <b>A Rust library for building a reinforcement learning environment for Pokemon Champions</b>
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT" /></a>
  <a href="https://github.com/utilForever/omniring/actions/workflows/rust.yml"><img src="https://github.com/utilForever/omniring/actions/workflows/rust.yml/badge.svg?branch=main" alt="Rust" /></a>
  <a href="https://github.com/utilForever/omniring/actions/workflows/typos.yml"><img src="https://github.com/utilForever/omniring/actions/workflows/typos.yml/badge.svg?branch=main" alt="Typos" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=alert_status" alt="Quality Gate Status" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=ncloc" alt="Lines of Code" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=coverage" alt="Coverage" /></a>
  <br />
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=sqale_rating" alt="Maintainability Rating" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=reliability_rating" alt="Reliability Rating" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=security_rating" alt="Security Rating" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=bugs" alt="Bugs" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=vulnerabilities" alt="Vulnerabilities" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_omniring"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_omniring&metric=sqale_index" alt="Technical Debt" /></a>
</p>

## What This Library Does

`omniring` provides the foundation for:

- Modeling Pokemon Champions as an environment suitable for reinforcement learning agents.
- Keeping simulation and game-state logic in a reusable Rust library crate.
- Supporting future training, evaluation, and integration workflows around deterministic environment behavior.

## Quick Start

### Prerequisites

- Rust stable toolchain with edition 2024 support
- Git

### 1. Clone

```bash
git clone https://github.com/utilForever/omniring.git
cd omniring
```

### 2. Check the Library

```bash
cargo check --all
cargo test --all
```

### 3. Run a Complete Battle

```bash
cargo run --example battle_demo
```

The scripted demo creates a level-50 Charizard and Venusaur with four moves each, then copies each Pokemon into a six-Pokemon roster. Both Trainers select slots `[0, 1, 2]` and field one Pokemon at a time. Each side uses its first move and automatically replaces fainted Pokemon until one selected team is defeated.

The output shows the seed, HP, actions (zero-based slots), step rewards, and the winner with the total reward. The demo uses seed `46` to reproduce its damage rolls. Forced replacements are separate steps and do not consume an attack turn. If the environment's turn limit is reached, the demo reports truncation without a winner.

Its end-to-end self-check runs with `cargo test --all`, or on its own:

```bash
cargo test --example battle_demo
```

### 4. Run the Environment Self-Checks

```bash
cargo test --test environment_battle
```

These focused checks use the real battle logic to cover reset, team selection, damage across steps, fainting, forced replacements, and terminal win/loss rewards. They also verify that unselected Pokemon keep their HP and that a completed environment can start a fresh episode. They run automatically with `cargo test --all`.

### Battle runtime state

`omniring::Battle` owns the runtime `BattleState` and its random stream. Both direct battles and `Environment::from_rosters` use the same resolver. Supplied Pokemon rosters provide calculation data; live HP, selection, active slots, and move availability come from `BattleState`. Failed turns leave the stored state and random stream unchanged, and observations are independent snapshots. Cloning a battle preserves its current random stream.

### Views for either Trainer

Use the existing `BattleSide` selector with `Environment::observation(side)` and `Environment::legal_actions(side)`. In each observation, `player` is the requested Trainer and `opponent` is the other Trainer. The Trainer sees its own full selection; only opposing slots that have entered battle are revealed as selected. A switch-in stays revealed even if it immediately faints, and reset clears both sides' reveal history.

```rust
use omniring::{Action, BattleSide, Environment};

let mut env = Environment::from_rosters_with_seed(player, opponent, [0, 1, 2], 46)?;
env.step(Action::SelectTeam([0, 1, 2]), Action::SelectTeam([5, 4, 3]))?;

let player_view = env.observation(BattleSide::Player)?;
let opponent_view = env.observation(BattleSide::Opponent)?;
let player_action = env.legal_actions(BattleSide::Player)[0];
let opponent_action = env.legal_actions(BattleSide::Opponent)[0];
let outcome = env.step(player_action, opponent_action)?;
let player_reward = outcome.reward_for(BattleSide::Player);
let opponent_reward = outcome.reward_for(BattleSide::Opponent);
assert_eq!(opponent_reward, -player_reward);
```

Queries borrow the same canonical battle state and return independent observation snapshots without advancing the RNG. Battle actions remain ordered by move slot, then roster slot. A fainted side gets only valid replacement switches; terminated or truncated episodes return no legal actions. Direct state users can also call `BattleState::legal_actions(side)` and `validate_action(side, action)`; the existing player-only methods remain available.

During preview, observations swap the two public rosters and legal actions list ordered selections for the requested roster, excluding fainted leads (120 choices when all six Pokemon are alive). Submit both Trainers' `SelectTeam` actions through `step`; the opponent's explicit selection overrides the constructor's default for that episode. Invalid selections leave the environment in preview. For compatibility, an opponent `Move` or `Switch` input during preview still uses the constructor's default. Both reset methods retain that default, so policies can choose again each episode. `step` arguments and event side labels always use the original player/opponent identities; `StepOutcome.observation` and `.reward` retain the original player's perspective. Query the environment after each step for the other observation, and use `reward_for` for either reward. Events remain a replay/debug trace that can include unexecuted opposing choices, so they are not a policy observation.

### Bounded episodes

Each environment has a fixed safety limit of `omniring::MAX_EPISODE_TURNS` (1,000 turns). Successful attack turns and voluntary switches count, including turns where both sides only switch. Team selection, forced replacement steps, and failed steps do not count. Both `reset()` and `reset_with_seed(seed)` restore the full turn budget.

Stop a rollout when `outcome.terminated || outcome.truncated`. On turn 1,000, a real terminal battle result takes precedence: `terminated` is true and `truncated` is false. Otherwise, `truncated` is true and `terminated` stays false, including in the battle observation. Truncation adds no win/loss reward or `BattleCompleted` event; the last turn's ordinary HP/faint rewards and events are preserved. Further steps return `ActionError::EpisodeTruncated` until reset. Actual terminal battles still return `ActionError::BattleTerminated`.

This is an environment safety limit, not a draw, forfeit, or tournament timer. Direct `Battle` use is unchanged. Callers constructing `StepOutcome` must supply the new `truncated` field, and exhaustive `ActionError` matches must handle the new variant.

Run the switch-only truncation and reset check:

```bash
cargo test --test environment_battle switch_only_episode_stops_at_the_turn_limit
```

### Reproducible battles

Use `Battle::with_seed(state, seed)` for a direct battle or `Environment::from_rosters_with_seed(player, opponent, selection, seed)` for an environment. Speed ties and damage rolls draw from one battle-owned RNG. The existing constructors choose a random seed once at construction.

```rust
let mut env = Environment::from_rosters_with_seed(player, opponent, [0, 1, 2], 46)?;
env.step(Action::SelectTeam([0, 1, 2]), Action::Move(0))?;
let first = env.step(Action::Move(0), Action::Move(0))?;

env.reset_with_seed(46);
env.step(Action::SelectTeam([0, 1, 2]), Action::Move(0))?;
assert_eq!(env.step(Action::Move(0), Action::Move(0))?, first);
```

`reset()` restores the initial HP and team preview while continuing the random stream. `reset_with_seed(seed)` also restarts that stream. Reproduction requires the same initial state, rosters, seed, action sequence, library versions, and target platform; `rand::rngs::SmallRng` does not promise identical results across versions or platforms.

Custom transitions passed to `Battle::play_turn` or `Environment::new` now receive a fourth argument, `&mut SmallRng`: use `|state, action, opponent_action, rng|`, or `_` for the last argument if no randomness is needed. `Environment::new_with_seed(preview, selection, seed, transition)` seeds a custom transition. Use the supplied RNG for random decisions. State captured by a callback is not reset or rolled back by the environment.

### Structured battle events

Each `StepOutcome.events` contains that step's `Vec<BattleEvent>` in resolution order. Direct battles expose the last successful turn's events through `Battle::events()`. Collect the returned steps or copy those slices to retain an episode history; the battle only retains its latest successful turn.

```rust
use omniring::BattleEvent;

let outcome = env.step(Action::Move(0), Action::Move(0))?;

for event in &outcome.events {
    if let BattleEvent::Damage { side, slot, damage, hp_before, hp_after } = event {
        assert_eq!(*damage, hp_before - hp_after);
    }
}
```

Events identify the `BattleSide` and zero-based roster/move slots, so identical Pokemon names are unambiguous. Each attack turn first records accepted `MoveSelected` choices (player then opponent), including a Pokemon that will faint before acting. These choices are for replay/debugging and include the opponent's unexecuted choice. Switches follow (player then opponent when both switch), then attack outcomes in priority/speed order. Damage is actual HP lost, capped at remaining HP; `Fainted` follows lethal damage. A failed accuracy check emits `Miss` without damage. Protect/Detect emit `MoveBlocked` for the blocked attack. Status and immune moves produce no damage event. `BattleCompleted` comes last with the winning side, or `None` for a draw.

Team selection returns no events. Forced replacement steps contain only switches; their ignored move inputs are not recorded as choices. Failed turns preserve the previous state, RNG, and events; reset starts with no history. Event collection makes no random draws and does not change combat behavior.

The core resolver now checks `Move.accuracy` using the battle-owned RNG before damage calculation. `None` and values at least 100 always hit the accuracy check; 0 always misses. Values from 1 to 99 consume one percentage roll. Guaranteed hits/misses and blocked moves consume no accuracy roll, and a miss consumes no damage roll. This adds combat behavior for moves below 100% accuracy, so seeded episodes using them can differ from earlier versions. Accuracy/evasion stages and status effects remain outside this implementation. `calculate_damage` still calculates damage only; accuracy belongs to turn resolution.

**Custom transition API change:** callbacks now return `Result<Vec<BattleEvent>, ActionError>` instead of `Result<(), ActionError>`. Replace `Ok(())` with `Ok(Vec::new())` when no attack outcomes are needed, or return them in their actual order. The battle adds move selections, action-driven switches, and completion; callbacks must report their own damage, faint, and miss events. Callback events are trusted and are not inferred or validated against HP changes.

### Replay a completed episode

`BattleReplay` stores the initial player and opponent rosters (including HP), default opponent selection, seed, and ordered `(player_action, opponent_action)` pairs. Record each successful `Environment::step` call, starting with the player's `Action::SelectTeam` and including voluntary switches and forced replacements. An explicit opponent `SelectTeam` during preview overrides the default; a `Move` or `Switch` input keeps it, as in a live episode. Previously ignored opponent `SelectTeam` inputs now take effect and must be valid.

```rust
use omniring::BattleReplay;

let replay = BattleReplay {
    player,
    opponent,
    opponent_selection: [0, 1, 2],
    seed: 46,
    actions, // Recorded pairs from team selection through termination or truncation.
};
let outcomes = replay.run()?;
let last = outcomes.last().unwrap();
assert!(last.terminated || last.truncated);
assert_eq!(replay.run()?, outcomes);
```

`run()` creates a fresh seeded environment and uses the normal `step` path. It returns every `StepOutcome` in order, preserving observations, rewards, structured events, and termination/truncation flags; the last observation is the state visible to the player when the episode ends. These step outcomes are the replay's event trace. The input stores no intermediate states, and there is no separate event engine or stable on-disk format. The same library-version and target-platform limits as seeded battles apply.

Errors distinguish invalid setup (`ReplayError::InvalidSetup`), a failed action with its zero-based step index and original `ActionError` (`ReplayError::InvalidAction`), and an empty or unfinished sequence (`ReplayError::Incomplete`). Extra actions after termination fail with `ActionError::BattleTerminated`; extra actions after truncation fail with `ActionError::EpisodeTruncated`.

Run the check that records a seeded battle, replays it twice, and checks invalid inputs:

```bash
cargo test --test environment_battle recorded_battle_replays_seeded_steps_and_rejects_invalid_sequences
```

## Development

Run the same core checks used in CI for code changes:

```bash
cargo check --all
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Optional local parity with CI:

```bash
cargo install cargo-udeps
cargo +nightly udeps --all-targets

cargo install typos-cli
typos
```

## License

<img align="right" src="https://149753425.v2.pressablecdn.com/wp-content/uploads/2009/06/OSIApproved_100X125.png" alt="Open Source Initiative approved license logo">

This project is licensed under the [MIT License](LICENSE):

Copyright &copy; 2026 [Chris Ohk](https://github.com/utilForever) and [Hyeok Kwon](https://github.com/namicad).

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
