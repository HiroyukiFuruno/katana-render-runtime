//! katana-diagram-renderer: versioned diagram rendering runtime.
//!
//! This crate owns Mermaid / Draw.io / ZenUML / PlantUML rendering responsibilities
//! extracted from KatanA. KatanA consumes this crate as a library through the
//! renderer runtime interface defined here.
//!
//! The crate deliberately excludes document export and viewer ownership.

pub mod markdown;
pub mod renderer;

pub use markdown::plantuml_renderer::{
    PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION, PlantUmlThemeCatalog,
};
pub use renderer::{
    DiagramKind, DrawioRenderer, MermaidRenderer, PlantUmlRenderer, RenderConfig, RenderContext,
    RenderDiagnostics, RenderError, RenderInput, RenderOutput, RenderPolicy, RenderThemeMode,
    RenderThemeSnapshot, Renderer, RendererProfile, RuntimePathResolver, RuntimeVersion,
};
