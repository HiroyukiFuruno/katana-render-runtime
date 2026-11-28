use super::{RasterTarget, SvgRasterizeOps, rasterizer_options};
use crate::markdown::svg_rasterize::SvgRasterizeError;
use crate::markdown::svg_rasterize::composite::SvgCompositeRenderer;

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

#[test]
fn raster_target_rejects_invalid_provenance_for_generated_viewport_clip()
-> Result<(), Box<dyn std::error::Error>> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#202020"/><svg x="1" y="1" width="8" height="8"><rect width="8" height="8" fill="#ff0000" style="mix-blend-mode:multiply"/></svg></svg>"##;
    let tree = resvg::usvg::Tree::from_str(svg, &rasterizer_options())
        .map_err(|error| error.to_string())?;
    assert!(SvgCompositeRenderer::needs_authored_clip_ids(&tree));
    let target = RasterTarget::new(tree.size(), 1.0);
    let rendered = target.render_with_text(&tree, svg, &Default::default(), None, None)?;
    assert!(rendered.pixel(5, 5).is_some_and(|pixel| pixel.alpha() > 0));
    assert!(matches!(
        target.render_with_text(&tree, "<svg>", &Default::default(), None, None),
        Err(SvgRasterizeError::ParseFailed(_))
    ));
    Ok(())
}
