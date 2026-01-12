# Governance Deployment Guide

This guide covers deployment strategies for the Aeterna Memory-Knowledge System's governance layer, supporting three distinct deployment modes to accommodate different organizational needs and infrastructure requirements.

## Overview of Deployment Modes

### Local Mode
Full governance stack runs locally within the application instance. This mode provides complete autonomy and is ideal for development, testing, or scenarios requiring offline operation.

**Characteristics:**
- All governance operations performed locally
- No external dependencies for governance
- Complete data sovereignty
- Suitable for single-node deployments

### Hybrid Mode
Combines local governance engine with remote synchronization capabilities. Local operations provide low-latency access while maintaining consistency with a central governance authority.

**Characteristics:**
- Local governance engine for fast operations
- Synchronization with remote Permit.io instance
- Graceful degradation when remote unavailable
- Cache with configurable TTL (default: 5 minutes)
- Pending change queue for eventual consistency

### Remote Mode
Thin client architecture where all governance operations are delegated to a remote governance service. This mode minimizes local resource requirements and centralizes governance logic.

**Characteristics:**
- All governance operations via remote API
- Minimal local resource footprint
- Centralized policy management
- Depends on remote service availability

## Prerequisites

### System Requirements
- **Rust**: 1.70+ with Edition 2024
- **PostgreSQL**: 16+ with pgvector extension
- **Redis**: 7+ for caching and pub/sub
- **Qdrant**: 1.12+ for vector storage
- **Docker**: 20.10+ (for containerized deployment)

### Network Requirements
- Outbound HTTPS connectivity (for Hybrid/Remote modes)
- Internal network connectivity between components
- Load balancer (recommended for production Hybrid deployments)

### Security Requirements
- API keys for remote governance services
- TLS certificates for inter-service communication
- Network segmentation for isolation (optional)

## Configuration

### Environment Variables

The system supports automatic configuration detection through environment variables:

```bash
# Deployment mode
export AETERNA_DEPLOYMENT_MODE=local|hybrid|remote

# Remote governance service URL (required for hybrid/remote)
export AETERNA_REMOTE_GOVERNANCE_URL=https://governance.example.com

# Force thin client mode (implies remote mode)
export AETERNA_THIN_CLIENT=true

# Enable/disable synchronization (hybrid mode only)
export AETERNA_SYNC_ENABLED=true
```

### Configuration File Structure

All modes support TOML configuration files. Create `config.toml` in your application directory or specify via `AETERNA_CONFIG_FILE` environment variable.

## Local Mode Deployment

### Configuration

```toml
[deployment]
mode = "local"
sync_enabled = true  # Always enabled in local mode

[providers.postgres]
host = "localhost"
port = 5432
database = "memory_knowledge"
username = "postgres"
password = "${POSTGRES_PASSWORD}"
pool_size = 10
timeout_seconds = 30

[providers.qdrant]
host = "localhost"
port = 6333
collection = "memory_embeddings"
timeout_seconds = 30

[providers.redis]
host = "localhost"
port = 6379
db = 0
pool_size = 10
timeout_seconds = 30

[sync]
enabled = true
sync_interval_seconds = 60
batch_size = 100
checkpoint_enabled = true
conflict_resolution = "prefer_knowledge"

[memory]
promotion_threshold = 0.8

[tools]
enabled = true
host = "localhost"
port = 8080
rate_limit_requests_per_minute = 60

[observability]
metrics_enabled = true
tracing_enabled = true
logging_level = "info"
metrics_port = 9090
```

### Docker Compose for Local Development

```yaml
version: '3.8'

services:
  aeterna-local:
    build: .
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - AETERNA_DEPLOYMENT_MODE=local
      - POSTGRES_PASSWORD=postgres
      - REDIS_URL=redis://redis:6379
      - QDRANT_URL=http://qdrant:6333
    depends_on:
      - postgres
      - qdrant
      - redis
    volumes:
      - ./config.toml:/app/config.toml

  postgres:
    image: pgvector/pgvector:pg16
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: memory_knowledge
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  qdrant:
    image: qdrant/qdrant:v1.12.0
    ports:
      - "6333:6333"
    volumes:
      - qdrant_data:/qdrant/storage
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:6333/health"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

volumes:
  postgres_data:
  qdrant_data:
  redis_data:
```

### Building and Running

```bash
# Build with local governance features
cargo build --release --features governance-local

# Run with environment variables
export AETERNA_DEPLOYMENT_MODE=local
export POSTGRES_PASSWORD=your_password
./target/release/aeterna

# Or with Docker Compose
docker-compose -f docker-compose.local.yml up -d
```

### Feature Flags

```toml
# Cargo.toml
[dependencies]
knowledge = { path = "./knowledge", features = ["local"] }
```

## Hybrid Mode Deployment

### Configuration

```toml
[deployment]
mode = "hybrid"
remote_url = "https://governance.yourcompany.com"
sync_enabled = true

[deployment.hybrid]
cache_ttl_seconds = 300  # 5 minutes default
sync_interval_seconds = 60
max_pending_changes = 1000
retry_attempts = 3
retry_backoff_seconds = 5

# Remote service authentication
[deployment.remote]
api_key = "${GOVERNANCE_API_KEY}"
timeout_seconds = 30
connection_pool_size = 10

# Fallback configuration when remote unavailable
[deployment.fallback]
grace_period_seconds = 300
max_stale_data_seconds = 3600
enable_local_validation = true

# Include all provider configurations from local mode
[providers.postgres]
# ... same as local mode

[providers.qdrant]
# ... same as local mode

[providers.redis]
# ... same as local mode
```

### Docker Compose for Hybrid Deployment

```yaml
version: '3.8'

services:
  aeterna-hybrid:
    build: .
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - AETERNA_DEPLOYMENT_MODE=hybrid
      - AETERNA_REMOTE_GOVERNANCE_URL=https://governance.yourcompany.com
      - GOVERNANCE_API_KEY=${GOVERNANCE_API_KEY}
      - POSTGRES_PASSWORD=postgres
      - REDIS_URL=redis://redis:6379
      - QDRANT_URL=http://qdrant:6333
    depends_on:
      - postgres
      - qdrant
      - redis
    volumes:
      - ./config.toml:/app/config.toml
    restart: unless-stopped

  # Load balancer for multiple hybrid instances
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      - ./ssl:/etc/nginx/ssl
    depends_on:
      - aeterna-hybrid
    restart: unless-stopped

  postgres:
    image: pgvector/pgvector:pg16
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: memory_knowledge
    volumes:
      - postgres_data:/var/lib/postgresql/data
    restart: unless-stopped

  qdrant:
    image: qdrant/qdrant:v1.12.0
    volumes:
      - qdrant_data:/qdrant/storage
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data
    restart: unless-stopped

volumes:
  postgres_data:
  qdrant_data:
  redis_data:
```

### Building and Running

```bash
# Build with hybrid governance features
cargo build --release --features governance-hybrid

# Run with environment variables
export AETERNA_DEPLOYMENT_MODE=hybrid
export AETERNA_REMOTE_GOVERNANCE_URL=https://governance.yourcompany.com
export GOVERNANCE_API_KEY=your_api_key
export POSTGRES_PASSWORD=your_password
./target/release/aeterna

# Or with Docker Compose
docker-compose -f docker-compose.hybrid.yml up -d
```

### Feature Flags

```toml
# Cargo.toml
[dependencies]
knowledge = { path = "./knowledge", features = ["hybrid"] }
```

## Remote Mode Deployment

### Configuration

```toml
[deployment]
mode = "remote"
remote_url = "https://governance.yourcompany.com"
sync_enabled = false  # Disabled in remote mode

[deployment.remote]
api_key = "${GOVERNANCE_API_KEY}"
timeout_seconds = 30
connection_pool_size = 10
health_check_interval_seconds = 60
max_retry_attempts = 3
retry_backoff_seconds = 5

# Minimal local configuration (only for non-governance features)
[providers]
# Only include if using local storage for non-governance data

[observability]
metrics_enabled = true
tracing_enabled = true
logging_level = "info"
metrics_port = 9090
```

### Docker Compose for Remote Deployment

```yaml
version: '3.8'

services:
  aeterna-remote:
    build: .
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - AETERNA_DEPLOYMENT_MODE=remote
      - AETERNA_REMOTE_GOVERNANCE_URL=https://governance.yourcompany.com
      - GOVERNANCE_API_KEY=${GOVERNANCE_API_KEY}
    volumes:
      - ./config.toml:/app/config.toml
    restart: unless-stopped
    resources:
      limits:
        memory: 512M
        cpus: '0.5'
      reservations:
        memory: 256M
        cpus: '0.25'

  # Optional: Local storage for non-governance data
  postgres:
    image: pgvector/pgvector:pg16
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: memory_knowledge
    volumes:
      - postgres_data:/var/lib/postgresql/data
    restart: unless-stopped

volumes:
  postgres_data:
```

### Building and Running

```bash
# Build with remote governance features
cargo build --release --features governance-remote

# Run with environment variables
export AETERNA_DEPLOYMENT_MODE=remote
export AETERNA_REMOTE_GOVERNANCE_URL=https://governance.yourcompany.com
export GOVERNANCE_API_KEY=your_api_key
./target/release/aeterna

# Or with Docker Compose
docker-compose -f docker-compose.remote.yml up -d
```

### Feature Flags

```toml
# Cargo.toml
[dependencies]
knowledge = { path = "./knowledge", features = ["remote"] }
```

## Infrastructure Requirements

### Scaling Considerations

#### Local Mode Scaling
- **Horizontal Scaling**: Not supported (each instance independent)
- **Vertical Scaling**: Increase CPU, memory, and storage
- **Storage**: Local PostgreSQL and Qdrant instances
- **Network**: Local network only

#### Hybrid Mode Scaling
- **Horizontal Scaling**: Supported with shared remote governance
- **Load Balancing**: Required for multiple instances
- **Caching**: Redis for local cache consistency
- **Storage**: Shared storage backend for persistence

#### Remote Mode Scaling
- **Horizontal Scaling**: Fully supported
- **Resource Requirements**: Minimal per instance
- **Dependency**: Remote governance service scalability
- **Network**: Reliable connection to remote service

### Resource Planning

| Component | Local Mode | Hybrid Mode | Remote Mode |
|-----------|------------|-------------|-------------|
| CPU | 2-4 cores | 2-4 cores | 1-2 cores |
| Memory | 4-8 GB | 4-8 GB | 1-2 GB |
| Storage | 50-100 GB | 50-100 GB | 10-20 GB |
| Network | Local | Local + Internet | Internet |

### High Availability

#### Local Mode HA
- PostgreSQL replication
- Qdrant clustering
- Redis clustering
- Application-level failover

#### Hybrid Mode HA
- Remote governance service HA
- Local cache replication
- Graceful degradation strategy
- Health check automation

#### Remote Mode HA
- Remote governance service HA
- Application auto-scaling
- Connection retry logic
- Circuit breaker pattern

## Health Checks and Monitoring

### Health Check Endpoints

```bash
# Application health
GET /health

# Detailed health with subsystems
GET /health/detailed

# Governance-specific health
GET /health/governance

# Metrics
GET /metrics
```

### Health Check Configuration

```yaml
# docker-compose.yml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 40s
```

### Monitoring Metrics

#### Application Metrics
- Request latency and error rates
- Governance operation counts
- Cache hit/miss ratios (Hybrid mode)
- Sync queue lengths (Hybrid mode)

#### Infrastructure Metrics
- CPU, memory, disk usage
- Database connection pools
- Network latency to remote services
- Queue depths and processing times

#### Alerting Thresholds

```yaml
# Prometheus alerting rules
groups:
  - name: aeterna-governance
    rules:
      - alert: GovernanceRemoteDown
        expr: up{job="aeterna"} == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Aeterna governance service is down"

      - alert: GovernanceSyncQueueHigh
        expr: governance_sync_queue_length > 100
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Governance sync queue is growing"

      - alert: GovernanceCacheLowHitRate
        expr: governance_cache_hit_rate < 0.8
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "Governance cache hit rate is low"
```

## Migration Between Modes

### Local to Hybrid Migration
1. Deploy remote governance service
2. Update configuration to hybrid mode
3. Gradually migrate policies and data
4. Monitor sync process
5. Update client applications

### Hybrid to Remote Migration
1. Ensure all data synchronized to remote
2. Update configuration to remote mode
3. Remove local governance components
4. Update deployment scripts
5. Monitor for connectivity issues

### Mode Switching Procedure

```bash
#!/bin/bash
# Mode migration script

CURRENT_MODE=$(grep 'mode = ' config.toml | cut -d'"' -f2)
TARGET_MODE=$1

echo "Migrating from $CURRENT_MODE to $TARGET_MODE mode"

case $TARGET_MODE in
  "local")
    sed -i 's/mode = ".*"/mode = "local"/' config.toml
    sed -i '/remote_url/d' config.toml
    ;;
  "hybrid")
    sed -i 's/mode = ".*"/mode = "hybrid"/' config.toml
    if ! grep -q 'remote_url' config.toml; then
      echo 'remote_url = "https://governance.example.com"' >> config.toml
    fi
    ;;
  "remote")
    sed -i 's/mode = ".*"/mode = "remote"/' config.toml
    ;;
esac

echo "Migration complete. Please review config.toml and restart the service."
```

## Troubleshooting

### Common Issues

#### Local Mode Issues
- **Problem**: Governance validation fails
- **Solution**: Check local policy files and database connectivity
- **Command**: `curl http://localhost:8080/health/detailed`

#### Hybrid Mode Issues
- **Problem**: Sync queue growing
- **Solution**: Check remote connectivity and API key validity
- **Command**: `grep "sync_pending_changes" /var/log/aeterna.log`

#### Remote Mode Issues
- **Problem**: Connection timeouts
- **Solution**: Verify network connectivity and remote service status
- **Command**: `curl -H "Authorization: Bearer $API_KEY" https://governance.example.com/health`

### Debug Commands

```bash
# Check deployment mode
curl http://localhost:8080/health | jq '.deployment.mode'

# Check sync status (hybrid mode)
curl http://localhost:8080/health/governance | jq '.sync_state'

# Check remote connectivity
curl -H "Authorization: Bearer $API_KEY" \
  https://governance.example.com/api/v1/health

# View current configuration
curl http://localhost:8080/config | jq '.deployment'
```

## Security Considerations

### API Key Management
- Use environment variables for API keys
- Rotate keys regularly
- Implement key revocation procedures
- Audit key usage

### Network Security
- TLS encryption for all remote communication
- Network segmentation for governance services
- VPN or private links for hybrid deployments
- Firewall rules for service access

### Data Protection
- Encrypt sensitive data at rest
- Implement access controls
- Regular security audits
- Compliance with data protection regulations

## Best Practices

### Configuration Management
- Use configuration templates for different environments
- Validate configuration before deployment
- Store secrets securely (not in config files)
- Version control configuration changes

### Deployment Automation
- Infrastructure as Code (Terraform, CloudFormation)
- CI/CD pipelines for automated deployments
- Blue-green deployments for zero downtime
- Automated rollback capabilities

### Operational Excellence
- Comprehensive logging and monitoring
- Disaster recovery procedures
- Regular backup and restore testing
- Performance tuning and optimization

## Additional Resources

- [Architecture Overview](../architecture/overview.md)
- [Configuration Reference](../configuration/)
- [API Documentation](../api/)
- [Monitoring Guide](../monitoring/)
- [Security Guidelines](../security/)