use super::request::HtmlFontRequest;
use resvg::usvg::fontdb::Database;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::markdown::svg_rasterize) enum DiscoveryMode {
    Eager,
    #[cfg(target_os = "macos")]
    KnownOnly,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::markdown::svg_rasterize) struct DatabaseKey {
    pub(in crate::markdown::svg_rasterize) request: HtmlFontRequest,
    pub(in crate::markdown::svg_rasterize) discovery: DiscoveryMode,
}

impl DatabaseKey {
    pub(in crate::markdown::svg_rasterize) fn eager(request: HtmlFontRequest) -> Self {
        Self {
            request,
            discovery: DiscoveryMode::Eager,
        }
    }
}

pub(in crate::markdown::svg_rasterize) struct MeasuredDatabase {
    pub(in crate::markdown::svg_rasterize) key: DatabaseKey,
    pub(in crate::markdown::svg_rasterize) database: Arc<Database>,
}

pub(super) struct CacheValue {
    pub(super) database: Arc<Database>,
    #[cfg(target_os = "macos")]
    pub(super) proof: Option<Arc<super::proof::CreationProof>>,
}

#[cfg(target_os = "macos")]
pub(super) struct KnownCacheValue {
    pub(super) database: Arc<Database>,
    pub(super) proof: Arc<super::proof::CreationProof>,
}
