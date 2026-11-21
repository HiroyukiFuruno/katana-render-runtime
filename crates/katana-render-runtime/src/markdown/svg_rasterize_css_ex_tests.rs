use super::font::bundled_font_db;
use resvg::usvg;

const PARENT_FONT_SIZE: f32 = 16.0;
const EX_MULTIPLIER: f32 = 4.0;
const EM_WIDTH: f32 = PARENT_FONT_SIZE;
const PERCENT_WIDTH: f32 = 50.0;
const PX_WIDTH: f32 = 7.0;

#[test]
fn css_ex_uses_bundled_font_metrics_and_preserves_other_units() -> Result<(), String> {
    let options = usvg::Options {
        fontdb: bundled_font_db(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(test_svg(), &options).map_err(|error| error.to_string())?;
    let expected_ex = oracle_ex(PARENT_FONT_SIZE, EX_MULTIPLIER)?;
    assert_text_font_size(&tree, expected_ex)?;
    assert_path_width(&tree, "ex-rect", expected_ex)?;
    assert_path_width(&tree, "em-rect", EM_WIDTH)?;
    assert_path_width(&tree, "percent-rect", PERCENT_WIDTH)?;
    assert_path_width(&tree, "px-rect", PX_WIDTH)
}

fn test_svg() -> &'static str {
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
      <g font-family="Noto Sans" font-size="16">
        <text id="ex-text" x="0" y="20" font-size="4ex">x</text>
        <rect id="ex-rect" x="0" y="30" width="4ex" height="1"/>
        <rect id="em-rect" x="0" y="40" width="1em" height="1"/>
        <rect id="percent-rect" x="0" y="50" width="50%" height="1"/>
        <rect id="px-rect" x="0" y="60" width="7px" height="1"/>
      </g>
    </svg>"#
}

fn oracle_ex(font_size: f32, multiplier: f32) -> Result<f32, String> {
    let face = rustybuzz::ttf_parser::Face::parse(
        include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"),
        0,
    )
    .map_err(|error| format!("Noto Sans oracle parse failed: {error:?}"))?;
    let x_height = face
        .x_height()
        .map(f32::from)
        .filter(|height| *height > 0.0)
        .or_else(|| {
            let glyph = face.glyph_index('x')?;
            Some(face.glyph_bounding_box(glyph)?.y_max as f32)
        })
        .ok_or("Noto Sans x-height oracle missing")?;
    let ratio = x_height / face.units_per_em() as f32;
    Ok(multiplier * (font_size * ratio))
}

fn assert_text_font_size(tree: &usvg::Tree, expected: f32) -> Result<(), String> {
    let node = tree.node_by_id("ex-text").ok_or("ex text missing")?;
    let usvg::Node::Text(text) = node else {
        return Err("ex node is not text".to_string());
    };
    let glyph = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .next()
        .ok_or("ex glyph missing")?;
    assert!((glyph.font_size() - expected).abs() < 0.0001);
    Ok(())
}

fn assert_path_width(tree: &usvg::Tree, id: &str, expected: f32) -> Result<(), String> {
    let node = tree
        .node_by_id(id)
        .ok_or_else(|| format!("missing path {id}"))?;
    let width = node.abs_bounding_box().width();
    assert!((width - expected).abs() < 0.0001, "width for {id}: {width}");
    Ok(())
}
