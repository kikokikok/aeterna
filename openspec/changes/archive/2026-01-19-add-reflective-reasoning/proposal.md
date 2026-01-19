# Change: Reflective Memory Reasoning (MemR³)

## Why
Standard vector-based retrieval often suffers from "semantic drift" and noise, especially in complex agent trajectories. Reflective Memory Reasoning (based on recent MemR³ research, ArXiv Dec 2025) introduces a reasoning step *before* retrieval:

- **Reduces Noise**: Prevents irrelevant memory fragments from polluting the LLM context.
- **Multi-Hop Retrieval**: Enables agents to formulate retrieval queries that link different pieces of information.
- **Query Refinement**: Translates user intent into optimized semantic and factual search parameters.

## What Changes
- **New Component**: `ReflectiveReasoner` in the `memory` crate.
- **New Tool**: `memory_reason` tool to allow agents to generate search strategies.
- **Modified Memory System**: Integration points for reasoning-driven filtering.

## Impact
- Affected specs: `memory-system`, `tool-interface`
- Affected code: `memory/`, `tools/`
- Performance: Adds a pre-retrieval reasoning step (~100-300ms depending on LLM).
