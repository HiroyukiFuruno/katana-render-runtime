use super::tests::renderer;

#[test]
fn nonempty_painted_range_wraps_real_svg_content_with_opacity() {
    let mut renderer = renderer();
    let start = renderer.svg.len();
    renderer.svg.push_str("<rect/>");

    renderer.wrap_painted_range(start, 0.5);

    assert_eq!(&renderer.svg[start..], r#"<g opacity="0.5"><rect/></g>"#);
}
