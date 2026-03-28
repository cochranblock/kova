# Export Control Assessment — ITAR and EAR

**Product:** kova v0.7.0
**Date:** 2026-03-27

## ITAR (International Traffic in Arms Regulations)

**kova is not ITAR-controlled.**

- kova is not on the United States Munitions List (USML).
- kova does not contain classified information.
- kova does not contain defense articles or defense services.
- kova is a general-purpose software development tool (code generation, build automation, local AI inference).

## EAR (Export Administration Regulations)

### Classification

kova contains encryption functionality (AES-256-GCM, HKDF, Argon2id) and falls under **EAR Category 5 Part 2 — Information Security**.

### ECCN

**5D002** — Software for information security that uses or performs cryptographic functions.

### License Exception TSU (Technology and Software Unrestricted)

kova qualifies for **License Exception TSU** under **15 CFR 740.13(e)** — publicly available encryption source code.

Qualifying criteria:

1. **Publicly available source code:** kova is open source under the Unlicense (public domain dedication). Source code is published on GitHub.

2. **No restriction on redistribution:** The Unlicense places no restrictions on copying, modification, or distribution.

3. **TSU notification:** Per 740.13(e), the exporter must send an email notification to BIS (crypt@bis.doc.gov) and ENC (enc@nsa.gov) with:
   - URL of the publicly available source code
   - Project name and description
   - Confirmation that the source code is publicly available

### Cryptographic Functions

| Function | Algorithm | Key Size | Purpose |
|---|---|---|---|
| Symmetric encryption | AES-256-GCM | 256-bit | Encrypting local data at rest |
| Key derivation | HKDF (SHA-256) | 256-bit | Deriving encryption keys |
| Password hashing | Argon2id | N/A | Password-based key derivation |
| TLS | rustls (via reqwest) | Up to 256-bit | Optional HTTPS client |

### Encryption Implementation

All cryptographic code is implemented by third-party open-source Rust crates sourced from crates.io. kova does not implement custom cryptographic algorithms. The crate authors have their own EAR compliance obligations.

**Source:** `Cargo.toml` (dependency declarations), crypto design for `src/secrets.rs`.

## Countries and Entities

Under License Exception TSU, kova can be exported to most destinations without a license. However, TSU does **not** authorize export to:

- Embargoed countries (currently: Cuba, Iran, North Korea, Syria, and the Crimea/Donetsk/Luhansk regions of Ukraine)
- Denied persons or entities on the BIS Entity List
- Persons or entities on the SDN (Specially Designated Nationals) list

## AI/ML Components

kova includes local AI inference capabilities:
- **kalosm** (GGUF model execution)
- **candle** (safetensors model execution)

These are general-purpose ML inference engines. They execute publicly available model weights (e.g., Qwen2.5-Coder from Hugging Face). The ML components:

- Do not contain export-controlled algorithms beyond standard neural network inference.
- Do not provide cryptanalysis capabilities.
- Are not designed for military or intelligence applications.
- Use publicly available model architectures (transformer-based LLMs).

ML inference engines are generally classified as **EAR99** (no license required for most destinations).

## Recommendations

1. **File TSU notification** with BIS and ENC if distributing binaries internationally.
2. **Screen recipients** against BIS Entity List and OFAC SDN list.
3. **Document** that all crypto is from publicly available open-source libraries.
4. If adding classified or CUI-processing features, reassess ITAR/EAR classification.

## Summary

| Regulation | Status |
|---|---|
| ITAR | Not controlled. Not a defense article. |
| EAR ECCN | 5D002 (encryption software) |
| License Exception | TSU (740.13(e) — publicly available source code) |
| Action Required | TSU notification to BIS/ENC |
