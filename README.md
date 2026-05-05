<p align="center">
  <img src="assets/kcf-icon.png" width="128" alt="katana-canvas-forge icon">
</p>

<h1 align="center">katana-canvas-forge</h1>

<p align="center">
  A Rust rendering core and <code>kcf</code> CLI for Mermaid, Draw.io, and
  document export workflows.
</p>

<p align="center">
  <strong><a href="#installation">Installation</a></strong> |
  <strong><a href="#cli-usage">CLI Usage</a></strong> |
  <strong><a href="#library-api">Library API</a></strong> |
  <strong><a href="#layout">Layout</a></strong> |
  <strong><a href="docs/release.md">Release</a></strong>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://github.com/HiroyukiFuruno/katana-canvas-forge/actions/workflows/ci.yml"><img src="https://github.com/HiroyukiFuruno/katana-canvas-forge/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/HiroyukiFuruno/katana-canvas-forge/releases/latest"><img src="https://img.shields.io/github/v/release/HiroyukiFuruno/katana-canvas-forge" alt="Latest Release"></a>
  <a href="https://crates.io/crates/katana-canvas-forge"><img src="https://img.shields.io/crates/v/katana-canvas-forge.svg" alt="crates.io"></a>
  <a href="https://docs.rs/katana-canvas-forge"><img src="https://docs.rs/katana-canvas-forge/badge.svg" alt="docs.rs"></a>
  <img src="https://img.shields.io/badge/cli-kcf-2563EB" alt="CLI: kcf">
</p>

---

## What is kcf

`katana-canvas-forge` provides the portable rendering layer extracted from
[KatanA](https://github.com/HiroyukiFuruno/KatanA). It keeps diagram rendering,
document export, reference generation, and score comparison in a standalone Rust
crate so downstream applications can integrate the same behavior without
depending on KatanA Desktop internals.

The project is intentionally narrow: it owns rendering and export behavior, not
the editor, preview UI, workspace state, or chat surface.

## Features

- **Mermaid rendering** through the official Mermaid JavaScript runtime.
- **Draw.io rendering** through transferred KatanA-compatible runtime logic.
- **HTML / PDF / PNG / JPEG export** from rendered Markdown-derived output.
- **Reference snapshots** for committed Mermaid and Draw.io fixtures.
- **Image scoring** against official renderer output for regression tracking.
- **`kcf` CLI** for render, export, reference update, comparison, and benchmark
  workflows.

## Installation

Use the library from Rust:

```bash
cargo add katana-canvas-forge
```

Install the CLI:

```bash
cargo install katana-canvas-forge-cli
```

The installed binary is `kcf`.

## CLI Usage

Render diagrams:

```bash
kcf mermaid render input.md output.svg
kcf drawio render diagram.drawio output.svg
```

Export Markdown-derived output:

```bash
kcf export html input.md output.html
kcf export pdf input.md output.pdf
kcf export png input.md output.png
kcf export jpg input.md output.jpg
```

Run reference comparison:

```bash
kcf reference mermaid-compare
kcf reference drawio-compare
```

## Library API

Embed `katana-canvas-forge` when an application needs diagram rendering or
document export in-process.

Primary integration points:

- `RenderInput`
- `RenderOutput`
- `DiagramKind`
- Mermaid renderer
- Draw.io renderer
- HTML / PDF / PNG / JPEG exporters

The API keeps KatanA integration needs in mind, but the crate remains standalone.
Consumers should treat KatanA UI state, editor state, and workspace navigation as
their own responsibilities.

## Non-Goals

- Markdown parsing, preview UI, editor UI, theme state, or any KatanA UI
  concern. This crate must not depend on `egui`, KatanA preview widgets,
  or KatanA UI state.
- Viewer rendering for CSV / PDF / Office files. These are planned as later
  kcf changes and are separate from v0.1.0 export support.
- LLM chat UI / agent protocols. See
  [`katana-chat-ui`](https://github.com/HiroyukiFuruno/katana-chat-ui).

## Layout

```
crates/
  katana-canvas-forge/         # library
  katana-canvas-forge-cli/     # `kcf` CLI binary
scripts/
  mermaid/                     # official reference generation and scoring
  drawio/                      # official reference generation and scoring
tests/fixtures/
  mermaid/                     # Mermaid input and committed reference images
  drawio/                      # Draw.io input and committed reference images
docs/                          # release and coding notes
```

## License

MIT — see [LICENSE](LICENSE).
