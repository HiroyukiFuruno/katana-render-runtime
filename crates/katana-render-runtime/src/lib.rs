//! katana-render-runtime: versioned render runtime.
//!
//! This crate owns Mermaid / Draw.io / ZenUML / PlantUML / MathJax / HTML rendering
//! responsibilities extracted from KatanA. KatanA consumes this crate as a
//! library through the renderer runtime interface defined here.
//!
//! The crate deliberately excludes document export and viewer ownership.
//! It receives already-classified input strings and does not parse Markdown ASTs.
//! Static HTML document parsing and CSS resolution remain export-only renderer
//! internals. Interactive viewers use the in-process Rust/V8 browser-session API.

pub mod markdown;
pub mod renderer;
mod system;

pub use markdown::plantuml_renderer::{
    PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION, PlantUmlThemeCatalog,
};
pub use renderer::{
    DiagramKind, DrawioRenderer, HTML_BROWSER_MAX_SOURCE_BYTES, HtmlBrowserError, HtmlBrowserFrame,
    HtmlBrowserInput, HtmlBrowserNavigation, HtmlBrowserNavigationEvent, HtmlBrowserOrigin,
    HtmlBrowserPixelFormat, HtmlBrowserSession, HtmlBrowserSessionState, HtmlBrowserSource,
    HtmlBrowserViewport, HtmlRenderInput, HtmlRenderOutput, HtmlRenderer, HtmlRuntime,
    HtmlRuntimeError, HtmlRuntimeSession, MathJaxRenderer, MermaidRenderer, PlantUmlRenderer,
    RenderConfig, RenderContext, RenderDiagnostics, RenderError, RenderInput, RenderKind,
    RenderOutput, RenderPolicy, RenderThemeMode, RenderThemeSnapshot, Renderer, RendererProfile,
    RuntimePathResolver, RuntimeVersion,
};
