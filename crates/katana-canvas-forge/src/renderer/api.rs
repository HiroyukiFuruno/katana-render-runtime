use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagramKind {
    Mermaid,
    Drawio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeVersion {
    pub name: String,
    pub version: String,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendererProfile {
    pub id: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderConfig {
    pub vendor_config: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderPolicy {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub padding: Option<u32>,
    pub background: Option<String>,
    pub cache_profile: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderContext {
    pub theme_fingerprint: Option<String>,
    pub document_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderInput {
    pub kind: DiagramKind,
    pub source: String,
    pub config: RenderConfig,
    pub policy: RenderPolicy,
    pub context: RenderContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDiagnostics {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderOutput {
    pub svg: String,
    pub width: f32,
    pub height: f32,
    pub view_box: String,
    pub runtime: RuntimeVersion,
    pub profile: RendererProfile,
    pub diagnostics: RenderDiagnostics,
    pub cache_fingerprint: String,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("{kind} runtime is not installed: {install_path}")]
    NotInstalled {
        kind: String,
        download_url: String,
        install_path: PathBuf,
    },
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("runtime path resolution failed: {0}")]
    RuntimeResolution(String),
    #[error("unsupported diagram kind")]
    UnsupportedKind,
}

pub trait Renderer {
    fn render(&self, input: &RenderInput) -> Result<RenderOutput, RenderError>;
}
