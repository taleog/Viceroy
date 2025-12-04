# Contributing to Viceroy

Thank you for your interest in contributing to Viceroy! This document provides guidelines and workflows for contributing.

## Development Status

Viceroy is currently in **private alpha** (version 0.1.0-alpha.x). This means:
- The app is not publicly released
- APIs and features may change without notice
- We welcome contributions but expect rough edges

## Getting Started

### Prerequisites

- macOS (Intel or Apple Silicon)
- Rust toolchain (2021 edition)
- Git

### Setup

```bash
# Clone the repository
git clone https://github.com/taleog/Viceroy.git
cd Viceroy

# Build the project
cargo build

# Run tests
cargo test

# Run the app
cargo run
```

## Development Workflow

### Branch Naming

Use descriptive branch names:
- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation updates
- `refactor/description` - Code refactoring

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:
```
feat(search): add contact search support
fix(clipboard): resolve duplicate entry detection
docs(readme): update installation instructions
```

### Pull Request Process

1. Create a feature branch from `main`
2. Make your changes
3. Update documentation if needed
4. Update CHANGELOG.md under `[Unreleased]`
5. Run linting and tests:
   ```bash
   make fmt
   make lint
   make test
   ```
6. Push and create a pull request
7. Fill out the PR template completely

### Code Review

All PRs require review before merging. Reviewers will check:
- Code quality and style
- Test coverage
- Documentation updates
- Changelog entries

## Code Style

- Follow Rust idioms and best practices
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting (warnings are errors)
- Keep functions focused and reasonably sized
- Add comments for complex logic

### macOS-Specific Guidelines

- Never call AppKit methods from background threads
- Use `dispatch::Queue::main()` to marshal results to the main thread
- Use `msg_send!` for Objective-C interop, always wrapped in `unsafe`
- Test on both Intel and Apple Silicon when possible

## Testing

### Running Tests

```bash
# Run all tests
make test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

### Test Guidelines

- Add tests for new functionality
- Tests should be deterministic
- Use `tempfile` for tests that need filesystem access
- Integration tests go in `tests/` directory

## Documentation

- Update README.md for user-facing changes
- Update docs/roadmap.md for feature planning
- Update docs/issues.md for known issues
- Add inline documentation for public APIs

## Issue Reporting

### Bug Reports

Use the bug report issue template. Include:
- Clear description of the issue
- Steps to reproduce
- Expected vs actual behavior
- System information (macOS version, architecture)
- Logs if applicable

### Feature Requests

Use the feature request issue template. Include:
- Clear description of the feature
- Use case and motivation
- Proposed implementation (optional)

## Release Process

Releases follow semantic versioning:
- **Alpha** (0.x.y-alpha.z): Early development, breaking changes expected
- **Beta** (0.x.y-beta.z): Feature complete, bug fixes only
- **Release** (x.y.z): Stable releases

### Version Bumping

1. Update `version` in `Cargo.toml`
2. Update `CHANGELOG.md`:
   - Move items from `[Unreleased]` to new version section
   - Add release date
3. Create a git tag: `git tag v0.1.0-alpha.2`

## Getting Help

- Check existing issues and documentation
- Open a new issue with questions
- Be patient and respectful

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
