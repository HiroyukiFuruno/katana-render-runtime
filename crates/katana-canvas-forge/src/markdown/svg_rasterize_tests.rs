use super::{
    RasterTarget, SvgRasterizeOps, effective_scale, parse_light_dark_function, rasterizer_options,
};

#[test]
fn rasterize_svg_returns_pixels_for_simple_svg() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#fff"/></svg>"##;
    let image = SvgRasterizeOps::rasterize_svg(svg, 1.0);

    assert!(image.as_ref().is_ok_and(|it| it.width == 10));
    assert!(image.as_ref().is_ok_and(|it| it.height == 10));
    assert!(image.as_ref().is_ok_and(|it| !it.rgba.is_empty()));
}

#[test]
fn rasterize_svg_reports_parse_errors() {
    let image = SvgRasterizeOps::rasterize_svg("<svg>", 1.0);

    assert!(image.is_err());
}

#[test]
fn preprocess_handles_foreign_objects_entities_and_light_dark_colors() {
    let svg = r##"<svg fill="light-dark(#111, #eee)">&nbsp;<foreignObject><div>skip</div></foreignObject></svg>"##;
    let prepared = SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let malformed = SvgRasterizeOps::preprocess_for_rasterizer(
        r##"<svg fill="light-dark(#111"><foreignObject><div></svg>"##,
    );
    let self_closed = SvgRasterizeOps::preprocess_for_rasterizer(r#"<svg><foreignObject /></svg>"#);

    assert!(prepared.contains("&#160;"));
    assert!(prepared.contains("#111"));
    assert!(!prepared.contains("foreignObject"));
    assert!(!self_closed.contains("foreignObject"));
    assert!(malformed.contains("light-dark("));
    assert!(malformed.contains("foreignObject"));
    assert_eq!(
        parse_light_dark_function("#123, rgb(1, 2, 3))"),
        Some((18, "#123"))
    );
    assert_eq!(parse_light_dark_function("#123"), None);
    assert!(effective_scale(10.0, 10.0, -1.0).is_sign_positive());
}

#[test]
fn raster_target_reports_pixmap_allocation_failure() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"##;
    let tree = resvg::usvg::Tree::from_str(svg, &rasterizer_options());
    let target = RasterTarget {
        display_width: 1.0,
        display_height: 1.0,
        effective_scale: 1.0,
        width: 0,
        height: 0,
    };

    assert!(tree.as_ref().is_ok_and(|it| target.render(it).is_err()));
}
