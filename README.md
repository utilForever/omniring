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

The output shows the seed, HP, actions (zero-based slots), step rewards, and the winner with the total reward. The demo uses seed `46` to reproduce its damage rolls. Forced replacements are separate steps and do not consume an attack turn. The demo fails if it cannot finish within 100 steps.

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

For direct battles, bind rosters once with `Battle::with_rosters(state, player, opponent)`, then call `play_turn_with_rosters(player_action, opponent_action)`. The battle owns immutable rosters; cloning a battle shares that data while copying its runtime state and random stream. Callers migrating from the previous API should move the roster arguments from each turn call to the constructor.

`Battle::new(state)` and `Battle::with_seed(state, seed)` remain available for custom `play_turn` resolvers. Calling `play_turn_with_rosters` without bound rosters returns `ActionError::MissingRosters` without changing the state or random stream. `Environment::from_rosters` continues to retain its rosters in the transition closure and requires no API changes.

### Reproducible battles

Use `Battle::with_rosters_and_seed(state, player, opponent, seed)` for a direct battle or `Environment::from_rosters_with_seed(player, opponent, selection, seed)` for an environment. Speed ties and damage rolls draw from one battle-owned RNG. The existing constructors choose a random seed once at construction.

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

### Replay a completed battle

`BattleReplay` stores the initial player and opponent rosters (including HP), opponent selection, seed, and ordered `(player_action, opponent_action)` pairs. Record each successful `Environment::step` call, starting with the player's `Action::SelectTeam` and including voluntary switches and forced replacements. The opponent action during team preview is ignored, as in a live episode.

```rust
use omniring::BattleReplay;

let replay = BattleReplay {
    player,
    opponent,
    opponent_selection: [0, 1, 2],
    seed: 46,
    actions, // Recorded pairs from team selection through the terminal step.
};
let outcomes = replay.run()?;
assert!(outcomes.last().unwrap().terminated);
assert_eq!(replay.run()?, outcomes);
```

`run()` creates a fresh seeded environment and uses the normal `step` path. It returns every `StepOutcome` in order, preserving observations, rewards, and termination flags; the last observation is the terminal state visible to the player. These step outcomes are the replay's event trace. The input stores no intermediate states, and there is no separate event engine or stable on-disk format. The same library-version and target-platform limits as seeded battles apply.

Errors distinguish invalid setup (`ReplayError::InvalidSetup`), a failed action with its zero-based step index and original `ActionError` (`ReplayError::InvalidAction`), and an empty or unfinished sequence (`ReplayError::Incomplete`). Extra actions after termination fail with `ActionError::BattleTerminated`.

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
