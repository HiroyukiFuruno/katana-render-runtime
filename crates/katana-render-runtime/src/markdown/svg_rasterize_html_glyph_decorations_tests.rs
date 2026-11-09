use super::super::{composite::TextRasterizer, font_file_fixture};
use super::{ALPHA_CHANNEL, HtmlGlyphRasterizer, Prepared, PreparedSpan, RGBA_CHANNELS};
use resvg::usvg::{Node, Options, PaintOrder, Tree};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tiny_skia::{Color, Pixmap, Transform};

#[path = "svg_rasterize_html_glyph_decorations_scale_tests.rs"]
mod scale_tests;

#[path = "svg_rasterize_html_glyph_stroke_alignment_tests.rs"]
mod stroke_alignment_tests;

const WIDTH: u32 = 192;
const HEIGHT: u32 = 96;
const STROKE: &str = "stroke='#ccc' stroke-width='6'";
const UNDERLINE: &str = "text-decoration='underline'";
const CONTENT: &str = "Links";
const TRANSLATION_X: f32 = 9.0;
static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn fixture(
    style: &str,
    content: &str,
) -> Result<(Tree, font_file_fixture::FontFileFixture), String> {
    let index = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("krr-decoration-{}-{index}.ttf", std::process::id()));
    let fixture = font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{WIDTH}' height='{HEIGHT}'><defs><linearGradient id='grad'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><text id='run' x='24' y='48' font-family='Noto Sans' font-size='28' fill='#555' {style}>{content}</text></svg>"
    );
    let tree = Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(fixture.database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    Ok((tree, fixture))
}

fn text(tree: &Tree) -> Result<&resvg::usvg::Text, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("decoration text missing".to_owned());
    };
    Ok(text)
}

fn render(style: &str, transform: Transform) -> Result<Pixmap, String> {
    render_sized(style, transform, WIDTH, HEIGHT)
}

fn render_sized(
    style: &str,
    transform: Transform,
    width: u32,
    height: u32,
) -> Result<Pixmap, String> {
    let (tree, _fixture) = fixture(style, CONTENT)?;
    let text = text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(prepared) = rasterizer.prepare(text)? else {
        return Err("decorated text was not prepared".to_owned());
    };
    assert_prepared(text, &prepared);
    let mut pixmap = Pixmap::new(width, height).ok_or("pixmap unavailable")?;
    assert!(rasterizer.render(text, transform, &mut pixmap.as_mut())?);
    assert!(
        pixmap
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    );
    Ok(pixmap)
}

fn assert_prepared(text: &resvg::usvg::Text, prepared: &[PreparedSpan]) {
    assert_eq!(text.layouted().len(), 1);
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].baseline_shift, 0.0);
    if super::decorations::has_effects(&text.layouted()[0]) {
        assert!(prepared[0].effects.is_some());
        let span = &text.layouted()[0];
        if super::decorations::has_decorations(span) {
            assert!(prepared[0].x_alignment.is_none());
            assert!(prepared[0].chunk_alignment.is_none());
            assert!(prepared[0].fallback_alignment.is_none());
        }
    }
}

#[test]
fn solid_stroke_and_underline_are_both_drawn_with_fill() -> Result<(), String> {
    let plain = render("", Transform::identity())?;
    let stroke = render(STROKE, Transform::identity())?;
    let underline = render(UNDERLINE, Transform::identity())?;
    let both = render(&format!("{STROKE} {UNDERLINE}"), Transform::identity())?;
    assert_ne!(plain.data(), stroke.data());
    assert_ne!(plain.data(), underline.data());
    assert_ne!(both.data(), stroke.data());
    assert_ne!(both.data(), underline.data());
    let middle = render(
        &format!("{STROKE} {UNDERLINE} text-anchor='middle'"),
        Transform::identity(),
    )?;
    assert_ne!(both.data(), middle.data());
    Ok(())
}

#[test]
fn stroke_extents_survive_crop_and_translate_with_text() -> Result<(), String> {
    let plain = render("", Transform::identity())?;
    let stroke = render(STROKE, Transform::identity())?;
    let moved = render(STROKE, Transform::from_translate(TRANSLATION_X, 0.0))?;
    let plain_bounds = alpha_bounds(&plain).ok_or("plain glyph bounds missing")?;
    let bounds = alpha_bounds(&stroke).ok_or("stroke bounds missing")?;
    let moved_bounds = alpha_bounds(&moved).ok_or("translated stroke bounds missing")?;
    assert!(bounds.0 < plain_bounds.0 && bounds.1 > plain_bounds.1);
    assert_eq!(
        moved_bounds,
        (
            bounds.0 + TRANSLATION_X as u32,
            bounds.1 + TRANSLATION_X as u32
        )
    );
    Ok(())
}

fn alpha_bounds(pixmap: &Pixmap) -> Option<(u32, u32)> {
    let mut min = WIDTH;
    let mut max = 0;
    for (index, pixel) in pixmap.data().chunks_exact(RGBA_CHANNELS).enumerate() {
        if pixel[ALPHA_CHANNEL] != 0 {
            let x = index as u32 % pixmap.width();
            min = min.min(x);
            max = max.max(x);
        }
    }
    (min <= max).then_some((min, max))
}

#[test]
fn unsupported_effects_leave_existing_pixels_unchanged() -> Result<(), String> {
    for (style, content) in [
        ("stroke='url(#grad)'", CONTENT),
        ("stroke='#ccc' stroke-dasharray='2 2'", CONTENT),
        ("stroke='#ccc'", "<tspan>Li</tspan><tspan>nks</tspan>"),
    ] {
        assert_rejected(style, content)?;
    }
    Ok(())
}

#[test]
fn underline_with_multiple_layout_spans_is_rejected_without_painting() -> Result<(), String> {
    let content = "<tspan font-size='20'>Li</tspan><tspan font-size='28'>nks</tspan>";
    let (tree, _fixture) = fixture(UNDERLINE, content)?;
    let text = text(&tree)?;
    assert_eq!(text.layouted().len(), 2);
    assert!(text.layouted().iter().any(super::decorations::has_effects));
    assert_tree_rejected(&tree)
}

#[test]
fn decorated_nonzero_baseline_is_rejected_without_painting() -> Result<(), String> {
    let style = "text-decoration='underline' dominant-baseline='central'";
    let (tree, _fixture) = fixture(style, CONTENT)?;
    let text = text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    assert_nonzero_effect_span_prepares(&rasterizer, text)?;
    assert!(matches!(
        rasterizer.prepare(text)?,
        super::Prepared::Unsupported
    ));
    assert_tree_rejected(&tree)
}

fn assert_nonzero_effect_span_prepares(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &resvg::usvg::Text,
) -> Result<(), String> {
    let [span] = text.layouted() else {
        return Err("baseline guard requires one layout span".to_owned());
    };
    if !super::decorations::has_effects(span) || span.positioned_glyphs.is_empty() {
        return Err("baseline guard requires an effected glyph span".to_owned());
    }
    let policy = super::baseline::resolved_policy(rasterizer, text)?
        .ok_or("central baseline policy missing")?;
    let mut prepared = Vec::new();
    if !rasterizer.prepare_span(span, Some(policy), &mut prepared)? {
        return Err("baseline guard span was rejected before preparation".to_owned());
    }
    if prepared.len() != 1 || prepared[0].baseline_shift == 0.0 {
        return Err("baseline guard requires one shifted prepared span".to_owned());
    }
    Ok(())
}

#[test]
fn nondefault_paint_order_without_stroke_is_rejected_without_painting() -> Result<(), String> {
    let (tree, _fixture) = fixture("paint-order='stroke fill'", CONTENT)?;
    let text = text(&tree)?;
    let [span] = text.layouted() else {
        return Err("paint-order fixture must produce one layout span".to_owned());
    };
    assert!(span.visible);
    assert!(!span.positioned_glyphs.is_empty());
    assert!(span.fill.is_some());
    assert!(span.stroke.is_none());
    assert_eq!(span.paint_order, PaintOrder::StrokeAndFill);
    assert!(!super::decorations::has_effects(span));
    assert_tree_rejected(&tree)
}

fn assert_rejected(style: &str, content: &str) -> Result<(), String> {
    let (tree, _fixture) = fixture(style, content)?;
    assert_tree_rejected(&tree)
}

fn assert_tree_rejected(tree: &Tree) -> Result<(), String> {
    let text = text(tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap unavailable")?;
    pixmap.fill(Color::WHITE);
    let before = pixmap.data().to_vec();
    assert!(!rasterizer.render(text, Transform::identity(), &mut pixmap.as_mut())?);
    assert_eq!(pixmap.data(), before);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn multiple_physical_faces_with_stroke_are_rejected_without_painting() -> Result<(), String> {
    let content = "A日本";
    let families = "Trebuchet MS, Hiragino Kaku Gothic ProN";
    let database = super::super::font::html_font_db_for_text(families, content);
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' x='24' y='48' font-family='Trebuchet MS' font-size='28' fill='#555' {STROKE}>{content}</text></svg>"
    );
    let tree = Tree::from_str(
        &svg,
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let text = text(&tree)?;
    assert_eq!(text.layouted().len(), 1);
    let glyphs = &text.layouted()[0].positioned_glyphs;
    let first = glyphs.first().ok_or("fixture glyphs missing")?;
    assert!(glyphs.iter().any(|glyph| glyph.font != first.font));
    assert_tree_rejected(&tree)
}

#[test]
fn translucent_decoration_paths_decline_native_text_without_painting() -> Result<(), String> {
    for decoration in ["underline", "line-through"] {
        let style = format!("text-decoration='{decoration}' fill-opacity='0.5'");
        let (tree, _fixture) = fixture(&style, CONTENT)?;
        let text = text(&tree)?;
        let [span] = text.layouted() else {
            return Err("expected one decorated layout span".into());
        };
        let path = match decoration {
            "underline" => span.underline.as_ref(),
            _ => span.line_through.as_ref(),
        }
        .ok_or("decoration path missing")?;
        assert_eq!(path.fill().map(|fill| fill.opacity().get()), Some(0.5));
        assert!(super::decorations::prepare(span)?.is_none());
        assert_tree_rejected(&tree)?;
    }
    Ok(())
}
