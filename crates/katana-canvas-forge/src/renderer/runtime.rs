use crate::markdown::{
    drawio_renderer::{DRAWIO_JS_CHECKSUM, DRAWIO_JS_VERSION},
    mermaid_renderer::{MERMAID_JS_CHECKSUM, MERMAID_JS_VERSION},
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
}
