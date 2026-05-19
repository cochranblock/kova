# kova

On-device AI augmentation engine. Local inference, agentic tool loop, distributed C2 mesh. Zero cloud. One binary.

## What It Is

kova is the AI augmentation engine at the center of The Cochran Block's stack. It runs LLM inference locally on-device, manages an agentic tool loop, and coordinates a distributed mesh of Claude Code instances (IRONHIVE). No cloud round-trips for inference — the model weights live on the machine.

## Core Architecture

| Component | Description |
|-----------|-------------|
| Inference engine | Local GGUF/safetensors model runner via candle |
| Tool loop | Agentic planner — reads context, selects tools, executes, observes |
| C2 mesh | IRONHIVE coordinator — routes tasks across bt/gd/lf/n4 nodes |
| NanoSign | Blake3-based artifact signing and provenance chain |
| Tokenizer | HuggingFace-compatible BPE tokenizer |
| TUI | Ratatui-based terminal interface |

## Key Identifiers

kova uses tokenized identifiers (`f###`, `t###`) throughout the codebase. The [Compression Map](compression_map.md) documents what each token refers to.

## Build

```bash
cargo build --release --features serve   # includes HTTP server
cargo build --release                    # TUI only
cargo test --release
```
<!-- COCHRANBLOCK-BRAND-FOOTER:START -->

---

<sub>&#9656; **THE COCHRAN BLOCK, LLC** &#183; CAGE `1CQ66` &#183; UEI `W7X3HAQL9CF9` &#183; UNLICENSE &#183; [cochranblock.org](https://cochranblock.org)</sub>
<!-- COCHRANBLOCK-BRAND-FOOTER:END -->
