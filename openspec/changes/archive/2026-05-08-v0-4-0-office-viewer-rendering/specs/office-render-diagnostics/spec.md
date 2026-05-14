## ADDED Requirements

### Requirement: Office viewer diagnostics are owned by KDV

Office viewer diagnostics SHALL be defined by KDV instead of KDR.

#### Scenario: Office diagnostics are needed

- **WHEN** unsupported format, broken file, password protected file, macro enabled file, or rendering warning diagnostics are needed for viewer display
- **THEN** KDV defines the public diagnostics
- **THEN** KDR does not create substitute diagnostics
