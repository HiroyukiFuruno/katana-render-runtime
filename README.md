<h1 align="center">katana-canvas-forge</h1>

<p align="center">
  <code>kcf</code> — versioned diagram rendering and document export runtime for
  <a href="https://github.com/HiroyukiFuruno/KatanA">KatanA</a>.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/status-scaffolding-orange" alt="Status: scaffolding">
</p>

---

## Status

Scaffolding. The runtime interface, Mermaid.js pinned bundle management,
Draw.io rendering, and HTML / PDF / PNG / JPEG export responsibilities are
being migrated from [KatanA](https://github.com/HiroyukiFuruno/KatanA)
during the `v0.22.11` change
(`openspec/changes/v0-22-11-renderer-runtime-interface-and-versioning`).

## Scope

- Mermaid rendering through a Rust-managed JavaScript runtime, with the
  official `mermaid.min.js` bundle pinned by version + checksum.
- Draw.io rendering as a sibling backend behind the same renderer interface
  where compatible, otherwise as a separately documented backend.
- HTML / PDF / PNG / JPEG export from rendered output.
- Reference-image generation and scoring against the upstream Mermaid.js
  reference renderer.
- A library API consumed by KatanA and a `kcf` CLI for single-shot render,
  reference update, comparison, and benchmarking.

## Non-Scope

- Markdown parsing, preview UI, editor UI, theme state, or any KatanA UI
  concern. This crate must not depend on `egui`, KatanA preview widgets,
  or KatanA UI state.
- Document viewers (PDF / CSV / Office) — those remain in KatanA.
- LLM chat UI / agent protocols — see
  [`katana-chat-ui`](https://github.com/HiroyukiFuruno/katana-chat-ui).

## Layout

```
crates/
  katana-canvas-forge/         # library
  katana-canvas-forge-cli/     # `kcf` CLI binary
vendor/
  mermaid/<version>/           # pinned Mermaid.js bundle + checksum
docs/                          # design and migration notes
```

## License

MIT — see [LICENSE](LICENSE).
