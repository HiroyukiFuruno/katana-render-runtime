use super::{assert_rgba_pixel, parse_writer_svg, render_tree, usvg, writer_options};
use rstest::rstest;
use usvg::filter::{ColorChannel, Input, Kind};

const TRANSPARENT: (u8, u8, u8, u8) = (0, 0, 0, 0);
const CANVAS_LAST_PIXEL: (u32, u32) = (3, 3);
const OPAQUE: f32 = 1.0;
const HALF_OPACITY: f32 = 0.5;
const POSITIVE_DISPLACEMENT_SCALE: f32 = 2.0;

#[rstest]
#[case::red_positive(ColorChannel::R, "#ff0000", 2.0, (0, 0, 255, 255), TRANSPARENT)]
#[case::green_positive(ColorChannel::G, "#00ff00", 2.0, (0, 0, 255, 255), TRANSPARENT)]
#[case::blue_positive(ColorChannel::B, "#0000ff", 2.0, (0, 0, 255, 255), TRANSPARENT)]
#[case::alpha_positive(ColorChannel::A, "#000000", 2.0, (0, 0, 255, 255), TRANSPARENT)]
#[case::red_negative(ColorChannel::R, "#ff0000", -2.0, (255, 0, 0, 255), (0, 0, 255, 255))]
#[case::green_negative(ColorChannel::G, "#00ff00", -2.0, (255, 0, 0, 255), (0, 0, 255, 255))]
#[case::blue_negative(ColorChannel::B, "#0000ff", -2.0, (255, 0, 0, 255), (0, 0, 255, 255))]
#[case::alpha_negative(ColorChannel::A, "#000000", -2.0, (255, 0, 0, 255), (0, 0, 255, 255))]
fn displacement_map_roundtrip_preserves_channel_and_pixels(
    #[case] channel: ColorChannel,
    #[case] flood_color: &str,
    #[case] scale: f32,
    #[case] expected: (u8, u8, u8, u8),
    #[case] outside: (u8, u8, u8, u8),
) -> Result<(), String> {
    assert_roundtrip(channel, flood_color, scale, OPAQUE, expected, outside)
}

#[rstest]
#[case::translucent_red(ColorChannel::R, "#ff0000", (0, 0, 255, 255), TRANSPARENT)]
#[case::translucent_green(ColorChannel::G, "#00ff00", (0, 0, 255, 255), TRANSPARENT)]
#[case::translucent_blue(ColorChannel::B, "#0000ff", (0, 0, 255, 255), TRANSPARENT)]
#[case::translucent_alpha(ColorChannel::A, "#000000", (255, 255, 255, 255), (255, 255, 255, 255))]
fn translucent_displacement_map_uses_straight_rgb_and_preserves_alpha(
    #[case] channel: ColorChannel,
    #[case] flood_color: &str,
    #[case] sample: (u8, u8, u8, u8),
    #[case] outside: (u8, u8, u8, u8),
) -> Result<(), String> {
    assert_roundtrip(
        channel,
        flood_color,
        POSITIVE_DISPLACEMENT_SCALE,
        HALF_OPACITY,
        sample,
        outside,
    )
}

fn assert_roundtrip(
    channel: ColorChannel,
    flood_color: &str,
    scale: f32,
    opacity: f32,
    sample: (u8, u8, u8, u8),
    outside: (u8, u8, u8, u8),
) -> Result<(), String> {
    let input = displacement_svg(channel, flood_color, scale, opacity);
    let original = parse_writer_svg(&input, "displacement-map")?;
    assert_semantics(&original, channel, scale)?;
    let output = original.to_string(&writer_options("displacement-map"));
    let reparsed = parse_writer_svg(&output, "displacement-map-roundtrip")?;
    assert_semantics(&reparsed, channel, scale)?;
    assert_pixels(&original, &reparsed, sample, outside)
}

fn displacement_svg(channel: ColorChannel, flood_color: &str, scale: f32, opacity: f32) -> String {
    let channel = match channel {
        ColorChannel::R => "R",
        ColorChannel::G => "G",
        ColorChannel::B => "B",
        ColorChannel::A => "A",
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
          <defs><filter id="displace" filterUnits="userSpaceOnUse"
            primitiveUnits="userSpaceOnUse" color-interpolation-filters="sRGB"
            x="0" y="0" width="4" height="4">
            <feFlood x="0" y="0" width="4" height="4" flood-color="{flood_color}"
              flood-opacity="{opacity}" result="map"/>
            <feDisplacementMap in="SourceGraphic" in2="map" scale="{scale}"
              xChannelSelector="{channel}" yChannelSelector="{channel}"/>
          </filter></defs>
          <g filter="url(#displace)">
            <rect width="4" height="4" fill="white"/>
            <rect width="1" height="1" fill="red"/>
            <rect x="2" y="2" width="1" height="1" fill="blue"/>
          </g>
        </svg>"##
    )
}

fn assert_semantics(tree: &usvg::Tree, channel: ColorChannel, scale: f32) -> Result<(), String> {
    let map = displacement_primitive(tree).ok_or("feDisplacementMap primitive missing")?;
    assert_eq!(map.input1(), &Input::SourceGraphic);
    assert_eq!(map.input2(), &Input::Reference("map".to_owned()));
    assert_eq!(map.scale(), scale);
    assert_eq!(map.x_channel_selector(), channel);
    assert_eq!(map.y_channel_selector(), channel);
    Ok(())
}

fn displacement_primitive(tree: &usvg::Tree) -> Option<&usvg::filter::DisplacementMap> {
    tree.filters()
        .iter()
        .flat_map(|filter| filter.primitives())
        .find_map(|primitive| match primitive.kind() {
            Kind::DisplacementMap(map) => Some(map),
            _ => None,
        })
}

fn assert_pixels(
    original: &usvg::Tree,
    reparsed: &usvg::Tree,
    sample: (u8, u8, u8, u8),
    outside: (u8, u8, u8, u8),
) -> Result<(), String> {
    let before = render_tree(original)?;
    let after = render_tree(reparsed)?;
    assert_eq!(before.data(), after.data(), "roundtrip pixels");
    for pixmap in [&before, &after] {
        assert_rgba_pixel(pixmap, (1, 1), sample)?;
        assert_rgba_pixel(pixmap, CANVAS_LAST_PIXEL, outside)?;
    }
    Ok(())
}
