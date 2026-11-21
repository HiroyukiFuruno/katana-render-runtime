mod cache;
mod diagram_type;
mod js_runtime;
mod js_runtime_scripts;
pub mod render;
pub mod resolve;
pub mod types;
#[cfg(test)]
mod zenuml_v8_runtime;

pub use crate::markdown::runtime_assets::{MERMAID_JS_CHECKSUM, MERMAID_JS_VERSION};
pub use resolve::MermaidBinaryOps;
pub use types::MermaidRenderOps;

use crate::markdown::color_preset::DiagramColorPreset;
use std::hash::{Hash, Hasher};

fn hash_mermaid_source_and_preset<H: Hasher>(
    hasher: &mut H,
    source: &str,
    preset: &DiagramColorPreset,
) {
    source.hash(hasher);
    preset.mermaid_theme.hash(hasher);
    preset.background.hash(hasher);
    preset.text.hash(hasher);
    preset.fill.hash(hasher);
    preset.stroke.hash(hasher);
    preset.arrow.hash(hasher);
}
