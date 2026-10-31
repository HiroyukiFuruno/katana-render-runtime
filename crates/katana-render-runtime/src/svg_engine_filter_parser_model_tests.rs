use super::{resvg, usvg};
use rstest::rstest;
use tiny_skia::{Pixmap, Transform};

#[path = "svg_engine_filter_input_model_tests.rs"]
mod input_model_tests;

#[path = "svg_engine_filter_convolve_model_tests.rs"]
mod convolve_model_tests;

#[path = "svg_engine_filter_parser_boundary_tests.rs"]
mod parser_boundary_tests;

const SIDE: u32 = 16;
const IDENTITY: &str = "<feComponentTransfer/>";
const OVERFLOW_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">
      <defs><filter id="f" filterUnits="userSpaceOnUse" primitiveUnits="userSpaceOnUse"
        x="0" y="0" width="8" height="8"><feMorphology radius="2e38"/></filter></defs>
      <g id="filtered" transform="scale(2)" filter="url(#f)"><rect x="1" y="1" width="4" height="4" fill="red"/></g></svg>"##;

fn fixture(primitive: &str) -> Result<usvg::Tree, String> {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">
        <defs><filter id="f" filterUnits="userSpaceOnUse" primitiveUnits="userSpaceOnUse"
          x="0" y="0" width="16" height="16" color-interpolation-filters="sRGB">{primitive}</filter></defs>
        <g filter="url(#f)"><rect x="3" y="3" width="5" height="8" fill="#e04020"/>
        <rect x="8" y="6" width="4" height="5" fill="#2080c0" fill-opacity="0.5"/></g></svg>"##
    );
    usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn render(tree: &usvg::Tree) -> Result<Pixmap, String> {
    let mut image = Pixmap::new(SIDE, SIDE).ok_or("pixmap unavailable")?;
    resvg::render(tree, Transform::identity(), &mut image.as_mut());
    Ok(image)
}

#[rstest]
#[case::red_table("feFuncR", "table")]
#[case::green_table("feFuncG", "table")]
#[case::blue_table("feFuncB", "table")]
#[case::alpha_table("feFuncA", "table")]
#[case::red_discrete("feFuncR", "discrete")]
#[case::green_discrete("feFuncG", "discrete")]
#[case::blue_discrete("feFuncB", "discrete")]
#[case::alpha_discrete("feFuncA", "discrete")]
fn missing_transfer_values_preserve_identity(
    #[case] channel: &str,
    #[case] kind: &str,
) -> Result<(), String> {
    let candidate = fixture(&format!(
        "<feComponentTransfer><{channel} type='{kind}'/></feComponentTransfer>"
    ))?;
    assert_empty_transfer(&candidate, channel, kind)?;
    let reference = fixture(IDENTITY)?;
    let actual = render(&candidate)?;
    let expected = render(&reference)?;
    assert!(expected.data().iter().any(|channel| *channel != 0));
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

fn assert_empty_transfer(tree: &usvg::Tree, channel: &str, kind: &str) -> Result<(), String> {
    let filter = tree.filters().first().ok_or("filter missing")?;
    let primitive = filter.primitives().first().ok_or("primitive missing")?;
    let usvg::filter::Kind::ComponentTransfer(transfer) = primitive.kind() else {
        return Err("component transfer missing".to_owned());
    };
    let actual = match channel {
        "feFuncR" => transfer.func_r(),
        "feFuncG" => transfer.func_g(),
        "feFuncB" => transfer.func_b(),
        "feFuncA" => transfer.func_a(),
        _ => return Err("unknown transfer channel".to_owned()),
    };
    match (kind, actual) {
        ("table", usvg::filter::TransferFunction::Table(values))
        | ("discrete", usvg::filter::TransferFunction::Discrete(values))
            if values.is_empty() =>
        {
            Ok(())
        }
        _ => Err("empty transfer table missing".to_owned()),
    }
}

#[rstest]
#[case::zero("0")]
#[case::zero_pair("0 0")]
#[case::negative("-1")]
#[case::negative_axis("0 -1")]
fn nonpositive_morphology_radius_preserves_the_filter_input(
    #[case] radius: &str,
) -> Result<(), String> {
    let candidate = fixture(&format!(
        "<feMorphology in='SourceGraphic' operator='erode' radius='{radius}'/>"
    ))?;
    let reference = fixture(IDENTITY)?;
    let actual = render(&candidate)?;
    let expected = render(&reference)?;
    assert!(expected.data().iter().any(|channel| *channel != 0));
    assert_eq!(actual.data(), expected.data(), "radius={radius}");
    Ok(())
}

#[test]
fn omitted_morphology_radius_preserves_the_filter_input() -> Result<(), String> {
    let actual = render(&fixture(
        "<feMorphology in='SourceGraphic' operator='erode'/>",
    )?)?;
    let expected = render(&fixture(IDENTITY)?)?;
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

#[rstest]
#[case::vertical("0 1", (4, 3), (3, 5))]
#[case::horizontal("1 0", (3, 5), (5, 3))]
fn zero_axis_erodes_only_along_the_other_axis(
    #[case] radius: &str,
    #[case] removed: (u32, u32),
    #[case] preserved: (u32, u32),
) -> Result<(), String> {
    let primitive =
        format!("<feMorphology in='SourceGraphic' operator='erode' radius='{radius}'/>");
    let image = render(&fixture(&primitive)?)?;
    assert_eq!(
        image
            .pixel(removed.0, removed.1)
            .ok_or("missing pixel")?
            .alpha(),
        0
    );
    assert!(
        image
            .pixel(preserved.0, preserved.1)
            .ok_or("missing pixel")?
            .alpha()
            > 0
    );
    Ok(())
}

#[rstest]
#[case::erode("erode", "1 1")]
#[case::dilate("dilate", "1 1")]
#[case::vertical("erode", "0 1")]
#[case::horizontal("dilate", "1 0")]
#[case::identity("erode", "0 0")]
fn morphology_writer_roundtrip_preserves_the_result(
    #[case] operator: &str,
    #[case] radius: &str,
) -> Result<(), String> {
    let primitive =
        format!("<feMorphology in='SourceGraphic' operator='{operator}' radius='{radius}'/>");
    let tree = fixture(&primitive)?;
    let xml = tree.to_string(&usvg::WriteOptions::default());
    let reparsed =
        usvg::Tree::from_str(&xml, &usvg::Options::default()).map_err(|error| error.to_string())?;
    assert_eq!(render(&tree)?.data(), render(&reparsed)?.data());
    if radius != "0 0" {
        assert_ne!(render(&tree)?.data(), render(&fixture(IDENTITY)?)?.data());
    }
    Ok(())
}

#[test]
fn generated_filter_result_avoids_an_existing_name() -> Result<(), String> {
    let tree = fixture("<feFlood result='result2'/><feOffset/>")?;
    let filter = tree.filters().first().ok_or("filter missing")?;
    let names: Vec<_> = filter
        .primitives()
        .iter()
        .map(|primitive| primitive.result())
        .collect();
    assert_eq!(names, ["result2", "result3"]);
    Ok(())
}

#[test]
fn overflowing_transformed_radius_clears_the_filter_without_panicking() -> Result<(), String> {
    let candidate = usvg::Tree::from_str(OVERFLOW_SVG, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_overflow_preconditions(&candidate)?;
    let control = OVERFLOW_SVG.replace("radius=\"2e38\"", "radius=\"0\"");
    let control = usvg::Tree::from_str(&control, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert!(render(&control)?.data().iter().any(|channel| *channel != 0));
    assert!(
        render(&candidate)?
            .data()
            .iter()
            .all(|channel| *channel == 0)
    );
    Ok(())
}

fn assert_overflow_preconditions(candidate: &usvg::Tree) -> Result<(), String> {
    let filter = candidate.filters().first().ok_or("filter missing")?;
    let primitive = filter.primitives().first().ok_or("morphology missing")?;
    let usvg::filter::Kind::Morphology(morphology) = primitive.kind() else {
        return Err("morphology missing".to_owned());
    };
    let radius = morphology.radius_x().get();
    assert!(radius.is_finite() && radius > f32::MAX / 2.0);
    let Some(usvg::Node::Group(group)) = candidate.node_by_id("filtered") else {
        return Err("filtered group missing".to_owned());
    };
    let scale = group.abs_transform().get_scale().0;
    assert!(scale.is_finite() && scale == 2.0 && (radius * scale).is_infinite());
    Ok(())
}
