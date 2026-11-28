use super::{ProbeResult, Value};
use crate::markdown::svg_rasterize::font::html_rasterizer_options;
use resvg::usvg::{self, fontdb::Source};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const NORMAL_FONT_WEIGHT: u16 = 400;

#[test]
fn bbox_observation_records_real_text_and_fresh_json() -> ProbeResult<()> {
    let record = super::super::observe("Noto Sans", NORMAL_FONT_WEIGHT)?;
    assert_bbox_record(&record)?;
    let directory = super::super::super::probe_test_support::TempProbeDirectory::create()?;
    let output = directory.file("bbox.json");
    super::super::write_fresh(&output, &serde_json::to_vec(&record)?)?;
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(output)?)?,
        record
    );
    Ok(())
}

#[test]
fn recursive_observer_uses_checked_in_font_in_isolated_database() -> ProbeResult<()> {
    let (record, calls, base_faces, final_base_faces, shared) = observe_bundled_noto()?;
    assert!(calls > 0);
    assert_eq!(base_faces, final_base_faces);
    assert!(!shared);
    assert_bbox_record(&record)?;
    let directory = super::super::super::probe_test_support::TempProbeDirectory::create()?;
    let output = directory.file("recursive.json");
    super::super::write_fresh(&output, &serde_json::to_vec(&record)?)?;
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(output)?)?,
        record
    );
    Ok(())
}

#[test]
fn recursive_observer_rejects_missing_and_non_text_svg_nodes() -> ProbeResult<()> {
    for (svg, expected) in [
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect id="other" width="20" height="20"/></svg>"#,
            "text node missing",
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect id="krr-text-measure" width="20" height="20"/></svg>"#,
            "expected text node",
        ),
    ] {
        let (options, face) = noto_options(svg)?;
        let tree = usvg::Tree::from_str(svg, &options)?;
        let bbox = super::super::text_node(&tree).map(|_| ());
        let recursive = super::observe_tree(&tree, "Noto", NORMAL_FONT_WEIGHT, face).map(|_| ());
        for result in [bbox, recursive] {
            let error = result.err().ok_or("invalid probe node was accepted")?;
            assert_eq!(error.to_string(), expected);
        }
    }
    Ok(())
}

#[test]
fn recursive_observer_propagates_svg_parse_failure_with_the_real_font() -> ProbeResult<()> {
    let font =
        include_bytes!("../../assets/test-fixtures/font-probes/recursive-latin-crsv-normal.ttf");
    let result = super::observe_svg("<svg", "Animal", NORMAL_FONT_WEIGHT, font);
    let error = result.err().ok_or("malformed probe SVG was accepted")?;
    assert!(error.downcast_ref::<usvg::Error>().is_some(), "{error}");
    Ok(())
}

const NON_ALIAS_NOTO_FAMILY: &str = "Noto Sans";
const NON_ALIAS_NOTO_TEXT: &str = "Noto";

fn noto_options(svg: &str) -> ProbeResult<(usvg::Options<'static>, usvg::fontdb::ID)> {
    let mut options = html_rasterizer_options(svg);
    let mut database = usvg::fontdb::Database::new();
    let loaded = database.load_font_source(Source::Binary(Arc::new(FONT_BYTES.to_vec())));
    if loaded.len() != 1 {
        return Err("expected one checked-in Noto face".into());
    }
    let id = *loaded.first().ok_or("checked-in Noto Sans face missing")?;
    options.fontdb = Arc::new(database);
    Ok((options, id))
}

fn non_alias_svg() -> String {
    let family = super::super::escape_xml_attribute(NON_ALIAS_NOTO_FAMILY);
    let text = super::super::escape_xml_text(NON_ALIAS_NOTO_TEXT);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="320" height="40"><text id="krr-text-measure" x="0" y="28" font-family="{family}" font-size="14" font-weight="{NORMAL_FONT_WEIGHT}" font-style="normal" letter-spacing="0">{text}</text></svg>"#
    )
}

fn parsed_probe_spans(svg: &str, options: &usvg::Options<'static>) -> ProbeResult<Vec<Value>> {
    super::super::super::super::text_shaping::with_validated_tree_parse(|| {
        let tree = usvg::Tree::from_str(svg, options)?;
        let node = tree
            .node_by_id("krr-text-measure")
            .ok_or("text node missing")?;
        let usvg::Node::Text(text) = node else {
            return Err("expected text node".into());
        };
        super::super::observe_spans(text, tree.fontdb())
    })
}

fn count_original_and_alias_selectors(
    options: &mut usvg::Options<'static>,
    selected: usvg::fontdb::ID,
) -> (Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let original = std::mem::replace(
        &mut options.font_resolver.select_font,
        usvg::FontResolver::default_font_selector(),
    );
    let original_calls = Arc::new(AtomicUsize::new(0));
    let observed_original_calls = Arc::clone(&original_calls);
    options.font_resolver.select_font = Box::new(move |font, database| {
        observed_original_calls.fetch_add(1, Ordering::Relaxed);
        original(font, database)
    });
    let alias_calls = Arc::new(AtomicUsize::new(0));
    super::install_alias(options, selected, Arc::clone(&alias_calls));
    (original_calls, alias_calls)
}

fn assert_selected_noto_face(spans: &[Value], selected: usvg::fontdb::ID) -> ProbeResult<()> {
    let expected = format!("{selected:?}");
    let mut glyph_count = 0;
    for span in spans {
        let glyphs = span["glyphs"].as_array().ok_or("glyph records missing")?;
        for glyph in glyphs {
            assert_eq!(glyph["font_id"].as_str(), Some(expected.as_str()));
            glyph_count += 1;
        }
    }
    assert!(glyph_count > 0, "Noto text produced no glyphs");
    Ok(())
}

#[test]
fn non_alias_svg_uses_the_original_html_font_selector() -> ProbeResult<()> {
    let svg = non_alias_svg();
    let (official_options, selected) = noto_options(&svg)?;
    let expected = parsed_probe_spans(&svg, &official_options)?;
    assert_selected_noto_face(&expected, selected)?;

    let mut wrapped_options = html_rasterizer_options(&svg);
    wrapped_options.fontdb = Arc::clone(&official_options.fontdb);
    let (original_calls, alias_calls) =
        count_original_and_alias_selectors(&mut wrapped_options, selected);
    let actual = parsed_probe_spans(&svg, &wrapped_options)?;

    assert!(original_calls.load(Ordering::Relaxed) > 0);
    assert_eq!(alias_calls.load(Ordering::Relaxed), 0);
    assert_eq!(actual, expected);
    assert_selected_noto_face(&actual, selected)?;
    Ok(())
}

fn observe_bundled_noto() -> ProbeResult<(Value, usize, usize, usize, bool)> {
    let svg = super::super::probe_svg("Noto Sans", NORMAL_FONT_WEIGHT);
    let mut options = html_rasterizer_options(&svg);
    let base = Arc::clone(&options.fontdb);
    let base_faces = base.faces().count();
    let mut database = base.as_ref().clone();
    let loaded = database.load_font_source(Source::Binary(Arc::new(FONT_BYTES.to_vec())));
    let id = *loaded.first().ok_or("checked-in Noto Sans face missing")?;
    let selected = Arc::new(AtomicUsize::new(0));
    super::install_alias(&mut options, id, Arc::clone(&selected));
    options.fontdb = Arc::new(database);
    let record = super::super::super::super::text_shaping::with_validated_tree_parse(|| {
        let tree = super::super::super::usvg::Tree::from_str(&svg, &options)?;
        let mut row = super::observe_tree(&tree, "Noto Sans", NORMAL_FONT_WEIGHT, id)?;
        row["tree_database_shared_with_probe_options"] =
            serde_json::json!(Arc::ptr_eq(&options.fontdb, tree.fontdb()));
        Ok::<_, Box<dyn std::error::Error>>(row)
    })?;
    Ok((
        record,
        selected.load(Ordering::Relaxed),
        base_faces,
        base.faces().count(),
        Arc::ptr_eq(&base, &options.fontdb),
    ))
}

fn assert_bbox_record(record: &Value) -> ProbeResult<()> {
    let bbox = &record["bbox"];
    for key in ["x", "y", "width", "height"] {
        assert!(
            bbox[key].as_f64().is_some_and(f64::is_finite),
            "{key} missing"
        );
    }
    assert!(bbox["width"].as_f64().is_some_and(|value| value > 0.0));
    assert!(bbox["height"].as_f64().is_some_and(|value| value > 0.0));
    assert!(
        record["spans"]
            .as_array()
            .is_some_and(|spans| !spans.is_empty())
    );
    assert!(record["missing_glyph_id_zero_count"].as_u64().is_some());
    Ok(())
}

#[path = "svg_rasterize_text_probe_geometry_boundary_tests.rs"]
mod geometry_boundaries;
