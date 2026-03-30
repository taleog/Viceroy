# Contributing to Viceroy

Thanks for helping improve Viceroy.

This guide is for contributors working on:
- the macOS client
- the Windows client
- the self-hosted sync server
- documentation and release tooling

## Project Status

Viceroy is still in early alpha.

That means:
- features are still moving quickly
- some architecture is still settling
- release builds are usable but not fully polished
- contributions are welcome, especially fixes, tests, docs, and platform polish

## Before You Start

Please check:
- the open issues
- [`docs/issues.md`](./docs/issues.md)
- [`docs/roadmap.md`](./docs/roadmap.md)

If you are changing sync behavior, also read:
- [`docs/sync-server.md`](./docs/sync-server.md)
- [`docs/sync-model.md`](./docs/sync-model.md)

## Development Setup

### Prerequisites

- Rust toolchain
- Git
- macOS for native launcher work
- Windows for Windows client work

### First-time setup

```bash
git clone https://github.com/taleog/Viceroy.git
cd Viceroy
make setup
make check
```

### Useful commands

```bash
make help
make run
make fmt
make lint
make test
make check
make app
make install-app
cargo run --bin viceroy-sync-server
```

## Workflow

### Branch names

Use short descriptive branches:
- `feature/...`
- `fix/...`
- `docs/...`
- `refactor/...`
- `test/...`

### Commit messages

Use Conventional Commits:

```text
type(scope): summary
```

Examples:

```text
feat(sync): add websocket reconnect backoff
fix(clipboard): stabilize macOS paste handoff
docs(readme): simplify installation guide
```

### Pull requests

Before opening a PR:
1. Run `make check`
2. Update docs for any user-visible change
3. Update `CHANGELOG.md` under `Unreleased` when appropriate
4. Re-read your diff for platform-specific regressions

Good PRs usually include:
- the user-visible change
- any platform caveats
- testing performed
- screenshots if the UI changed

## Code Expectations

- Prefer clear, boring code over clever code
- Keep shared logic platform-neutral where possible
- Keep platform-specific behavior isolated when necessary
- Add comments for tricky behavior, not obvious behavior
- Avoid breaking sync invariants without updating the docs

## Platform Notes

### macOS

- AppKit work must stay on the main thread
- Objective-C interop should stay tightly scoped in `unsafe`
- Accessibility and frontmost-app behavior are easy to regress, so test them directly
- Release packaging changes should be validated through the generated `.app` or `.dmg`, not only `cargo run`

### Windows

- Keep parity with shared backend behavior when possible
- Test installer behavior when changing release packaging
- When changing settings or sync UX, verify the Windows app still matches the shared config model

### Sync

If you change sync behavior:
- preserve idempotent retries
- preserve source-device echo prevention
- preserve resumable catch-up behavior
- update [`docs/sync-model.md`](./docs/sync-model.md)
- update [`docs/sync-server.md`](./docs/sync-server.md) if setup or deployment changes

## Testing

Minimum expectations:
- unit or integration tests for new logic where practical
- manual verification for UI/platform changes
- `make lint`
- `make test`

Useful commands:

```bash
cargo test test_name
cargo test -- --nocapture
cargo clippy --all-targets --all-features -- -D warnings
```

## Documentation Expectations

Please update docs in the same PR when you change:
- installation steps
- settings shape
- sync behavior
- release packaging
- user-facing UI or workflows

Main docs to keep aligned:
- [`README.md`](./README.md)
- [`docs/installing.md`](./docs/installing.md)
- [`docs/troubleshooting.md`](./docs/troubleshooting.md)
- [`docs/sync-server.md`](./docs/sync-server.md)
- [`docs/sync-model.md`](./docs/sync-model.md)
- [`docs/roadmap.md`](./docs/roadmap.md)

## Release Notes

Viceroy currently uses alpha releases:
- `0.x.y-alpha.z`

Typical release flow:
1. update `Cargo.toml`
2. move notes from `Unreleased` into a dated section in `CHANGELOG.md`
3. push `main`
4. create or reuse the release tag
5. let GitHub Actions publish the assets

## Getting Help

If you are unsure where to make a change:
- open a draft PR
- open an issue
- leave notes about the tradeoffs you are seeing

Clear partial progress is better than silent abandoned work.

## License

By contributing, you agree that your contributions are licensed under the MIT License.
