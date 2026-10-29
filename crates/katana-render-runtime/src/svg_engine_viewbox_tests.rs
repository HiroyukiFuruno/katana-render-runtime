use rstest::rstest;
use tiny_skia::Transform;

use super::usvg;

const EXPECTED_COMPONENTS: usize = 3;

#[rstest]
#[case::en_c4_container(1272, 886, 1248, "861.1719", "861.1848754882812", [0.9999849, 12.00942, 81.99894])]
#[case::ja_c4_container(1272, 897, 1248, "872.6719", "872.6860580444336", [0.9999838, 12.010125, 81.99886])]
#[case::ja_c4_component(956, 792, 932, "767.2969", "767.3117218017578", [0.9999807, 12.009007, 81.99865])]
fn nested_viewbox_matches_browser_transform(
    #[case] frame_width: u32,
    #[case] frame_height: u32,
    #[case] viewport_width: u32,
    #[case] viewport_height: &str,
    #[case] viewbox_height: &str,
    #[case] expected: [f32; EXPECTED_COMPONENTS],
) -> Result<(), String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{frame_width}" height="{frame_height}" viewBox="0 0 {frame_width} {frame_height}"><svg x="12" y="12" width="{viewport_width}" height="{viewport_height}" viewBox="0 -70 {viewport_width} {viewbox_height}"><path id="target" d="M0 0 H20 V20 H0 Z"/></svg></svg>"#
    );
    let tree =
        usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|error| error.to_string())?;
    let node = tree.node_by_id("target").ok_or("target path missing")?;
    let [scale, x, y] = expected;
    assert_eq!(
        node.abs_transform(),
        Transform::from_row(scale, 0.0, 0.0, scale, x, y)
    );
    Ok(())
}
