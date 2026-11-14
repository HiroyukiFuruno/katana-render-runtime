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

extern crate self as markup5ever_rcdom;
/* WHY: 既存の内部SVG参照を維持し、別crateの公開を配布条件にしない。 */
extern crate self as resvg;

#[cfg(test)]
pub(crate) use svg_engine::resvg::tiny_skia;
pub(crate) use svg_engine::{
    resvg::{render, render_node},
    usvg,
};

mod css_parser_shared;
mod html_dom;
pub mod markdown;
pub mod renderer;
mod resource_bytes;
mod svg_engine;
mod system;
mod v8_script;

pub use html_dom::{Handle, Node, NodeData, RcDom, SerializableHandle, WeakHandle};
pub use markdown::plantuml_renderer::{
    PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION, PlantUmlThemeCatalog,
};
pub use renderer::{
    DiagramKind, DrawioRenderer, HTML_BROWSER_MAX_SOURCE_BYTES, HtmlArtifactError,
    HtmlArtifactProvenance, HtmlArtifactStore, HtmlBrowserError, HtmlBrowserFrame,
    HtmlBrowserInput, HtmlBrowserNavigation, HtmlBrowserNavigationEvent, HtmlBrowserOrigin,
    HtmlBrowserOverflowPolicy, HtmlBrowserPixelFormat, HtmlBrowserScrollMetrics,
    HtmlBrowserSession, HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport,
    HtmlFrameArtifact, HtmlRenderInput, HtmlRenderOutput, HtmlRenderer, HtmlRuntime,
    HtmlRuntimeError, HtmlRuntimeSession, MathJaxRenderer, MermaidRenderer, PlantUmlRenderer,
    RenderConfig, RenderContext, RenderDiagnostics, RenderError, RenderInput, RenderKind,
    RenderOutput, RenderPolicy, RenderThemeMode, RenderThemeSnapshot, Renderer, RendererProfile,
    RuntimePathResolver, RuntimeVersion,
};
