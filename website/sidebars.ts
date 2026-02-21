import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'intro',
    {
      type: 'category',
      label: 'Getting Started',
      collapsed: false,
      items: [
        'helm/quickstart-local',
        'helm/quickstart-hybrid',
        'helm/quickstart-remote',
        'guides/cli-quick-reference',
      ],
    },
    {
      type: 'category',
      label: 'Concepts',
      items: [
        'specs/overview',
        'specs/core-concepts',
        'architecture-overview',
        'sequence-diagrams',
        'comprehensive-ux-dx-guide',
      ],
    },
    {
      type: 'category',
      label: 'Memory System',
      items: ['specs/memory-system'],
    },
    {
      type: 'category',
      label: 'Knowledge Repository',
      items: [
        'specs/knowledge-repository',
        'specs/memory-knowledge-sync',
      ],
    },
    {
      type: 'category',
      label: 'Governance',
      items: [
        'governance/policy-model',
        'governance/api-reference',
        'governance/deployment-guide',
        'governance/drift-tuning',
        'governance/troubleshooting',
        'guides/ux-first-governance',
        'guides/agent-governance-integration',
      ],
    },
    {
      type: 'category',
      label: 'CCA — Confucius Code Agent',
      items: [
        'cca/overview',
        'cca/architecture',
        'cca/configuration',
        'cca/api-reference',
        'cca/extension-guide',
        'cca/redis-schema',
        'cca/migrations',
      ],
    },
    {
      type: 'category',
      label: 'Integrations',
      items: [
        'integrations/mcp-server',
        'integrations/opencode-integration',
        'guides/provider-adapters',
        'codesearch-integration',
        'codesearch-repository-management',
      ],
    },
    {
      type: 'category',
      label: 'Security',
      items: [
        'security/rbac-matrix',
        'security/rbac-testing-procedures',
        'security/tenant-isolation-testing',
        'guides/gdpr-procedures',
      ],
    },
    {
      type: 'category',
      label: 'Helm Deployment',
      items: [
        'helm/architecture',
        'helm/local-mode',
        'helm/hybrid-mode',
        'helm/ha-requirements',
        'helm/sizing-guide',
        'helm/production-checklist',
        'helm/security',
        'helm/external-secrets',
        'helm/sops-secrets',
        'helm/upgrade',
        'helm/cnpg-upgrade',
        'helm/restore',
        'helm/logging',
        'helm/tracing',
        'helm/chart-versioning',
      ],
    },
    {
      type: 'category',
      label: 'Operations',
      items: [
        'guides/ha-deployment',
        'guides/disaster-recovery-runbook',
        'guides/observability-runbook',
        'guides/managed-observability',
        'guides/cost-optimization',
      ],
    },
    {
      type: 'category',
      label: 'Examples',
      items: [
        'examples/strangler-fig-migration',
        'examples/policies/README',
        'examples/policies/security-baseline',
        'examples/policies/code-quality',
        'examples/policies/architecture-constraints',
        'examples/policies/dependency-management',
        'examples/policies/team-conventions',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: [
        'specs/tool-interface',
        'specs/configuration',
        'specs/adapter-architecture',
        'specs/migration',
      ],
    },
  ],
};

export default sidebars;
