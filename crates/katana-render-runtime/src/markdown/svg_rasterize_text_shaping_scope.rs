use super::{family_scope_cache, font, harfrust, measurement_token};
#[cfg(windows)]
use std::{path::Path, sync::Arc};

#[cfg(windows)]
pub(in super::super) fn enable_windows_font_snapshots() {
    font::enable_windows_font_snapshots();
}

#[cfg(windows)]
pub(in super::super) fn windows_font_snapshot(path: &Path) -> Option<Arc<Vec<u8>>> {
    font::windows_font_snapshot(path)
}

#[derive(Clone, Default)]
pub(crate) struct HtmlFontFrameCache {
    #[cfg(target_os = "macos")]
    native_files: super::native_html_runs::NativeFontFileCache,
}

impl HtmlFontFrameCache {
    pub(crate) fn with_frame<T>(&self, operation: impl FnOnce() -> T) -> T {
        use super::super::font::platform::{PlatformFontStrategy, font_platform};

        font_platform().with_temporary_font_objects(|| {
            /* WHY: layoutとrasterで共有する書体の選択結果はframeごとに再評価する。 */
            let _font_scope = harfrust::face_cache::FontFaceCacheScope::enter();
            #[cfg(target_os = "macos")]
            let _fallback_scope = super::native_html_runs::Scope::enter(&self.native_files);
            #[cfg(target_os = "macos")]
            let _native_html_faces = super::super::native_html_faces::Scope::enter();
            operation()
        })
    }
}

pub(in crate::markdown::svg_rasterize) fn with_render_scoped_stamp_batch<T>(
    operation: impl FnMut() -> T,
) -> T {
    let mut operation = operation;
    font::with_render_scoped_stamp_batch(|| {
        let _family_scope = family_scope_cache::Scope::enter();
        let _font_scope = harfrust::face_cache::FontFaceCacheScope::enter();
        let _token_scope = measurement_token::Scope::enter();
        operation()
    })
}
