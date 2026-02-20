# Design: Production Improvements

## Context

Aeterna has solid foundations with knowledge graph (DuckDB) and comprehensive multi-tenancy already implemented. However, gaps exist in production-grade features needed for enterprise deployment at scale.

**What Works Well**:
- Knowledge graph fully implemented with DuckDB
- Multi-tenancy enforced across all storage types (PostgreSQL RLS, Redis scoping, all vector DBs)
- Basic memory and knowledge operations
- OpenSpec-driven development process

**What's Missing**:
- High Availability / Disaster Recovery
- Advanced observability (beyond basic metrics)
- Cost optimization mechanisms
- Complete CCA implementation
- Real-time collaboration
- Encryption at rest
- GDPR compliance

## Goals

1. **Production Readiness**: Achieve 99.9% availability with automated failover
2. **Cost Efficiency**: Reduce operational costs by 50-70% through optimization
3. **Observability**: Complete visibility into system behavior and costs
4. **Security**: Enterprise-grade encryption and compliance
5. **Performance**: Support 100k+ users with horizontal scaling

## Non-Goals

- Multi-modal memory (images/audio/video) is Phase 4
- Real-time video collaboration
- Custom LLM training
- Federated learning across organizations

## High-Level Architecture

### Current Architecture (Simplified)
```
┌─────────────────────────────────────────┐
│            API Gateway                   │
└────────────┬────────────────────────────┘
             │
    ┌────────┴────────┐
    │                 │
┌───▼──────┐    ┌────▼─────┐
│ Memory   │    │Knowledge │
│ Service  │    │ Service  │
└───┬──────┘    └────┬─────┘
    │                │
    └────────┬───────┘
             │
┌────────────▼───────────────┐
│   Storage Layer (Single)   │
│  - PostgreSQL              │
│  - Redis                   │
│  - Qdrant                  │
│  - DuckDB Graph            │
└────────────────────────────┘
```

### Target Architecture (HA + Scale)
```
┌──────────────────────────────────────────────────┐
│         Load Balancer (Multi-AZ)                 │
└────────────┬─────────────────────────────────────┘
             │
    ┌────────┴────────────────────┐
    │                             │
┌───▼──────────┐    ┌─────────────▼──────────┐
│ Memory Svc   │    │  Knowledge Svc         │
│ (3 replicas) │    │  (2 replicas)          │
└───┬──────────┘    └────┬───────────────────┘
    │                    │
    └─────────┬──────────┘
              │
┌─────────────▼──────────────────────────────────┐
│        Storage Layer (HA)                      │
│                                                │
│  ┌──────────────┐  ┌──────────────┐          │
│  │ PostgreSQL   │  │  Qdrant      │          │
│  │ (Patroni)    │  │  (Cluster)   │          │
│  │ - Primary    │  │  - 3 nodes   │          │
│  │ - 2 Replicas │  │  - RF=2      │          │
│  └──────────────┘  └──────────────┘          │
│                                                │
│  ┌──────────────┐  ┌──────────────┐          │
│  │ Redis        │  │  DuckDB      │          │
│  │ (Sentinel)   │  │  + S3        │          │
│  │ - 3 nodes    │  │  (HA via S3) │          │
│  └──────────────┘  └──────────────┘          │
└────────────────────────────────────────────────┘
             │
┌────────────▼──────────────────────────────────┐
│         Observability Stack                   │
│  - OpenTelemetry Collector                    │
│  - Prometheus / Datadog                       │
│  - Jaeger / Zipkin                            │
│  - Cost Tracking DB                           │
└───────────────────────────────────────────────┘
```

## Decisions

### 1. HA Strategy: Patroni + Qdrant Cluster + Redis Sentinel

**Decision**: Use Patroni for PostgreSQL, native cluster for Qdrant, Sentinel for Redis

**Alternatives Considered**:
- **Alternative 1**: Single-node with backups only
  - ❌ Doesn't meet 99.9% SLA (allows 8+ hours downtime/year)
- **Alternative 2**: Managed services (AWS RDS, Elasticache)
  - ✅ Simpler operations
  - ❌ Higher costs
  - ❌ Vendor lock-in
  - **Decision**: Document as option but default to self-managed

**Rationale**: Patroni is battle-tested, Qdrant has native clustering, Redis Sentinel is standard. Together they provide:
- Automated failover (< 30 seconds)
- No single point of failure
- Horizontal read scaling
- Cost-effective vs managed services

### 2. Observability: Datadog vs Grafana Cloud

**Decision**: Default to Grafana Cloud, support Datadog as option

**Alternatives**:
- **Datadog**: Best-in-class, expensive ($100+/host/mo)
- **Grafana Cloud**: Good enough, cost-effective ($8-50/user/mo)
- **Self-hosted**: Cheapest, highest operational burden

**Rationale**: Grafana Cloud provides 80% of Datadog features at 20% of cost, suitable for most deployments. Enterprise customers can opt for Datadog.

### 3. Cost Optimization: Semantic Caching + Tiered Storage

**Decision**: Implement both semantic caching and tiered storage

**Why Semantic Caching**:
- Exact match cache catches duplicates (20-40% hit rate expected)
- Semantic similarity cache (0.98+) catches near-duplicates (additional 20-40%)
- Combined: 40-80% reduction in embedding API calls
- ROI: Positive after ~1M embeddings

**Why Tiered Storage**:
- Hot tier (Redis): < 7 days, frequently accessed
- Warm tier (PostgreSQL): 7-90 days, occasionally accessed
- Cold tier (S3 + Parquet): > 90 days, rarely accessed
- Storage cost reduction: ~40%

### 4. CCA Implementation: Full Stack

**Decision**: Implement all 4 CCA components (Context Architect, Note-Taking, Hindsight, Meta-Agent)

**Why All 4**:
- **Context Architect**: +7.6% performance (proven in research)
- **Note-Taking**: Captures execution trajectories for learning
- **Hindsight Learning**: Prevents repeated errors
- **Meta-Agent**: Autonomous improvement

**Implementation Order**:
1. Context Architect (highest ROI, easiest)
2. Hindsight Learning (high value, moderate complexity)
3. Note-Taking (foundation for meta-learning)
4. Meta-Agent (most complex, requires 1-3)

### 5. Real-Time Collaboration: WebSocket + Redis Pub/Sub

**Decision**: WebSocket for client connections, Redis pub/sub for fanout

**Architecture**:
```
Client A ←─→ WS Server 1 ──┐
                            ├──→ Redis Pub/Sub ──┐
Client B ←─→ WS Server 2 ──┘                     ├──→ Broadcast
                                                  │
Client C ←─→ WS Server 3 ────────────────────────┘
```

**Why This Pattern**:
- Horizontally scalable (add more WS servers)
- No single point of failure
- Low latency (Redis < 5ms)
- Battle-tested pattern

### 6. Encryption: TDE + Field-Level + KMS

**Decision**: Multi-layer encryption strategy

**Layers**:
1. **Transport**: TLS 1.3 (already implemented)
2. **At-Rest (Database)**: Transparent Data Encryption
3. **Field-Level**: AES-256-GCM for sensitive fields
4. **Key Management**: AWS KMS or HashiCorp Vault

**Why Multi-Layer**:
- Defense in depth
- Compliance requirements (SOC2, ISO27001)
- Different threat models

### 7. GDPR Compliance: Built-In, Not Bolt-On

**Decision**: Implement GDPR features natively

**Features**:
- Right to be forgotten (soft delete + anonymization)
- Data export (JSON format)
- Consent management (per-tenant)
- Audit trail (immutable)

**Why Native**:
- Easier to maintain
- Better performance
- No external dependencies

## Risks & Trade-offs

### Risk 1: Patroni Complexity

**Risk**: Patroni adds operational complexity (etcd/Consul dependency)

**Mitigation**:
- Comprehensive runbooks
- Automated DR drills
- Training for ops team
- Fallback to managed RDS documented

### Risk 2: Cost Optimization Complexity

**Risk**: Semantic caching adds code complexity, potential for bugs

**Mitigation**:
- Extensive testing (property-based tests)
- Feature flag (can disable if issues)
- Gradual rollout (1% → 10% → 100%)
- Monitoring cache hit rates

### Risk 3: WebSocket Connection Limits

**Risk**: 10k concurrent WebSocket connections per server

**Mitigation**:
- Use long polling fallback for low-priority clients
- Horizontal scaling (add servers)
- Connection pooling
- Presence heartbeat optimization

### Risk 4: CCA LLM Costs

**Risk**: Context Architect calls LLM for every context assembly

**Mitigation**:
- Cache summarizations (content-hash based)
- Use cheaper models (gpt-3.5-turbo vs gpt-4)
- Batch summarization jobs
- Per-tenant budgets

## Migration Plan

### Phase 1 (Months 1-2): Foundation

**Week 1-2**: HA Setup
- Deploy Patroni cluster
- Deploy Qdrant cluster
- Deploy Redis Sentinel
- Test failover scenarios

**Week 3-4**: Observability
- Setup Grafana Cloud
- Add trace correlation
- Implement cost tracking
- Create dashboards

**Week 5-6**: Cost Optimization
- Implement semantic caching
- Add tiered storage
- Test cache hit rates

**Week 7-8**: Security & Compliance
- Enable encryption at rest
- Implement GDPR features
- Add field-level encryption

### Phase 2 (Months 3-4): Scale

**Week 9-10**: Horizontal Scaling
- Extract services
- Configure autoscaling
- Load test

**Week 11-12**: Real-Time Collaboration
- Implement WebSocket server
- Add presence detection
- Test concurrent users

### Phase 3 (Months 5-6): Advanced

**Week 13-16**: Complete CCA
- Context Architect
- Note-Taking Agent
- Hindsight Learning
- Meta-Agent

**Week 17-18**: Research Integrations
- MemR³
- MoA
- Matryoshka embeddings

**Week 19-20**: Few-Shot Learning
- Example selector
- Active learning

### Phase 4 (Months 7-8): Polish

**Week 21-24**: Multi-Modal + Ecosystem
- Image support
- Additional managed services
- Final testing & documentation

## Open Questions

1. **Patroni vs CloudNativePG**: Should we use CloudNativePG operator instead?
   - CloudNativePG is newer, more Kubernetes-native
   - Patroni is more mature, more examples
   - **Decision needed by**: Phase 1 start

2. **Datadog vs Grafana**: Which should be default?
   - Datadog: Better UX, higher cost
   - Grafana: Good enough, cost-effective
   - **Current lean**: Grafana default, Datadog optional

3. **Semantic cache threshold**: 0.98 or 0.99 similarity?
   - 0.98: Higher hit rate, slight quality degradation
   - 0.99: Lower hit rate, better quality
   - **Decision**: Make configurable, default 0.98

4. **Cold storage format**: Parquet or Avro?
   - Parquet: Better for analytics, slightly larger
   - Avro: Better for streaming, slightly smaller
   - **Current lean**: Parquet (already used in DuckDB)
