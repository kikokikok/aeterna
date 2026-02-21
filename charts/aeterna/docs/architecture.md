# Architecture Overview

Aeterna is an enterprise-grade framework for AI agent memory and knowledge management. This document describes the system architecture, deployment patterns, and component interactions within the Helm chart.

## System Architecture

The following diagram illustrates the high-level architecture of Aeterna and its dependencies when deployed via Helm.

```text
                                 ┌────────────────┐
                                 │    OpenCode    │
                                 │ (User Interface)
                                 └───────┬────────┘
                                         │ (MCP)
                                         ▼
                                 ┌────────────────┐
                                 │    Ingress     │
                                 └───────┬────────┘
                                         │
                                         ▼
                                 ┌────────────────┐
                                 │    Service     │
                                 │ (8080 / 9090)  │
                                 └───────┬────────┘
                                         │
               ┌─────────────────────────┴─────────────────────────┐
               │                     Aeterna Pod                   │
               │  ┌─────────────────────┐   ┌───────────────────┐  │
               │  │  Aeterna Server     │   │Codesearch Sidecar │  │
               │  │  (Main Container)   │◄─►│    (Optional)     │  │
               │  └──────────┬──────────┘   └─────────┬─────────┘  │
               └─────────────┼────────────────────────┼────────────┘
                             │                        │
       ┌─────────────────────┴────────────────────────┴─────────────┐
       │                                                            │
       ▼                                                            ▼
┌──────────────┐             ┌──────────────┐             ┌────────────────┐
│  PostgreSQL  │             │    Qdrant    │             │  Cache Layer   │
│ (CloudNative)│             │(Vector Store)│             │(Dragonfly/Valk)│
└──────────────┘             └──────────────┘             └────────────────┘
       ▲                            ▲                             ▲
       │                            │                             │
       └────────────────────────────┼─────────────────────────────┘
                                    │
                                    ▼
                     ┌──────────────────────────────┐
                     │          OPAL Stack          │
                     │ ┌────────┐ ┌────────┐ ┌────────┐
                     │ │ Server │ │ Agent  │ │ Fetcher│
                     │ └────────┘ └────────┘ └────────┘
                     └──────────────┬───────────────┘
                                    │
                                    ▼
                     ┌──────────────────────────────┐
                     │        Observability         │
                     │ ┌──────────────┐ ┌──────────┐ │
                     │ │ServiceMonitor│ │OTel Trace│ │
                     │ └──────────────┘ └──────────┘ │
                     └──────────────────────────────┘
```

## Deployment Modes

Aeterna supports three distinct deployment modes controlled by the `deploymentMode` value.

### Local Mode
The default mode where all components run inside the Kubernetes cluster. It provides the best data sovereignty and lowest latency for single-cluster setups.

```text
┌─────────────────────────────────────────────────┐
│                Kubernetes Cluster               │
│                                                 │
│   ┌───────────────┐       ┌─────────────────┐   │
│   │ Aeterna Pods  │◄─────►│ Local Services  │   │
│   │ (Application) │       │ (DB, Vector,    │   │
│   └───────────────┘       │  Cache, OPAL)   │   │
│                           └─────────────────┘   │
└─────────────────────────────────────────────────┘
```

### Hybrid Mode
This mode uses a local cache for high-performance memory access while synchronizing core memory and knowledge with a central Aeterna server.

```text
┌─────────────────────────┐         ┌─────────────────────────┐
│   Kubernetes Cluster    │         │     Central Server      │
│                         │         │                         │
│   ┌───────────────┐     │         │   ┌─────────────────┐   │
│   │ Aeterna Pods  │◄────┼─────────┼──►│  Global Memory  │   │
│   └───────┬───────┘     │         │   │  & Knowledge    │   │
│           │             │         │   └─────────────────┘   │
│   ┌───────▼───────┐     │         │                         │
│   │  Local Cache  │     │         │                         │
│   └───────────────┘     │         │                         │
└─────────────────────────┘         └─────────────────────────┘
```

### Remote Mode
A thin client deployment where the local cluster acts as a proxy to a central Aeterna server. This minimizes local resource usage.

```text
┌─────────────────────────┐         ┌─────────────────────────┐
│   Kubernetes Cluster    │         │     Central Server      │
│                         │         │                         │
│   ┌───────────────┐     │         │   ┌─────────────────┐   │
│   │ Aeterna (Thin)│◄────┼─────────┼──►│  Global Memory  │   │
│   │     Pod       │     │         │   │  & Knowledge    │   │
│   └───────────────┘     │         │   └─────────────────┘   │
└─────────────────────────┘         └─────────────────────────┘
```

## Component Details

| Component | Purpose | Port | Subchart |
|-----------|---------|------|----------|
| Aeterna Server | Core logic, API, and tool provider | 8080 | N/A |
| Codesearch | Sidecar for semantic code search | 9090 | N/A |
| PostgreSQL | Structured data and metadata storage | 5432 | cnpg |
| Qdrant | Vector database for semantic memory | 6333 | qdrant |
| Dragonfly | High-performance memory cache | 6379 | dragonfly |
| Valkey | Alternative Redis-compatible cache | 6379 | valkey |
| OPAL Server | Policy administration and broadcast | 7002 | N/A |
| Cedar Agent | Local policy decision point | 8180 | N/A |
| OPAL Fetcher | Real-time policy data synchronization | N/A | N/A |

## Data Flow

Request processing in Aeterna follows a structured flow through the system layers:

1. **Ingress Layer**: External requests from OpenCode or other agents enter via the Ingress controller.
2. **Authorization**: The request is validated by the Cedar Agent using policies managed by OPAL.
3. **Reasoning (Reflective)**: If enabled, the system performs pre-retrieval reasoning to identify required context.
4. **Retrieval**: The server queries the memory hierarchy.
    - **Working Memory**: Checked in the local cache (Dragonfly/Valkey).
    - **Semantic Memory**: Retrieved from the vector store (Qdrant/pgvector).
    - **Knowledge**: Retrieved from the PostgreSQL-backed knowledge repository.
5. **Code Intelligence**: If code context is needed, the Codesearch sidecar provides semantic search and call graph analysis.
6. **Execution**: The request is processed, often involving an LLM call with the gathered context.
7. **Memory Storage**: New interactions are stored back into the appropriate memory layers.

## Network Policies

When `networkPolicy.enabled` is set to true, the chart enforces strict traffic control:

- **Ingress**: Only allows traffic on ports 8080 (App) and 9090 (Metrics) from authorized sources.
- **Egress**: Restricts outbound connections to only the required infrastructure components (PostgreSQL, Vector DB, Cache, OPAL).
- **Isolation**: Prevents unauthorized lateral movement between pods within the same namespace.
- **Sidecar Communication**: Allows local loopback traffic between the Aeterna server and the Codesearch sidecar.
