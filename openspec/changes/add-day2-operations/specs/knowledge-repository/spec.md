## MODIFIED Requirements

### Requirement: Knowledge Item Deletion Cascade
The system SHALL ensure that deleting a knowledge item cascades to all dependent records.

#### Scenario: Delete cascades to promotion requests
- **WHEN** a knowledge item is deleted
- **THEN** the system SHALL delete all promotion requests referencing the item as source or target

#### Scenario: Delete cascades to knowledge relations
- **WHEN** a knowledge item is deleted
- **THEN** the system SHALL delete all knowledge relations where source_id or target_id matches the deleted item

## ADDED Requirements

### Requirement: Promotion Request Garbage Collection
The system SHALL clean up stale and orphaned promotion requests.

#### Scenario: Rejected promotion cleanup
- **WHEN** a promotion request has been in rejected or abandoned status for more than 30 days
- **THEN** the system SHALL delete the promotion request and its associated proposal file

#### Scenario: Orphaned promotion detection
- **WHEN** the reconciliation job finds promotion requests referencing source memories that no longer exist
- **THEN** the system SHALL create a RequireApproval remediation request to delete the orphaned promotions
