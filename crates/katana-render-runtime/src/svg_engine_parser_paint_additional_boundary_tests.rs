use super::super::{center_pixel, sample_path, usvg};
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
fn mixed_gradient_chain_inherits_shared_attributes_but_stops_coordinate_lookup()
-> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs><linearGradient id="base" x1="2" gradientUnits="userSpaceOnUse" spreadMethod="reflect">
      <stop stop-color="blue"/><stop offset="1" stop-color="red"/></linearGradient>
      <radialGradient id="bridge" href="#base"/>
      <linearGradient id="alias" href="#bridge" x2="8"/></defs>
      <rect id="sample" width="8" height="8" fill="url(#alias)"/></svg>"##;
    let tree = parse_with_languages(source, &[])?;
    let fill = sample_path(&tree)?.fill().ok_or("gradient fill missing")?;
    let usvg::Paint::LinearGradient(gradient) = fill.paint() else {
        return Err("linked fill is not a linear gradient".to_owned());
    };
    assert_eq!(gradient.x1(), 0.0);
    assert_eq!(gradient.x2(), 8.0);
    assert!(gradient.transform().is_identity());
    assert_eq!(gradient.spread_method(), usvg::SpreadMethod::Reflect);
    assert_eq!(gradient.stops().len(), 2);
    Ok(())
}

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
fn circular_svg_child_references_render_like_the_broken_cycle() -> Result<(), String> {
    for (content, cyclic, detached) in [
        (
            r##"<defs><pattern id="a" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="url(#b)"/></pattern><pattern id="b" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="lime"/><rect width="8" height="8" fill="url(#a)"/></pattern></defs><rect width="8" height="8" fill="url(#a) lime"/>"##,
            r##"height="8" fill="url(#a)"/>"##,
            r#"height="8" fill="none"/>"#,
        ),
        (
            r##"<defs><clipPath id="a"><rect width="8" height="8" clip-path="url(#b)"/></clipPath><clipPath id="b"><rect width="8" height="8" clip-path="url(#a)"/></clipPath></defs><rect width="8" height="8" fill="lime" clip-path="url(#a)"/>"##,
            r##"height="8" clip-path="url(#a)"/>"##,
            r#"height="8" clip-path="none"/>"#,
        ),
        (
            r##"<defs><filter id="loop"><feImage href="#shape"/></filter></defs><rect id="shape" width="8" height="8" fill="lime" filter="url(#loop)"/>"##,
            r##"filter="url(#loop)""##,
            r#"filter="none""#,
        ),
    ] {
        let source = svg_child_references_document(content);
        let control = source.replace(cyclic, detached);
        assert_ne!(source, control);
        let actual = rendered_svg(&source)?;
        let expected = rendered_svg(&control)?;
        assert_eq!(pixel(&expected, CENTER_SAMPLE, CENTER_SAMPLE)?, GREEN_PIXEL);
        assert_eq!(actual, expected, "circular reference {cyclic}");
    }
    Ok(())
}

#[test]
fn svg_child_references_without_a_cycle_keep_their_effects() -> Result<(), String> {
    for (content, expected_center) in [
        (
            r##"<defs><pattern id="a" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="url(#b)"/></pattern><pattern id="b" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="url(#c)"/></pattern><pattern id="c" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="lime"/></pattern></defs><rect width="8" height="8" fill="url(#a) red"/>"##,
            GREEN_PIXEL,
        ),
        (
            r##"<defs><clipPath id="a"><rect width="8" height="8" clip-path="url(#b)"/></clipPath><clipPath id="b"><rect width="8" height="8" clip-path="url(#c)"/></clipPath><clipPath id="c"><rect width="4" height="8"/></clipPath></defs><rect width="8" height="8" fill="lime" clip-path="url(#a)"/>"##,
            TRANSPARENT_PIXEL,
        ),
        (
            r##"<defs><filter id="image"><feImage href="#shape"/></filter><rect id="shape" width="8" height="8" fill="lime" filter="opacity(100%)"/></defs><rect width="8" height="8" fill="red" filter="url(#image)"/>"##,
            GREEN_PIXEL,
        ),
    ] {
        let source = svg_child_references_document(content);
        let pixels = rendered_svg(&source)?;
        assert_eq!(pixel(&pixels, 0, CENTER_SAMPLE)?, GREEN_PIXEL, "{content}");
        assert_eq!(
            pixel(&pixels, CENTER_SAMPLE, CENTER_SAMPLE)?,
            expected_center,
            "{content}"
        );
    }
    Ok(())
}

fn svg_child_references_document(content: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">{content}</svg>"#)
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
