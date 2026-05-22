use crate::markdown::{
    drawio_renderer::{DRAWIO_JS_CHECKSUM, DRAWIO_JS_VERSION},
    mermaid_renderer::{MERMAID_JS_CHECKSUM, MERMAID_JS_VERSION},
    plantuml_renderer::{PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION},
    runtime_assets::{MATHJAX_JS_CHECKSUM, MATHJAX_JS_VERSION},
};

#[derive(Clone, Copy)]
pub(super) struct RuntimeDescriptor {
    pub(super) name: &'static str,
    pub(super) version: &'static str,
    pub(super) checksum: &'static str,
    pub(super) profile_id: &'static str,
}

impl RuntimeDescriptor {
    pub(super) fn mermaid() -> Self {
        Self {
            name: "Mermaid.js",
            version: MERMAID_JS_VERSION,
            checksum: MERMAID_JS_CHECKSUM,
            profile_id: "katana-mermaid",
        }
    }

    pub(super) fn drawio() -> Self {
        Self {
            name: "Draw.io",
            version: DRAWIO_JS_VERSION,
            checksum: DRAWIO_JS_CHECKSUM,
            profile_id: "katana-drawio",
        }
    }

    pub(super) fn plantuml() -> Self {
        Self {
            name: "PlantUML",
            version: PLANTUML_JAR_VERSION,
            checksum: PLANTUML_JAR_CHECKSUM,
            profile_id: "katana-plantuml-jvm",
        }
    }

    pub(super) fn mathjax() -> Self {
        Self {
            name: "MathJax",
            version: MATHJAX_JS_VERSION,
            checksum: MATHJAX_JS_CHECKSUM,
            profile_id: "katana-mathjax-svg",
        }
    }
}
