use super::usvg;
use rstest::rstest;

fn parse_path(attributes: &str) -> Result<usvg::Tree, String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'><path id='sample' d='M 2 2 H 18 V 18 Z' {attributes}/></svg>"
    );
    usvg::Tree::from_str(&source, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn sample_path(tree: &usvg::Tree) -> Result<&usvg::Path, String> {
    match tree.node_by_id("sample") {
        Some(usvg::Node::Path(path)) => Ok(path),
        _ => Err("sample path missing".to_owned()),
    }
}

#[rstest]
#[case::negative("2 -1", None)]
#[case::zero("0 0", None)]
#[case::odd("2 3 4", Some(&[2.0, 3.0, 4.0, 2.0, 3.0, 4.0][..]))]
#[case::even("2 3", Some(&[2.0, 3.0][..]))]
fn stroke_dasharray_rejects_invalid_and_repeats_odd_lists(
    #[case] dasharray: &str,
    #[case] expected: Option<&[f32]>,
) -> Result<(), String> {
    let tree = parse_path(&format!("stroke='black' stroke-dasharray='{dasharray}'"))?;
    let stroke = sample_path(&tree)?.stroke().ok_or("stroke missing")?;
    assert_eq!(stroke.dasharray(), expected);
    Ok(())
}

#[test]
fn invalid_stroke_line_values_use_svg_defaults() -> Result<(), String> {
    let tree = parse_path("stroke='black' stroke-linecap='invalid' stroke-linejoin='invalid'")?;
    let stroke = sample_path(&tree)?.stroke().ok_or("stroke missing")?;
    assert_eq!(stroke.linecap(), usvg::LineCap::Butt);
    assert_eq!(stroke.linejoin(), usvg::LineJoin::Miter);
    Ok(())
}

#[test]
fn invalid_fill_rule_uses_nonzero_default() -> Result<(), String> {
    let tree = parse_path("fill-rule='invalid'")?;
    let fill = sample_path(&tree)?.fill().ok_or("fill missing")?;
    assert_eq!(fill.rule(), usvg::FillRule::NonZero);
    Ok(())
}

#[rstest]
#[case::explicit_color("url(#missing) #123456", "", Some((18, 52, 86)))]
#[case::current_color("url(#missing) currentColor", "color='#abcdef'", Some((171, 205, 239)))]
#[case::none("url(#missing) none", "", None)]
fn missing_paint_server_uses_the_authored_fallback(
    #[case] fill: &str,
    #[case] extra: &str,
    #[case] expected: Option<(u8, u8, u8)>,
) -> Result<(), String> {
    let tree = parse_path(&format!("fill='{fill}' stroke='black' {extra}"))?;
    let actual = match sample_path(&tree)?.fill() {
        Some(fill) => match fill.paint() {
            usvg::Paint::Color(color) => Some((color.red, color.green, color.blue)),
            _ => return Err("fallback was not a color".to_owned()),
        },
        None => None,
    };
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn malformed_fill_and_stroke_fall_back_independently() -> Result<(), String> {
    let tree = parse_path("fill='invalid' stroke='invalid'")?;
    let path = sample_path(&tree)?;
    let fill = path.fill().ok_or("default black fill missing")?;
    assert!(matches!(fill.paint(), usvg::Paint::Color(color)
        if (color.red, color.green, color.blue) == (0, 0, 0)));
    assert!(path.stroke().is_none());
    Ok(())
}

#[test]
fn current_color_resolves_independently_for_fill_and_stroke() -> Result<(), String> {
    let tree = parse_path("color='#123456' fill='currentColor' stroke='currentColor'")?;
    let path = sample_path(&tree)?;
    let fill = path.fill().ok_or("fill missing")?;
    let stroke = path.stroke().ok_or("stroke missing")?;
    for paint in [fill.paint(), stroke.paint()] {
        assert!(matches!(paint, usvg::Paint::Color(color)
            if (color.red, color.green, color.blue) == (18, 52, 86)));
    }
    Ok(())
}

#[test]
fn inherited_current_color_uses_child_color_after_style_override() -> Result<(), String> {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
      <g fill="currentColor" color="red"><path id="sample" d="M2 2 H18 V18 Z"
      fill="inherit" style="color: #123456"/></g></svg>"#;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let fill = sample_path(&tree)?.fill().ok_or("fill missing")?;
    assert!(matches!(fill.paint(), usvg::Paint::Color(color)
        if (color.red, color.green, color.blue) == (18, 52, 86)));
    Ok(())
}

#[test]
fn inherited_initial_color_keeps_other_inline_paint_declarations() -> Result<(), String> {
    let tree = parse_path("fill='currentColor' style='color: inherit; stroke: currentColor'")?;
    let path = sample_path(&tree)?;
    let fill = path.fill().ok_or("fill missing")?;
    let stroke = path.stroke().ok_or("stroke missing")?;
    for paint in [fill.paint(), stroke.paint()] {
        assert!(matches!(paint, usvg::Paint::Color(color) if *color == usvg::Color::black()));
    }
    Ok(())
}
