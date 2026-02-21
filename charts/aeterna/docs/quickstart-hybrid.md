# Hybrid Mode Deployment Guide

## Overview

Hybrid mode combines a local cache and policy engine with a remote central Aeterna server. The local deployment handles policy evaluation and fast data access while the central server manages long term persistence and shared knowledge. This setup works well for multi cluster environments or edge locations that need local performance but centralized data control.

## When to Use Hybrid Mode

- Multiple clusters that must share the same underlying data.
- Edge or branch deployments with intermittent connectivity to the main site.
- Scenarios requiring local policy evaluation to keep authorization latency low.
- Organizations that want local caching paired with a central source of truth.

## Architecture

This diagram shows how the local hybrid instance interacts with the central server.

```
[Local Cluster]                    [Central Cluster]
Aeterna (hybrid) ──sync──────────► Aeterna (central)
├── Dragonfly (cache)              ├── PostgreSQL
├── Qdrant (vector cache)          ├── Qdrant
├── Cedar Agent (local policy)     ├── OPAL Server
└── OPAL Fetcher (disabled)        └── Full stack
```

## Prerequisites

- A central Aeterna server that is already deployed and reachable.
- Valid API keys or OAuth2 credentials for the central instance.
- Kubernetes version 1.25 or higher.
- Helm version 3.10 or higher.

## Step 1: Configure Central Server Access

The local instance needs to authenticate with the central server. Create a Kubernetes secret to store your API key or credentials safely.

```bash
kubectl create secret generic aeterna-central-auth \
  --from-literal=api-key=YOUR_CENTRAL_API_KEY
```

## Step 2: Install Aeterna in Hybrid Mode

Use the provided hybrid configuration to install the chart. Point the installation to your central server URL and reference the secret you created.

```bash
helm install aeterna aeterna/aeterna -f examples/values-hybrid.yaml \
  --set central.url=https://aeterna-central.example.com \
  --set central.existingSecret=aeterna-central-auth
```

## Step 3: Verify Sync

Check the logs of the Aeterna pod to ensure the memory synchronization is active. You should see entries confirming that the local cache is pulling updates from the central server.

```bash
kubectl logs -l app.kubernetes.io/name=aeterna -c aeterna
```

Look for messages about "sync successful" or "memory layer updated" to confirm connectivity.

## Step 4: Configure Local Cache

In hybrid mode, Aeterna uses Dragonfly and Qdrant as local caches. Dragonfly stores frequently accessed metadata while Qdrant keeps a local copy of vector embeddings. This allows the system to perform complex searches and policy checks without hitting the central database every time. The local instance automatically manages cache expiration based on the sync interval.

## Step 5: Verify Cedar Agent (Local Policy)

Hybrid mode enables the Cedar Agent locally while disabling the OPAL Fetcher. This means policies are evaluated on the local cluster using data from the local cache. Verify the agent status by checking its sidecar container.

```bash
kubectl logs -l app.kubernetes.io/name=aeterna -c cedar-agent
```

The logs will show policy evaluation requests being handled locally.

## Key Configuration

Most hybrid settings live in `examples/values-hybrid.yaml`. Key values include:

- `deploymentMode: hybrid`: Activates the hybrid logic.
- `central.url`: The endpoint for the remote central server.
- `central.auth`: Authentication details for the remote connection.
- `postgresql.bundled: false`: Disables the local database since data lives centrally.
- `opal.cedarAgent.enabled: true`: Ensures local policy evaluation.
- `opal.fetcher.enabled: false`: Prevents the local instance from trying to fetch data updates directly from Git or other sources.

## Sync Configuration

The synchronization behavior depends on these specific settings:

- `sync.interval`: Controls how often the local cache polls the central server for changes.
- `sync.batchSize`: Determines how many records are synchronized in a single request.
- `sync.memoryLayers`: Defines which data types are cached locally versus what stays remote.

Adjust these values to balance local performance against network usage.

## Troubleshooting

- **Central server unreachable**: Verify the network path and check if the `central.url` is correct. Ensure any load balancers or firewalls allow traffic between clusters.
- **Sync failures**: Check the API key in your secret. Look for authentication errors in the Aeterna logs.
- **Stale policies**: If local evaluations don't match the central server, shorten the `sync.interval` or manually trigger a sync if supported.
