//! katana-canvas-forge: versioned diagram rendering and document export runtime.
//!
//! This crate owns Mermaid / Draw.io rendering and HTML / PDF / PNG / JPEG export
//! responsibilities extracted from KatanA. KatanA consumes this crate as a
//! library through the renderer runtime interface defined here.
//!
//! Status: v0.1.0 transfers the KatanA Mermaid / Draw.io / export runtime into
//! this independent crate.

pub mod exporter;
pub mod markdown;
pub mod renderer;
mod system;

pub use renderer::{
    DiagramKind, DrawioRenderer, MermaidRenderer, RenderConfig, RenderContext, RenderDiagnostics,
    RenderError, RenderInput, RenderOutput, RenderPolicy, RenderThemeMode, RenderThemeSnapshot,
    Renderer, RendererProfile, RuntimePathResolver, RuntimeVersion,
};
