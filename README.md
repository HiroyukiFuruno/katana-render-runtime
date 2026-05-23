<p align="center">
  <img src="assets/krr-icon.png" width="128" alt="katana-render-runtime icon">
</p>

<h1 align="center">katana-render-runtime</h1>

<p align="center">
  A Rust runtime for rendering diagram and TeX source strings into SVG.
</p>

<p align="center">
  <strong><a href="#installation">Installation</a></strong> |
  <strong><a href="#library-api">Library API</a></strong> |
  <strong><a href="#cli">CLI</a></strong> |
  <strong><a href="#migration">Migration</a></strong> |
  <strong><a href="#supported-rendering">Supported Rendering</a></strong> |
  <strong><a href="docs/release.md">Release</a></strong>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://github.com/HiroyukiFuruno/katana-render-runtime/actions/workflows/test-and-build.yml"><img src="https://github.com/HiroyukiFuruno/katana-render-runtime/actions/workflows/test-and-build.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/HiroyukiFuruno/katana-render-runtime/releases/latest"><img src="https://img.shields.io/github/v/release/HiroyukiFuruno/katana-render-runtime" alt="Latest Release"></a>
  <a href="https://crates.io/crates/katana-render-runtime"><img src="https://img.shields.io/crates/v/katana-render-runtime.svg" alt="crates.io"></a>
  <a href="https://docs.rs/katana-render-runtime"><img src="https://docs.rs/katana-render-runtime/badge.svg" alt="docs.rs"></a>
  <img src="https://img.shields.io/badge/cli-krr-2563EB" alt="CLI: krr">
</p>

---

## Overview

`katana-render-runtime` is a Katana-series Rust library for SVG rendering.

It accepts a `RenderInput` made from a `RenderKind`, source string, render config, render policy, and render context. It returns a `RenderOutput` containing SVG, dimensions, runtime metadata, diagnostics, and a cache fingerprint.

Upstream applications own Markdown AST parsing, preview UI state, and HTML / PDF / PNG / JPG export surfaces. This crate receives normalized source strings and renders SVG output.

## Features

- Render Mermaid / Draw.io / ZenUML / PlantUML diagrams into SVG.
- Render TeX input into SVG output through MathJax v4.
- Use one `RenderInput` / `RenderOutput` contract across every renderer.
- Apply theme and dark-mode data through `RenderThemeSnapshot`.
- Return SVG dimensions, `viewBox`, runtime version data, diagnostics, and cache fingerprints.
- Resolve packaged runtime assets for the supported renderers.

## Installation

Install the latest published crate:

```bash
cargo add katana-render-runtime
```

Existing consumers that still need the compatibility wrapper can use:

```bash
cargo add katana-diagram-renderer
```

## CLI

Install the `krr` CLI:

```bash
cargo install katana-render-runtime-cli
```

Common usage examples:

```bash
# Render a Mermaid file to SVG
krr mermaid render --input examples/sample.mmd --output out.svg

# Render a PlantUML file with a theme
krr plantuml render --input diagram.puml --output out.svg --theme cyborg --theme-mode light

# Render without `--output` and print SVG to stdout
krr drawio render --input diagram.drawio

# Show supported subcommands and options
krr --help
```

Reference and quality-check commands:

```bash
# Compare generated output to fixtures
krr mermaid compare --fixtures tests/fixtures/mermaid --min-score 99.0

# Update fixtures
krr drawio reference-update --fixtures tests/fixtures/drawio
```

## Library API

Primary entry points:

- `RenderInput`
- `RenderOutput`
- `RenderKind`
- `MermaidRenderer`
- `DrawioRenderer`
- `PlantUmlRenderer`
- `MathJaxRenderer`

`RenderInput.source` must already contain the diagram or TeX source to render. The renderer returns SVG through `RenderOutput.svg`; related metadata is carried in the same output value.

## Migration

`katana-diagram-renderer` is a compatibility wrapper from v0.3.0. New code should depend on `katana-render-runtime`.

```toml
[dependencies]
katana-render-runtime = "0.3"
```

Existing consumers can temporarily keep the wrapper dependency:

```toml
[dependencies]
katana-diagram-renderer = "0.3"
```

## Supported Rendering

| `RenderKind` | Input | Output |
| --- | --- | --- |
| `Mermaid` | Mermaid source, including ZenUML diagrams | SVG |
| `Drawio` | Draw.io diagram source | SVG |
| `PlantUml` | PlantUML source | SVG |
| `MathJax` | TeX source | SVG |

Each renderer exposes the same `Renderer` trait, so rendering code can switch renderer implementations without changing the output contract.

## Non-Goals

- Markdown AST parsing.
- HTML / PDF / PNG / JPG export surface generation.
- Preview UI, editor UI, or KatanA UI state ownership.
- KDV pagination or document export.

## Layout

```text
crates/
  katana-render-runtime/          # Rendering runtime library
  katana-diagram-renderer/        # Compatibility wrapper
  katana-render-runtime-cli/      # krr CLI binary
scripts/
  mermaid/                        # Mermaid reference rendering and scoring
  drawio/                         # Draw.io reference rendering and scoring
  runtime-assets/                 # runtime asset latest / update helpers
tests/fixtures/
  mermaid/
  drawio/
  plantuml/
docs/
```

## License

MIT - see [LICENSE](LICENSE).
