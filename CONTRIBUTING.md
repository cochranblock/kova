# Contributing to Kova

Thank you for your interest in contributing to Kova. This guide outlines the process and expectations for contributing code, documentation, and feedback.

## Getting Started

### Prerequisites
- Rust 1.70+
- Tokio async runtime knowledge (helpful)
- Git

### Local Setup
```bash
git clone https://github.com/cochranblock/kova.git
cd kova
cargo build
cargo test
```

### Using Project Aliases
This project uses tokenized command aliases. See `.kova-aliases` for the full map. Examples:
```bash
kx0     # cargo build
kx1     # cargo check
kx3     # cargo clippy
kt      # cargo test
kc      # kova chat (REPL)
kg      # kova gui
```

## Development Workflow

### 1. Create a Branch
```bash
git checkout -b feature/your-feature-name
```

Use descriptive, kebab-case branch names:
- `feature/add-webhook-support`
- `fix/memory-leak-in-parser`
- `docs/update-contributing-guide`

### 2. Make Changes
- Write clear, idiomatic Rust code.
- Follow the tokenization map (see `docs/compression_map.md`).
- Add tests for new functionality.
- Update documentation as needed.

### 3. Commit
Use atomic, well-described commits:
```bash
git commit -m "feat: Add webhook support for event handlers

- Implement WebhookHandler trait
- Add webhook registration to config
- Include unit tests

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

**Commit Message Format:**
- Start with a type: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`
- Keep the first line under 50 characters
- Include a body explaining why, not what (the diff shows what)
- Always include the Copilot co-author trailer

### 4. Test
```bash
cargo test              # Unit and integration tests
cargo clippy           # Linting
cargo fmt --check      # Format check
```

All tests must pass before opening a PR.

### 5. Push and Open a PR
```bash
git push origin feature/your-feature-name
```

In the PR description:
- Link related issues: `Fixes #123`
- Describe the motivation and approach
- Note any breaking changes
- Add screenshots or demos for features

### 6. Code Review
- Address feedback promptly
- Push updates to the same branch (no new PR needed)
- Ping reviewers after updates

## Code Style

### Rust
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common errors
- No unnecessary `#[allow(...)]` suppressions
- Comment only code that needs clarification (not obvious code)
- Prefer explicit variable names over cryptic abbreviations

### Tokenization
Kova uses compressed identifiers (e.g., `f0`, `t5`, `s12`) to reduce token consumption when feeding code to LLMs. See [docs/compression_map.md](docs/compression_map.md) for the canonical mapping. All tokens must be documented:
- `f0-f160+` = functions
- `t0-t108+` = types
- `s0-s*` = struct fields
- `c1-c9, ci` = node commands
- `x0-x9` = cargo commands
- `p0-p9` = project aliases
- `n0-n3` = worker nodes

When adding new identifiers, update the map immediately.

### Documentation
- Keep READMEs up to date
- Add docstrings to public APIs
- Use clear language, avoid jargon where possible
- Link to `docs/ARCHITECTURE.md` for complex features

## Testing

### Unit Tests
```rust
#[test]
fn test_agent_loop_processes_tool_calls() {
    let result = process_tool_call("read", "src/main.rs");
    assert!(result.is_ok());
}
```

### Integration Tests
Place in `tests/` directory and test end-to-end workflows (REPL, agent loop, node commands).

### Test Coverage
Aim for >70% coverage on new code.

### Quality Gate
```bash
cargo run -p kova --bin kova-test --features tests
```

This runs the full CI pipeline (compile → unit tests → integration tests → HTTP tests).

## Reporting Issues

### Security Issues
Please **do not** open a public issue. Email security@cochranblock.org with details.

### Bug Reports
Use the issue template and include:
- Environment (macOS/Linux, Rust version, etc.)
- Steps to reproduce
- Expected vs. actual behavior
- Logs or error messages

### Feature Requests
Describe the use case and desired behavior. Include examples if possible.

## Kova-Specific Guidelines

### Swarm Commands (c1-c9, ci)
If modifying node commands, test on actual cluster:
```bash
kova c2 ncmd nstat         # Check node status
kova c2 ncmd nbuild        # Broadcast build
```

### Tokenized Cargo Wrapper (x0-x9)
If updating cargo command handling, ensure all tokens remain consistent and documented in the map.

### Local LLM Inference
If modifying inference logic, test with:
```bash
kova model install
kova chat "Test message"
```

### GUI Changes
If updating egui GUI, test with:
```bash
kg
```

### HTTP API Changes
If modifying Axum endpoints, test with:
```bash
ks
curl http://localhost:8080/api/health
```

## Deployment & Release

- Only maintainers can deploy to production (see `OWNERS.yaml`)
- Follow semver versioning (MAJOR.MINOR.PATCH)
- Tag releases: `git tag -a v1.2.3 -m "Release v1.2.3"`
- Push tags: `git push origin v1.2.3`
- Deploy via: `kova c2 ncmd ndeploy --target n1`

## Code of Conduct

We are committed to providing a welcoming and inclusive environment. Be respectful, constructive, and professional in all interactions.

---

**Questions?** Open a discussion in [GitHub Discussions](https://github.com/cochranblock/kova/discussions) or post in #team-kova Slack. Thanks for contributing!
