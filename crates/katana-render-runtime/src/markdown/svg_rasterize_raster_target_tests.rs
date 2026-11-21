use super::{RasterTarget, SvgRasterizeOps, rasterizer_options};

#[test]
fn raster_target_reports_pixmap_allocation_failure() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"##;
    let compatible_svg = SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let tree = resvg::usvg::Tree::from_str(&compatible_svg, &rasterizer_options());
    let target = RasterTarget {
        display_width: 1.0,
        display_height: 1.0,
        effective_scale: 1.0,
        width: 0,
        height: 0,
    };

    assert!(tree.as_ref().is_ok_and(|it| {
        target
            .render(it, &compatible_svg, &Default::default())
            .is_err()
    }));
}
