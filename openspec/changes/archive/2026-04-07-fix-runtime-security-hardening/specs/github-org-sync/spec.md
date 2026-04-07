## ADDED Requirements

### Requirement: Verified Sync Webhook Boundary
The system SHALL verify webhook authenticity before processing sync-triggering GitHub organization events.

#### Scenario: Team or membership event without valid signature
- **WHEN** a `team`, `membership`, `member`, or `organization` webhook arrives without the required valid signature
- **THEN** the system SHALL reject the event before any incremental sync or tenant mutation work begins
- **AND** no sync job SHALL be scheduled from that event
