use super::{FONT_FAMILY, MULTILINE, additional_middle_layout_supported, fixture, source, text};
use crate::markdown::svg_rasterize::{
    composite::SvgCompositeRenderer, html_glyph::HtmlGlyphRasterizer, provenance::AuthoredClipInfo,
};
use resvg::usvg::{Options, TextFlow, Tree};
use tiny_skia::{Pixmap, Transform};

const SVG_WIDTH: u32 = 120;
const SVG_HEIGHT: u32 = 80;
const VISIBLE_TEXT_ROWS: [(u32, u32); 2] = [(10, 30), (50, 70)];

#[test]
fn middle_multichunk_text_path_is_excluded_and_remains_visible() -> Result<(), String> {
    let tree = text_path_fixture()?;
    let run = text(&tree)?;
    let chunks = run.chunks();
    assert!(chunks.len() >= 2);
    assert!(matches!(chunks[0].text_flow(), TextFlow::Linear));
    assert!(
        chunks
            .iter()
            .skip(1)
            .any(|chunk| matches!(chunk.text_flow(), TextFlow::Path(_)))
    );
    let control = fixture(MULTILINE, "")?;
    let first = chunks[0].spans().first().ok_or("first span missing")?;
    let control_first = text(&control)?.chunks()[0]
        .spans()
        .first()
        .ok_or("control span missing")?;
    assert_eq!(first.font(), control_first.font());
    assert_eq!(first.dominant_baseline(), control_first.dominant_baseline());
    assert_eq!(first.letter_spacing(), 0.0);
    assert_eq!(first.word_spacing(), 0.0);
    assert!(!additional_middle_layout_supported(run));
    assert_visible(&tree)
}

fn text_path_fixture() -> Result<Tree, String> {
    let body = "<tspan x='0' dy='0'>AB</tspan><textPath href='#curve'>CD</textPath>";
    let svg = source(body, "", FONT_FAMILY)
        .replace(
            "<svg xmlns=",
            &format!("<svg width='{SVG_WIDTH}' height='{SVG_HEIGHT}' xmlns="),
        )
        .replace(
            "<text id='run'",
            "<defs><path id='curve' d='M0 60 H100'/></defs><text id='run'",
        );
    let font_fixture = fixture("AB", "")?;
    let options = Options {
        fontdb: font_fixture.fontdb().clone(),
        ..Options::default()
    };
    Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}

fn assert_visible(tree: &Tree) -> Result<(), String> {
    let mut pixels = Pixmap::new(SVG_WIDTH, SVG_HEIGHT).ok_or("pixmap allocation failed")?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    SvgCompositeRenderer::render_with_text(
        tree,
        Transform::identity(),
        &mut pixels.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    for (start, end) in VISIBLE_TEXT_ROWS {
        assert!(
            (start..end).any(|y| (0..SVG_WIDTH)
                .any(|x| { pixels.pixel(x, y).is_some_and(|pixel| pixel.alpha() > 0) })),
            "text missing in rows {start}..{end}"
        );
    }
    Ok(())
}
