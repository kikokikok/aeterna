## 1. Runtime correctness
- [ ] 1.1 Correct the shipped container entrypoint so the default image executes a supported runtime command
- [ ] 1.2 Correct the Helm migration job to invoke the supported migration command and flags
- [ ] 1.3 Add tests that execute the exact shipped entrypoint and migration commands

## 2. Runtime operations behavior
- [ ] 2.1 Replace placeholder CLI behavior in memory, sync, check, and codesearch-related commands with real backend-backed execution or explicit unsupported errors
- [ ] 2.2 Replace synthetic or static health/metrics responses in agent-a2a and related services with dependency-aware checks
- [ ] 2.3 Implement or explicitly remove incomplete JWT/auth paths so production auth behavior is fail-closed
- [ ] 2.4 Implement real thread/session persistence or mark unsupported runtime paths as unavailable instead of returning stub success

## 3. Helm and deployment hardening
- [ ] 3.1 Ensure generated secrets are reused across upgrades or externally managed without rotation surprises
- [ ] 3.2 Correct dependency wiring for PostgreSQL, cache, CodeSearch, OPAL, and ingress/TLS defaults
- [ ] 3.3 Correct example values files and validation templates so documented production examples render to the intended topology
- [ ] 3.4 Align network policy, backup hooks, and dependency version pinning with documented production behavior

## 4. CI and release validation
- [ ] 4.1 Add CI coverage gates that enforce the documented minimum thresholds
- [ ] 4.2 Add smoke tests for Helm template/install command correctness and shipped runtime commands
- [ ] 4.3 Update image/build workflows so all referenced deployable images are either built and published or removed from supported deployment paths

## 5. Docs and operator guidance
- [ ] 5.1 Reconcile README, INSTALL, deployment docs, and examples with the actual supported deployment/runtime paths
- [ ] 5.2 Document secret management, ingress TLS, migration, and upgrade procedures for the supported Helm path
- [ ] 5.3 Remove or clearly deprecate divergent deployment paths that are not supported in production
