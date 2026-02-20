# deployment Specification

## Purpose
Deployment configuration, high availability infrastructure, disaster recovery, horizontal scaling, and encryption at rest for Aeterna enterprise deployments.
## Requirements
### Requirement: Deployment Configuration

The system SHALL support multiple deployment modes WITH high availability options.

#### Scenario: Production HA Deployment
- **WHEN** deploying to production
- **THEN** Helm chart deploys:
  - PostgreSQL with Patroni (1 primary + 2 replicas)
  - Qdrant cluster (3 nodes, RF=2)
  - Redis Sentinel (3 nodes)
  - Memory service (3+ replicas)
  - Knowledge service (2+ replicas)
- **AND** configures PodDisruptionBudgets
- **AND** enables automatic backups
- **AND** configures monitoring

### Requirement: OpenTofu Multi-Cloud Provisioning
The platform SHALL provide OpenTofu Infrastructure-as-Code modules for automated, highly-available deployment across GCP, AWS, and Azure.

#### Scenario: Provisioning AWS
- **WHEN** applying the AWS module
- **THEN** an EKS cluster, Multi-AZ RDS Postgres, and ElastiCache Redis are provisioned
- **AND** IAM Roles for Service Accounts (IRSA) are generated and bound to Aeterna components

### Requirement: Cloud KMS Encryption
The deployment modules MUST enforce Customer-Managed Encryption Keys (CMEK) at rest for all stateful stores.

#### Scenario: GCP Encryption
- **WHEN** provisioning GCS Buckets or Cloud SQL in GCP
- **THEN** the resources must be encrypted with a Cloud KMS key provisioned by the module

## Overview

This specification defines deployment patterns for the Memory-Knowledge System. The architecture supports multiple deployment models to accommodate different organizational requirements, from local development to enterprise-scale production.

## Deployment Models

### Model Comparison

| Model | Memory Provider | Knowledge Store | Vector DB | Use Case |
|-------|-----------------|-----------------|-----------|----------|
| **Local** | Letta (local) | SQLite | Qdrant (local) | Development, single-user |
| **Self-Hosted** | OpenMemory | PostgreSQL | Qdrant/Milvus | Team/org, data sovereignty |
| **Cloud** | Mem0 Cloud | Managed | Managed | SaaS, minimal ops |
| **Hybrid** | Mem0 Cloud | Self-hosted | Self-hosted | Sensitive knowledge, cloud memory |

### Architecture Diagram

```mermaid
graph TB
    subgraph "Client Layer"
        A1[Agent 1]
        A2[Agent 2]
        A3[Agent N]
    end
    
    subgraph "API Gateway"
        GW[Load Balancer]
    end
    
    subgraph "Service Layer"
        MS[Memory Service]
        KS[Knowledge Service]
        SS[Sync Service]
    end
    
    subgraph "Storage Layer"
        MP[(Memory Provider)]
        KR[(Knowledge Repo)]
        VDB[(Vector DB)]
        Cache[(Redis Cache)]
    end
    
    A1 & A2 & A3 --> GW
    GW --> MS & KS & SS
    MS --> MP
    MS --> Cache
    KS --> KR
    KS --> VDB
    SS --> MS & KS
```

---

## Local Development

### Prerequisites

- Docker Desktop or Podman
- 8GB RAM minimum
- 20GB disk space

### Docker Compose (Local)

```yaml
# docker-compose.local.yml
version: '3.8'

services:
  # Memory Provider - Letta (local mode)
  letta:
    image: letta/letta-server:latest
    ports:
      - "8283:8283"
    environment:
      - LETTA_SERVER_MODE=local
      - LETTA_LLM_PROVIDER=openai
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    volumes:
      - letta_data:/root/.letta

  # Vector Database - Qdrant
  qdrant:
    image: qdrant/qdrant:latest
    ports:
      - "6333:6333"
      - "6334:6334"
    volumes:
      - qdrant_data:/qdrant/storage
    environment:
      - QDRANT__SERVICE__GRPC_PORT=6334

  # Knowledge Repository - SQLite (embedded in service)
  knowledge-service:
    build:
      context: ./services/knowledge
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=sqlite:///data/knowledge.db
      - VECTOR_DB_URL=http://qdrant:6333
    volumes:
      - knowledge_data:/data
    depends_on:
      - qdrant

  # Sync Service
  sync-service:
    build:
      context: ./services/sync
      dockerfile: Dockerfile
    ports:
      - "8081:8081"
    environment:
      - MEMORY_PROVIDER_URL=http://letta:8283
      - KNOWLEDGE_SERVICE_URL=http://knowledge-service:8080
    depends_on:
      - letta
      - knowledge-service

volumes:
  letta_data:
  qdrant_data:
  knowledge_data:
```

### Quick Start (Local)

```bash
# Clone the reference implementation
git clone https://github.com/memory-knowledge-spec/reference-impl.git
cd reference-impl

# Set environment variables
cp .env.example .env
# Edit .env with your API keys

# Start services
docker compose -f docker-compose.local.yml up -d

# Verify health
curl http://localhost:8080/health
curl http://localhost:8283/health
curl http://localhost:6333/health
```

---

## Self-Hosted Production

### Infrastructure Requirements

| Component | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| **Memory Service** | 2 CPU, 4GB RAM | 4 CPU, 8GB RAM | Per instance |
| **Knowledge Service** | 2 CPU, 4GB RAM | 4 CPU, 8GB RAM | Per instance |
| **PostgreSQL** | 2 CPU, 4GB RAM, 100GB SSD | 4 CPU, 16GB RAM, 500GB SSD | Primary + replica |
| **Qdrant** | 4 CPU, 8GB RAM, 100GB SSD | 8 CPU, 32GB RAM, 500GB NVMe | Per node |
| **Redis** | 1 CPU, 2GB RAM | 2 CPU, 4GB RAM | Cache layer |

### Docker Compose (Production)

```yaml
# docker-compose.prod.yml
version: '3.8'

services:
  # Memory Provider - OpenMemory
  openmemory:
    image: mem0ai/openmemory:latest
    deploy:
      replicas: 2
      resources:
        limits:
          cpus: '4'
          memory: 8G
    ports:
      - "8283:8283"
    environment:
      - OPENMEMORY_MODE=production
      - DATABASE_URL=postgresql://${DB_USER}:${DB_PASS}@postgres:5432/memory
      - VECTOR_DB_URL=http://qdrant:6333
      - REDIS_URL=redis://redis:6379
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    depends_on:
      - postgres
      - qdrant
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8283/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Knowledge Service
  knowledge-service:
    image: memory-knowledge-spec/knowledge-service:latest
    deploy:
      replicas: 2
      resources:
        limits:
          cpus: '4'
          memory: 8G
    ports:
      - "8080:8080"
    environment:
      - NODE_ENV=production
      - DATABASE_URL=postgresql://${DB_USER}:${DB_PASS}@postgres:5432/knowledge
      - VECTOR_DB_URL=http://qdrant:6333
      - REDIS_URL=redis://redis:6379
      - JWT_SECRET=${JWT_SECRET}
    depends_on:
      - postgres
      - qdrant
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Sync Service
  sync-service:
    image: memory-knowledge-spec/sync-service:latest
    deploy:
      replicas: 1
      resources:
        limits:
          cpus: '2'
          memory: 4G
    ports:
      - "8081:8081"
    environment:
      - MEMORY_PROVIDER_URL=http://openmemory:8283
      - KNOWLEDGE_SERVICE_URL=http://knowledge-service:8080
      - SYNC_INTERVAL_MS=60000
      - REDIS_URL=redis://redis:6379
    depends_on:
      - openmemory
      - knowledge-service

  # PostgreSQL
  postgres:
    image: postgres:16-alpine
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 16G
    environment:
      - POSTGRES_USER=${DB_USER}
      - POSTGRES_PASSWORD=${DB_PASS}
      - POSTGRES_DB=memory
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init-db.sql:/docker-entrypoint-initdb.d/init.sql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${DB_USER}"]
      interval: 10s
      timeout: 5s
      retries: 5

  # Qdrant Vector Database
  qdrant:
    image: qdrant/qdrant:latest
    deploy:
      resources:
        limits:
          cpus: '8'
          memory: 32G
    ports:
      - "6333:6333"
      - "6334:6334"
    volumes:
      - qdrant_data:/qdrant/storage
    environment:
      - QDRANT__SERVICE__GRPC_PORT=6334
      - QDRANT__CLUSTER__ENABLED=false
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:6333/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Redis Cache
  redis:
    image: redis:7-alpine
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 4G
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

  # Nginx Load Balancer
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./certs:/etc/nginx/certs:ro
    depends_on:
      - openmemory
      - knowledge-service

volumes:
  postgres_data:
  qdrant_data:
  redis_data:
```

### Database Initialization

```sql
-- init-db.sql
-- Create databases
CREATE DATABASE memory;
CREATE DATABASE knowledge;

-- Memory database schema
\c memory;

CREATE TABLE IF NOT EXISTS memories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id VARCHAR(255) NOT NULL,
    user_id VARCHAR(255),
    session_id VARCHAR(255),
    content TEXT NOT NULL,
    metadata JSONB DEFAULT '{}',
    embedding_id VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_memories_agent ON memories(agent_id);
CREATE INDEX idx_memories_user ON memories(user_id);
CREATE INDEX idx_memories_session ON memories(session_id);
CREATE INDEX idx_memories_created ON memories(created_at);

-- Knowledge database schema
\c knowledge;

CREATE TABLE IF NOT EXISTS knowledge_items (
    id VARCHAR(255) PRIMARY KEY,
    type VARCHAR(50) NOT NULL,
    layer VARCHAR(50) NOT NULL,
    title VARCHAR(500) NOT NULL,
    summary TEXT,
    content TEXT NOT NULL,
    content_hash VARCHAR(64) NOT NULL,
    severity VARCHAR(20) DEFAULT 'info',
    status VARCHAR(50) DEFAULT 'draft',
    tags JSONB DEFAULT '[]',
    constraints JSONB DEFAULT '[]',
    metadata JSONB DEFAULT '{}',
    version INTEGER DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    promoted_at TIMESTAMPTZ,
    promoted_from VARCHAR(255)
);

CREATE INDEX idx_knowledge_type ON knowledge_items(type);
CREATE INDEX idx_knowledge_layer ON knowledge_items(layer);
CREATE INDEX idx_knowledge_status ON knowledge_items(status);
CREATE INDEX idx_knowledge_tags ON knowledge_items USING GIN(tags);

CREATE TABLE IF NOT EXISTS knowledge_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id VARCHAR(255) NOT NULL REFERENCES knowledge_items(id),
    version INTEGER NOT NULL,
    content TEXT NOT NULL,
    content_hash VARCHAR(64) NOT NULL,
    changed_by VARCHAR(255),
    change_reason TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_history_item ON knowledge_history(item_id);
```

### Nginx Configuration

```nginx
# nginx.conf
events {
    worker_connections 1024;
}

http {
    upstream memory_service {
        least_conn;
        server openmemory:8283;
    }

    upstream knowledge_service {
        least_conn;
        server knowledge-service:8080;
    }

    upstream sync_service {
        server sync-service:8081;
    }

    server {
        listen 80;
        server_name _;
        return 301 https://$host$request_uri;
    }

    server {
        listen 443 ssl http2;
        server_name memory-knowledge.example.com;

        ssl_certificate /etc/nginx/certs/fullchain.pem;
        ssl_certificate_key /etc/nginx/certs/privkey.pem;
        ssl_protocols TLSv1.2 TLSv1.3;

        # Memory API
        location /api/memory/ {
            proxy_pass http://memory_service/;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }

        # Knowledge API
        location /api/knowledge/ {
            proxy_pass http://knowledge_service/;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }

        # Sync API
        location /api/sync/ {
            proxy_pass http://sync_service/;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
        }

        # Health check
        location /health {
            return 200 'OK';
            add_header Content-Type text/plain;
        }
    }
}
```

---

## Kubernetes Deployment

### Namespace and ConfigMap

```yaml
# k8s/namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: memory-knowledge
  labels:
    app.kubernetes.io/name: memory-knowledge

---
# k8s/configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: memory-knowledge-config
  namespace: memory-knowledge
data:
  SYNC_INTERVAL_MS: "60000"
  LOG_LEVEL: "info"
  VECTOR_DIMENSIONS: "1536"
```

### Secrets

```yaml
# k8s/secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: memory-knowledge-secrets
  namespace: memory-knowledge
type: Opaque
stringData:
  OPENAI_API_KEY: "${OPENAI_API_KEY}"
  DB_USER: "mkuser"
  DB_PASS: "${DB_PASSWORD}"
  JWT_SECRET: "${JWT_SECRET}"
  REDIS_PASSWORD: "${REDIS_PASSWORD}"
```

### Memory Service Deployment

```yaml
# k8s/memory-service.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: memory-service
  namespace: memory-knowledge
spec:
  replicas: 3
  selector:
    matchLabels:
      app: memory-service
  template:
    metadata:
      labels:
        app: memory-service
    spec:
      containers:
        - name: openmemory
          image: mem0ai/openmemory:latest
          ports:
            - containerPort: 8283
          envFrom:
            - configMapRef:
                name: memory-knowledge-config
            - secretRef:
                name: memory-knowledge-secrets
          env:
            - name: DATABASE_URL
              value: "postgresql://$(DB_USER):$(DB_PASS)@postgres-service:5432/memory"
            - name: VECTOR_DB_URL
              value: "http://qdrant-service:6333"
            - name: REDIS_URL
              value: "redis://:$(REDIS_PASSWORD)@redis-service:6379"
          resources:
            requests:
              cpu: "1"
              memory: "2Gi"
            limits:
              cpu: "4"
              memory: "8Gi"
          livenessProbe:
            httpGet:
              path: /health
              port: 8283
            initialDelaySeconds: 30
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /ready
              port: 8283
            initialDelaySeconds: 5
            periodSeconds: 5

---
apiVersion: v1
kind: Service
metadata:
  name: memory-service
  namespace: memory-knowledge
spec:
  selector:
    app: memory-service
  ports:
    - port: 8283
      targetPort: 8283
```

### Knowledge Service Deployment

```yaml
# k8s/knowledge-service.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: knowledge-service
  namespace: memory-knowledge
spec:
  replicas: 3
  selector:
    matchLabels:
      app: knowledge-service
  template:
    metadata:
      labels:
        app: knowledge-service
    spec:
      containers:
        - name: knowledge
          image: memory-knowledge-spec/knowledge-service:latest
          ports:
            - containerPort: 8080
          envFrom:
            - configMapRef:
                name: memory-knowledge-config
            - secretRef:
                name: memory-knowledge-secrets
          env:
            - name: DATABASE_URL
              value: "postgresql://$(DB_USER):$(DB_PASS)@postgres-service:5432/knowledge"
            - name: VECTOR_DB_URL
              value: "http://qdrant-service:6333"
            - name: REDIS_URL
              value: "redis://:$(REDIS_PASSWORD)@redis-service:6379"
          resources:
            requests:
              cpu: "1"
              memory: "2Gi"
            limits:
              cpu: "4"
              memory: "8Gi"
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 30
            periodSeconds: 10

---
apiVersion: v1
kind: Service
metadata:
  name: knowledge-service
  namespace: memory-knowledge
spec:
  selector:
    app: knowledge-service
  ports:
    - port: 8080
      targetPort: 8080
```

### Qdrant StatefulSet

```yaml
# k8s/qdrant.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: qdrant
  namespace: memory-knowledge
spec:
  serviceName: qdrant
  replicas: 3
  selector:
    matchLabels:
      app: qdrant
  template:
    metadata:
      labels:
        app: qdrant
    spec:
      containers:
        - name: qdrant
          image: qdrant/qdrant:latest
          ports:
            - containerPort: 6333
            - containerPort: 6334
          env:
            - name: QDRANT__CLUSTER__ENABLED
              value: "true"
            - name: QDRANT__CLUSTER__P2P__PORT
              value: "6335"
          volumeMounts:
            - name: qdrant-storage
              mountPath: /qdrant/storage
          resources:
            requests:
              cpu: "2"
              memory: "8Gi"
            limits:
              cpu: "8"
              memory: "32Gi"
  volumeClaimTemplates:
    - metadata:
        name: qdrant-storage
      spec:
        accessModes: ["ReadWriteOnce"]
        storageClassName: fast-ssd
        resources:
          requests:
            storage: 100Gi

---
apiVersion: v1
kind: Service
metadata:
  name: qdrant-service
  namespace: memory-knowledge
spec:
  selector:
    app: qdrant
  ports:
    - name: http
      port: 6333
      targetPort: 6333
    - name: grpc
      port: 6334
      targetPort: 6334
```

### Horizontal Pod Autoscaler

```yaml
# k8s/hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: memory-service-hpa
  namespace: memory-knowledge
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: memory-service
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80

---
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: knowledge-service-hpa
  namespace: memory-knowledge
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: knowledge-service
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

### Ingress

```yaml
# k8s/ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: memory-knowledge-ingress
  namespace: memory-knowledge
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/proxy-body-size: "50m"
spec:
  tls:
    - hosts:
        - memory-knowledge.example.com
      secretName: memory-knowledge-tls
  rules:
    - host: memory-knowledge.example.com
      http:
        paths:
          - path: /api/memory
            pathType: Prefix
            backend:
              service:
                name: memory-service
                port:
                  number: 8283
          - path: /api/knowledge
            pathType: Prefix
            backend:
              service:
                name: knowledge-service
                port:
                  number: 8080
```

---

## Cloud Deployment

### AWS Architecture

```mermaid
graph TB
    subgraph "AWS Cloud"
        subgraph "VPC"
            subgraph "Public Subnet"
                ALB[Application Load Balancer]
            end
            
            subgraph "Private Subnet - Compute"
                ECS[ECS Fargate Cluster]
                MS[Memory Service Tasks]
                KS[Knowledge Service Tasks]
            end
            
            subgraph "Private Subnet - Data"
                RDS[(RDS PostgreSQL)]
                EC[(ElastiCache Redis)]
                OS[(OpenSearch)]
            end
        end
        
        S3[(S3 - Backups)]
        CW[CloudWatch]
    end
    
    ALB --> ECS
    ECS --> MS & KS
    MS & KS --> RDS & EC & OS
    ECS --> CW
    RDS --> S3
```

### Terraform Configuration

```hcl
# main.tf
terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}

# VPC
module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "5.0.0"

  name = "memory-knowledge-vpc"
  cidr = "10.0.0.0/16"

  azs             = ["${var.aws_region}a", "${var.aws_region}b", "${var.aws_region}c"]
  private_subnets = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
  public_subnets  = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]

  enable_nat_gateway = true
  single_nat_gateway = false
}

# RDS PostgreSQL
module "rds" {
  source  = "terraform-aws-modules/rds/aws"
  version = "6.0.0"

  identifier = "memory-knowledge-db"

  engine               = "postgres"
  engine_version       = "16.1"
  family               = "postgres16"
  major_engine_version = "16"
  instance_class       = "db.r6g.large"

  allocated_storage     = 100
  max_allocated_storage = 500

  db_name  = "memoryknowledge"
  username = var.db_username
  port     = 5432

  multi_az               = true
  db_subnet_group_name   = module.vpc.database_subnet_group
  vpc_security_group_ids = [module.security_group.security_group_id]

  backup_retention_period = 7
  skip_final_snapshot     = false
  deletion_protection     = true

  performance_insights_enabled = true
}

# ElastiCache Redis
module "elasticache" {
  source  = "terraform-aws-modules/elasticache/aws"
  version = "1.0.0"

  cluster_id           = "memory-knowledge-cache"
  engine               = "redis"
  node_type            = "cache.r6g.large"
  num_cache_nodes      = 2
  parameter_group_name = "default.redis7"
  port                 = 6379
  subnet_group_name    = module.vpc.elasticache_subnet_group_name
  security_group_ids   = [module.security_group.security_group_id]
}

# ECS Cluster
resource "aws_ecs_cluster" "main" {
  name = "memory-knowledge-cluster"

  setting {
    name  = "containerInsights"
    value = "enabled"
  }
}

# ECS Task Definition - Memory Service
resource "aws_ecs_task_definition" "memory_service" {
  family                   = "memory-service"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = 2048
  memory                   = 4096
  execution_role_arn       = aws_iam_role.ecs_execution.arn
  task_role_arn            = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([
    {
      name  = "memory-service"
      image = "mem0ai/openmemory:latest"
      portMappings = [
        {
          containerPort = 8283
          hostPort      = 8283
          protocol      = "tcp"
        }
      ]
      environment = [
        { name = "DATABASE_URL", value = "postgresql://${var.db_username}:${var.db_password}@${module.rds.db_instance_endpoint}/memory" },
        { name = "REDIS_URL", value = "redis://${module.elasticache.cache_nodes[0].address}:6379" }
      ]
      secrets = [
        { name = "OPENAI_API_KEY", valueFrom = aws_secretsmanager_secret.openai.arn }
      ]
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          awslogs-group         = "/ecs/memory-service"
          awslogs-region        = var.aws_region
          awslogs-stream-prefix = "ecs"
        }
      }
    }
  ])
}
```

### Using Mem0 Cloud

For teams preferring managed services:

```typescript
// Configuration for Mem0 Cloud
const config: MemoryKnowledgeConfig = {
  memory: {
    provider: 'mem0',
    config: {
      apiKey: process.env.MEM0_API_KEY,
      organizationId: process.env.MEM0_ORG_ID,
      projectId: process.env.MEM0_PROJECT_ID,
    },
  },
  knowledge: {
    // Self-hosted knowledge for data sovereignty
    provider: 'postgresql',
    config: {
      connectionString: process.env.KNOWLEDGE_DB_URL,
    },
  },
  vector: {
    // Use Mem0's built-in vector storage
    provider: 'mem0',
  },
};
```

---

## Hybrid Deployment

For organizations requiring data sovereignty for knowledge but cloud convenience for memory:

```mermaid
graph TB
    subgraph "Cloud (Mem0)"
        M0[Mem0 Cloud API]
        MV[(Mem0 Vector Store)]
    end
    
    subgraph "On-Premise"
        KS[Knowledge Service]
        PG[(PostgreSQL)]
        QD[(Qdrant)]
    end
    
    subgraph "Sync Layer"
        SS[Sync Service]
    end
    
    Agent --> M0
    Agent --> KS
    M0 --> MV
    KS --> PG
    KS --> QD
    SS --> M0
    SS --> KS
```

### Hybrid Configuration

```json
{
  "memory": {
    "provider": "mem0",
    "config": {
      "apiKey": "${MEM0_API_KEY}",
      "organizationId": "org_xxx"
    }
  },
  "knowledge": {
    "provider": "postgresql",
    "config": {
      "connectionString": "postgresql://localhost:5432/knowledge"
    }
  },
  "vector": {
    "provider": "qdrant",
    "config": {
      "url": "http://localhost:6333"
    }
  },
  "sync": {
    "enabled": true,
    "mode": "hybrid",
    "memoryToKnowledge": {
      "enabled": true,
      "promotionThreshold": 0.8
    },
    "knowledgeToMemory": {
      "enabled": true,
      "summaryOnly": true
    }
  }
}
```

---

## Monitoring & Observability

### Prometheus Metrics

```yaml
# prometheus/rules.yaml
groups:
  - name: memory-knowledge
    rules:
      - alert: HighMemoryLatency
        expr: histogram_quantile(0.99, rate(memory_operation_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: High memory operation latency

      - alert: KnowledgeSyncFailed
        expr: increase(knowledge_sync_failures_total[1h]) > 5
        for: 10m
        labels:
          severity: critical
        annotations:
          summary: Knowledge sync failures detected

      - alert: VectorDBUnhealthy
        expr: up{job="qdrant"} == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: Vector database is down
```

### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "Memory-Knowledge System",
    "panels": [
      {
        "title": "Memory Operations/sec",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(memory_operations_total[5m])",
            "legendFormat": "{{operation}}"
          }
        ]
      },
      {
        "title": "Knowledge Items by Layer",
        "type": "piechart",
        "targets": [
          {
            "expr": "knowledge_items_total",
            "legendFormat": "{{layer}}"
          }
        ]
      },
      {
        "title": "Sync Queue Depth",
        "type": "gauge",
        "targets": [
          {
            "expr": "sync_queue_depth"
          }
        ]
      }
    ]
  }
}
```

---

## Backup & Disaster Recovery

### Backup Strategy

| Component | Method | Frequency | Retention |
|-----------|--------|-----------|-----------|
| PostgreSQL | pg_dump + WAL archiving | Continuous | 30 days |
| Qdrant | Snapshots | Daily | 7 days |
| Redis | RDB + AOF | Hourly | 24 hours |
| Configs | Git | On change | Forever |

### Backup Script

```bash
#!/bin/bash
# backup.sh

set -euo pipefail

BACKUP_DIR="/backups/$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

# PostgreSQL
pg_dump -h postgres -U mkuser memory | gzip > "$BACKUP_DIR/memory.sql.gz"
pg_dump -h postgres -U mkuser knowledge | gzip > "$BACKUP_DIR/knowledge.sql.gz"

# Qdrant snapshot
curl -X POST "http://qdrant:6333/collections/memories/snapshots"
curl -X POST "http://qdrant:6333/collections/knowledge/snapshots"

# Upload to S3
aws s3 sync "$BACKUP_DIR" "s3://memory-knowledge-backups/$BACKUP_DIR"

# Cleanup old local backups
find /backups -type d -mtime +7 -exec rm -rf {} +

echo "Backup completed: $BACKUP_DIR"
```

### Recovery Procedure

```bash
#!/bin/bash
# restore.sh

BACKUP_DATE=$1

# Download from S3
aws s3 sync "s3://memory-knowledge-backups/backups/$BACKUP_DATE" /restore/

# Restore PostgreSQL
gunzip -c /restore/memory.sql.gz | psql -h postgres -U mkuser memory
gunzip -c /restore/knowledge.sql.gz | psql -h postgres -U mkuser knowledge

# Restore Qdrant
curl -X PUT "http://qdrant:6333/collections/memories/snapshots/recover" \
  -H "Content-Type: application/json" \
  -d "{\"location\": \"/restore/memories_snapshot.snapshot\"}"

echo "Restore completed from: $BACKUP_DATE"
```

---

## Security Considerations

### Network Security

```yaml
# Network policies for Kubernetes
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: memory-knowledge-network-policy
  namespace: memory-knowledge
spec:
  podSelector: {}
  policyTypes:
    - Ingress
    - Egress
  ingress:
    - from:
        - namespaceSelector:
            matchLabels:
              name: ingress-nginx
      ports:
        - protocol: TCP
          port: 8080
        - protocol: TCP
          port: 8283
  egress:
    - to:
        - podSelector:
            matchLabels:
              app: postgres
      ports:
        - protocol: TCP
          port: 5432
    - to:
        - podSelector:
            matchLabels:
              app: qdrant
      ports:
        - protocol: TCP
          port: 6333
    - to:
        - podSelector:
            matchLabels:
              app: redis
      ports:
        - protocol: TCP
          port: 6379
```

### Encryption

| Data State | Method |
|------------|--------|
| In Transit | TLS 1.3 |
| At Rest (DB) | AES-256 |
| At Rest (Vectors) | Volume encryption |
| Secrets | AWS KMS / HashiCorp Vault |

---

## Related Specifications

- [05-adapter-architecture.md](./05-adapter-architecture.md) - Provider adapters
- [07-configuration.md](./07-configuration.md) - Configuration schema
- [09-migration.md](./09-migration.md) - Data migration procedures
