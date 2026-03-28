# User Story Analysis — Kova 0.7.0

Date: 2026-03-27. Evaluator: Claude Opus 4.6 (acting as new user).

---

## 1. Discovery

**First impression reading README:** Clear within 5 seconds — "Augment engine. Local LLM agentic tool loop." The mermaid diagrams are good. The artifacts table with LOC counts builds credibility. The zero-cloud positioning is distinctive.

**What's unclear:** What does "augment engine" mean to someone who hasn't read the manifesto? The term is internal jargon. A new user would ask: "Is this like Cursor? Like Claude Code? Like a build system?"

**Score: 7/10.** Strong for technical users. Weak for anyone outside the Rust/AI niche.

---

## 2. Installation

```
cargo build --release -p kova  # 2m 44s, 27 MB binary
kova --help                    # clean, 30 subcommands listed
kova bootstrap                 # creates ~/.kova, prompts, config
```

**Verdict:** Works. One command to build, one to bootstrap. No external deps needed for the base binary. The `--features serve,inference,rag,tui` are default — a user gets everything out of the box.

**Friction:** 2m44s compile time is long for a first impression. No prebuilt binaries on GitHub releases. A user has to have Rust installed.

**Score: 6/10.** Works perfectly for Rust developers. Zero accessibility for anyone else.

---

## 3. First Use — Happy Path

**Scenario:** User runs `kova` with no args expecting a REPL.

**Result:** `Error: Device not configured (os error 6)` when piped. In a real terminal it opens the REPL correctly, but the error message is cryptic for a pipe/non-TTY context.

**Scenario:** User runs `kova chat` in a terminal.

**Result:** Requires `--features inference` (Kalosm). Default build includes it. REPL starts, shows banner, waits for input. Agent loop works — sends to local Qwen2.5-Coder, streams response, executes tool calls.

**Score: 7/10.** Works when you know the right incantation.

---

## 4. Second Use Case — Deploy

**Scenario:** User wants to deploy code to worker nodes.

`kova c2 nodes` — shows lf/gd/bt/st. Clear.
`kova c2 build --broadcast -p kova --release` — syncs and builds on all nodes.

**Friction:** User must have SSH keys set up to all nodes. No guidance on what the nodes are or how to configure them. `kova c2 inspect` shows hardware specs but requires network access.

**Score: 6/10.** Powerful if you have the infrastructure. No onramp for those who don't.

---

## 5. Edge Cases

| Input | Result | Verdict |
|-------|--------|---------|
| `kova foobar` | "unrecognized subcommand" + help hint | Good |
| `kova x x99` | "invalid value, did you mean x9?" | Excellent |
| `kova git` (no args) | "required arguments not provided" | Good |
| `kova` (piped) | "Device not configured (os error 6)" | Bad — should say "REPL requires a terminal" |
| `kova review` (no git repo) | Exits with error about git | Acceptable |

**Score: 7/10.** Clap gives great error messages. The REPL TTY error is the one bad spot.

---

## 6. Feature Gap Analysis

What a user would expect but can't do:

1. **No `kova init` for new projects.** Bootstrap sets up ~/.kova but doesn't scaffold a new Rust project with kova's patterns.
2. **No `kova deploy` shortcut.** Must use `kova c2 build --broadcast`. A `kova deploy` alias would match mental model.
3. **No web dashboard.** `kova serve` has API endpoints but the WASM client is minimal — no node status, no deployment history.
4. **No model download command.** `kova model install` exists but requires manual GGUF download. Should auto-pull from HuggingFace.
5. **No Windows support.** SSH commands, AppleScript, CoreGraphics — deeply macOS/Linux.

---

## 7. Documentation Gap

Questions a user would have that docs don't answer:

1. How do I set up worker nodes from scratch?
2. What models are supported and where do I get them?
3. What's the difference between `kova chat` and `kova tui`?
4. How does the MoE tournament work in practice?
5. What's the minimum hardware for running local inference?

---

## 8. Competitor Check

| Tool | Overlap | Kova Advantage | Kova Disadvantage |
|------|---------|----------------|-------------------|
| Claude Code | Agent loop + tools | Local-first, no API costs, distributed | No cloud model quality |
| Cursor | Code generation | Single binary, swarm orchestration | No IDE integration |
| Aider | Local LLM coding | Multi-node MoE, tournament training | Less model support |
| Ollama | Local inference | Full agent loop, not just inference | Kalosm less mature than llama.cpp |

**Honest assessment:** Kova is unique in combining local inference + agent loop + swarm orchestration + tournament-based model selection in a single binary. No competitor does all four. But it requires significant infrastructure (4 worker nodes) to fully use.

---

## 9. Verdict

| Category | Score | Notes |
|----------|-------|-------|
| Usability | 6/10 | Works well for the author. New users need hand-holding |
| Completeness | 8/10 | 30 subcommands, all functional. Only daemon is stubbed |
| Error Handling | 7/10 | Clap errors are excellent. REPL TTY error is bad |
| Documentation | 5/10 | README is strong. No user guide, no tutorials |
| Would You Pay | 4/10 | Too infrastructure-dependent. Need plug-and-play mode |

**Overall: 6/10.** Impressive engineering. Not yet shippable to customers who aren't the author.

---

## 10. Top 3 Fixes

### Fix 1: REPL TTY error message
When stdin is not a TTY, print a helpful message instead of "Device not configured."

### Fix 2: Tokenization coverage to 100%
60 untokenized symbols breaks P13. Assign tokens to all public symbols.

### Fix 3: `kova deploy` alias
Add a `deploy` subcommand that maps to `c2 build --broadcast --release`.

---

*Analysis by Claude Opus 4.6 — user simulation, not code review.*
