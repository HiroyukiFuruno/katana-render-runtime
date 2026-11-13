use super::super::probe_test_support::required_path;
use super::{ProbeResult, Value, json, usvg};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SFNT_SHA: &str = "f0b8414e5c903579bfd9dbca230a49f9063d92c6d622c6700f121a50a61e06ba";
const CSS_ALIAS: &str = "Recursive Variable";

#[test]
#[ignore = "diagnostic only; verified public SFNT and fresh absolute TMP JSON required"]
fn records_public_recursive_in_isolated_font_options() -> ProbeResult<()> {
    let input = required_path("KRR_RECURSIVE_PROBE_SFNT_PATH")?;
    let output = required_path("KRR_RECURSIVE_PROBE_JSON_PATH")?;
    let bytes = std::fs::read(&input)?;
    assert_eq!(super::sha256(&bytes), SFNT_SHA, "unexpected SFNT input");
    let rows = [
        ("動物", 700),
        ("+int 年齢", 400),
        ("Animal", 700),
        ("+int age", 400),
    ]
    .into_iter()
    .map(|(text, weight)| observe(text, weight, &bytes))
    .collect::<ProbeResult<Vec<_>>>()?;
    let record = json!({"schema":"isolated-public-recursive-probe-r1", "rows":rows,
        "sfnt_sha256":SFNT_SHA, "coordinate_domain":"identity local SVG user units",
        "browser_actual_glyph_face_known":false, "production_font_database_modified":false,
        "fast_width_path_tested":false, "score_accepted":false});
    super::write_fresh(&output, &serde_json::to_vec_pretty(&record)?)?;
    assert_eq!(
        super::sha256(&std::fs::read(input)?),
        SFNT_SHA,
        "SFNT input changed"
    );
    Ok(())
}

fn observe(text: &str, weight: u16, bytes: &[u8]) -> ProbeResult<Value> {
    let svg = super::probe_svg(text, weight);
    let (options, recursive, selected, base) = isolated_options(&svg, bytes)?;
    let count = base.faces().count();
    let mut record = super::super::super::text_shaping::with_validated_tree_parse(|| {
        let tree = usvg::Tree::from_str(&svg, &options)?;
        let mut record = observe_tree(&tree, text, weight, recursive)?;
        record["tree_database_shared_with_probe_options"] =
            json!(Arc::ptr_eq(&options.fontdb, tree.fontdb()));
        Ok::<_, Box<dyn std::error::Error>>(record)
    })?;
    let calls = selected.load(Ordering::Relaxed);
    assert!(calls > 0, "alias selector was not called");
    assert_eq!(base.faces().count(), count, "base database changed");
    assert!(
        !Arc::ptr_eq(&base, &options.fontdb),
        "probe must use cloned database"
    );
    record["alias_selector_calls"] = json!(calls);
    record["base_database_faces_before_after"] = json!(count);
    record["base_database_isolated"] = json!(true);
    Ok(record)
}

fn observe_tree(
    tree: &usvg::Tree,
    text: &str,
    weight: u16,
    recursive: usvg::fontdb::ID,
) -> ProbeResult<Value> {
    let node = tree
        .node_by_id("krr-text-measure")
        .ok_or("text node missing")?;
    let usvg::Node::Text(text_node) = node else {
        return Err("expected text node".into());
    };
    let bbox = node.abs_bounding_box();
    assert!(bbox.width().is_finite() && bbox.width() > 0.0);
    assert!(bbox.height().is_finite() && bbox.height() > 0.0);
    let spans = super::observe_spans(text_node, tree.fontdb())?;
    let missing = super::missing_glyph_count(&spans)?;
    assert_eq!(missing, 0, "missing glyph in controlled probe");
    if text.is_ascii() {
        assert_recursive_glyphs(&spans, recursive)?;
    }
    Ok(
        json!({"text":text, "requested_family":super::FAMILY,"requested_weight":weight,
        "bbox":{"x":bbox.x(),"y":bbox.y(),"width":bbox.width(),"height":bbox.height()},
        "missing_glyph_id_zero_count":missing,"injected_face_id":format!("{:?}",recursive),
        "spans":spans}),
    )
}

type Isolated = (
    usvg::Options<'static>,
    usvg::fontdb::ID,
    Arc<AtomicUsize>,
    Arc<usvg::fontdb::Database>,
);
fn isolated_options(svg: &str, bytes: &[u8]) -> ProbeResult<Isolated> {
    let mut options = super::html_rasterizer_options(svg);
    let base = Arc::clone(&options.fontdb);
    let mut database = base.as_ref().clone();
    let loaded = database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes.to_vec())));
    assert_eq!(loaded.len(), 1, "expected one SFNT face");
    let id = *loaded.first().ok_or("Recursive SFNT face unavailable")?;
    let face = database.face(id).ok_or("Recursive metadata unavailable")?;
    assert_eq!(face.post_script_name, "RecursiveSansLinearLight-Regular");
    assert_eq!(face.weight.0, 300, "unexpected static/default font weight");
    options.fontdb = Arc::new(database);
    let selected = Arc::new(AtomicUsize::new(0));
    install_alias(&mut options, id, Arc::clone(&selected));
    Ok((options, id, selected, base))
}

fn install_alias(
    options: &mut usvg::Options<'static>,
    id: usvg::fontdb::ID,
    selected: Arc<AtomicUsize>,
) {
    let original = std::mem::replace(
        &mut options.font_resolver.select_font,
        usvg::FontResolver::default_font_selector(),
    );
    options.font_resolver.select_font = Box::new(move |font, database| {
        if font.families().iter().any(|family| {
            matches!(family,
            usvg::FontFamily::Named(name) if name.eq_ignore_ascii_case(CSS_ALIAS))
        }) {
            selected.fetch_add(1, Ordering::Relaxed);
            Some(id)
        } else {
            original(font, database)
        }
    });
}

fn assert_recursive_glyphs(spans: &[Value], id: usvg::fontdb::ID) -> ProbeResult<()> {
    let expected = format!("{:?}", id);
    let mut count = 0;
    for span in spans {
        let glyphs = span["glyphs"].as_array().ok_or("glyph records missing")?;
        for glyph in glyphs {
            assert_eq!(
                glyph["font_id"].as_str(),
                Some(expected.as_str()),
                "Latin did not use injected Recursive"
            );
            count += 1;
        }
    }
    assert!(count > 0, "Latin glyph records empty");
    Ok(())
}

#[path = "svg_rasterize_text_probe_contract_tests.rs"]
mod contract_tests;

#[path = "svg_rasterize_text_probe_normal_lane_tests.rs"]
mod normal_lane_tests;
