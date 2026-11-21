use super::super::style::CssStyle;
use super::inherit_svg_text_style;
use resvg::usvg::{self, Node, Paint, Tree};

type Rgb = (u8, u8, u8);

const PARENT_RGB: Rgb = (224, 224, 224);
const CHANGED_PARENT_RGB: Rgb = (34, 68, 102);
const INNER_RGB: Rgb = (0, 255, 0);
const EXPLICIT_STROKE_RGB: Rgb = (18, 52, 86);
const HALF_ALPHA_U8_VALUE: f32 = 128.0 / 255.0;
const PARENT_COLOR_CASES: [(&str, Rgb); 2] =
    [("#e0e0e0", PARENT_RGB), ("#224466", CHANGED_PARENT_RGB)];
const CURRENT_COLOR_MARKUP: &str = r##"<path id="parent-path" d="M0 1H10" fill="none" stroke="currentColor"/><line id="parent-line" x1="0" y1="2" x2="10" y2="2" stroke="currentColor"/><g color="#00ff00"><path id="inner-color" d="M0 3H10" fill="none" stroke="currentColor"/></g><path id="explicit-stroke" d="M0 4H10" fill="none" color="#ff0000" stroke="#123456"/>"##;

#[test]
fn embedded_svg_preserves_current_color_and_explicit_strokes() -> Result<(), String> {
    for (parent_color, expected_rgb) in PARENT_COLOR_CASES {
        let tree = parse_wrapped_svg(CURRENT_COLOR_MARKUP, parent_color)?;
        assert_parent_strokes(&tree, svg_color(expected_rgb))?;
        assert_eq!(stroke_color(&tree, "inner-color")?, svg_color(INNER_RGB));
        assert_eq!(
            stroke_color(&tree, "explicit-stroke")?,
            svg_color(EXPLICIT_STROKE_RGB)
        );
    }
    Ok(())
}

#[test]
fn current_color_alpha_becomes_stroke_opacity() -> Result<(), String> {
    let tree = parse_wrapped_svg(
        r#"<path id="alpha-stroke" d="M0 1H10" fill="none" stroke="currentColor"/>"#,
        "rgba(224, 224, 224, 0.5)",
    )?;
    let Some(Node::Path(path)) = tree.node_by_id("alpha-stroke") else {
        return Err("SVG alpha path missing".to_owned());
    };
    let stroke = path.stroke().ok_or("SVG alpha path has no stroke")?;
    assert_eq!(stroke.paint(), &Paint::Color(svg_color(PARENT_RGB)));
    assert!((stroke.opacity().get() - HALF_ALPHA_U8_VALUE).abs() < f32::EPSILON);
    Ok(())
}

fn parse_wrapped_svg(markup: &str, parent_color: &str) -> Result<Tree, String> {
    let mut style = CssStyle::browser_default();
    style.color = parent_color.to_owned();
    let wrapped = inherit_svg_text_style(markup, &style);
    let document =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="8">{wrapped}</svg>"#);
    Tree::from_str(&document, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn assert_parent_strokes(tree: &Tree, expected: usvg::Color) -> Result<(), String> {
    assert_eq!(stroke_color(tree, "parent-path")?, expected);
    assert_eq!(stroke_color(tree, "parent-line")?, expected);
    Ok(())
}

fn svg_color((red, green, blue): Rgb) -> usvg::Color {
    usvg::Color::new_rgb(red, green, blue)
}

fn stroke_color(tree: &Tree, id: &str) -> Result<usvg::Color, String> {
    let Some(Node::Path(path)) = tree.node_by_id(id) else {
        return Err(format!("SVG path {id} missing"));
    };
    let Some(stroke) = path.stroke() else {
        return Err(format!("SVG path {id} has no stroke"));
    };
    match stroke.paint() {
        Paint::Color(color) => Ok(*color),
        _ => Err(format!("SVG path {id} stroke is not a solid color")),
    }
}
