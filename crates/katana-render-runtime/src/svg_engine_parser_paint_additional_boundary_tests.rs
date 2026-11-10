use super::super::center_pixel;
use super::{
    GRADIENT_FALLBACK_COLOR, VIEWPORT, assert_solid_fill_color, parse_with_languages, pixel,
    rendered_svg,
};

const GREEN_PIXEL: [u8; 4] = [0, 255, 0, 255];
const TRANSPARENT_PIXEL: [u8; 4] = [0, 0, 0, 0];
const FIRST_GAP_X: usize = 2;
const REPEATED_TILE_X: usize = 4;
const CENTER_SAMPLE: usize = VIEWPORT as usize / 2;

const MIXED_UNITS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="p" patternUnits="userSpaceOnUse" patternContentUnits="objectBoundingBox" width="4" height="4"><rect width="0.25" height="1" fill="#00ff00"/></pattern></defs><rect width="8" height="8" fill="url(#p)"/></svg>"##;
const USER_UNITS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="p" patternUnits="userSpaceOnUse" patternContentUnits="userSpaceOnUse" width="4" height="4"><rect width="2" height="8" fill="#00ff00"/></pattern></defs><rect width="8" height="8" fill="url(#p)"/></svg>"##;

#[test]
fn singular_gradient_leaves_underlying_paint_visible() -> Result<(), String> {
    for gradient in ["linearGradient", "radialGradient"] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
            <defs><{gradient} id="paint" gradientTransform="matrix(1 1 1 1 0 0)">
            <stop stop-color="red"/><stop offset="1" stop-color="white"/>
            </{gradient}></defs><rect width="8" height="8" fill="#00ff00"/>
            <rect width="8" height="8" fill="url(#paint)"/></svg>"##
        );
        let control = source.replace("matrix(1 1 1 1 0 0)", "matrix(1 0 0 1 0 0)");
        assert_eq!(center_pixel(&source, &[])?, GREEN_PIXEL, "{gradient}");
        assert_ne!(center_pixel(&control, &[])?, GREEN_PIXEL, "{gradient}");
    }
    Ok(())
}

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

#[test]
fn acyclic_paint_server_references_resolve_contents() -> Result<(), String> {
    for definitions in [
        r##"<linearGradient id="a" href="#b"/><linearGradient id="b"><stop stop-color="#00ff00"/><stop offset="1" stop-color="#00ff00"/></linearGradient>"##,
        r##"<radialGradient id="a" href="#b"/><radialGradient id="b"><stop stop-color="#00ff00"/><stop offset="1" stop-color="#00ff00"/></radialGradient>"##,
        r##"<pattern id="a" href="#b"/><pattern id="b" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#00ff00"/></pattern>"##,
    ] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{VIEWPORT}" height="{VIEWPORT}"><defs>{definitions}</defs><rect width="{VIEWPORT}" height="{VIEWPORT}" fill="url(#a) #123456"/></svg>"##
        );
        let rendered = rendered_svg(&source)?;
        assert_eq!(pixel(&rendered, CENTER_SAMPLE, CENTER_SAMPLE)?, GREEN_PIXEL);
    }
    Ok(())
}

#[test]
fn gradients_with_local_stops_ignore_wrong_type_href() -> Result<(), String> {
    for tag in ["linearGradient", "radialGradient"] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="wrong" width="1" height="1"><rect width="1" height="1" fill="blue"/></pattern><{tag} id="gradient" href="#wrong"><stop stop-color="red"/><stop offset="1" stop-color="red"/></{tag}></defs><rect width="8" height="8" fill="url(#gradient) green"/></svg>"##
        );
        let control = source.replace(" href=\"#wrong\"", "");
        assert_ne!(source, control);
        let actual = rendered_svg(&source)?;
        let expected = rendered_svg(&control)?;
        assert_eq!(
            pixel(&expected, CENTER_SAMPLE, CENTER_SAMPLE)?,
            [255, 0, 0, 255]
        );
        assert_eq!(actual, expected, "{tag} local stops changed");
    }
    Ok(())
}

#[test]
fn single_stop_radial_gradient_preserves_color_and_opacity() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><radialGradient id="single" color="#123456"><stop stop-color="currentColor" stop-opacity="0.5"/></radialGradient></defs><rect id="sample" width="8" height="8" fill="url(#single)"/></svg>"##;
    let control = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#123456" fill-opacity="0.5"/></svg>"##;
    let tree = parse_with_languages(source, &[])?;
    let fill = super::path_by_id(&tree, "sample")?
        .fill()
        .ok_or("single-stop radial fill missing")?;
    let super::usvg::Paint::Color(color) = fill.paint() else {
        return Err("single-stop radial gradient did not become a solid color".to_owned());
    };
    assert_eq!(
        (color.red, color.green, color.blue),
        GRADIENT_FALLBACK_COLOR
    );
    assert_eq!(fill.opacity().get(), 0.5);
    let actual = rendered_svg(source)?;
    let expected = rendered_svg(control)?;
    assert_eq!(actual, expected);
    assert_ne!(
        pixel(&actual, CENTER_SAMPLE, CENTER_SAMPLE)?,
        TRANSPARENT_PIXEL
    );
    Ok(())
}
