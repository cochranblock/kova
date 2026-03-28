# FedRAMP Applicability Notes

**Product:** kova v0.7.0
**Date:** 2026-03-27

## FedRAMP Does Not Apply

kova is **not a cloud service**. It is a locally-installed, single-binary tool that runs entirely on the user's machine. FedRAMP (Federal Risk and Authorization Management Program) authorizes cloud service offerings (CSOs). kova has no cloud component.

## Architecture

```
User Machine
  └── kova (single binary, 27 MB)
       ├── Local storage: ~/.kova/ (sled database)
       ├── Local inference: GGUF/safetensors models
       ├── Optional: HTTP listener (kova serve, localhost)
       └── Optional: SSH to worker nodes (local network)
```

- No SaaS component.
- No IaaS component.
- No PaaS component.
- No multi-tenant architecture.
- No cloud data storage.
- No cloud compute.

## Deployment in FedRAMP-Authorized Environments

If kova is installed on a system within a FedRAMP-authorized environment (e.g., a GovCloud VM, an authorized IaaS instance), it operates as a locally-installed tool **within the host system's existing authorization boundary**.

In this scenario:
- kova inherits the security controls of the host system.
- kova does not introduce new authorization boundary components.
- kova's local data (`~/.kova/`) falls under the host system's data protection controls.
- The HTTP listener (`kova serve`) must comply with the host system's network security controls.
- SSH connections to worker nodes must comply with the host system's access control policies.

## Controls kova Supports

Even though FedRAMP authorization is not required, kova's design supports several FedRAMP control families:

| Control Family | kova Support |
|---|---|
| AC (Access Control) | CLI runs as invoking user, no privilege escalation |
| AU (Audit) | LLM traces logged to sled (`src/trace.rs`) |
| CM (Configuration Management) | Single binary, no configuration drift, `~/.kova/config.toml` |
| IA (Identification & Authentication) | SSH CA for node access (`src/ssh_ca.rs`) |
| SC (System & Communications Protection) | AES-256-GCM encryption, rustls TLS, no plaintext secrets |
| SI (System & Information Integrity) | clippy + TRIPLE SIMS test gate, no panics in release |

## Optional Cloudflare Tunnel

kova can optionally be exposed to the internet via a Cloudflare tunnel, managed by the separate `approuter` application. This tunnel:
- Is configured independently of kova.
- Uses Cloudflare's infrastructure (which has FedRAMP authorization at Moderate level).
- kova itself does not manage or control the tunnel.

If the Cloudflare tunnel is used in a federal context, the tunnel component falls under Cloudflare's FedRAMP authorization, not kova's.

**Source:** `CLAUDE.md` (hosting schematic), `src/serve.rs`.
