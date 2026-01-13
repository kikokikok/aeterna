# Tasks: UX-First Architecture for Aeterna

## 1. Policy Skill Implementation

- [ ] 1.1 Define PolicyDraft struct and storage (Redis with TTL)
- [ ] 1.2 Implement LLM-powered PolicyTranslator (natural language → structured intent → Cedar)
- [ ] 1.3 Implement `aeterna_policy_draft` tool with full validation pipeline
- [ ] 1.4 Implement `aeterna_policy_validate` tool (Cedar validation without LLM)
- [ ] 1.5 Implement `aeterna_policy_propose` tool with proposal storage and notification
- [ ] 1.6 Implement `aeterna_policy_list` tool with natural language summaries
- [ ] 1.7 Implement `aeterna_policy_explain` tool (Cedar → natural language)
- [ ] 1.8 Implement `aeterna_policy_simulate` tool with scenario execution
- [ ] 1.9 Write comprehensive tests for all policy tools (>80% coverage)

## 2. Governance Administration Implementation

- [ ] 2.1 Define GovernanceConfig struct and storage schema
- [ ] 2.2 Implement `aeterna_governance_configure` tool
- [ ] 2.3 Implement `aeterna_governance_roles` tool (list, assign, revoke)
- [ ] 2.4 Implement approval workflow engine (state machine)
- [ ] 2.5 Implement `aeterna_governance_approve` tool
- [ ] 2.6 Implement `aeterna_governance_reject` tool
- [ ] 2.7 Implement `aeterna_governance_audit` tool with filtering
- [ ] 2.8 Write comprehensive tests for governance tools (>80% coverage)

## 3. Onboarding Skill Implementation

- [ ] 3.1 Define Organization, Team, Project entity schemas
- [ ] 3.2 Implement `aeterna_org_init` tool with default governance levels
- [ ] 3.3 Implement `aeterna_team_create` tool with inheritance
- [ ] 3.4 Implement `aeterna_project_init` tool with git auto-detection
- [ ] 3.5 Implement `aeterna_user_register` tool
- [ ] 3.6 Implement `aeterna_agent_register` tool with delegation chain validation
- [ ] 3.7 Implement default governance templates (standard, strict, permissive)
- [ ] 3.8 Write comprehensive tests for onboarding tools (>80% coverage)

## 4. Context Resolution Implementation

- [ ] 4.1 Define context.toml schema and parser
- [ ] 4.2 Implement context resolution priority chain
- [ ] 4.3 Implement git remote pattern matching for project detection
- [ ] 4.4 Implement git user.email detection for user identity
- [ ] 4.5 Implement environment variable context override
- [ ] 4.6 Implement `aeterna_context_resolve` tool
- [ ] 4.7 Implement `aeterna_context_set` tool
- [ ] 4.8 Implement `aeterna_context_clear` tool
- [ ] 4.9 Write tests for context resolution priority order

## 5. Memory Discovery Implementation

- [ ] 5.1 Enhance `aeterna_memory_search` with natural language interpretation
- [ ] 5.2 Implement `aeterna_memory_browse` tool with categorization
- [ ] 5.3 Implement `aeterna_memory_promote` tool with governance integration
- [ ] 5.4 Implement `aeterna_memory_attribute` tool for provenance
- [ ] 5.5 Implement auto-promotion based on reward threshold
- [ ] 5.6 Write tests for memory discovery tools (>80% coverage)

## 6. Knowledge Discovery Implementation

- [ ] 6.1 Enhance `aeterna_knowledge_search` with semantic search
- [ ] 6.2 Implement `aeterna_knowledge_browse` tool
- [ ] 6.3 Implement `aeterna_knowledge_propose` tool with NL interpretation
- [ ] 6.4 Implement `aeterna_knowledge_explain` tool
- [ ] 6.5 Integrate knowledge proposal with governance workflow
- [ ] 6.6 Write tests for knowledge discovery tools (>80% coverage)

## 7. CLI Implementation

### 7.1 Core Commands
- [ ] 7.1.1 Implement `aeterna init` command (setup wizard)
- [ ] 7.1.2 Implement `aeterna status` command
- [ ] 7.1.3 Implement `aeterna sync` command
- [ ] 7.1.4 Implement `aeterna check` command

### 7.2 Policy Commands
- [ ] 7.2.1 Implement `aeterna policy create` command (interactive + non-interactive)
- [ ] 7.2.2 Implement `aeterna policy list` command
- [ ] 7.2.3 Implement `aeterna policy explain` command
- [ ] 7.2.4 Implement `aeterna policy simulate` command
- [ ] 7.2.5 Implement `aeterna policy draft` subcommands (show, submit)

### 7.3 Governance Commands
- [ ] 7.3.1 Implement `aeterna govern configure` command
- [ ] 7.3.2 Implement `aeterna govern roles` subcommands
- [ ] 7.3.3 Implement `aeterna govern pending/approve/reject` commands
- [ ] 7.3.4 Implement `aeterna govern audit` command with export formats
- [ ] 7.3.5 Implement `aeterna govern status` command

### 7.4 Context Commands
- [ ] 7.4.1 Implement `aeterna context show` command
- [ ] 7.4.2 Implement `aeterna context set` command
- [ ] 7.4.3 Implement `aeterna context clear` command

### 7.5 Memory Commands
- [ ] 7.5.1 Implement `aeterna memory search` command
- [ ] 7.5.2 Implement `aeterna memory browse` command
- [ ] 7.5.3 Implement `aeterna memory add` command
- [ ] 7.5.4 Implement `aeterna memory promote` command
- [ ] 7.5.5 Implement `aeterna memory where` command

### 7.6 Knowledge Commands
- [ ] 7.6.1 Implement `aeterna knowledge search` command
- [ ] 7.6.2 Implement `aeterna knowledge browse` command
- [ ] 7.6.3 Implement `aeterna knowledge propose` command
- [ ] 7.6.4 Implement `aeterna knowledge explain` command

### 7.7 Organization Commands
- [ ] 7.7.1 Implement `aeterna org create` command
- [ ] 7.7.2 Implement `aeterna team create` command
- [ ] 7.7.3 Implement `aeterna project init` command
- [ ] 7.7.4 Implement `aeterna user invite` command
- [ ] 7.7.5 Implement `aeterna agent register` command
- [ ] 7.7.6 Implement `aeterna agent list` command

### 7.8 Admin Commands
- [ ] 7.8.1 Implement `aeterna admin health` command
- [ ] 7.8.2 Implement `aeterna admin validate` command
- [ ] 7.8.3 Implement `aeterna admin migrate` command
- [ ] 7.8.4 Implement `aeterna admin drift` command
- [ ] 7.8.5 Implement `aeterna admin export/import` commands

### 7.9 CLI Infrastructure
- [ ] 7.9.1 Implement shell completion generation (bash, zsh, fish)
- [ ] 7.9.2 Implement error messages with suggestions
- [ ] 7.9.3 Write CLI integration tests

## 8. Meta-Governance Implementation

- [ ] 8.1 Define meta-governance policy schema
- [ ] 8.2 Implement default meta-governance policies (who can govern)
- [ ] 8.3 Implement delegation rules for AI agents
- [ ] 8.4 Implement escalation workflow
- [ ] 8.5 Implement human confirmation gate for agent actions
- [ ] 8.6 Write tests for meta-governance scenarios

## 9. Translation Layer

- [ ] 9.1 Build translation example library (50+ examples)
- [ ] 9.2 Implement template-based translation for common patterns
- [ ] 9.3 Implement LLM fallback for complex patterns
- [ ] 9.4 Implement Cedar error → natural language translator
- [ ] 9.5 Add caching for repeated translations
- [ ] 9.6 Write translation accuracy tests

## 10. Integration & Documentation

- [ ] 10.1 Integrate all new tools with OpenCode plugin
- [ ] 10.2 Integrate all tools with MCP server
- [ ] 10.3 Write user documentation for policy creation UX
- [ ] 10.4 Write administrator documentation for governance setup
- [ ] 10.5 Write onboarding quickstart guide (5-minute setup)
- [ ] 10.6 Write agent developer guide for governance integration
- [ ] 10.7 Create example policies library with explanations
- [ ] 10.8 Write persona-based workflow guides (Developer, Tech Lead, Architect, Admin)

## 11. Testing & Validation

### 11.1 Unit Tests (>80% coverage per module)
- [ ] 11.1.1 Policy skill unit tests
- [ ] 11.1.2 Governance administration unit tests
- [ ] 11.1.3 Onboarding skill unit tests
- [ ] 11.1.4 Context resolution unit tests
- [ ] 11.1.5 Memory discovery unit tests
- [ ] 11.1.6 Knowledge discovery unit tests

### 11.2 Integration Tests
- [ ] 11.2.1 End-to-end test: Human creates policy via AI assistant
- [ ] 11.2.2 End-to-end test: Admin configures meta-governance via CLI
- [ ] 11.2.3 End-to-end test: Agent proposes policy autonomously
- [ ] 11.2.4 End-to-end test: Multi-approver workflow
- [ ] 11.2.5 End-to-end test: Audit export for compliance
- [ ] 11.2.6 End-to-end test: Project init with auto-detection
- [ ] 11.2.7 End-to-end test: Memory promotion with governance
- [ ] 11.2.8 End-to-end test: Knowledge proposal workflow

### 11.3 Performance Tests
- [ ] 11.3.1 Load test: 100 concurrent policy simulations
- [ ] 11.3.2 Load test: Context resolution under load
- [ ] 11.3.3 Load test: Memory search with large corpus

### 11.4 Security Tests
- [ ] 11.4.1 Security test: Agent cannot bypass delegation rules
- [ ] 11.4.2 Security test: Role escalation prevention
- [ ] 11.4.3 Security test: Cross-tenant isolation
- [ ] 11.4.4 Security test: Token expiration and revocation

## 12. OPAL Integration & Organizational Referential

### 12.1 PostgreSQL Referential Schema
- [ ] 12.1.1 Create companies table with slug, name, settings
- [ ] 12.1.2 Create organizations table with company_id FK
- [ ] 12.1.3 Create teams table with org_id FK
- [ ] 12.1.4 Create projects table with team_id FK and git_remote
- [ ] 12.1.5 Create users table with email, idp_subject, idp_provider
- [ ] 12.1.6 Create agents table with delegation chain and capabilities
- [ ] 12.1.7 Create memberships table (user ↔ team with role)
- [ ] 12.1.8 Create git_remote_patterns table for auto-detection
- [ ] 12.1.9 Create email_domain_patterns table for company detection
- [ ] 12.1.10 Create referential_audit_log table
- [ ] 12.1.11 Create v_hierarchy view for OPAL fetcher
- [ ] 12.1.12 Create v_user_permissions view for Cedar
- [ ] 12.1.13 Create v_agent_permissions view for Cedar
- [ ] 12.1.14 Create PostgreSQL triggers for pg_notify on changes
- [ ] 12.1.15 Write migration scripts from existing schema (if any)

### 12.2 OPAL Data Fetcher
- [ ] 12.2.1 Create opal-fetcher crate with Axum HTTP server
- [ ] 12.2.2 Implement /hierarchy endpoint returning Cedar entities
- [ ] 12.2.3 Implement /users endpoint with memberships
- [ ] 12.2.4 Implement /agents endpoint with delegation chain
- [ ] 12.2.5 Implement PostgreSQL LISTEN for real-time updates
- [ ] 12.2.6 Implement OPAL PubSub client for publishing updates
- [ ] 12.2.7 Write Dockerfile for opal-fetcher
- [ ] 12.2.8 Write tests for entity transformation (>80% coverage)

### 12.3 Cedar Schema & Policies
- [ ] 12.3.1 Define aeterna.cedarschema with Company/Org/Team/Project entities
- [ ] 12.3.2 Define User and Agent principal entities
- [ ] 12.3.3 Define actions: ViewKnowledge, EditKnowledge, ProposeKnowledge, etc.
- [ ] 12.3.4 Write rbac.cedar with role-based permit policies
- [ ] 12.3.5 Write agent-delegation.cedar with delegation constraints
- [ ] 12.3.6 Write explicit forbid policies for agent limitations
- [ ] 12.3.7 Validate Cedar policies with cedar-policy-validator
- [ ] 12.3.8 Write policy tests using Cedar test framework

### 12.4 Cedar Agent Integration
- [ ] 12.4.1 Add CedarContextResolver struct to Aeterna core
- [ ] 12.4.2 Implement resolve_user() querying Cedar Agent
- [ ] 12.4.3 Implement resolve_project() from git remote via Cedar
- [ ] 12.4.4 Implement check_authorization() for permit/forbid decisions
- [ ] 12.4.5 Implement get_accessible_layers() for layer discovery
- [ ] 12.4.6 Add circuit breaker with fallback to cached context
- [ ] 12.4.7 Update Part 8 ContextResolver to use CedarContextResolver
- [ ] 12.4.8 Write integration tests for Cedar Agent queries

### 12.5 OPAL Server Deployment
- [ ] 12.5.1 Add OPAL Server to docker-compose.yml
- [ ] 12.5.2 Configure OPAL broadcast channel (PostgreSQL)
- [ ] 12.5.3 Configure OPAL policy repository (Git)
- [ ] 12.5.4 Configure OPAL data sources (opal-fetcher endpoints)
- [ ] 12.5.5 Generate OPAL auth keys (public/private)
- [ ] 12.5.6 Add Cedar Agent to docker-compose.yml
- [ ] 12.5.7 Write Kubernetes manifests for OPAL Server (HA)
- [ ] 12.5.8 Write Kubernetes DaemonSet for Cedar Agent
- [ ] 12.5.9 Write Helm chart values for OPAL stack

### 12.6 IdP Synchronization
- [ ] 12.6.1 Create idp-sync crate with scheduled job
- [ ] 12.6.2 Implement OktaClient for user/group fetching
- [ ] 12.6.3 Implement AzureAdClient via Microsoft Graph
- [ ] 12.6.4 Implement user sync logic (create/update/deactivate)
- [ ] 12.6.5 Implement group → team membership mapping
- [ ] 12.6.6 Implement webhook receiver for real-time IdP events
- [ ] 12.6.7 Write tests for IdP sync logic (>80% coverage)

### 12.7 Migration & Compatibility
- [ ] 12.7.1 Implement parallel context resolution (heuristic + Cedar)
- [ ] 12.7.2 Add context resolution comparison logging
- [ ] 12.7.3 Implement feature flag for Cedar Agent switchover
- [ ] 12.7.4 Implement audit mode (log-only) for Cedar authorization
- [ ] 12.7.5 Document migration runbook for existing deployments
- [ ] 12.7.6 Write migration scripts for existing organizational data

### 12.8 OPAL Integration Tests
- [ ] 12.8.1 E2E test: User context resolution via Cedar Agent
- [ ] 12.8.2 E2E test: Project detection from git remote
- [ ] 12.8.3 E2E test: Authorization permit/deny scenarios
- [ ] 12.8.4 E2E test: Agent delegation chain validation
- [ ] 12.8.5 E2E test: Real-time data sync (PostgreSQL → OPAL → Cedar)
- [ ] 12.8.6 E2E test: IdP sync creates users and memberships
- [ ] 12.8.7 E2E test: Circuit breaker fallback on Cedar Agent failure
- [ ] 12.8.8 Load test: 1000 concurrent authorization requests
