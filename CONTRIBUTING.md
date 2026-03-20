# Contributing to Darya

Thanks for your interest in contributing to Darya! This document covers everything you need to get started.

## Development Environment

### Requirements

- **Rust** — install via [rustup](https://rustup.rs/) (stable toolchain)
- **macOS with iTerm2** — Darya targets iTerm2 on macOS exclusively
- **Git** — with worktree support (any recent version)

### Setup

```sh
git clone https://github.com/steventohme/darya.git
cd darya
cargo build
```

## Running Tests

Darya uses a 4-layer test strategy: state machine tests, component tests, widget snapshots (via [insta](https://insta.rs/)), and PTY callback tests.

```sh
# Run the full test suite
cargo test

# If you've changed UI snapshots, review and accept them
cargo insta review
```

All tests must pass before submitting a PR.

## Code Style

- **Formatting:** Run `cargo fmt` before committing. CI enforces `cargo fmt --check`.
- **Lints:** Run `cargo clippy -- -D warnings` and fix any warnings. CI enforces this as well.
- **Config:** The project uses `rustfmt.toml` for consistent formatting across contributors.

## Submitting Changes

### Pull Requests

1. Fork the repo and create a feature branch from `main`.
2. Make your changes, keeping commits focused and well-described.
3. Add or update tests for any new or changed behavior.
4. Ensure `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` all pass.
5. Open a PR against `main` with a clear description of what changed and why.

### What Makes a Good PR

- **Small and focused** — one logical change per PR.
- **Tested** — new features include tests; bug fixes include a regression test where feasible.
- **Documented** — update the README if you're adding user-facing features or changing shortcuts.

## Reporting Issues

When filing a bug report, please include:

- Your macOS version and terminal emulator
- Rust toolchain version (`rustc --version`)
- Steps to reproduce the issue
- Expected vs. actual behavior
- Any relevant logs or screenshots

For feature requests, describe the use case and how you envision it working.

## Questions?

Open an issue — we're happy to help you get started.
