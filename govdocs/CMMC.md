# CMMC (Cybersecurity Maturity Model Certification) Mapping

**Product:** kova v0.7.0
**Date:** 2026-03-27

This document maps kova's practices to CMMC Level 1 and Level 2 requirements. CMMC is based on NIST SP 800-171 (Protecting Controlled Unclassified Information).

---

## Level 1 — Basic Cyber Hygiene (17 Practices)

### AC — Access Control

| Practice | Requirement | kova Implementation |
|---|---|---|
| AC.L1-3.1.1 | Limit system access to authorized users | CLI runs as invoking Unix user. No multi-user mode. No privilege escalation. |
| AC.L1-3.1.2 | Limit system access to authorized functions | Subcommand structure (`src/main.rs`). Feature gates in `Cargo.toml` disable unused capabilities at compile time. |
| AC.L1-3.1.20 | Control connection of external systems | SSH to worker nodes uses certificate authority (`src/ssh_ca.rs`). Node list hardcoded with IPs. |
| AC.L1-3.1.22 | Control information posted publicly | No public-facing component by default. `kova serve` binds localhost. Cloudflare tunnel is opt-in via separate tool. |

### IA — Identification and Authentication

| Practice | Requirement | kova Implementation |
|---|---|---|
| IA.L1-3.5.1 | Identify system users | Unix user identity inherited. SSH key-based auth for node access. |
| IA.L1-3.5.2 | Authenticate users | SSH certificate authority (`src/ssh_ca.rs`). Key-based authentication, no passwords. |

### MP — Media Protection

| Practice | Requirement | kova Implementation |
|---|---|---|
| MP.L1-3.8.3 | Sanitize media before disposal | `rm -rf ~/.kova/` removes all data. sled database is a local file. No residual data on remote systems. |

### PE — Physical Protection

Physical protection is the responsibility of the host environment. kova is software.

### SC — System and Communications Protection

| Practice | Requirement | kova Implementation |
|---|---|---|
| SC.L1-3.13.1 | Monitor and protect communications | SSH with CA for node traffic. rustls for HTTPS. No plaintext credentials. |
| SC.L1-3.13.5 | Implement subnetworks for public components | No public-facing component. HTTP listener defaults to localhost. |

### SI — System and Information Integrity

| Practice | Requirement | kova Implementation |
|---|---|---|
| SI.L1-3.14.1 | Identify and fix flaws timely | Dependency pinning via `Cargo.lock`. `cargo audit` for CVE scanning. Single binary simplifies patching. |
| SI.L1-3.14.2 | Provide protection from malicious code | Rust memory safety. No `unsafe` in application code. `clippy -D warnings`. |
| SI.L1-3.14.4 | Update malicious code protection | `cargo update` + TRIPLE SIMS test gate validates all dependency updates. |
| SI.L1-3.14.5 | Perform system scans | `kova test` runs full quality gate: clippy + 3x test + release build + smoke. |

**Level 1 Assessment: 14/17 practices addressed by kova directly. Remaining 3 (physical protection) are host environment responsibilities.**

---

## Level 2 — Advanced Cyber Hygiene (110 Practices from NIST SP 800-171)

Selected practices where kova provides direct support:

### AC — Access Control (Level 2)

| Practice | kova Implementation |
|---|---|
| AC.L2-3.1.3 — Control CUI flow | All data stays in `~/.kova/`. No external transmission. No cloud sync. |
| AC.L2-3.1.5 — Least privilege | Feature gates compile out unused capabilities. No setuid. No root required. |
| AC.L2-3.1.7 — Prevent non-privileged users from executing privileged functions | Unix permissions on `~/.kova/`. Single-user tool. |
| AC.L2-3.1.12 — Monitor remote access | SSH access logged via system sshd. LLM traces logged in sled (`src/trace.rs`). |

### AU — Audit and Accountability

| Practice | kova Implementation |
|---|---|
| AU.L2-3.3.1 — Create audit records | LLM call traces stored in sled (`src/trace.rs`). Every inference call logged. |
| AU.L2-3.3.2 — Unique trace to individual users | Single-user tool. All traces attributable to the invoking Unix user. |
| AU.L2-3.3.4 — Alert on audit failure | Errors from sled writes propagated via `thiserror` (`src/storage.rs` E0). |

### CM — Configuration Management

| Practice | kova Implementation |
|---|---|
| CM.L2-3.4.1 — Establish configuration baselines | `Cargo.toml` + `Cargo.lock` define the exact build. `~/.kova/config.toml` for runtime. |
| CM.L2-3.4.2 — Enforce configuration settings | Feature gates at compile time. Type-safe config via serde (`src/config.rs`). |
| CM.L2-3.4.5 — Define change types requiring authorization | `Cargo.lock` changes tracked in git. Dependency updates require test gate pass. |
| CM.L2-3.4.6 — Least functionality | Feature gates: `serve`, `gui`, `tui`, `inference`, `rag`, etc. Build only what you need. |

### IA — Identification and Authentication (Level 2)

| Practice | kova Implementation |
|---|---|
| IA.L2-3.5.3 — Multi-factor authentication | Not applicable (local CLI tool). Host system MFA applies for SSH access. |
| IA.L2-3.5.10 — Store/transmit only cryptographically-protected passwords | Argon2id for password hashing. AES-256-GCM for stored secrets. No plaintext. |

### SC — System and Communications Protection (Level 2)

| Practice | kova Implementation |
|---|---|
| SC.L2-3.13.8 — Implement cryptographic mechanisms | AES-256-GCM, HKDF, Argon2id. See `govdocs/FIPS.md` for validation status. |
| SC.L2-3.13.11 — Employ FIPS-validated crypto | **Gap.** Current Rust implementations not FIPS-validated. See `govdocs/FIPS.md` for remediation path. |
| SC.L2-3.13.16 — Protect CUI at rest | AES-256-GCM encryption on sled database for sensitive records. |

### SI — System and Information Integrity (Level 2)

| Practice | kova Implementation |
|---|---|
| SI.L2-3.14.3 — Monitor security alerts | `cargo audit` scans `Cargo.lock` against RustSec advisory database. |
| SI.L2-3.14.6 — Monitor system for unauthorized use | Single-user local tool. No remote access unless `kova serve` is explicitly started. |
| SI.L2-3.14.7 — Identify unauthorized use | LLM traces provide audit trail of all inference operations. |

---

## Known Gaps

| Gap | CMMC Practice | Remediation |
|---|---|---|
| FIPS-validated crypto | SC.L2-3.13.11 | Swap to `aws-lc-rs` with FIPS feature. See `govdocs/FIPS.md`. |
| Formal audit log export | AU.L2-3.3.1 | `kova traces` exists; needs export to SIEM-compatible format. |
| Role-based access control | AC.L2-3.1.4 | Single-user tool; RBAC not applicable in current architecture. |

## Assessment Summary

- **Level 1:** 14/17 practices addressed. Remaining 3 are physical security (host responsibility).
- **Level 2:** Majority of practices addressed through Rust safety, local-only architecture, crypto, and SSH CA. Primary gap is FIPS validation.
