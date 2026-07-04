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
