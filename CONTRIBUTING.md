# Contributing to GraphLite

Thank you for your interest in contributing to GraphLite! We're excited to have you join our community. This guide will help you get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [AI-Assisted Development Philosophy](#ai-assisted-development-philosophy)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Coding Standards](#coding-standards)
- [Testing Requirements](#testing-requirements)
- [Submitting Changes](#submitting-changes)
- [Review Process](#review-process)
- [Community](#community)

---

## Code of Conduct

All contributors are expected to follow these standards:

- **Treat everyone with respect** - We're building this together
- **Be constructive in feedback** - Focus on improvements, not criticism
- **Welcome newcomers** - Help new contributors get started
- **Collaborate openly** - Share knowledge and help others learn
- **Assume good intentions** - Give others the benefit of the doubt

---

## AI-Assisted Development Philosophy

GraphLite was built using modern AI-assisted development practices, leveraging **Claude Code by Anthropic** as a pair programming partner throughout the development process.

### Our Philosophy

We believe in **transparent, hybrid development** where AI and humans work together:

- **AI accelerates** coding, testing, and documentation
- **Humans provide** architecture design, goals, direction, and validation
- **Collaboration produces** faster and better results

### AI Use in Contributions

We believe in **full transparency** about AI involvement and **welcome AI-assisted contributions**.

**If you use AI tools (Claude, Copilot, etc.) in your contributions:**

**Allowed:**
- Using AI to write boilerplate code
- AI-generated tests and documentation
- AI assistance with debugging and optimization
- Using AI to learn and understand the codebase

**Required:**
- **Human reviews all AI-generated code** - You must understand what the code does
- **Design documentation** - Provide a concise, readable design doc explaining your approach
- **Automated tests** - Add comprehensive test suites
- **Manual testing** - Include a script of manual tests run along with results
- **Take ownership** - You are responsible for code quality and correctness

**Not Allowed:**
- Copy-pasting AI code without understanding
- Submitting code with "the AI said it works" as the only verification
- PRs without tests or documentation
- Code that violates architectural patterns specified in [Critical Rules](#critical-rules).

**Transparency:**
- You may mention AI tools used in PR descriptions (optional for small changes)
- Be transparent about AI use for significant contributions
- Your contributions will be licensed under Apache-2.0
- You retain copyright on your contributions

**Remember:** AI is a tool, not a replacement for engineering judgment. We value well-designed, thoroughly tested contributions over speed.

---

## Getting Started

### Prerequisites

Before you begin, ensure you have the following installed:

- **Rust** (1.70 or higher): Install via [rustup](https://rustup.rs/)
- **Git**: For version control
- **Cargo**: Comes with Rust installation

Optional for specific contributions:
- **Python 3.8+**: For Python binding development
- **Java 11+**: For Java binding development
- **Docker**: For integration testing

### First-Time Contributors

If you're new to open source, welcome! Here are some good first steps:

1. **Read the documentation**: Start with [README.md](../README.md) and [Quick Start](../Quick%20Start.md)
2. **Look for "good first issue" labels**: These are beginner-friendly tasks
3. **Join our Discord**: Get help from the community (link in README)
4. **Start small**: Documentation fixes, test improvements, small bug fixes

## Development Setup

### 1. Fork and Clone

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/GraphLite.git
cd GraphLite

# Add upstream remote
git remote add upstream https://github.com/ORIGINAL_OWNER/GraphLite.git
```

### 2. Build the Project

```bash
# Build all components
cargo build --all

# Run the CLI
cargo run --bin graphlite-cli

# Run tests to verify setup
cargo test --all
```

### 3. Install Development Tools

```bash
# Install pre-commit hooks (if available)
./scripts/install-hooks.sh

# Install formatting tools
rustup component add rustfmt clippy

# Optional: Install cargo-watch for auto-recompilation
cargo install cargo-watch
```

### 4. Verify Your Setup

```bash
# Run the full test suite
cargo test --all

# Check code formatting
cargo fmt -- --check

# Run linter
cargo clippy -- -D warnings

# Run examples
cd examples-core/fraud_detection
cargo run
```

## How to Contribute

### Reporting Bugs

Before creating a bug report:
1. **Check existing issues**: Someone might have already reported it
2. **Try latest version**: The bug might already be fixed
3. **Minimal reproduction**: Create the smallest example that shows the bug

**Bug Report Template:**

```markdown
**Environment:**
- OS: [e.g., macOS 14.0, Ubuntu 22.04]
- Rust version: [run `rustc --version`]
- GraphLite version: [e.g., 0.1.0]

**Description:**
Clear description of the bug

**Steps to Reproduce:**
1. Step one
2. Step two
3. See error

**Expected Behavior:**
What you expected to happen

**Actual Behavior:**
What actually happened

**Code Sample:**
```rust
// Minimal code that reproduces the issue
```

**Error Messages:**
```
Paste any error messages or stack traces
```

**Additional Context:**
Any other relevant information
```

### Suggesting Features

We love feature ideas! Before suggesting:
1. **Check the roadmap**: It might already be planned ([docs/ROADMAP.md](../docs/ROADMAP.md))
2. **Check existing feature requests**: Search open issues
3. **Consider scope**: Does it fit GraphLite's vision as an embedded graph DB?

**Feature Request Template:**

```markdown
**Problem Statement:**
What problem does this solve?

**Proposed Solution:**
How would you like this feature to work?

**Alternatives Considered:**
What other approaches did you consider?

**Use Case:**
Describe your specific use case

**Implementation Ideas:**
(Optional) Any thoughts on implementation?

**Breaking Changes:**
Would this require breaking changes?
```

### Contributing Code

We accept pull requests for:
- **Bug fixes**: Always welcome!
- **Documentation improvements**: Documentation correction and more examples
- **Test improvements**: Better coverage, edge cases
- **Performance optimizations**: With benchmarks showing improvement
- **New features**: Discuss in an issue first (before coding)

## Coding Standards

### Rust Style Guide

GraphLite follows the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and standard Rust conventions:

#### Formatting
```bash
# Auto-format all code
cargo fmt --all

# Check formatting without modifying
cargo fmt --all -- --check
```

#### Linting
```bash
# Run Clippy linter
cargo clippy --all -- -D warnings

# Fix auto-fixable issues
cargo clippy --all --fix
```

#### Code Organization

- **Modules**: One file per module, clear separation of concerns
- **Visibility**: Use `pub(crate)` for internal APIs, `pub` only for public API
- **Error handling**: Use `Result<T, Error>` consistently, avoid `unwrap()` in library code
- **Documentation**: All public items must have doc comments

#### Naming Conventions

```rust
// Module names: snake_case
mod query_executor;

// Type names: PascalCase
struct QueryCoordinator;
enum StatementType;

// Functions and variables: snake_case
fn process_query() {}
let session_id = "...";

// Constants: SCREAMING_SNAKE_CASE
const MAX_QUERY_SIZE: usize = 1024;

// Lifetimes: short, descriptive
fn parse<'a>(input: &'a str) -> Result<Node<'a>>;
```

#### Documentation Comments

```rust
/// Brief one-line description.
///
/// More detailed explanation if needed. Can include:
/// - Multiple paragraphs
/// - Examples
/// - Error conditions
///
/// # Examples
///
/// ```
/// use graphlite::QueryCoordinator;
///
/// let coordinator = QueryCoordinator::from_path("./mydb")?;
/// let session = coordinator.create_simple_session("user")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns `Err` if:
/// - Database path is invalid
/// - Permissions are insufficient
///
/// # Panics
///
/// This function panics if called from within an async context.
pub fn process_query(&self, query: &str) -> Result<String> {
    // Implementation
}
```

### Critical Rules

GraphLite has the following specific architectural patterns that are expressed as rules below.
They can be enforced by using github hooks. Use [scripts/install_hooks.sh](../scripts/install_hooks.sh).

Key rules:

1. **ExecutionContext Management**: Never create new `ExecutionContext` instances
2. **Storage Manager Singleton**: Never create new `StorageManager` instances during query execution
3. **Read vs Write Locks**: Use read locks for read operations, write locks only for writes
4. **CatalogManager Singleton**: Get from session context, don't create new instances
5. **Async Runtime Management**: Don't create new Tokio runtimes repeatedly
6. **Helper Method Pattern**: Access fields directly, not recursively
7. **Async Context Detection**: Check if in async context before using `block_on()`
8. **Test Integrity**: Fix bugs in code, not in tests (unless test is wrong)
9. **Session Manager Isolation**: Use global SessionManager, don't create test-specific instances
10. **API Boundary**: Use `QueryCoordinator` as the only public API

**Before submitting a PR, verify your changes don't violate these rules.**

## Testing Requirements

### Test Coverage

All contributions must include tests:

- **Bug fixes**: Add test that reproduces the bug (should fail before fix, pass after)
- **New features**: Unit tests + integration tests
- **Refactoring**: Existing tests must pass
- **Performance improvements**: Include benchmarks

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run tests in specific module
cargo test --package graphlite --lib ast::tests

# Run integration tests only
cargo test --test '*'

# Run with logging
RUST_LOG=debug cargo test
```

### Writing Tests

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_parsing() {
        let query = "MATCH (n) RETURN n";
        let result = parse_query(query);
        assert!(result.is_ok());

        let statement = result.unwrap();
        assert_eq!(statement.statement_type, StatementType::Query);
    }

    #[test]
    fn test_invalid_query() {
        let query = "INVALID SYNTAX";
        let result = parse_query(query);
        assert!(result.is_err());
    }
}
```

#### Integration Tests

```rust
// In tests/integration_test.rs
use graphlite::QueryCoordinator;
use tempfile::TempDir;

#[test]
fn test_create_and_query_graph() {
    let temp_dir = TempDir::new().unwrap();
    let coordinator = QueryCoordinator::from_path(temp_dir.path()).unwrap();
    let session = coordinator.create_simple_session("test_user").unwrap();

    // Create schema
    coordinator.process_query("CREATE SCHEMA /test", &session).unwrap();
    coordinator.process_query("USE SCHEMA /test", &session).unwrap();

    // Create graph and add data
    coordinator.process_query("CREATE GRAPH test_graph", &session).unwrap();
    coordinator.process_query(
        "INSERT (:Person {name: 'Alice', age: 30})",
        &session
    ).unwrap();

    // Query data
    let result = coordinator.process_query(
        "MATCH (p:Person) RETURN p.name, p.age",
        &session
    ).unwrap();

    assert!(result.contains("Alice"));
    assert!(result.contains("30"));
}
```

#### Test Isolation

Use the `TestFixture` pattern for integration tests:

```rust
use graphlite::test_utils::TestFixture;

#[test]
fn test_with_isolation() {
    let fixture = TestFixture::new().unwrap();

    // Each test gets unique schema/graph names
    fixture.execute("INSERT (:Node {id: 1})").unwrap();

    let result = fixture.query("MATCH (n) RETURN n").unwrap();
    assert_eq!(result.len(), 1);

    // Cleanup happens automatically when fixture drops
}
```

## Submitting Changes

### 1. Create a Branch

```bash
# Update your fork
git fetch upstream
git checkout main
git merge upstream/main

# Create feature branch
git checkout -b feature/your-feature-name
# or for bug fixes
git checkout -b fix/issue-123
```

### 2. Make Your Changes

```bash
# Make changes, commit often
git add .
git commit -m "Add feature: brief description

More detailed explanation of what changed and why.

Fixes #123"
```

### Commit Message Guidelines

Follow conventional commits format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `perf`: Performance improvements
- `chore`: Build process, tooling changes

**Examples:**

```
feat(parser): add support for OPTIONAL MATCH clause

Implements optional pattern matching according to ISO GQL spec.
Includes parser changes, executor logic, and comprehensive tests.

Closes #45
```

```
fix(executor): prevent stack overflow in recursive WITH clauses

Added depth tracking to prevent infinite recursion when WITH clauses
reference themselves. Added regression test.

Fixes #123
```

```
docs(readme): update installation instructions for Python bindings

Clarified Python version requirements and added troubleshooting section
for common installation issues.
```

### 3. Test Your Changes

```bash
# Run full test suite
cargo test --all

# Check formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --all -- -D warnings

# Run code pattern checks (REQUIRED)
./scripts/check_code_patterns.sh

# Test examples still work
cd examples-core/fraud_detection
cargo run
```

### 4. Push and Create Pull Request

```bash
# Push to your fork
git push origin feature/your-feature-name

# Go to GitHub and create Pull Request
```

### Pull Request Template

```markdown
## Description

Brief description of what this PR does

## Motivation

Why is this change needed? What problem does it solve?

## Changes

- Change 1
- Change 2
- Change 3

## Testing

How was this tested?
- [ ] Unit tests added
- [ ] Integration tests added
- [ ] Manual testing performed
- [ ] Examples verified

**Testing Steps for Reviewers:**

Provide clear, step-by-step instructions for reviewers to test your changes:

1. Step-by-step instructions on how to build/run
2. Expected behavior at each step
3. How to verify the fix/feature works
4. Any specific test commands to run

## Checklist

- [ ] Code follows project style guidelines
- [ ] Tests pass locally (`cargo test --all`)
- [ ] Code pattern checks pass (`./scripts/check_code_patterns.sh`)
- [ ] Documentation updated (if applicable)
- [ ] Documentation is concise and curated (no AI-generated fluff)
- [ ] CHANGELOG.md updated (for notable changes)
- [ ] No CLAUDE.md rules violated
- [ ] Commit messages follow guidelines
- [ ] PR description is professional (no emojis, no copy-paste from AI tools)
- [ ] Clear testing steps provided for reviewers to verify the changes

## Related Issues

Closes #123
Related to #456
```

## Review Process

### What to Expect

1. **Initial Response**: Within 72 hours (usually faster)
2. **Review Feedback**: Maintainers may request changes
3. **CI Checks**: All tests must pass
4. **Approval**: At least one maintainer approval required
5. **Merge**: Squash and merge into main branch

### Review Criteria

Reviewers will check:
- **Correctness**: Does it work as intended?
- **Tests**: Adequate test coverage?
- **Style**: Follows coding standards?
- **Documentation**: Clear docs and comments?
- **Architecture**: Fits with existing design?
- **Breaking Changes**: Necessary and documented?

### Responding to Feedback

```markdown
Thanks for the review! I've made the following changes:

1. Fixed the lifetime issue in parse_query()
2. Added integration test for edge case
3. Regarding the async approach - could you clarify what you mean?

Latest commit: abc123
```

## Recognition

We appreciate all contributors! Your contributions will be recognized:

- **Contributors list**: Added to README.md
- **Release notes**: Mentioned in CHANGELOG.md
- **GitHub**: Automatic contributor badge
- **Social media**: Shoutouts for significant contributions

## Community

### Communication Channels

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: Questions, ideas, general discussion
- **Discord**: Real-time chat (link in README)
- **Twitter**: @graphlite_db (announcements)

### Getting Help

If you're stuck:
1. Check [documentation](../docs/)
2. Search existing issues
3. Ask in Discord
4. Create a GitHub Discussion

### Maintainers

Current maintainers:
- @yourusername (Lead maintainer)

## Additional Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [ISO GQL Specification](https://www.iso.org/standard/76120.html)
- [GraphLite Architecture](../docs/Architecture.md)
- [Testing Guide](../docs/Testing%20Guide.md)

## Contribution Ideas

Not sure where to start? Here are some ideas:

### Documentation
- Fix typos or improve clarity
- Add more examples
- Write tutorials or blog posts
- Translate documentation

### Testing
- Improve test coverage
- Add edge case tests
- Performance benchmarks
- Stress testing

### Features (Discuss First!)
- ISO GQL compliance improvements
- Query optimization
- New aggregation functions
- Additional language bindings

### Performance
- Query optimizer improvements
- Memory usage optimization
- Storage engine tuning
- Benchmark suite

### Tooling
- IDE integrations
- CLI improvements
- Better error messages
- Developer tools

---

## Thank You!

Your contributions make GraphLite better for everyone. We're grateful for your time and effort. Welcome to the community!

**Questions?** Feel free to ask in GitHub Discussions or Discord. We're here to help!

---

*Last Updated: November 2025*
