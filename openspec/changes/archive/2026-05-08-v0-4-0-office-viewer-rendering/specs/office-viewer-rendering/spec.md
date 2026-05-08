## ADDED Requirements

### Requirement: Office viewer rendering is migrated to KDV

KCF SHALL NOT implement Word, Excel, or PPTX viewer rendering after the KDV responsibility boundary is adopted.

#### Scenario: Office viewer work starts

- **WHEN** Word, Excel, or PPTX viewer rendering work is planned
- **THEN** the implementation belongs to KDV
- **THEN** KCF does not add Office viewer APIs, fixtures, or CLI entrypoints
