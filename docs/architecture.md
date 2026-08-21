# Kova Architecture: Modular, Plugin-Driven Design

## 📌 Overview
This document describes the architecture of Kova, which is designed to be modular, plugin-driven, and secure.

## 🧱 Core Philosophy
- **"Intent-to-Rust"** binary engine: All user intentions are translated into Rust-based plugins that run within a sandboxed execution environment.
- **Zero external API dependencies**: Everything is self-contained, with no reliance on cloud APIs or third-party services.
- **Everything is a plugin**: Each tool or function is a Rust plugin that can be dynamically loaded, configured, and executed.
- **Reversible Session Ledger**: Immutable event logs track all tool calls and git diffs for full temporal rollback.

## 🧰 Key Modules & Components
- **Tool Registry (MCP-compatible)**: Tools are discovered at runtime via a tool registry.
- **Execution Sandbox**: Tools are executed in isolated sandboxes (e.g., local PTY, Docker, or overlay).
- **Reversible Session Ledger**: Immutable event logs track all tool calls and git diffs.
- **OS Interception & Security Matrix**: Dynamic man page RAG, LIFO AST Stack Parser, and FS Risk Topography.
- **Web Search & Sensory Pipeline (Kova-Sight)**: No external search APIs, fast path (DOM Text), and tactical vision fallback.

## 🚀 Next Steps
1. Refactor the core modules
2. Implement the modular architecture
3. Improve documentation and set up CI/CD pipeline