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
