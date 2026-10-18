use super::{SVG, assert_real_render, event_count, print_tree_snapshot, raster_observer};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use resvg::usvg::{self, fontdb::Source};

const NESTED_SELECTOR_COUNT: usize = 2;

#[test]
fn nested_html_scope_rejects_known_admission_and_restores_outer() -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let tree = nested_tree_with_scope_sample()?;
    assert!(!text_shaping::known_memo_usable());
    let snapshot = observer.snapshot();
    print_tree_snapshot(&snapshot);
    assert_nested_selectors(&snapshot)?;
    assert_nested_routes(&snapshot)?;
    assert!(
        tree.fontdb()
            .faces()
            .all(|face| !matches!(face.source, Source::SharedFile(_, _)))
    );
    assert_real_render(&tree)?;
    Ok(())
}

fn nested_tree_with_scope_sample() -> Result<usvg::Tree, String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let outer = text_shaping::known_memo_usable();
        let nested = text_shaping::with_validated_tree_parse(text_shaping::known_memo_usable);
        let tree = super::lazy_parse::parse(SVG, SVG).map_err(|error| error.to_string())?;
        let restored = text_shaping::known_memo_usable();
        eprintln!(
            "nested Tree memo usability: outer={outer}, nested={nested}, restored={restored}"
        );
        assert_eq!((outer, nested, restored), (true, false, true));
        Ok(tree)
    })
}

fn assert_nested_selectors(snapshot: &raster_observer::Snapshot) -> Result<(), String> {
    assert_eq!(snapshot.dropped, 0);
    let selectors = snapshot
        .events
        .iter()
        .filter(|event| event["stage"] == "selector")
        .collect::<Vec<_>>();
    assert_eq!(selectors.len(), NESTED_SELECTOR_COUNT);
    let known = selectors.first().ok_or("known selector missing")?;
    let eager = selectors.last().ok_or("eager selector missing")?;
    assert_eq!(known["route"], "known_probe");
    assert_eq!(known["admitted"], false);
    assert_eq!(known["actualFace"]["sourceKind"], "SharedFile");
    assert_eq!(eager["route"], "eager");
    assert_eq!(eager["actualFace"]["sourceKind"], "File");
    assert_selector_metadata(known, eager);
    Ok(())
}

fn assert_selector_metadata(known: &serde_json::Value, eager: &serde_json::Value) {
    assert_eq!(known["families"], eager["families"]);
    for key in [
        "faceIndex",
        "families",
        "postScriptName",
        "sourcePath",
        "style",
        "stretch",
        "weight",
    ] {
        assert!(!known["actualFace"][key].is_null());
        assert!(!eager["actualFace"][key].is_null());
        assert_eq!(known["actualFace"][key], eager["actualFace"][key]);
    }
}

fn assert_nested_routes(snapshot: &raster_observer::Snapshot) -> Result<(), String> {
    assert_eq!(event_count(snapshot, "route", "known_return"), 0);
    assert_eq!(event_count(snapshot, "route", "eager_reparse"), 1);
    assert_eq!(event_count(snapshot, "stage", "known_parse_result"), 1);
    assert_eq!(event_count(snapshot, "stage", "eager_parse_attempt"), 1);
    assert_eq!(event_count(snapshot, "stage", "eager_parse_result"), 1);
    assert_eq!(event_count(snapshot, "stage", "fallback"), 0);
    let known = snapshot
        .events
        .iter()
        .find(|event| event["stage"] == "known_parse_result")
        .ok_or("known parse result missing")?;
    assert_known_rejected(known);
    let eager = snapshot
        .events
        .iter()
        .find(|event| event["stage"] == "eager_parse_result")
        .ok_or("eager parse result missing")?;
    assert_eq!(eager["success"], true);
    assert_admission_rejected(snapshot)?;
    Ok(())
}

fn assert_known_rejected(known: &serde_json::Value) {
    assert_eq!(known["success"], true);
    assert_eq!(known["selected"], true);
    assert_eq!(known["valid"], false);
    assert!(known["treeParseNs"].as_u64().is_some());
}

fn assert_admission_rejected(snapshot: &raster_observer::Snapshot) -> Result<(), String> {
    let admission = snapshot
        .events
        .iter()
        .find(|event| event["stage"] == "admission_result")
        .ok_or("admission result missing")?;
    assert!(admission["expectedFace"].is_null());
    assert_eq!(
        admission["reason"],
        "admission_returned_none_inner_reason_unobserved"
    );
    Ok(())
}
