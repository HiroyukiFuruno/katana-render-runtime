use super::{
    GRADIENT_FALLBACK_COLOR, assert_solid_fill_color, parse_with_languages, pixel, rendered_svg,
};

const GREEN_PIXEL: [u8; 4] = [0, 255, 0, 255];
const TRANSPARENT_PIXEL: [u8; 4] = [0, 0, 0, 0];
const FIRST_GAP_X: usize = 2;
const REPEATED_TILE_X: usize = 4;

const MIXED_UNITS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="p" patternUnits="userSpaceOnUse" patternContentUnits="objectBoundingBox" width="4" height="4"><rect width="0.25" height="1" fill="#00ff00"/></pattern></defs><rect width="8" height="8" fill="url(#p)"/></svg>"##;
const USER_UNITS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="p" patternUnits="userSpaceOnUse" patternContentUnits="userSpaceOnUse" width="4" height="4"><rect width="2" height="8" fill="#00ff00"/></pattern></defs><rect width="8" height="8" fill="url(#p)"/></svg>"##;

#[test]
fn user_space_tiles_keep_size_when_content_uses_object_bbox() -> Result<(), String> {
    let mixed = rendered_svg(MIXED_UNITS)?;
    let control = rendered_svg(USER_UNITS)?;
    assert_eq!(mixed, control);
    assert_eq!(pixel(&mixed, 0, 0)?, GREEN_PIXEL);
    assert_eq!(pixel(&mixed, FIRST_GAP_X, 0)?, TRANSPARENT_PIXEL);
    assert_eq!(pixel(&mixed, REPEATED_TILE_X, 0)?, GREEN_PIXEL);
    Ok(())
}

#[test]
fn cyclic_empty_paint_server_references_use_authored_fallback() -> Result<(), String> {
    for definitions in [
        r##"<linearGradient id="a" href="#b"/><linearGradient id="b" href="#a"/>"##,
        r##"<radialGradient id="a" href="#b"/><radialGradient id="b" href="#a"/>"##,
        r##"<pattern id="a" width="4" height="4" href="#b"/><pattern id="b" href="#a"/>"##,
    ] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs>{definitions}</defs><rect id="sample" width="8" height="8" fill="url(#a) #123456"/></svg>"##
        );
        let tree = parse_with_languages(&source, &[])?;
        assert_solid_fill_color(&tree, "sample", GRADIENT_FALLBACK_COLOR)?;
    }
    Ok(())
}
