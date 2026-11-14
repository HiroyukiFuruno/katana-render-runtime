use super::{ProbeResult, Value, usvg};
use crate::markdown::svg_rasterize::font::html_rasterizer_options;
use std::sync::{Arc, atomic::Ordering};

const WIDTH: u32 = 200;
const HEIGHT: u32 = 80;
const CLIP_WIDTH: f32 = 25.0;
const BOLD_FONT_WEIGHT: u16 = 700;
const PIXEL_CHANNELS: usize = 4;
const ALPHA_CHANNEL_INDEX: usize = 3;
const BBOX_COORDINATE_COUNT: usize = 4;
const ALIAS_FAMILY: &str = "Recursive Variable, Noto Sans";

#[derive(Debug, PartialEq)]
struct Snapshot {
    bbox: [f32; BBOX_COORDINATE_COUNT],
    spans: Vec<Value>,
    requested_weights: Vec<u16>,
    rgba: Vec<u8>,
}

fn scene(clipped: bool) -> String {
    let clip = if clipped {
        "clip-path=\"url(#window)\""
    } else {
        ""
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}"><defs><clipPath id="window" clipPathUnits="userSpaceOnUse"><rect x="0" y="0" width="{CLIP_WIDTH}" height="{HEIGHT}"/></clipPath></defs><g transform="translate(7 3)"><g {clip}><text id="krr-text-measure" x="0" y="40" font-family="{ALIAS_FAMILY}" font-size="14" font-weight="{BOLD_FONT_WEIGHT}"><tspan>Noto</tspan><tspan font-family="Noto Sans" font-weight="400"> Sans</tspan></text></g></g></svg>"#
    )
}

fn render_pixels(tree: &usvg::Tree) -> ProbeResult<Vec<u8>> {
    let mut pixels = tiny_skia::Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap unavailable")?;
    resvg::render(tree, tiny_skia::Transform::identity(), &mut pixels.as_mut());
    Ok(pixels.data().to_vec())
}

fn observe_geometry(svg: &str, options: &usvg::Options<'static>) -> ProbeResult<Snapshot> {
    crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        let tree = usvg::Tree::from_str(svg, options)?;
        let node = tree
            .node_by_id("krr-text-measure")
            .ok_or("text node missing")?;
        let usvg::Node::Text(text) = node else {
            return Err("expected real text node".into());
        };
        let bbox = node.abs_bounding_box();
        let snapshot = Snapshot {
            bbox: [bbox.x(), bbox.y(), bbox.width(), bbox.height()],
            spans: super::super::super::observe_spans(text, tree.fontdb())?,
            requested_weights: text
                .chunks()
                .iter()
                .flat_map(|chunk| chunk.spans())
                .map(|span| span.font().weight())
                .collect(),
            rgba: render_pixels(&tree)?,
        };
        Ok(snapshot)
    })
}

fn assert_real_nested_text(snapshot: &Snapshot, id: usvg::fontdb::ID) -> ProbeResult<()> {
    assert!(snapshot.bbox.iter().all(|value| value.is_finite()));
    assert!(snapshot.bbox[2] > CLIP_WIDTH && snapshot.bbox[3] > 0.0);
    assert!(snapshot.requested_weights.contains(&BOLD_FONT_WEIGHT));
    assert!(
        snapshot
            .requested_weights
            .contains(&super::NORMAL_FONT_WEIGHT)
    );
    assert_eq!(
        super::super::super::missing_glyph_count(&snapshot.spans)?,
        0
    );
    super::assert_selected_noto_face(&snapshot.spans, id)?;
    Ok(())
}

fn alpha_pixels(snapshot: &Snapshot) -> usize {
    snapshot
        .rgba
        .chunks_exact(PIXEL_CHANNELS)
        .filter(|pixel| pixel[ALPHA_CHANNEL_INDEX] > 0)
        .count()
}

fn face_ids(database: &usvg::fontdb::Database) -> Vec<usvg::fontdb::ID> {
    database.faces().map(|face| face.id).collect()
}

fn assert_regular_font(database: &usvg::fontdb::Database, id: usvg::fontdb::ID) -> ProbeResult<()> {
    let face = database
        .face(id)
        .ok_or("actual bundled Noto face missing")?;
    assert_eq!(face.weight.0, super::NORMAL_FONT_WEIGHT);
    Ok(())
}

fn compare_original_and_alias(svg: &str) -> ProbeResult<Snapshot> {
    let (official, id) = super::noto_options(svg)?;
    assert_regular_font(&official.fontdb, id)?;
    let weak = Arc::downgrade(&official.fontdb);
    let original_ids = face_ids(&official.fontdb);
    let expected = observe_geometry(svg, &official)?;
    assert_real_nested_text(&expected, id)?;
    let mut wrapped = html_rasterizer_options(svg);
    wrapped.fontdb = Arc::clone(&official.fontdb);
    let (original_calls, alias_calls) = super::count_original_and_alias_selectors(&mut wrapped, id);
    let actual = observe_geometry(svg, &wrapped)?;
    assert!(original_calls.load(Ordering::Relaxed) > 0);
    assert!(alias_calls.load(Ordering::Relaxed) > 0);
    assert_eq!(
        actual, expected,
        "alias changed real glyphs, bbox or rendered pixels"
    );
    assert_eq!(face_ids(&official.fontdb), original_ids);
    drop(wrapped);
    drop(official);
    assert!(
        weak.upgrade().is_none(),
        "snapshot retained font database after tree drop"
    );
    Ok(actual)
}

#[test]
fn original_and_alias_preserve_nested_weight_fallback_bbox_and_pixels() -> ProbeResult<()> {
    /* WHY: 太字実体がない実Noto DBへの公式weight fallbackとalias経路を同じ描画条件で対照する。 */
    let snapshot = compare_original_and_alias(&scene(false))?;
    assert!(alpha_pixels(&snapshot) > 0);
    Ok(())
}

#[test]
fn original_and_alias_preserve_clipping_without_clipping_text_measurement() -> ProbeResult<()> {
    let unclipped = compare_original_and_alias(&scene(false))?;
    let clipped = compare_original_and_alias(&scene(true))?;
    assert_eq!(
        clipped.bbox, unclipped.bbox,
        "clip must not rewrite glyph ink measurement"
    );
    assert_eq!(clipped.spans, unclipped.spans);
    assert!(alpha_pixels(&clipped) > 0);
    assert!(
        alpha_pixels(&clipped) < alpha_pixels(&unclipped),
        "clip negative control did not cut actual ink"
    );
    Ok(())
}
