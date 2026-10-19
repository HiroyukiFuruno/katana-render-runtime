# Supplemental HTML diagram consumer fixtures

These fixtures embed independent SVG bytes so the same HTML source can be compared through Chrome and the public KRR HTML runtime. They sit outside `tests/fixtures/html/aspects/` and are not catalog cases; the existing 84 HTML conditions remain unchanged.

## Mermaid class diagram

- Fixture: `mermaid-class-diagram.html`
- Public Markdown source: `tests/fixtures/mermaid/ja/03-02-class-diagram.md`
- Accepted independent SVG: `tmp/quality/mermaid-current-owned-raw117-r9-execution-r1/raw117/ja/output/03-02-class-diagram.svg`
- Accepted reader evidence: `tmp/quality/mermaid-current-owned-raw117-r9-execution-r1/strict-official-reader-after-radii-deps-r1/result.json` (exit 0); pair evidence binds source/rendered SVG SHA-256 and records exact equality.
- Embedded SVG SHA-256: `6fa7e568294faed8a36fb767c2ec96c7d6db408bb0f23c30e15c48ebecdcfaef`
- License: KRR repository MIT license.
- Reproduction route: `just mermaid-reference-strict tests/fixtures/mermaid/ja/03-02-class-diagram.md <fresh-output-dir>` (strict vendor-original/formal mode). The generated KRR renderer SVG is not the expected reference.

## Draw.io class diagram

- Fixture: `drawio-class-diagram.html`
- Upstream source: `tests/fixtures/drawio/official/templates/software/class_1.drawio`
- Independent official SVG: `tests/fixtures/drawio/official/templates/software/official/class_1.svg`
- Upstream: `jgraph/drawio-diagrams` commit `7b70ed6f96738fed4044a25a314733f8544f076e`.
- Embedded SVG SHA-256: `03c3dd46a29706011faae890098ef38ff4310618d7527dac7ada3dd35df40255`
- License: Apache-2.0; see `tests/fixtures/drawio/official/LICENSE.apache-2.0.txt`.
- Reproduction route: `just drawio-reference tests/fixtures/drawio/official/templates/software/class_1.drawio` with the managed Draw.io runtime and resource manifest. The KRR renderer SVG is not the expected reference.
