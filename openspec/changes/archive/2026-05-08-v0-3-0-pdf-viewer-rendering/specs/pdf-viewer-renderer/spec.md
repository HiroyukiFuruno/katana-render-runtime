## ADDED Requirements

### Requirement: PDF viewer rendering is migrated to KDV

KDR SHALL NOT implement PDF viewer rendering after the KDV responsibility boundary is adopted.

#### Scenario: PDF viewer work starts

- **WHEN** PDF viewer rendering work is planned
- **THEN** the implementation belongs to KDV
- **THEN** KDR does not add PDF viewer APIs, fixtures, or CLI entrypoints
