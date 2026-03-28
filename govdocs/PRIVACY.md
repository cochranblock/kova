# Privacy Impact Assessment

**Product:** kova v0.7.0
**Date:** 2026-03-27

## Data Collection Summary

**kova collects zero personal data.** No PII. No telemetry. No analytics. No tracking.

---

## Data Stored Locally

All data resides in `~/.kova/` on the user's machine:

| Data Type | Storage | Encryption | Retention |
|---|---|---|---|
| Conversation history | sled database | AES-256-GCM at rest | User-controlled (local) |
| Model weights | GGUF/safetensors files | None (public models) | User-controlled |
| Configuration | TOML file | None (non-sensitive) | User-controlled |
| LLM traces | sled database | Same as conversations | User-controlled |
| RAG embeddings | sled database | Same as conversations | User-controlled |

**Source:** `src/storage.rs` (sled backend), `src/config.rs` (`~/.kova/config.toml`), `src/trace.rs` (LLM traces), `src/rag.rs` (embeddings).

## Network Behavior

kova makes **zero network calls** in default operation. Network activity occurs only when the user explicitly invokes:

| Action | Network Call | User Initiated |
|---|---|---|
| `kova serve` | Binds local HTTP listener | Yes (explicit command) |
| `kova model install` | Downloads from Hugging Face Hub | Yes (explicit command) |
| `kova c2` / `kova deploy` | SSH to configured worker nodes | Yes (explicit command) |
| `kova cluster` | Connects to configured cluster nodes | Yes (explicit config) |

No background network calls. No heartbeats. No update checks. No telemetry endpoints.

**Source:** `src/serve.rs`, `src/model.rs`, `src/c2.rs`, `src/cluster.rs`.

## PII Analysis

| PII Category | Collected | Stored | Transmitted |
|---|---|---|---|
| Name | No | No | No |
| Email | No | No | No |
| IP Address | No | No | No |
| Location | No | No | No |
| Device ID | No | No | No |
| Usage patterns | No | No | No |
| Biometrics | No | No | No |

## Conversation Data

Users interact with kova via chat (`kova chat`, `kova tui`). Conversation content:
- Stored locally in sled database at `~/.kova/`.
- Encrypted at rest via AES-256-GCM.
- Never transmitted to any external service.
- Inference runs locally via kalosm/candle (no API calls to cloud LLM providers).
- User can delete all data by removing `~/.kova/`.

**Source:** `src/repl.rs`, `src/agent_loop.rs`, `src/inference/local.rs`.

## GDPR Compliance

- **No data processing of EU personal data.** kova does not collect, store, or transmit personal data to any server.
- **No data controller/processor relationship.** The software runs entirely on the user's machine.
- **Right to erasure:** `rm -rf ~/.kova/` removes all data.
- **Data portability:** sled database is a local file; users own and control it.

## CCPA Compliance

- **No consumer data collection.** kova does not collect information that identifies, relates to, or could be linked to a consumer or household.
- **No sale of data.** No data exists to sell.
- **No data sharing with third parties.** No network calls, no third-party services.

## Children's Privacy (COPPA)

kova does not collect any information from any users, including children. No COPPA obligations apply.

## Federal Privacy Act

kova does not maintain any system of records on individuals. No Privacy Act obligations apply when deployed in a federal environment, as the tool stores only user-generated technical artifacts (code, configurations, model weights) with no linkage to individual identity.

## Data Breach Risk

The only sensitive data is conversation history (encrypted in sled). A breach would require:
1. Physical or remote access to the user's machine.
2. Access to `~/.kova/` directory.
3. Decryption of AES-256-GCM encrypted records.

kova does not create any network-accessible data store. The sled database has no listener, no port, no API.
