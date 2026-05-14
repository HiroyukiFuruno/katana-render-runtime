//! katana-diagram-renderer: versioned diagram rendering runtime.
//!
//! This crate owns Mermaid / Draw.io / ZenUML rendering responsibilities
//! extracted from KatanA. KatanA consumes this crate as a library through the
//! renderer runtime interface defined here.
//!
//! The crate deliberately excludes document export and viewer ownership.

pub mod markdown;
pub mod renderer;

pub use renderer::{
    DiagramKind, DrawioRenderer, MermaidRenderer, RenderConfig, RenderContext, RenderDiagnostics,
    RenderError, RenderInput, RenderOutput, RenderPolicy, RenderThemeMode, RenderThemeSnapshot,
    Renderer, RendererProfile, RuntimePathResolver, RuntimeVersion,
};
