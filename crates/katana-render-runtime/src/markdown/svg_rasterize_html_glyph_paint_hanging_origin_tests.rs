use crate::markdown::svg_rasterize::{
    font::html_font_db_for_text, html_glyph::HtmlGlyphRasterizer, text_shaping::loaded_html_font,
};
use resvg::usvg::{Node, Options, Tree, fontdb::Database};
use skrifa::{FontRef, Tag};

const FONT_FAMILY: &str = "Trebuchet MS";
const BASE_TABLE_FONT_FAMILY: &str = "Hiragino Kaku Gothic ProN";
const HANGING_SAMPLE_12: &str = "Portability";
const HANGING_SAMPLE_16: &str = "Mermaid runtime";
/* WHY: R2563の独立Chrome 155 raw SKP origin。差分はsource yから8.796875px。 */
const PORTABILITY_CHROME_ORIGIN: f32 = 20_979.0 / 64.0;
/* WHY: R2563の独立Chrome 155 raw SKP origin。source y=-350から12px。 */
const TITLE_CHROME_ORIGIN: f32 = -338.0;
const HANGING_ORIGIN_TOLERANCE: f32 = 0.0001;
const HANGING_ORIGIN_CASE_COUNT: usize = 6;
const HANGING_ORIGIN_CASES: [HangingOriginCase; HANGING_ORIGIN_CASE_COUNT] = [
    HangingOriginCase {
        font_size: 12.0,
        source_y: 319.0,
        expected_y: PORTABILITY_CHROME_ORIGIN,
        sample: HANGING_SAMPLE_12,
        alignment_attribute: "",
    },
    HangingOriginCase {
        font_size: 12.0,
        source_y: 319.0,
        expected_y: PORTABILITY_CHROME_ORIGIN,
        sample: HANGING_SAMPLE_12,
        alignment_attribute: "alignment-baseline=\"auto\"",
    },
    HangingOriginCase {
        font_size: 12.0,
        source_y: 319.0,
        expected_y: PORTABILITY_CHROME_ORIGIN,
        sample: HANGING_SAMPLE_12,
        alignment_attribute: "alignment-baseline=\"baseline\"",
    },
    HangingOriginCase {
        font_size: 16.0,
        source_y: -350.0,
        expected_y: TITLE_CHROME_ORIGIN,
        sample: HANGING_SAMPLE_16,
        alignment_attribute: "",
    },
    HangingOriginCase {
        font_size: 16.0,
        source_y: -350.0,
        expected_y: TITLE_CHROME_ORIGIN,
        sample: HANGING_SAMPLE_16,
        alignment_attribute: "alignment-baseline=\"auto\"",
    },
    HangingOriginCase {
        font_size: 16.0,
        source_y: -350.0,
        expected_y: TITLE_CHROME_ORIGIN,
        sample: HANGING_SAMPLE_16,
        alignment_attribute: "alignment-baseline=\"baseline\"",
    },
];
const BASE_TABLE_TAG: Tag = Tag::from_be_bytes(*b"BASE");

#[derive(Clone, Copy)]
struct HangingOriginCase {
    font_size: f32,
    source_y: f32,
    expected_y: f32,
    sample: &'static str,
    alignment_attribute: &'static str,
}

#[test]
fn hanging_origins_match_chrome_layout_units_for_dominant_and_alignment() -> Result<(), String> {
    for case in HANGING_ORIGIN_CASES {
        let tree = origin_tree(FONT_FAMILY, &case)?;
        let actual = snapped_origin(&tree)?;
        assert!(
            (actual - case.expected_y).abs() < HANGING_ORIGIN_TOLERANCE,
            "{} origin {actual}, expected Chrome origin {}",
            case.alignment_attribute,
            case.expected_y
        );
    }
    Ok(())
}

#[test]
fn hanging_policy_rejects_unsupported_baseline_contexts() -> Result<(), String> {
    for unsupported_attributes in [
        "transform=\"scale(2 1)\"",
        "writing-mode=\"tb\"",
        "alignment-baseline=\"hanging\"",
    ] {
        let case = HANGING_ORIGIN_CASES[0];
        let tree = origin_tree_with_attributes(FONT_FAMILY, &case, unsupported_attributes)?;
        assert!(
            !has_resolved_text_policy(&tree)?,
            "{unsupported_attributes}"
        );
    }
    Ok(())
}

#[test]
fn hanging_policy_keeps_font_with_open_type_base_table_on_usvg_path() -> Result<(), String> {
    let case = HANGING_ORIGIN_CASES[0];
    let tree = origin_tree(BASE_TABLE_FONT_FAMILY, &case)?;
    let face_id = source_face_id(&tree)?;
    assert!(
        has_base_table(tree.fontdb(), face_id),
        "BASE test font missing table"
    );
    assert!(!has_resolved_text_policy(&tree)?);
    Ok(())
}

fn origin_tree(family: &str, case: &HangingOriginCase) -> Result<Tree, String> {
    origin_tree_with_attributes(family, case, "")
}

fn origin_tree_with_attributes(
    family: &str,
    case: &HangingOriginCase,
    extra_attributes: &str,
) -> Result<Tree, String> {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="{}" font-size="{}" font-family="{}" text-anchor="middle" dominant-baseline="hanging" {} {}>{}</text></svg>"#,
        case.source_y,
        case.font_size,
        family,
        case.alignment_attribute,
        extra_attributes,
        case.sample
    );
    let database = html_font_db_for_text(family, case.sample);
    Tree::from_str(
        &source,
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn snapped_origin(tree: &Tree) -> Result<f32, String> {
    let spans = prepared_spans(tree)?;
    let span = spans.first().ok_or("hanging glyph span missing")?;
    let glyph = span.glyphs.first().ok_or("hanging glyph missing")?;
    let transform = super::normalized_transform(span, glyph, 1.0);
    let mut positions = [skia_safe::Point::new(transform.tx, transform.ty)];
    super::apply_run_origin_snap(span, &mut positions, 1.0);
    Ok(positions[0].y)
}

fn prepared_spans(tree: &Tree) -> Result<Vec<super::super::PreparedSpan>, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("hanging text node missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let super::super::Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("hanging text was not prepared".to_owned());
    };
    Ok(spans)
}

fn has_resolved_text_policy(tree: &Tree) -> Result<bool, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("hanging text node missing".to_owned());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    Ok(super::super::baseline::resolved_policy(&rasterizer, text)?.is_some())
}

fn source_face_id(tree: &Tree) -> Result<resvg::usvg::fontdb::ID, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("hanging text node missing".to_owned());
    };
    let font = text
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .map(|span| span.font())
        .ok_or("hanging source span missing".to_owned())?;
    loaded_html_font(font, tree.fontdb()).ok_or("hanging source font missing".to_owned())
}

fn has_base_table(database: &Database, face_id: resvg::usvg::fontdb::ID) -> bool {
    database
        .with_face_data(face_id, |bytes, index| {
            FontRef::from_index(bytes, index)
                .ok()?
                .table_data(BASE_TABLE_TAG)
                .map(|_| ())
        })
        .flatten()
        .is_some()
}
