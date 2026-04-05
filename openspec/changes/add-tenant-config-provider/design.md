## Context

The existing tenant control plane can create tenants and persist tenant repository bindings, but it does not provide a single tenant-scoped configuration model that both operators and deployment automation can consume. Secret values are intentionally excluded from binding records, yet the platform still lacks a clean contract for where tenant config lives, how secret references are administered, and how environment-specific deployment assets materialize those settings safely in Kubernetes.

This change introduces a provider abstraction so Aeterna can manage tenant config independently from any one secret backend while still delivering a practical first implementation for Kubernetes deployment. The first concrete provider will store tenant config in a ConfigMap and tenant secret values in a paired Secret using the tenant's stable unique identifier, which keeps runtime segregation explicit and makes GitOps/deployment wiring simpler for private cluster deployments.

## Goals / Non-Goals

**Goals:**
- Define a canonical tenant configuration document that separates non-secret config from secret references.
- Add a `TenantConfigProvider` abstraction so tenant config storage is pluggable.
- Implement a Kubernetes-backed provider using one ConfigMap and one Secret per tenant, named from the tenant UUID.
- Support explicit GlobalAdmin and TenantAdmin workflows with proper segregation of what each role can mutate.
- Make the deployment repo and tenant provisioning flow render the same canonical tenant configuration contract.

**Non-Goals:**
- Replacing all secret backends with Vault or ESO in the first phase.
- Storing raw secret values in control-plane database tables.
- Inferring tenant config ownership from namespace alone without explicit tenant identity.
- Solving cross-cluster tenant config replication in the first phase.

## Decisions

### Canonical tenant config is a structured document plus secret references
Tenant configuration will be represented as structured data containing tenant metadata, deployment/runtime settings, and logical secret-reference entries. The config document may reference secret values by logical name and concrete storage location, but it must not embed raw secret values.

**Alternatives considered:**
- **Continue storing ad hoc config fields in separate models**: rejected because admin and deployment flows remain fragmented.
- **Store all tenant config in raw YAML blobs only**: rejected because validation, CLI UX, and role-based segregation become weak.

### Provider abstraction owns persistence and validation boundaries
Add a `TenantConfigProvider` trait responsible for reading, writing, validating, and listing tenant config and secret-reference metadata. Business logic will speak to the trait instead of directly to Kubernetes resources.

**Alternatives considered:**
- **Hard-code Kubernetes APIs into tenant admin flows**: rejected because it prevents future provider implementations and harms testability.

### First provider implementation is Kubernetes ConfigMap + Secret
The first concrete implementation will store tenant config in a ConfigMap named from the stable tenant ID and store tenant secret values in a Secret paired to that same tenant ID. Secret references inside tenant config will point to keys within the tenant Secret, not to arbitrary cross-tenant resources.

Resource naming:
- ConfigMap: `aeterna-tenant-<tenant-id>`
- Secret: `aeterna-tenant-<tenant-id>-secret`

**Alternatives considered:**
- **Vault/ESO as the only first implementation**: rejected because it increases implementation scope and deployment coupling before the canonical config contract exists.
- **Single shared Secret for all tenants**: rejected because it weakens tenant segregation and admin boundaries.

### Secret values remain separate from config and persisted control-plane records
Raw tenant secret values must only be written to the provider's secret storage surface. API responses, CLI output, audit events, and database-backed config records will expose only logical secret names and references.

### Shared Git provider connections remain platform-owned and tenant-visible by policy
GitHub connectivity for tenant knowledge repositories will be modeled as a platform-owned Git provider connection that holds reusable connection metadata and secret material such as GitHub App certificate references. Tenant configuration may reference only an allowed connection identifier; it must not duplicate PEM material or provider credentials into tenant-owned config.

**Alternatives considered:**
- **Store GitHub App identity independently in every tenant**: rejected because certificate rotation and installation metadata would be duplicated and hard to audit.
- **Single implicit global GitHub connection with no visibility policy**: rejected because different tenants may require different GitHub Apps or approved provider connectivity.

### GlobalAdmin and TenantAdmin authority must be explicitly segregated
GlobalAdmin may create tenants, bootstrap tenant config containers, and manage environment/deployment-owned fields across tenants. TenantAdmin may update only tenant-owned config and logical secret entries for its tenant and must not mutate other tenants or platform-owned deployment fields.

### Deployment repo consumes provider-shaped artifacts
A private deployment repo will materialize the Kubernetes provider contract using tenant-scoped manifests or generated values so the same tenant config model flows from control plane to cluster deployment.

## Risks / Trade-offs

- **[Risk] ConfigMap becomes a dumping ground for unrelated settings** → Mitigation: validate a typed tenant config schema and keep explicit sections for runtime/deployment/secretRefs.
- **[Risk] TenantAdmin can point references at another tenant's Secret** → Mitigation: enforce provider validation that only the tenant's paired Secret and allowed keys are addressable.
- **[Risk] Deployment repo drifts from control-plane contract** → Mitigation: generate/render provider-shaped artifacts from the same schema and cover them in E2E tests.
- **[Risk] Kubernetes-only first implementation constrains future providers** → Mitigation: keep the trait contract backend-neutral and test through trait-level fixtures.

## Migration Plan

1. Define the `tenant-config-provider` capability and modify deployment/multi-tenant governance requirements.
2. Introduce typed tenant config/secret-reference models and the `TenantConfigProvider` trait.
3. Implement the Kubernetes ConfigMap/Secret provider and provider-level validation.
4. Add control-plane API and CLI flows for tenant config inspection and mutation.
5. Integrate tenant provisioning and a private deployment repo with provider-shaped artifacts.
6. Add E2E and deployment verification for representative tenant bootstrap flows.

## Open Questions

- Should provider-backed tenant config also persist a mirrored non-secret summary in Postgres for fast list/show operations, or should Kubernetes be the source of truth for the first phase?
- Should deployment-owned fields live in the same config document with ownership markers, or in a separate platform patch document keyed by tenant ID?
