# Assumed Breach Threat Model

> **Operating assumption: every component below is already compromised. Design for damage containment and loud detection, not for prevention.**

This document is the canonical threat model for every project in the `cochranblock/*` portfolio. Each project adapts the Threat Surface section for its own context but shares the same first principles, mitigations, and verification protocol.

---

## First Principles

1. **Every record that matters has an external witness.** Hashes published to public git (or equivalent neutral timestamp authority) so tampering requires simultaneously corrupting your system AND the public chain.
2. **No single point of compromise.** Signing keys in hardware (YubiKey / TPM / Secure Enclave). Never in software. Never in env vars. Never in config files.
3. **Default air-gap.** No network dependency for correctness. Network is for backup + publishing hashes, both signed, both verifiable post-hoc.
4. **Append-only everything.** No delete path in any storage layer. Corrections are reversing entries referencing the original. Standard accounting discipline, enforced in code.
5. **Cryptographic audit chain.** Every day's state derives from the previous day's hash. Tampering with any day invalidates every subsequent day.
6. **Disclosure of methodology is a security feature.** If an auditor can independently verify the algorithm, they can independently verify the outputs. No "trust us" layers.
7. **Separation of duties enforced in software.** Entry, approval, and audit live in different trust zones. Compromise of one does not compromise the others.
8. **Redundancy across trust zones.** Local + different-cloud + different-format + offline. Attacker must compromise all to hide damage.
9. **Test breach scenarios regularly.** Triple Sims applied to tamper detection. If the chain does not detect a simulated tamper, the chain is broken.

---

## Threat Surface (kova-specific)

Kova is an augment engine — LLM-driven code generation, agentic tool execution, and distributed C2 over SSH to worker nodes. It does **not** emit legal, financial, or regulatory records directly. Its threat surface centers on **execution authority** (kova runs arbitrary tools on behalf of the operator) and **generated-code propagation** (kova-authored code lands in downstream repos whose commits *are* records of consequence).

### Applicable threats

- **Binary compromise → poisoned tool loop.** The `kova` binary executes `bash`/`write`/`edit`/`cargo`/`git` tools (`src/tools.rs`, f140-f146) against the local filesystem and against worker nodes over SSH. A tampered binary becomes a worm that edits source in every project the operator touches and backdoors every `cargo run` it executes.
- **Pipeline code generation compromise.** The code-gen pipeline (`src/pipeline/`, f81/f91-f93/f116-f118) writes generated code into downstream projects (cochranblock, approuter, oakilydokily, ronin-sites). A compromised pipeline lands poisoned code in those repos and in their commit history, propagating into *their* public chains.
- **C2 worker-node compromise.** `src/c2.rs` (f119-f121) issues SSH commands to n0/n1/n2/n3 (lf, gd, bt, st). Theft of the operator's SSH keys — or a compromised `kova` binary — pivots to all worker nodes, including the tunnel host (gd) that fronts Cloudflare-tunneled products.
- **sled memory/context tampering.** Agent memory, prompt history, and tokenized command state live in local sled trees. Rewriting them rewrites the agent's narrative of prior sessions — forensically corrosive if kova outputs are ever cited in prior-art or attribution disputes.
- **Local LLM weight swap / prompt injection.** Inference runs through Candle (`src/inference.rs`, f76/f80; migrated from Kalosm — see commit c1c4084). Swapped model weights or a prompt-injection chain in loaded context silently biases every downstream code generation. Hard to detect without golden-output diffs.
- **Embedded crypto primitive corruption.** AES-256-GCM / HKDF / Argon2id are compiled in (P17). A malicious dep substitution that downgrades the cipher to something reversible retroactively weakens every wrapped secret kova has ever produced.
- **Supply chain (Cargo deps).** Candle, sled, axum, egui, and the tokio stack are the highest-blast-radius deps. A backdoored release propagates into every downstream build kova touches. Mitigation: `cargo audit` in the test binary, pinned `Cargo.lock`, reproducible-build target.
- **Physical device seizure (Mac Mini).** Holds sled stores, SSH keys to all worker nodes, and any hardware-key stubs. FileVault + hardware key stored physically separate from the device.
- **Worker-node compromise pivoting back.** A compromised worker node (gd/bt/lf/st) returning crafted output to kova's `node_cmd` (`src/node_cmd.rs`, f122-f132) could exploit a parser bug to escalate back into the C2 host. Treat node output as untrusted input.

### Not applicable

- **Public-chain hash publishing (N/A for kova itself).** Kova emits no records of legal, financial, or regulatory consequence. The public-chain pattern applies to *downstream* projects (cochranblock, ronin-sites, cochranblock accounting modules) that hold records of that kind. Kova's role in those chains is indirect — its generated code flows into those projects, and *they* commit their own chain.
- **Daily hash commits on kova state (N/A).** No per-day state to chain. Commits to the kova source repo already establish temporal witnesses for kova code changes. A future `kova-provenance` chain capturing `(model hash, prompt hash, output hash)` per generation is a possible extension, not a current requirement.
- **Hardware-key signing on kova outputs (N/A today).** Kova's outputs are source-code diffs applied to downstream repos. Signing discipline lives on *those* repos' commits, not on kova's intermediate artifacts.
- **DCAA/FAR audit scope (N/A).** Kova is not a record-of-consequence system for government-contract audit purposes. Downstream financial/accounting systems are.

---

## Mitigations

| Assume | Mitigation | Verification |
|--------|-----------|--------------|
| Binary compromised | Hardware-key signatures for every output of consequence | Anyone can verify the public key matches expected fingerprint |
| Storage compromised | Append-only sled trees. Delete is not a function, not a policy. | Hash chain breaks on any rewrite. External witness detects. |
| Network MITM | Air-gap capable. Network used only for signed backups + hash publishing. | NTP + GitHub timestamp + hardware counter cross-checked. |
| Signing key stolen | Daily hash committed to public git. Stolen key cannot retroactively change committed days. | Any day older than the public commit is immutable in evidence. |
| Audit log tampered | Separate sled tree, write-only from main app. Auditor tool reads both + cross-checks. | Compromise of main app leaves audit log intact. |
| Backup tampered | 3 different targets with 3 different credentials (local USB + off-site cloud + paper). | Attacker needs all three to hide damage. |
| Insider / self-tampering | No admin role. No delete. Reversing entries only. | Legal record immune to author second-thoughts. |
| Clock manipulation | Multiple time sources: local clock, NTP, git commit timestamp, hardware-key counter. | Divergence flags exception requiring supervisor approval. |
| Supply chain (deps) | `cargo audit` in CI. Pinned SBOM. Reproducible builds where possible. | Anyone can reproduce the binary from source + lockfile. |
| Physical device seizure | Full-disk encryption. Hardware key physically separate from device. | Stolen laptop without key is useless for forgery. |

---

## Public-Chain Deployment

This project publishes tamper-evident hashes to a public companion repo: `cochranblock/<project>-chain` (where `<project>` is the project name).

- **Daily cycle:** at 23:59 local, compute BLAKE3 of all records-of-consequence from the day. Sign with hardware key. Commit to chain repo. Push.
- **GitHub timestamp** on the commit = neutral third-party witness. Anyone can cold-verify records were not rewritten after commit time.
- **Verification:** `<project> verify` reads the chain and re-derives hashes. Any divergence = tampering detected.

This pattern is a private Certificate Transparency log for project state. Same primitive Google uses for TLS certs, applied to whatever the project tracks.

---

## Triple Sims for Tamper Detection

Standard Triple Sims gate (run 3x identically) extended with a tamper-scenario sim:

1. Normal run → produce canonical output
2. Simulated tampering (flip one bit in storage) → `verify` must flag it
3. Simulated clock rewind → `verify` must flag it

If any sim fails to detect, the chain is broken. Fix before merge.

---

## Scope of this Document

- Covers: any artifact this project emits that has legal, financial, or audit consequence.
- Does NOT cover: source code itself (public under Unlicense, not sensitive), build outputs (reproducible), marketing content (public by design).
- If your project emits no records of consequence, the relevant sections are zero-length and the public-chain deployment is skipped. Document that explicitly.

---

## Relation to Other Docs

- **TIMELINE_OF_INVENTION.md** — establishes priority dates for contributions. Feeds into the chain's initial state.
- **PROOF_OF_ARTIFACTS.md** — cryptographic signatures on release artifacts. Adjacent pattern, same first principles.
- **DCAA_COMPLIANCE.md** (where applicable) — how this threat model satisfies FAR/DFARS audit requirements.

---

## Status

- [ ] Threat Surface section adapted for this project
- [ ] Hardware-key signing integrated or N/A documented
- [ ] Public-chain repo created and connected or N/A documented
- [ ] Triple Sims tamper-detection test present or N/A documented
- [ ] External verification procedure documented

---

*Unlicensed. Public domain. Fork, strip attribution, adapt, ship.*

*Canonical source: cochranblock.org/threat-model — last revision 2026-04-14*
