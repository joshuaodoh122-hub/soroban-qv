# Contributing

Thank you for considering a contribution. This is a small but carefully designed
boilerplate — contributions that improve correctness, security, documentation,
or test coverage are especially welcome.

---

## Before you start

- Read [ARCHITECTURE.md](ARCHITECTURE.md) to understand the design rationale.
- Read [SECURITY.md](SECURITY.md) before touching any auth or economic logic.
- Check existing issues/PRs to avoid duplicating effort.

---

## Development setup

```bash
# Install Rust stable
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add the WASM target
rustup target add wasm32v1-none

# Clone and build
git clone https://github.com/joshuaodoh122-hub/soroban-qv-core
cd soroban-qv-core
cargo build --workspace --all-features
```

---

## Running checks locally

All of these must pass before opening a PR:

```bash
# Format
cargo fmt --all

# Lint (must be warning-free)
cargo clippy --workspace --all-features -- -D warnings
cargo clippy --workspace --target wasm32v1-none -- -D warnings

# Tests
cargo test --workspace --all-features

# WASM build
cargo build --workspace --target wasm32v1-none --release
```

CI runs the same checks on every PR. **Do not open a PR with red CI.**

---

## What makes a good PR

- **Tests first.** Every change to contract logic must include or update tests.
  For any new error path, add a test that triggers it. For any new happy path,
  add a test that exercises it.
- **Document design decisions.** If you make a choice that isn't obvious,
  explain it in a code comment or update ARCHITECTURE.md.
- **Small, focused changes.** One logical change per PR. Refactoring and feature
  work should be separate.
- **Update CHANGELOG.md.** Add a dated entry under `[Unreleased]` for every
  meaningful change.

---

## What we won't merge

- Fabricated test results or benchmarks.
- Changes that silently drop overflow checks or error handling.
- A dependency that isn't pinned to an exact version.
- Anything that introduces a global admin or hardcoded address.

---

## Reporting security issues

See [SECURITY.md](SECURITY.md).

---

## License

By contributing, you agree that your contributions will be licensed under the
MIT License.
