# Mermaid structural regression controls

These SVGs are byte-for-byte copies of the corresponding current
`official-dark` worktree files. Their known contract is 4 boxes, 5 members, 6
methods, and 3 hollow inheritance edges. They exist only as deterministic
structural controls for the class-reference validator and gate unit tests.

Their raw vendor provenance has not been verified. They must not be used as
official reference evidence, score goldens, or a recipe for generating
accepted SVGs. Their hashes identify the exact bytes used by those tests:

- `ja/03-02-class-diagram.svg` — SHA-256
  `6fa7e568294faed8a36fb767c2ec96c7d6db408bb0f23c30e15c48ebecdcfaef`
- `en/03-02-class-diagram-inheritance.svg` — SHA-256
  `26e76d3200d980c8577b45f0a0f2fe149dcb2329d8f4f23da246a149123451af`

The copies preserve existing structural expectations while keeping these unit
tests independent of mutable official-reference files. Official files and score
generation references remain unchanged.
