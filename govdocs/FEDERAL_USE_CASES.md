# Federal Agency Use Cases

**Product:** kova v0.7.0
**Date:** 2026-03-27

---

## Department of Defense (DoD)

### Air-Gapped AI Code Generation

kova runs local LLM inference via kalosm (GGUF) and candle (safetensors). No API calls to external services. No internet required after initial model download.

**Deployment:** Install the 27 MB binary + model weights on an air-gapped workstation. `kova chat` provides an agentic code assistant with file read/write/edit, bash execution, and glob/grep tools — all local.

**Source:** `src/inference/local.rs` (local inference), `src/agent_loop.rs` (tool loop), `src/tools.rs` (file/bash tools).

### Distributed Compute Orchestration

`kova c2` provides SSH-based swarm orchestration across worker nodes. Tokenized commands (c1-c9) for status, specs, inspect, and deploy operations. No cloud orchestrator. No Kubernetes dependency.

**Use case:** Distribute builds, tests, or inference across multiple machines in a SCIF or classified network segment.

**Source:** `src/c2.rs`, `src/node_cmd.rs`, `src/ssh_ca.rs` (certificate authority for node trust).

### Factory Pipeline

`kova factory` runs a full code generation pipeline: classify requirement, generate code, compile, review, fix loop. Entirely local. Suitable for producing Rust binaries on isolated networks.

**Source:** `src/factory.rs`, `src/pipeline/mod.rs`, `src/pipeline/fix_loop.rs`.

---

## Department of Homeland Security (DHS) / CISA

### Infrastructure Automation

kova's agentic mode (`kova chat`) can read, write, and edit files, execute bash commands, and search codebases — all via local LLM. Useful for automating configuration management, log analysis, and incident response scripting.

### Code Review Pipeline

`kova review` provides LLM-powered code review of staged changes or branch diffs. No code leaves the machine. Integrates with git via tokenized commands (`kova git g0` through `g9`).

**Source:** `src/review.rs`, `src/git_cmd.rs`.

### CI/CD Quality Gate

`kova ci` runs headless quality gates: check, clippy, test, with file watching. `kova test` runs the full TRIPLE SIMS gate (3x cargo test for flaky test detection). Deployable as a local CI server without Jenkins, GitHub Actions, or other cloud CI.

**Source:** `src/ci.rs`, `src/bin/kova-test.rs`.

---

## Department of Veterans Affairs (VA)

### On-Premises AI Assistant

kova provides a local AI development assistant with zero PHI exposure risk. Conversation history stays in `~/.kova/` (encrypted, AES-256-GCM). No data transmitted externally.

**Deployment:** Developer workstations for VA engineering teams. Single binary install, no infrastructure requirements. Supports CLI, TUI (`kova tui`), desktop GUI (`kova gui`), and web interface (`kova serve`).

### No PHI Exposure

- No PII/PHI collection. No telemetry. No analytics.
- Local-only storage. No cloud sync.
- Model inference runs on-device.
- `kova serve` defaults to localhost binding.

**Source:** `govdocs/PRIVACY.md`, `src/config.rs` (serve.bind configuration).

---

## Department of Justice (DOJ)

### Sensitive Document Analysis

kova's local inference engine processes text without network calls. `pdf-extract` reads PDF files. RAG (`kova rag`) indexes documents for semantic search. All processing stays on the local machine.

**Use case:** Analyze legal documents, briefs, or case files via LLM without sending content to external APIs.

**Source:** `src/rag.rs` (semantic indexing), `Cargo.toml` line 37 (`pdf-extract`), `src/context_loader.rs`.

### Audit Trail

Every LLM inference call is traced and stored in sled (`src/trace.rs`). Traces include input, output, latency, and model used. Exportable via `kova traces` and `kova export`.

**Source:** `src/trace.rs`, `src/training_data.rs`.

---

## NASA / Department of Energy (DOE)

### Scientific Computing Automation

kova's agentic tool loop can execute arbitrary bash commands, read/write files, and iterate on code — useful for automating data processing pipelines, simulation setup, and result analysis.

### Distributed Build Orchestration

`kova deploy` and `kova c2` coordinate builds across multiple worker nodes via SSH. Suitable for distributed compilation of large scientific codebases across HPC nodes.

**Source:** `src/c2.rs`, `src/main.rs` (Deploy subcommand, lines 100-109).

### Mixture of Experts

`kova moe` fans out code generation to N nodes, compiles all candidates, scores them, and picks the winner. A tournament-based approach to distributed AI code generation for scientific software.

**Source:** `src/moe.rs`, `src/micro/moe_tournament.rs`, `src/micro/tournament.rs`.

---

## General Services Administration (GSA)

### Single Binary Deployment

kova is a single 27 MB binary. No installer. No package manager. No runtime dependencies. No JVM. No Python. No Node.js. Reduces procurement and deployment complexity to: copy one file, run it.

```bash
# Full deployment
scp kova user@target:/usr/local/bin/
ssh user@target kova bootstrap
```

### Unlicense Eliminates Licensing Concerns

kova is released under the Unlicense (public domain dedication). No license fees. No license compliance tracking. No license negotiation. No vendor lock-in.

Federal agencies can:
- Copy, modify, and distribute without restriction.
- Include in classified or unclassified systems.
- Fork and maintain internally without upstream obligations.

**Source:** `Cargo.toml` line 1 (Unlicense header), every source file header.

### Multiple Interface Options

| Interface | Command | Use Case |
|---|---|---|
| CLI | `kova chat` | Headless, scriptable, SSH-friendly |
| TUI | `kova tui` | Terminal UI for developers |
| GUI | `kova gui` | Desktop application (egui) |
| Web | `kova serve` | Browser access for teams |
| Android | (build target) | Mobile field use |

All interfaces share the same binary. No separate server deployment for the web interface.

---

## Cross-Agency Benefits

| Capability | Federal Value |
|---|---|
| Air-gapped operation | Classified networks, SCIF environments |
| Zero cloud dependency | No ATO for cloud services required |
| Local AI inference | Sensitive data never leaves the machine |
| Single binary | Simplified ATO documentation, reduced attack surface |
| Unlicense | No procurement friction, no vendor dependency |
| SSH-based orchestration | Works with existing network infrastructure |
| TRIPLE SIMS testing | High assurance for critical software |
| Encrypted local storage | Data at rest protection without external KMS |
