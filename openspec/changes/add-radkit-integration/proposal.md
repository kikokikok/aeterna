# Change: Add Radkit (radkit.rs) Integration for A2A Interaction

## Why
To enable reliable, Agent-to-Agent (A2A) native communication and orchestration. Radkit provides a Rust SDK that guarantees A2A protocol compliance at compile time and offers a unified interface for multiple LLM providers, structured outputs, and stateful tool execution.

## What Changes
- **A2A Entry Point**: Implement a Radkit-powered agent that orchestrates Memory and Knowledge tools.
- **Skill Definitions**: Convert existing tool categories into Radkit `Skills` for better A2A discovery.
- **Unified Runtime**: Use Radkit's `Runtime` to serve as an A2A-compliant HTTP endpoint.
- **Thread Management**: Integrate Radkit `Thread` for conversation-level memory persistence.

## Impact
- **Affected specs**: `tool-interface`, `adapter-layer`
- **Affected code**: New `radkit-agent` crate or binary, `tools` crate integration.
- **Infrastructure**: Adds A2A-compliant discovery (Agent Cards) for the system.
