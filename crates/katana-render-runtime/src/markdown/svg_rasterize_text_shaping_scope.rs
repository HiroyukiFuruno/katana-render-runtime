use super::{family_scope_cache, font, harfrust, measurement_token};

pub(in crate::markdown::svg_rasterize) fn with_font_face_cache_scope<T>(
    operation: impl FnOnce() -> T,
) -> T {
    /* WHY: 同一frameのlayoutからraster完了までlibrary-owned metricsを共有する。 */
    let _font_scope = harfrust::face_cache::FontFaceCacheScope::enter();
    operation()
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
