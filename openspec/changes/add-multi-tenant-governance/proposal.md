# Change: Multi-Tenant Governance Architecture

## Why

To support 300+ developers across multiple teams/projects, Aeterna needs enterprise-grade multi-tenancy with:
- Hybrid deployment (local dev + central shared instance)
- Role-based access control for architects to govern knowledge/memory
- Semantic drift detection to identify when projects diverge from company standards
- Both real-time alerts and batch reporting for governance insights

## What Changes

- **BREAKING**: Add tenant isolation to all memory and knowledge operations
- Add ReBAC (Relationship-Based Access Control) using OpenFGA
- Add governance roles: Developer, Tech Lead, Architect, Admin
- Add drift detection engine with semantic similarity analysis
- Add scheduled batch jobs for complex drift analysis
- Add real-time event streaming for governance notifications
- Add governance dashboard API endpoints

## Impact

- Affected specs: `memory-system`, `knowledge-repository`, `sync-bridge`
- New spec: `multi-tenant-governance`
- Affected code: All crates require tenant context
- External dependencies: OpenFGA (or SpiceDB)
