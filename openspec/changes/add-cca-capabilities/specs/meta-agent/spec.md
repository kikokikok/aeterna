## ADDED Requirements

### Requirement: Build Phase
The system SHALL generate code implementations using patterns from note-taking agent and hindsight learning.

#### Scenario: Generate code with context
- **WHEN** build phase is initiated with requirements
- **THEN** system SHALL query note-taking agent for relevant patterns
- **AND** system SHALL query hindsight learning for error avoidance
- **AND** system SHALL assemble context using Context Architect
- **AND** system SHALL invoke LLM with assembled context to generate code

#### Scenario: Apply pattern guidance
- **WHEN** generating code
- **AND** matching patterns exist in notes
- **THEN** system SHALL include pattern examples in generation prompt
- **AND** system SHALL instruct LLM to follow established patterns
- **AND** system SHALL log applied patterns for traceability

#### Scenario: Apply hindsight avoidance
- **WHEN** generating code
- **AND** relevant hindsight notes exist
- **THEN** system SHALL include known error patterns to avoid
- **AND** system SHALL include recommended code structures
- **AND** system SHALL log hindsight guidance applied

### Requirement: Test Phase
The system SHALL execute test suites and capture structured output for analysis.

#### Scenario: Run test suite
- **WHEN** build phase completes with generated code
- **THEN** system SHALL execute configured test command
- **AND** system SHALL capture stdout, stderr, and exit code
- **AND** system SHALL parse test output into structured results

#### Scenario: Parse test results
- **WHEN** test execution completes
- **THEN** system SHALL extract: total tests, passed, failed, skipped
- **AND** system SHALL extract failure details (test name, error message, stack trace)
- **AND** system SHALL compute pass_rate = passed / total

#### Scenario: Handle test timeout
- **WHEN** test execution exceeds configured timeout
- **THEN** system SHALL terminate test process
- **AND** system SHALL record timeout as test failure
- **AND** system SHALL include timeout duration in results

### Requirement: Improve Phase
The system SHALL analyze test failures and generate fixes using hindsight knowledge.

#### Scenario: Analyze test failure
- **WHEN** test phase reports failures
- **THEN** system SHALL extract error signatures from failure details
- **AND** system SHALL query hindsight learning for matching resolutions
- **AND** system SHALL generate fix suggestions ranked by confidence

#### Scenario: Apply resolution fix
- **WHEN** resolution suggestions are available
- **THEN** system SHALL select highest-confidence applicable resolution
- **AND** system SHALL generate code changes based on resolution
- **AND** system SHALL log resolution application attempt

#### Scenario: Generate novel fix
- **WHEN** no matching resolutions found
- **THEN** system SHALL invoke LLM with failure context to generate fix
- **AND** system SHALL include error details and code context
- **AND** system SHALL flag as novel fix for potential hindsight capture

### Requirement: Iteration Control
The system SHALL limit improvement iterations to prevent infinite loops.

#### Scenario: Track iteration count
- **WHEN** improve phase generates fix
- **THEN** system SHALL increment iteration counter
- **AND** system SHALL check against max_iterations (default: 3)
- **AND** system SHALL continue to test phase if under limit

#### Scenario: Enforce iteration limit
- **WHEN** iteration count reaches max_iterations
- **AND** tests still fail
- **THEN** system SHALL halt improvement loop
- **AND** system SHALL transition to escalation phase
- **AND** system SHALL NOT attempt further fixes

### Requirement: Quality Gates
The system SHALL enforce quality gates before allowing commit.

#### Scenario: All tests pass gate
- **WHEN** test phase shows 100% pass rate
- **THEN** system SHALL mark quality gate: tests_pass = true
- **AND** system SHALL proceed to commit readiness check

#### Scenario: Linter pass gate
- **WHEN** linter is configured
- **THEN** system SHALL run linter on generated code
- **AND** system SHALL mark quality gate: linter_pass = true/false
- **AND** system SHALL block commit if linter fails

#### Scenario: Coverage gate
- **WHEN** coverage threshold is configured
- **THEN** system SHALL check coverage meets threshold
- **AND** system SHALL mark quality gate: coverage_pass = true/false
- **AND** system SHALL block commit if coverage insufficient

#### Scenario: All gates must pass
- **WHEN** checking commit readiness
- **THEN** system SHALL require all configured gates to pass
- **AND** system SHALL provide gate status summary
- **AND** system SHALL block commit if any gate fails

### Requirement: Escalation
The system SHALL escalate to user after exhausting improvement attempts.

#### Scenario: Escalate after max iterations
- **WHEN** max iterations reached without success
- **THEN** system SHALL generate escalation report with:
- **AND** attempted fixes and their outcomes
- **AND** remaining test failures with analysis
- **AND** suggested manual investigation areas

#### Scenario: Capture failure for hindsight
- **WHEN** escalation occurs
- **THEN** system SHALL capture error signatures from final failures
- **AND** system SHALL store as unresolved error patterns
- **AND** system SHALL tag with escalation context for future learning

#### Scenario: Notify user
- **WHEN** escalation is triggered
- **THEN** system SHALL format user-friendly message
- **AND** system SHALL include actionable next steps
- **AND** system SHALL offer to explain any attempted fix

### Requirement: Loop State Management
The system SHALL maintain state across build-test-improve iterations.

#### Scenario: Initialize loop state
- **WHEN** meta-agent loop starts
- **THEN** system SHALL create LoopState with: iteration=0, history=[], gates={}
- **AND** system SHALL persist state for recovery
- **AND** system SHALL set session_id for state isolation

#### Scenario: Record iteration history
- **WHEN** iteration completes
- **THEN** system SHALL append to history: phase, duration, outcome, changes
- **AND** system SHALL include diff of code changes per iteration
- **AND** system SHALL maintain history for debugging

#### Scenario: Recover from interruption
- **WHEN** loop is interrupted (crash, timeout)
- **AND** persisted state exists
- **THEN** system SHALL offer to resume from last iteration
- **AND** system SHALL restore full history and gates
- **AND** system SHALL log recovery event

### Requirement: Commit Generation
The system SHALL generate commits with meaningful messages when quality gates pass.

#### Scenario: Generate commit message
- **WHEN** all quality gates pass
- **THEN** system SHALL analyze changes made across iterations
- **AND** system SHALL generate concise commit message
- **AND** system SHALL include reference to requirements/task

#### Scenario: Include iteration metadata
- **WHEN** generating commit
- **THEN** system SHALL include metadata: iterations_used, patterns_applied, hindsight_used
- **AND** system SHALL include gate results summary
- **AND** system MAY include in commit body or as trailer

### Requirement: Observability
The system SHALL emit metrics and logs for meta-agent loop operations.

#### Scenario: Emit loop metrics
- **WHEN** loop completes (success or escalation)
- **THEN** system SHALL emit histogram: meta_agent.loop.iterations
- **AND** system SHALL emit histogram: meta_agent.loop.duration_ms
- **AND** system SHALL emit counter: meta_agent.loop.total with labels (outcome)

#### Scenario: Emit phase metrics
- **WHEN** phase completes
- **THEN** system SHALL emit histogram: meta_agent.phase.duration_ms with labels (phase)
- **AND** system SHALL emit counter: meta_agent.phase.total with labels (phase, outcome)

#### Scenario: Emit gate metrics
- **WHEN** quality gate is evaluated
- **THEN** system SHALL emit counter: meta_agent.gate.evaluations with labels (gate, result)
- **AND** system SHALL emit histogram: meta_agent.gate.latency_ms with labels (gate)
