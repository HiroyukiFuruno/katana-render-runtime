use super::{HtmlGlyphRasterizer, InstanceKey, VariableInstance};
use crate::markdown::svg_rasterize::html_glyph::{Prepared, PreparedSpan, TextRasterizer};
use resvg::usvg::{
    self, FontVariation, Node, Options, Tree,
    fontdb::{Database, ID, Source},
};
use skrifa::{FontRef, MetadataProvider, Tag};
use std::{path::PathBuf, rc::Rc, sync::Arc};
use tiny_skia::{Pixmap, Transform};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 100;
const FONT_SIZE: f32 = 20.0;
const REGULAR_WEIGHT: f32 = 400.0;
const BOLD_WEIGHT: f32 = 700.0;
const WEIGHT_AXIS: Tag = Tag::from_be_bytes(*b"wght");
const OPTICAL_AXIS: Tag = Tag::from_be_bytes(*b"opsz");

#[path = "svg_rasterize_html_glyph_tracking_tests.rs"]
mod tracking;

fn variable_tree(content: &str) -> Result<Tree, String> {
    let mut database = Database::new();
    let ids = database.load_font_source(Source::File(PathBuf::from(
        "/System/Library/Fonts/SFNS.ttf",
    )));
    let id = *ids.first().ok_or("SFNS variable face unavailable")?;
    let face = database.face(id).ok_or("SFNS face missing")?;
    let family = &face.families.first().ok_or("SFNS family missing")?.0;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}"><g font-family="{family}" font-size="{FONT_SIZE}">{content}</g></svg>"#,
    );
    Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(database),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn text<'a>(tree: &'a Tree, id: &str) -> Result<&'a usvg::Text, String> {
    let Some(Node::Text(text)) = tree.node_by_id(id) else {
        return Err(format!("variable fixture text {id} missing"));
    };
    Ok(text)
}

fn prepared(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
) -> Result<PreparedSpan, String> {
    let Prepared::Ready(mut spans) = rasterizer.prepare(text)? else {
        return Err("variable text fell back from Skia".into());
    };
    if spans.len() != 1 {
        return Err("variable fixture did not yield one prepared span".into());
    }
    spans.pop().ok_or("prepared variable span missing".into())
}

fn instance(span: &PreparedSpan) -> Result<&Rc<VariableInstance>, String> {
    span.variable_instance
        .as_ref()
        .ok_or("variable instance missing".into())
}

fn axis(instance: &VariableInstance, tag: Tag) -> Result<f32, String> {
    instance
        .applied_axes
        .iter()
        .find(|axis| axis.tag == tag.to_be_bytes())
        .map(|axis| axis.value)
        .ok_or_else(|| format!("applied axis {tag:?} missing"))
}

#[test]
fn variable_regular_and_bold_share_physical_face_without_changing_layout() -> Result<(), String> {
    let tree = variable_tree(
        r#"<text id="regular" x="10" y="30" font-weight="400">Tea Shop</text><text id="bold" x="10" y="60" font-weight="700">Tea Shop</text>"#,
    )?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut font_ids = Vec::new();
    for (id, weight) in [("regular", 400.0), ("bold", 700.0)] {
        let text = text(&tree, id)?;
        let span = prepared(&rasterizer, text)?;
        assert_preserved_layout(&span, text);
        assert_instance_readback(&span, weight)?;
        font_ids.push(span.font);
    }
    assert_eq!(font_ids[0], font_ids[1]);
    assert_eq!(rasterizer.variable_faces.borrow().bases.len(), 1);
    assert_eq!(rasterizer.variable_faces.borrow().instances.len(), 2);
    Ok(())
}

fn assert_preserved_layout(span: &PreparedSpan, text: &usvg::Text) {
    let original = &text.layouted()[0].positioned_glyphs;
    assert_eq!(span.glyphs.len(), original.len());
    for (actual, original) in span.glyphs.iter().zip(original) {
        assert_eq!(actual.id, original.id);
        assert_eq!(actual.transform(), original.transform());
    }
}

fn assert_instance_readback(span: &PreparedSpan, weight: f32) -> Result<(), String> {
    let instance = instance(span)?;
    assert_eq!(axis(instance, WEIGHT_AXIS)?, weight);
    let typeface = instance.face.typeface.as_skia()?;
    let readback = typeface
        .variation_design_position()
        .ok_or("readback missing")?;
    let actual = readback
        .iter()
        .map(|axis| FontVariation::new((*axis.axis).to_be_bytes(), axis.value))
        .collect::<Vec<_>>();
    assert_eq!(
        InstanceKey::new(span.font, &actual)?,
        InstanceKey::new(span.font, &instance.applied_axes)?
    );
    Ok(())
}

#[test]
fn optical_modes_and_sizes_reuse_only_the_same_applied_instance() -> Result<(), String> {
    let tree = variable_tree(
        r#"<text id="auto" x="10" y="25">Tea</text><text id="none" x="10" y="50" font-optical-sizing="none">Tea</text><text id="explicit" x="80" y="50" font-variation-settings="'opsz' 28">Tea</text><text id="resized" x="10" y="80" font-size="24" font-optical-sizing="none">Tea</text>"#,
    )?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let auto = prepared(&rasterizer, text(&tree, "auto")?)?;
    let none = prepared(&rasterizer, text(&tree, "none")?)?;
    let explicit = prepared(&rasterizer, text(&tree, "explicit")?)?;
    let resized = prepared(&rasterizer, text(&tree, "resized")?)?;
    assert_optical_modes(&tree, &auto, &none, &explicit)?;
    assert!(Rc::ptr_eq(instance(&none)?, instance(&resized)?));
    assert_ne!(
        instance(&auto)?.face.typeface.as_skia()?.unique_id(),
        instance(&explicit)?.face.typeface.as_skia()?.unique_id()
    );
    Ok(())
}

fn assert_optical_modes(
    tree: &Tree,
    auto: &PreparedSpan,
    none: &PreparedSpan,
    explicit: &PreparedSpan,
) -> Result<(), String> {
    let (minimum, maximum, default) = optical_axis_limits(tree, auto.font)?;
    assert_eq!(
        axis(instance(auto)?, OPTICAL_AXIS)?,
        FONT_SIZE.clamp(minimum, maximum)
    );
    assert_eq!(axis(instance(none)?, OPTICAL_AXIS)?, default);
    assert_eq!(
        axis(instance(explicit)?, OPTICAL_AXIS)?,
        28.0_f32.clamp(minimum, maximum)
    );
    Ok(())
}

fn optical_axis_limits(tree: &Tree, font_id: ID) -> Result<(f32, f32, f32), String> {
    tree.fontdb()
        .with_face_data(font_id, |data, index| {
            let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            let axis = font.axes().get_by_tag(OPTICAL_AXIS).ok_or("opsz missing")?;
            Ok::<_, String>((axis.min_value(), axis.max_value(), axis.default_value()))
        })
        .ok_or("font bytes missing")?
}

#[test]
fn variable_paint_capture_uses_cached_readback_and_renders_weight_difference() -> Result<(), String>
{
    let tree = variable_tree(
        r#"<text id="regular" x="10" y="30" font-weight="400">Tea Shop</text><text id="bold" x="10" y="30" font-weight="700">Tea Shop</text>"#,
    )?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let scope = super::super::paint::PaintCaptureScope::enter();
    let mut images = Vec::new();
    for id in ["regular", "bold"] {
        images.push(render_text_pixels(&rasterizer, text(&tree, id)?)?);
    }
    assert_ne!(images[0], images[1]);
    let snapshot = scope.snapshot();
    assert_eq!(snapshot.events.len(), 2);
    assert_eq!(snapshot.dropped, 0);
    assert_captured_weights(&snapshot.events)
}

fn render_text_pixels(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
) -> Result<Vec<u8>, String> {
    let mut pixels = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap allocation failed")?;
    assert!(rasterizer.render(text, Transform::identity(), &mut pixels.as_mut())?);
    assert!(pixels.data().iter().any(|byte| *byte != 0));
    Ok(pixels.data().to_vec())
}

fn assert_captured_weights(events: &[serde_json::Value]) -> Result<(), String> {
    for (event, weight) in events.iter().zip([REGULAR_WEIGHT, BOLD_WEIGHT]) {
        let axes = event["appliedVariationCoordinates"]
            .as_array()
            .ok_or("captured axes missing")?;
        let wght = axes
            .iter()
            .find(|axis| axis["tag"] == "wght")
            .ok_or("captured wght missing")?;
        assert_eq!(wght["value"].as_f64(), Some(f64::from(weight)));
    }
    Ok(())
}

#[test]
fn variable_instance_keys_reject_invalid_axes() {
    let id = ID::default();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(InstanceKey::new(id, &[FontVariation::new(*b"wght", value)]).is_err());
    }
    assert!(InstanceKey::new(id, &[FontVariation::new(*b"wght", REGULAR_WEIGHT); 2]).is_err());
}

#[test]
fn variable_instance_key_normalizes_axis_order_and_signed_zero() -> Result<(), String> {
    let font = ID::default();
    let weight = FontVariation::new(*b"wght", REGULAR_WEIGHT);
    let positive = FontVariation::new(*b"opsz", 0.0);
    let negative = FontVariation::new(*b"opsz", -0.0);
    let first = InstanceKey::new(font, &[weight, positive])?;
    assert_eq!(first, InstanceKey::new(font, &[negative, weight])?);
    assert_ne!(
        first,
        InstanceKey::new(font, &[FontVariation::new(*b"wght", BOLD_WEIGHT), positive])?
    );
    Ok(())
}

#[test]
fn primary_variable_face_requires_one_applied_instance_across_runs() -> Result<(), String> {
    let tree = variable_tree(
        r#"<text id="regular" x="10" y="30">Tea Shop</text><text id="mixed" x="10" y="60"><tspan font-weight="400">Tea</tspan><tspan font-weight="700"> Shop</tspan></text>"#,
    )?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let regular = text(&tree, "regular")?;
    let id = regular.layouted()[0].positioned_glyphs[0].font;
    assert!(rasterizer.primary_variable_face(regular, id)?.is_some());
    let mixed = text(&tree, "mixed")?;
    assert!(mixed.layouted().len() > 1);
    assert!(rasterizer.primary_variable_face(mixed, id)?.is_none());
    Ok(())
}

#[test]
fn variable_base_is_unavailable_for_binary_only_face_data() -> Result<(), String> {
    let (tree, id) = binary_variable_tree()?;
    let text = text(&tree, "binary")?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let span = text
        .layouted()
        .first()
        .ok_or("binary variable span missing")?;
    assert_eq!(span.positioned_glyphs[0].font, id);
    assert!(rasterizer.face_file(id)?.is_none());
    assert!(rasterizer.variable_instance(span, id, FONT_SIZE)?.is_none());
    assert!(rasterizer.variable_base(id)?.is_none());
    Ok(())
}

fn binary_variable_tree() -> Result<(Tree, ID), String> {
    let bytes = std::fs::read("/System/Library/Fonts/SFNS.ttf")
        .map_err(|error| format!("SFNS variable fixture could not be read: {error}"))?;
    let mut database = Database::new();
    let ids = database.load_font_source(Source::Binary(Arc::new(bytes)));
    let id = *ids.first().ok_or("SFNS binary variable face unavailable")?;
    let family = &database
        .face(id)
        .and_then(|face| face.families.first())
        .ok_or("SFNS binary family unavailable")?
        .0;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="binary" font-family="{family}" font-size="{FONT_SIZE}">Tea</text></svg>"#
    );
    let tree = Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(database),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    Ok((tree, id))
}
