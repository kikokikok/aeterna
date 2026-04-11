## ADDED Requirements

### Requirement: Human OpenCode Usage Playbook
The system SHALL document a practical daily usage model for humans working in OpenCode with the Aeterna plugin.

The guidance SHALL distinguish between retrieval, capture, and promotion workflows and SHALL explain when users should rely on automatic context injection versus explicitly asking the AI to use Aeterna tools.

#### Scenario: User learns the daily workflow
- **WHEN** a human user reads the OpenCode usage guide
- **THEN** the guide SHALL explain how to start by retrieving relevant context
- **AND** the guide SHALL explain when to add memory explicitly
- **AND** the guide SHALL explain when to promote stable insights into knowledge

#### Scenario: User distinguishes memory from knowledge
- **WHEN** a human user consults the playbook for what to store
- **THEN** the guide SHALL explain that memory is for practical retained context
- **AND** the guide SHALL explain that knowledge is for governed, reusable truth
- **AND** the guide SHALL include examples of good and bad usage for both

#### Scenario: User understands hierarchy and scope
- **WHEN** a human user needs to choose where information belongs
- **THEN** the guide SHALL describe the 7 memory layers
- **AND** the guide SHALL describe the knowledge types and scopes
- **AND** the guide SHALL give practical examples of which layer or scope to choose
