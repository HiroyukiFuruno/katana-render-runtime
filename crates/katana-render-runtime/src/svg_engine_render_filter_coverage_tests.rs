use super::super::compare::CHANNELS;
use super::{Mode, parse_source, render};
use std::path::Path;

const BLURRED_EDGE_SAMPLE_X: u32 = 5;
const BLURRED_EDGE_SAMPLE_Y: u32 = 4;

fn render_svg(source: &str) -> Result<tiny_skia::Pixmap, String> {
    let tree = parse_source(Path::new("filter-coverage.svg"), source.as_bytes())?;
    render(&tree, Mode::Extra(1.0))
}

fn rgba(image: &tiny_skia::Pixmap, x: u32, y: u32) -> Result<[u8; CHANNELS], String> {
    let pixel = image.pixel(x, y).ok_or("filter sample pixel missing")?;
    Ok([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()])
}

#[test]
fn cropped_filter_region_offsets_only_pixels_inside_the_region() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="8">
      <defs><filter id="f" filterUnits="userSpaceOnUse" x="4" y="0" width="8" height="8">
        <feOffset dx="2" dy="0"/>
      </filter></defs>
      <rect x="2" y="2" width="12" height="4" fill="red" filter="url(#f)"/>
    </svg>"##;
    let image = render_svg(source)?;
    assert_eq!(rgba(&image, 3, 3)?[3], 0);
    assert!(rgba(&image, 7, 3)?[0] > 0);
    assert_eq!(rgba(&image, 13, 3)?[3], 0);
    Ok(())
}

#[test]
fn singular_filtered_group_does_not_hide_visible_sibling() -> Result<(), String> {
    const FILTERED: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
          <defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter></defs>
          <g id="effect" filter="url(#blur)">
            <rect width="8" height="8" fill="red"/>
          </g>
        </svg>"#;
    const SIBLING: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
          <rect width="4" height="4" fill="blue"/>
        </svg>"#;
    let tree = parse_source(Path::new("singular-filter.svg"), FILTERED.as_bytes())?;
    let effect = tree.node_by_id("effect").ok_or("filter group missing")?;
    let crate::svg_engine::usvg::Node::Group(group) = effect else {
        return Err("filter fixture is not a group".into());
    };
    assert!(group.should_isolate() && !group.filters().is_empty());
    let mut actual = render_svg(SIBLING)?;
    let expected = actual.data().to_vec();
    let singular = tiny_skia::Transform::from_row(1.0, 1.0, 1.0, 1.0, 0.0, 0.0);
    assert!(singular.invert().is_none());
    crate::svg_engine::resvg::render_node(effect, singular, &mut actual.as_mut())
        .ok_or("filter fixture must have nonempty bounds")?;
    assert_eq!(actual.data(), expected);
    Ok(())
}

#[test]
fn isolated_group_outside_integer_bounds_leaves_existing_pixels_untouched() -> Result<(), String> {
    let tree = parse_source(
        Path::new("isolated-group-overflow.svg"),
        b"<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><g id='isolated' opacity='.5'><rect width='8' height='8' fill='red'/></g></svg>",
    )?;
    let group = tree
        .node_by_id("isolated")
        .ok_or("isolated group missing")?;
    let mut image = tiny_skia::Pixmap::new(8, 8).ok_or("pixmap unavailable")?;
    image.fill(tiny_skia::Color::from_rgba8(0, 0, 255, 255));
    let expected = image.data().to_vec();
    let outside = tiny_skia::Transform::from_scale(300_000_000.0, 1.0);
    crate::svg_engine::resvg::render_node(group, outside, &mut image.as_mut())
        .ok_or("isolated group bounds missing")?;
    assert_eq!(image.data(), expected);
    Ok(())
}

#[test]
fn filter_color_space_changes_blurred_edge_samples() -> Result<(), String> {
    const FILTER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8">
      <defs><filter id="f" color-interpolation-filters="COLOR_SPACE">
        <feGaussianBlur stdDeviation="1.25"/>
      </filter></defs><g filter="url(#f)">
      <rect x="2" y="1" width="4" height="6" fill="black"/>
      <rect x="6" y="1" width="4" height="6" fill="white"/></g>
    </svg>"##;
    let srgb = render_svg(&FILTER.replace("COLOR_SPACE", "sRGB"))?;
    let linear = render_svg(&FILTER.replace("COLOR_SPACE", "linearRGB"))?;
    let srgb_edge = rgba(&srgb, BLURRED_EDGE_SAMPLE_X, BLURRED_EDGE_SAMPLE_Y)?;
    let linear_edge = rgba(&linear, BLURRED_EDGE_SAMPLE_X, BLURRED_EDGE_SAMPLE_Y)?;
    assert!(srgb_edge[3] > 0 && linear_edge[3] > 0);
    assert!(linear_edge[0] > srgb_edge[0]);
    Ok(())
}

#[test]
fn zero_radius_morphology_and_zero_scale_displacement_preserve_source_pixels() -> Result<(), String>
{
    const SHAPE: &str = r##"<rect x="3" y="2" width="6" height="6" fill="#3060d0" FILTER/>"##;
    let plain = render_svg(&format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='12' height='10'>{}</svg>",
        SHAPE.replace("FILTER", "")
    ))?;
    for effect in [
        "<feMorphology operator='dilate' radius='0'/>",
        "<feTurbulence baseFrequency='0.13 0.17' numOctaves='1' result='map'/><feDisplacementMap in='SourceGraphic' in2='map' scale='0'/>",
    ] {
        let filtered = render_svg(&format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='12' height='10'><defs><filter id='f' color-interpolation-filters='sRGB'>{effect}</filter></defs>{}</svg>",
            SHAPE.replace("FILTER", "filter='url(#f)'")
        ))?;
        assert_eq!(filtered.data(), plain.data(), "{effect}");
    }
    Ok(())
}

#[test]
fn lighting_smaller_than_three_pixels_is_transparent() -> Result<(), String> {
    for (width, height) in [(2, 2), (2, 4), (4, 2)] {
        let source = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}'><defs><filter id='f' filterUnits='userSpaceOnUse' x='0' y='0' width='{width}' height='{height}'><feDiffuseLighting><feDistantLight azimuth='45' elevation='45'/></feDiffuseLighting></filter></defs><rect width='{width}' height='{height}' filter='url(#f)'/></svg>"
        );
        let image = render_svg(&source)?;
        assert_eq!((image.width(), image.height()), (width, height));
        assert!(image.data().iter().all(|channel| *channel == 0));
    }
    Ok(())
}

#[test]
fn point_light_behind_surface_has_zero_specular_at_singularity() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><defs><filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="4" height="4"><feSpecularLighting in="SourceAlpha" surfaceScale="0" specularConstant="1" specularExponent="1"><fePointLight x="1" y="1" z="-1"/></feSpecularLighting></filter></defs><rect width="4" height="4" filter="url(#f)"/></svg>"#;
    let image = render_svg(source)?;
    assert_eq!(rgba(&image, 1, 1)?, [0, 0, 0, 0]);
    assert!(rgba(&image, 0, 0)?[3] > 0);
    Ok(())
}
