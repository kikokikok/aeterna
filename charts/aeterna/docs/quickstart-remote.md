# Remote Mode Deployment Guide

## Overview
Remote mode allows you to run Aeterna as a thin client. This setup connects to a central Aeterna server for all its operations. You don't need local storage or local databases. It minimizes resource usage by offloading heavy lifting to the central cluster.

## When to Use Remote Mode
- Cost-sensitive edge deployments.
- Environments with reliable network paths to a central server.
- Quick setup scenarios where you don't want to manage databases.
- Developer workstations using Docker Desktop or minikube.

## Architecture
```
[Local Cluster]                   [Central Cluster]  
Aeterna (remote) ------------>   Aeterna (central)
  └── (no local storage)          ├── PostgreSQL
                                  ├── Qdrant
                                  ├── Dragonfly
                                  └── OPAL Stack
```

## Prerequisites
- A central Aeterna server must be deployed and accessible.
- Network connectivity (HTTPS) to the central server is required.
- You need an API key, OAuth2, or service account credentials.
- Kubernetes 1.25+ and Helm 3.10+ must be installed.

## Step 1: Create Credentials Secret
Run the following command to store your central server credentials:
```bash
kubectl create secret generic aeterna-central-credentials \
  --from-literal=api-key='your-api-key-here'
```

## Step 2: Create values-remote.yaml
Use this minimal configuration to enable remote mode:
```yaml
deploymentMode: remote
central:
  url: "https://aeterna-central.example.com"
  auth: apiKey
  existingSecret: "aeterna-central-credentials"
aeterna:
  enabled: true
  replicaCount: 1
postgresql:
  bundled: false
cnpg:
  enabled: false
dragonfly:
  enabled: false
qdrant:
  enabled: false
opal:
  enabled: false
cache:
  type: external
  external:
    enabled: false
```

## Step 3: Install
Install the chart with your remote values:
```bash
helm install aeterna-remote ./charts/aeterna -f values-remote.yaml
```

## Step 4: Verify Connection
Check the pod logs to ensure the connection is successful:
```bash
kubectl logs -l app.kubernetes.io/name=aeterna
```
You can also test the health endpoint or verify connectivity from the central server's dashboard.

## Resource Usage
Remote mode has a very small footprint. Since it doesn't run its own databases or search engines, it only requires a single Aeterna pod. This makes it ideal for resource-constrained environments.

## Connection Requirements
The remote cluster needs a stable connection to the central server. We recommend latency under 100ms for the best performance. If the central server becomes unreachable, the remote instance will enter a read-only or failover state depending on your configuration.

## Authentication Methods
You can choose from several authentication methods:
- **apiKey**: Uses a static key for simple setups.
- **oauth2**: Connects through an OAuth2 provider for better security.
- **serviceAccount**: Uses Kubernetes service account tokens for cluster-to-cluster auth.

## Troubleshooting
- **Connection Refused**: Verify the central server URL and ensure no firewalls block the traffic.
- **Auth Failures**: Check if the secret contains the correct key and the central server recognizes it.
- **Timeout**: Network latency might be too high or the central server is overloaded.
- **Central Server Down**: Remote operations will fail until the central server is back online.
