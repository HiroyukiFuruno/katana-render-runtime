## ADDED Requirements

### Requirement: CSV diagnostics are owned by KDV

CSV viewer diagnostics SHALL be defined by KDV instead of KCF.

#### Scenario: CSV parse or display diagnostics are needed

- **WHEN** CSV parse errors, encoding warnings, delimiter detection, or truncation metadata are needed for viewer display
- **THEN** KDV defines the public diagnostics
- **THEN** KCF does not create substitute diagnostics
