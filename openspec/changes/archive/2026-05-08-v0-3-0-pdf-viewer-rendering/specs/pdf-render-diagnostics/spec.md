## ADDED Requirements

### Requirement: PDF viewer diagnostics are owned by KDV

PDF viewer diagnostics SHALL be defined by KDV instead of KDR.

#### Scenario: PDF display diagnostics are needed

- **WHEN** backend missing, invalid PDF, password required, or unsupported feature diagnostics are needed for viewer display
- **THEN** KDV defines the public diagnostics
- **THEN** KDR does not create substitute diagnostics
